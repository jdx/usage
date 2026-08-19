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

# cobra builds its whole command tree on every iteration, which is the cost being compared and
# about a thousand times usage-go's. Fewer iterations, so cachegrind's 50x slowdown still
# finishes: 20 of them is 40M instructions, where 1000 would be two billion.
COBRA_BINDS=20

root=$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)
work=$(mktemp -d)
bin=$work/parse-n
# Only what this script made. cachegrind writes `cachegrind.out.<pid>` into the working
# directory by default, and a glob for those in the caller's would delete a report they were
# in the middle of reading — or one a concurrent valgrind was still writing. It is told where
# to put its own instead, which is the directory that goes away here.
trap 'rm -rf "$work"' EXIT

(cd "$root/go" && go build -o "$bin" ./internal/bench/parse-n)

# cobra, from the same spec — `xtask gen-shadow … cobra`, checked in under benches/go/cobra.
#
# Its own module, so cobra is not a dependency of `github.com/jdx/usage/go`, and its build is
# allowed to fail: it needs the dependency fetched, and a machine without a proxy should still
# get the row this harness is mainly about. What it must not do is silently report nothing, so
# the reason lands in the table.
cobra_bin=$work/cobra
cobra_why=""
if [ -d "$root/benches/go/cobra" ]; then
  if ! (cd "$root/benches/go/cobra" && go build -o "$cobra_bin" . 2>"$work/cobra.log"); then
    cobra_why="build failed: $(tr -d '\n' <"$work/cobra.log" | cut -c1-120)"
  fi
else
  cobra_why="benches/go/cobra is missing; run \`mise run gen-shadow\`"
fi
# shellcheck disable=SC2086
if [ -z "$cobra_why" ] && [ "$(PARSE_N=1 "$cobra_bin" $ARGV)" != "1" ]; then
  cobra_why="the shadow did not reach a subcommand"
fi

# Each harness prints 1 when the bind reached a subcommand. Anything else means the numbers
# below would be describing a rejected command line, which is cheap for the wrong reason.
# shellcheck disable=SC2086 # the argv is several words on purpose
if [ "$(PARSE_N=1 "$bin" $ARGV)" != "1" ]; then
  echo "the harness did not reach a subcommand, so there is nothing worth measuring" >&2
  exit 1
fi

instructions() {
  local binary=$1 n=$2
  # shellcheck disable=SC2086
  PARSE_N="$n" valgrind --tool=cachegrind --cache-sim=no --branch-sim=no \
    --cachegrind-out-file="$work/cachegrind.out.%p" "$binary" $ARGV 2>&1 |
    sed -n 's/.*I *refs: *//p' | tr -d ','
}

# How the wall column is measured, decided once.
#
# `date +%s%N` is GNU's. BSD `date` prints a literal `N`, which arithmetic under `set -u`
# then fails on — and it failed *before* the valgrind check below, so the wall-clock-only
# path meant for machines without cachegrind was the one path that could not run on a Mac.
#
#   PERF_GO_CLOCK=gnu   two reads of `date +%s%N` around the loop
#   PERF_GO_CLOCK=py    python times the loop itself
#   PERF_GO_CLOCK=none  neither, and the column says so
#
# Overridable so the fallbacks can be exercised: a path that only runs on a machine nobody
# here has is a path nobody has run.
if [ -n "${PERF_GO_CLOCK:-}" ]; then
  clock=$PERF_GO_CLOCK
elif [ -n "$(date +%s%N 2>/dev/null)" ] && [ "$(date +%s%N 2>/dev/null | tr -d '0-9')" = "" ]; then
  clock=gnu
elif command -v python3 >/dev/null 2>&1; then
  clock=py
else
  clock=none
fi

case $clock in
gnu | py | none) ;;
*)
  echo "PERF_GO_CLOCK=$clock is not one of gnu, py, none" >&2
  exit 2
  ;;
esac

runs=10

