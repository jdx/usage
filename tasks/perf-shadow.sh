#!/usr/bin/env bash
# The shadow comparison: what parsing a mise-sized command line costs each framework.
#
# Reported, never gated. The shadow is a fixture that grows on purpose — every property the
# derive learns to express adds to it — so comparing one commit's shadow against another's
# measures the fixture, not the parser. That is why these are not in `tak.toml`: tak gates
# everything it finds there, and a gate that fires when a fixture gets bigger teaches
# people to ignore it.
#
# What is worth watching here is the *ratio* between the two columns, which is a property of
# the parsers and holds steady while the fixture changes.
#
# Build the whole workspace before running this — `mise run perf` and `mise run perf:record`
# both do, and the numbers are only comparable to theirs if you did too. `cargo build
# --release -p gate` is not the same build: cargo unifies features across every member it is
# asked for, and the rest of this workspace turns on clap features the gate alone does not,
# which is worth 14% of clap's instruction count. usage's own figure does not move, so a
# gate-only build quietly flatters clap by comparison.
set -euo pipefail

out=${1:-/dev/stdout}

# Built somewhere else and moved into place at the end. The workflow calls this with
# `|| true`, because an informative table is not worth failing a run over — which means a
# failure part way through must not leave a truncated table behind for the comment to
# publish as though it were a comparison.
tmp=$(mktemp)
trap 'rm -f "$tmp" cachegrind.out.*' EXIT

# One argv for all four. `parse-n-clap` needs a program name in front of the words and
# already has one, because it collects `args_os()` whole with argv[0] included — so spelling
# `mise` here as well would hand clap one token too many. It does not fail on that, which is
# the trap: clap binds the extra word to the root's `[TASK]` positional and parses on, so the
# column would report a command line the other three never saw. A shadow that rejected the
# input would at least be loud about it.
ARGV="use -g node@20"

# The binaries take a repeat count from the environment, so differencing two runs of the
# *same* binary isolates the parse: N=1 minus N=0 is one cold parse in a fresh process.
# Differencing two different binaries would fold in whatever they each do before `main`.
instructions() {
  local binary=$1 n=$2
  # shellcheck disable=SC2086 # the argv is several words on purpose
  PARSE_N="$n" valgrind --tool=cachegrind --cache-sim=no --branch-sim=no \
    "./target/release/$binary" $ARGV 2>&1 |
    sed -n 's/.*I *refs: *//p' | tr -d ','
}

cold() {
  local binary=$1 zero one
  zero=$(instructions "$binary" 0)
  one=$(instructions "$binary" 1)
  echo $((one - zero))
}

# Each shadow prints 1 when the parse reached the subcommand. Anything else means the
# number below would be measuring a rejected command line.
verify() {
  local binary=$1 got
  # shellcheck disable=SC2086
  got=$(PARSE_N=1 "./target/release/$binary" $ARGV)
  if [ "$got" != "1" ]; then
    echo "$binary did not reach the subcommand (printed '$got'), so there is nothing" \
      "worth measuring" >&2
    exit 1
  fi
}

# Two significant figures either side of the decimal point, so a framework within a factor
# of usage reads as 1.5x rather than 1x and one three orders away still reads cleanly.
ratio() {
  awk -v a="$1" -v b="$2" 'BEGIN { if (a / b < 10) printf "%.1f", a / b; else printf "%d", a / b }'
}

if ! command -v valgrind >/dev/null 2>&1; then
  echo "valgrind is not installed, so there are no instruction counts to report" >&2
  exit 0
fi

for binary in parse-n parse-n-argh parse-n-clap parse-n-bpaf; do
  verify "$binary"
done

usage_cold=$(cold parse-n)
argh_cold=$(cold parse-n-argh)
clap_cold=$(cold parse-n-clap)
bpaf_cold=$(cold parse-n-bpaf)

{
  printf '### Shadow comparison\n\n'
  # The backticks in this block are markdown, not command substitution, and the single
  # quotes are what keeps them literal.
  # shellcheck disable=SC2016
  printf 'Parsing `mise use -g node@20` against a shadow of mise'"'"'s committed spec.\n'
  printf 'Reported, not gated: the shadow grows as the derive learns to express more, so\n'
  printf 'what to watch is the ratio rather than either column.\n\n'
  printf '| framework | instructions, cold parse | vs usage |\n'
  printf '|---|---:|---:|\n'
  printf '| usage | %s | — |\n' "$(printf "%'d" "$usage_cold")"
  for row in "argh:$argh_cold" "clap:$clap_cold" "bpaf:$bpaf_cold"; do
    printf '| %s | %s | %sx |\n' "${row%%:*}" \
      "$(printf "%'d" "${row#*:}")" \
      "$(ratio "${row#*:}" "$usage_cold")"
  done
  printf '\n'
  # Wall clock in the same process, which is the only way to see µs: a whole-process
  # measurement cannot resolve a 200ns parse.
  #
  # Two harnesses because they answer different questions. `time-parse` splits clap into
  # building its tree and parsing with it, which is what shows *where* clap's time goes.
  # `time-sweep` compares all four, and takes the minimum over many short rounds rather
  # than the mean of a few long ones — the estimator that holds up on a shared runner,
  # where anything else reports the neighbours.
  printf '```\n'
  ./target/release/time-sweep
  printf '\n'
  ./target/release/time-parse
  printf '```\n'
} >"$tmp"

cat "$tmp" >"$out"
