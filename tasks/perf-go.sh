#!/usr/bin/env bash
# What binding a mise-sized command line costs a Go CLI built on usage.
#
# Reported, never gated. The shadow is mise's committed spec, which grows on purpose, so
# comparing one commit's number against another's partly measures the fixture. What holds
# steady is the shape: a bind that costs about as much as one page fault, against a runtime
# floor three orders of magnitude larger.
#
# The protocol is `benches/gate`'s, with one change forced by the language. The Rust harness
# differences N=1 against N=0 and calls that a cold parse, because Rust's startup is
# deterministic to within a few hundred instructions. Go's is not: the runtime creates threads,
# starts the collector and varies with what the linker kept, and repeated N=0 runs here differ
# by ±50,000 instructions — twenty times the thing being measured. So the per-bind figure is
# amortized over many binds, where the jitter cancels, and the floor is reported beside it
# rather than subtracted and forgotten.
set -euo pipefail

out=${1:-/dev/stdout}

# One argv, the same one the Rust shadow uses, so the two tables describe the same work.
ARGV="use -g node@20"

# Enough binds that the startup jitter is a rounding error, few enough that cachegrind's
# 50x slowdown stays under a second.
BINDS=1000

root=$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)
work=$(mktemp -d)
bin=$work/parse-n
# Only what this script made. cachegrind writes `cachegrind.out.<pid>` into the working
# directory by default, and a glob for those in the caller's would delete a report they were
# in the middle of reading — or one a concurrent valgrind was still writing. It is told where
# to put its own instead, which is the directory that goes away here.
trap 'rm -rf "$work"' EXIT

(cd "$root/go" && go build -o "$bin" ./internal/bench/parse-n)

# Each harness prints 1 when the bind reached a subcommand. Anything else means the numbers
# below would be describing a rejected command line, which is cheap for the wrong reason.
# shellcheck disable=SC2086 # the argv is several words on purpose
if [ "$(PARSE_N=1 "$bin" $ARGV)" != "1" ]; then
  echo "the harness did not reach a subcommand, so there is nothing worth measuring" >&2
  exit 1
fi

instructions() {
  # shellcheck disable=SC2086
  PARSE_N="$1" valgrind --tool=cachegrind --cache-sim=no --branch-sim=no \
    --cachegrind-out-file="$work/cachegrind.out.%p" "$bin" $ARGV 2>&1 |
    sed -n 's/.*I *refs: *//p' | tr -d ','
}

# A clock with nanoseconds, from whatever this machine has.
#
# `date +%s%N` is GNU's; BSD `date` prints a literal `N`, which arithmetic under `set -u`
# then fails on — and it failed *before* the valgrind check below, so the wall-clock-only
# path meant for machines without cachegrind was the one path that could not run on a Mac.
# Checked once rather than per call, and reported as unavailable rather than guessed at.
now_ns() {
  case $clock in
  gnu) date +%s%N ;;
  py) python3 -c 'import time; print(time.time_ns())' ;;
  esac
}

# Which clock to read, detected once. Overridable so the fallbacks can be exercised: a path
# that only runs on a machine nobody here has is a path nobody has run.
#
#   PERF_GO_CLOCK=gnu   `date +%s%N`, which is GNU's
#   PERF_GO_CLOCK=py    python3, for a BSD `date` that has no %N
#   PERF_GO_CLOCK=none  neither, which reports the wall column as unavailable
if [ -n "${PERF_GO_CLOCK:-}" ]; then
  clock=$PERF_GO_CLOCK
elif [ "$(date +%s%N 2>/dev/null | tr -d '0-9')" = "" ] && [ -n "$(date +%s%N 2>/dev/null)" ]; then
  clock=gnu
elif command -v python3 >/dev/null 2>&1; then
  clock=py
else
  clock=none
fi

# The whole process, which is what a user waits for — measured from outside, because a timer
# inside the program cannot see the runtime starting up, and that is most of what is being
# reported here.
wall_ms() {
  local n=$1 start end
  if [ "$clock" = none ]; then
    echo "unavailable"
    return
  fi
  start=$(now_ns)
  for _ in 1 2 3 4 5 6 7 8 9 10; do
    # shellcheck disable=SC2086
    PARSE_N="$n" "$bin" $ARGV >/dev/null
  done
  end=$(now_ns)
  awk -v ns="$((end - start))" 'BEGIN { printf "%.2f", ns / 10 / 1000000 }'
}

# `ms` on a number, the word alone when there was no clock to read.
wall_cell() {
  if [ "$1" = "unavailable" ]; then
    echo "unavailable (no nanosecond clock)"
  else
    echo "$1 ms"
  fi
}

size=$(stat -c %s "$bin" 2>/dev/null || stat -f %z "$bin")
size_mb=$(awk -v b="$size" 'BEGIN { printf "%.2f", b / 1048576 }')
one_wall=$(wall_ms 1)

if ! command -v valgrind >/dev/null 2>&1; then
  {
    printf '### Go harness\n\n'
    printf 'valgrind is not installed, so there are no instruction counts — wall clock only.\n\n'
    printf '| measurement | value |\n|---|---:|\n'
    printf '| whole process, one bind | %s |\n' "$(wall_cell "$one_wall")"
    printf '| binary | %s MB |\n' "$size_mb"
  } >"$out"
  exit 0
fi

floor=$(instructions 0)
many=$(instructions "$BINDS")
per=$(( (many - floor) / BINDS ))

{
  printf '### Go harness\n\n'
  # The backticks are markdown; the single quotes keep them literal.
  # shellcheck disable=SC2016
  # shellcheck disable=SC2016 # the backticks are markdown, not command substitution
  printf 'Binding `mise %s` against mise'"'"'s committed spec, through the generated tables in\n' "$ARGV"
  # shellcheck disable=SC2016
  printf '`go/internal/shadow/mise`. Reproduce with `mise run perf:go`.\n\n'
  printf '| measurement | value |\n|---|---:|\n'
  printf '| one bind, amortized over %s | %s instructions |\n' "$BINDS" "$(printf "%'d" "$per")"
  printf '| Go runtime floor, no bind at all | %s instructions |\n' "$(printf "%'d" "$floor")"
  printf '| whole process, one bind | %s |\n' "$(wall_cell "$one_wall")"
  printf '| binary | %s MB |\n' "$size_mb"
  printf '\n'
  printf 'The floor is reported rather than subtracted once and forgotten: it is what a Go CLI\n'
  # shellcheck disable=SC2016
  printf 'pays before `main`, it is three orders of magnitude larger than the bind, and it\n'
  printf 'varies between runs by more than the bind costs. Any single-bind measurement here is\n'
  printf 'a measurement of the runtime.\n'
} >"$out"