# Ten whole processes, timed from outside: a timer inside the program cannot see the runtime
# starting up, and that is most of what is being reported here.
wall_ms() {
  local binary=$1 n=$2
  case $clock in
  gnu)
    local start end
    start=$(date +%s%N 2>/dev/null) || start=
    for _ in $(seq "$runs"); do
      # shellcheck disable=SC2086
      PARSE_N="$n" "$binary" $ARGV >/dev/null
    done
    end=$(date +%s%N 2>/dev/null) || end=
    # Validated rather than trusted: a `date` that answers with anything else would
    # otherwise be reported as 0.00 ms, which reads as a measurement.
    case $start$end in
    '' | *[!0-9]*)
      echo "unavailable"
      return
      ;;
    esac
    awk -v ns="$((end - start))" -v runs="$runs" \
      'BEGIN { printf "%.2f", ns / runs / 1000000 }'
    ;;
  py)
    # Timed inside python, not around two `python3` invocations. Reading the clock that way
    # put a whole interpreter startup — tens of milliseconds — inside an interval measuring
    # ten runs of about one millisecond each, so the fallback reported python rather than
    # the program it was pointed at.
    # shellcheck disable=SC2086
    python3 - "$binary" "$n" "$runs" $ARGV <<'PYTHON' 2>/dev/null || echo "unavailable"
import os, subprocess, sys, time

binary, parse_n, runs, *argv = sys.argv[1:]
env = dict(os.environ, PARSE_N=parse_n)
with open(os.devnull, "wb") as quiet:
    started = time.perf_counter_ns()
    for _ in range(int(runs)):
        subprocess.run([binary, *argv], stdout=quiet, env=env, check=False)
    elapsed = time.perf_counter_ns() - started
print("%.2f" % (elapsed / int(runs) / 1e6))
PYTHON
    ;;
  none) echo "unavailable" ;;
  esac
}

# `ms` on a number, the word alone when there was no clock to read.
wall_cell() {
  if [ "$1" = "unavailable" ]; then
    echo "unavailable (no nanosecond clock)"
  else
    echo "$1 ms"
  fi
}

size_mb() {
  local bytes
  bytes=$(stat -c %s "$1" 2>/dev/null || stat -f %z "$1")
  awk -v b="$bytes" 'BEGIN { printf "%.2f", b / 1048576 }'
}

size_mb=$(size_mb "$bin")
one_wall=$(wall_ms "$bin" 1)

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

floor=$(instructions "$bin" 0)
many=$(instructions "$bin" "$BINDS")
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

  printf '\n#### Against cobra\n\n'
  if [ -n "$cobra_why" ]; then
    printf 'Not measured this run — %s.\n' "$cobra_why"
  else
    cobra_floor=$(instructions "$cobra_bin" 0)
    cobra_many=$(instructions "$cobra_bin" "$COBRA_BINDS")
    cobra_per=$(( (cobra_many - cobra_floor) / COBRA_BINDS ))
    cobra_wall=$(wall_ms "$cobra_bin" 1)
    ratio=$(awk -v a="$cobra_per" -v b="$per" 'BEGIN { printf "%.0f", a / b }')

    # shellcheck disable=SC2016 # the backticks are markdown, not command substitution
    printf 'The same spec, declared in cobra by `xtask gen-shadow … cobra` and checked in under\n'
    # shellcheck disable=SC2016
    printf '`benches/go/cobra`, so the two rows describe the same CLI rather than two people'"'"'s\n'
    printf 'transcriptions of mise.\n\n'
    printf '| | one resolve | whole process | binary |\n|---|---:|---:|---:|\n'
    printf '| usage-go | %s | %s | %s MB |\n' \
      "$(printf "%'d" "$per")" "$(wall_cell "$one_wall")" "$size_mb"
    printf '| cobra | %s | %s | %s MB |\n' \
      "$(printf "%'d" "$cobra_per")" "$(wall_cell "$cobra_wall")" "$(size_mb "$cobra_bin")"
    printf '| ratio | %sx | | |\n' "$ratio"
    printf '\n'
    printf 'cobra'"'"'s figure includes building its command tree, because that is what it does on\n'
    # shellcheck disable=SC2016
    printf 'every process start: a `cobra.Command` per subcommand, each with its own flag set.\n'
    printf 'Hoisting it out of the loop would measure its parser against a program that had\n'
    printf 'already paid for its model, which no CLI gets to do. usage-go has no such step —\n'
    printf 'the tables are laid out by the linker — so the two figures are what each framework\n'
    printf 'costs to answer one command line in a fresh process.\n'
    printf '\nAmortized over %s iterations for usage-go and %s for cobra: one cobra resolve is\n' \
      "$BINDS" "$COBRA_BINDS"
    printf 'dear enough that a thousand of them under cachegrind would take minutes.\n'
  fi
} >"$out"
