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
set -euo pipefail

out=${1:-/dev/stdout}

# Built somewhere else and moved into place at the end. The workflow calls this with
# `|| true`, because an informative table is not worth failing a run over — which means a
# failure part way through must not leave a truncated table behind for the comment to
# publish as though it were a comparison.
tmp=$(mktemp)
trap 'rm -f "$tmp" cachegrind.out.*' EXIT

# The two binaries take a repeat count from the environment, so differencing two runs of the
# *same* binary isolates the parse: N=1 minus N=0 is one cold parse in a fresh process.
# Differencing two different binaries would fold in whatever they each do before `main`.
instructions() {
  local binary=$1 n=$2
  PARSE_N="$n" valgrind --tool=cachegrind --cache-sim=no --branch-sim=no \
    "./target/release/$binary" use -g node@20 2>&1 |
    sed -n 's/.*I *refs: *//p' | tr -d ','
}

cold() {
  local binary=$1 zero one
  zero=$(instructions "$binary" 0)
  one=$(instructions "$binary" 1)
  echo $((one - zero))
}

if ! command -v valgrind >/dev/null 2>&1; then
  echo "valgrind is not installed, so there are no instruction counts to report" >&2
  exit 0
fi

usage_cold=$(cold parse-n)
clap_cold=$(cold parse-n-clap)

{
  printf '### Shadow comparison\n\n'
  # The backticks in this block are markdown, not command substitution, and the single
  # quotes are what keeps them literal.
  # shellcheck disable=SC2016
  printf 'Parsing `mise use -g node@20` against a shadow of mise'"'"'s committed spec.\n'
  printf 'Reported, not gated: the shadow grows as the derive learns to express more, so\n'
  printf 'what to watch is the ratio rather than either column.\n\n'
  printf '| | usage | clap | ratio |\n'
  printf '|---|---:|---:|---:|\n'
  printf '| instructions, cold parse | %s | %s | %sx |\n' \
    "$(printf "%'d" "$usage_cold")" "$(printf "%'d" "$clap_cold")" \
    "$((clap_cold / usage_cold))"
  printf '\n'
  # Wall clock in the same process, which is the only way to see µs: a whole-process
  # measurement cannot resolve a 2µs parse.
  printf '```\n'
  ./target/release/time-parse
  printf '```\n'
} >"$tmp"

cat "$tmp" >"$out"
