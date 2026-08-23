#!/usr/bin/env bash
# What parsing a mise-sized command line costs a Go CLI, framework by framework.
#
# Reported, never gated. The shadows are mise's committed spec, which grows on purpose, so
# comparing one commit's number against another's partly measures the fixture. What holds
# steady is the shape: a parse that costs microseconds against frameworks that cost
# hundreds of them, and a Go runtime floor larger than either.
#
# Two measurements, in this order, because they answer different questions:
#
#   1. In-process parse throughput — `benches/go/cmd/sweep`, which is
#      `benches/gate/src/bin/time-sweep.rs` in Go: every parser run repeatedly in one
#      process, the fastest of many short rounds reported. This is the number the
#      landing page charts, and the same estimator the Rust card uses, because "what does
#      a parse cost" is a question about parsing.
#
#   2. Whole-process cost — the five `parse-n` binaries under cachegrind and a clock.
#      This is the number an adopter feels, and most of it is the Go runtime coming up:
#      ~0.95 ms and ~950,000 instructions before `main`, which no parser can touch. It is
#      reported beside the parse rather than subtracted from it, because a subtraction
#      hides how much of a Go CLI's latency is not the parser's to win.
#
# Why not difference N=1 against N=0 and call that a cold parse, the way the Rust harness
# does: Go's startup is not deterministic. The runtime creates threads, starts the
# collector and varies with what the linker kept, and repeated N=0 runs here differ by
# ±50,000 instructions — twenty times what a bind costs. So the instruction figures are
# amortized over many resolves, where the jitter cancels.
set -euo pipefail

out=${1:-/dev/stdout}

# One argv, the same one the Rust shadows use, so the two tables describe the same work.
ARGV="use -g node@20"

root=$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)
work=$(mktemp -d)
# Only what this script made. cachegrind writes `cachegrind.out.<pid>` into the working
# directory by default, and a glob for those in the caller's would delete a report they were
# in the middle of reading — or one a concurrent valgrind was still writing. It is told where
# to put its own instead, which is the directory that goes away here.
trap 'rm -rf "$work"' EXIT

# The usage-go harnesses live in the Go module itself and have no dependencies to fetch, so
# this much works on a machine with no network.
bin=$work/parse-n
(cd "$root/go" && go build -o "$work/" ./internal/bench/parse-n ./internal/bench/bind-n)

# Each harness prints 1 when the parse reached a subcommand. Anything else means the numbers
# below would be describing a rejected command line, which is cheap for the wrong reason.
# shellcheck disable=SC2086 # the argv is several words on purpose
if [ "$(PARSE_N=1 "$bin" $ARGV)" != "1" ]; then
  echo "the harness did not reach a subcommand, so there is nothing worth measuring" >&2
  exit 1
fi

# The other three frameworks live in `benches/go`, the one module in the repository that
# depends on them — and its build is allowed to fail, since it needs those modules fetched
# and a machine without a proxy should still get the row this harness is mainly about. What
# it must not do is silently report nothing, so the reason lands in the report.
shadows_why=""
if ! (cd "$root/benches/go" && go build -o "$work/" ./cmd/... 2>"$work/build.log"); then
  shadows_why="build failed: $(tr -d '\n' <"$work/build.log" | cut -c1-160)"
fi

# How the wall column is measured, decided once.
#
# The figure wanted is the *minimum* of many whole processes, for the reason the sweep
# takes minima: noise from other tenants is additive, so the fastest run is the one least
# spoiled by them. That needs a clock this script can read between runs without paying for
# it — `date` is a fork, and a fork costs about what a whole Go process does, so timing
# each run with two of them would measure `date`.
#
#   PERF_GO_CLOCK=bash  bash 5's $EPOCHREALTIME, read in-process, once per run
#   PERF_GO_CLOCK=py    python times each run and reports the fastest
#   PERF_GO_CLOCK=none  neither, and the column says so
#
# Overridable so the fallbacks can be exercised: a path that only runs on a machine nobody
# here has is a path nobody has run. macOS ships bash 3.2, which has no $EPOCHREALTIME,
# which is what the python path is for.
if [ -n "${PERF_GO_CLOCK:-}" ]; then
  clock=$PERF_GO_CLOCK
elif [ -n "${EPOCHREALTIME:-}" ]; then
  clock=bash
elif command -v python3 >/dev/null 2>&1; then
  clock=py
else
  clock=none
fi

case $clock in
bash | py | none) ;;
*)
  echo "PERF_GO_CLOCK=$clock is not one of bash, py, none" >&2
  exit 2
  ;;
esac

# Enough processes that the fastest is a process that got a clear run at the machine, few
# enough that the slowest row here — kong, at several milliseconds a parse — still finishes
# in about a second. Fifty was not enough: the floor and usage-go's row are separated by
# less than a fork's worth of noise, and at fifty runs the floor came out *slower* than the
# row that does a parse on top of it.
runs=200

# Whole processes, timed from outside: a timer inside the program cannot see the runtime
# starting up, and that is most of what is being reported here.
wall_ms() {
  local binary=$1 n=$2
  case $clock in
  bash)
    local start end pairs=""
    for _ in $(seq "$runs"); do
      start=$EPOCHREALTIME
      # shellcheck disable=SC2086
      PARSE_N="$n" "$binary" $ARGV >/dev/null
      end=$EPOCHREALTIME
      # Collected and reduced once at the end. An `awk` per run would be a fork inside the
      # loop, which is the cost this clock exists to avoid — though it would land between
      # two measured intervals rather than inside one.
      pairs="$pairs$start $end
"
    done
    # Validated rather than trusted: a shell whose $EPOCHREALTIME is not a number would
    # otherwise be reported as 0.00 ms, which reads as a measurement.
    printf '%s' "$pairs" | awk '
      { ms = ($2 - $1) * 1000; if (ms <= 0) bad = 1; if (best == "" || ms < best) best = ms }
      END { if (bad || best == "") { print "unavailable" } else { printf "%.2f", best } }'
    ;;
  py)
    # Timed inside python, not around two `python3` invocations. Reading the clock that way
    # put a whole interpreter startup — tens of milliseconds — inside an interval measuring
    # a program that takes about one.
    # shellcheck disable=SC2086
    python3 - "$binary" "$n" "$runs" $ARGV <<'PYTHON' 2>/dev/null || echo "unavailable"
import os, subprocess, sys, time

binary, parse_n, runs, *argv = sys.argv[1:]
env = dict(os.environ, PARSE_N=parse_n)
best = None
with open(os.devnull, "wb") as quiet:
    for _ in range(int(runs)):
        started = time.perf_counter_ns()
        subprocess.run([binary, *argv], stdout=quiet, env=env, check=False)
        elapsed = time.perf_counter_ns() - started
        if best is None or elapsed < best:
            best = elapsed
print("%.2f" % (best / 1e6))
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

# Instructions for `n` parses, cachegrind, with the runtime pinned to one thread.
#
# GOMAXPROCS=1 is what makes this a measurement. valgrind serializes every thread onto one
# core, and a Go runtime with more than one to schedule spends the wait spinning — so the
# count picks up instructions proportional to *wall time*, which under cachegrind is fifty
# times a normal run's and varies with whatever else the machine is doing. Unpinned, twenty
# cobra resolves read 56M on one run and 5,002M on the next. Pinned, three consecutive runs
# agreed to 0.1%.
instructions() {
  local binary=$1 n=$2
  # shellcheck disable=SC2086
  GOMAXPROCS=1 PARSE_N="$n" valgrind --tool=cachegrind --cache-sim=no --branch-sim=no \
    --cachegrind-out-file="$work/cachegrind.out.%p" "$binary" $ARGV 2>&1 |
    sed -n 's/.*I *refs: *//p' | tr -d ','
}

# Nanoseconds as something to read: µs to two significant figures until a parse costs
# milliseconds, which kong's does.
ns_cell() {
  awk -v ns="$1" 'BEGIN {
    if (ns < 1000) { printf "%.0f ns", ns }
    else if (ns < 100000) { printf "%.1f µs", ns / 1000 }
    else if (ns < 1000000) { printf "%.0f µs", ns / 1000 }
    else { printf "%.2f ms", ns / 1000000 }
  }'
}

# Each framework: the label the sweep prints, the parse-n binary, and how many resolves to
# amortize its instruction count over. Few enough that cachegrind's 50x slowdown stays
# inside a few seconds, many enough that the ±50,000-instruction startup jitter is a
# rounding error against the total.
#
# usage-go gets a thousand because one of its parses is the smallest thing here; kong gets
# two because one of them is 57 million instructions, and a thousand would take minutes.
#
# Two usage-go rows, for the reason `parse-n`'s own comment gives: the row in the comparison
# is the typed front door, because that is the whole of what the other three do, and the
# binder alone is reported separately rather than in their column.
frameworks=(
  "usage-go, argv -> struct|parse-n|1000"
  "usage-go, argv -> events|bind-n|1000"
  "cobra|parse-n-cobra|20"
  "urfave/cli v3|parse-n-urfave|20"
  "kong|parse-n-kong|2"
)

# The sweep, once, tab separated: label, then min, p01, p10 and median in nanoseconds.
sweep_tsv=""
if [ -z "$shadows_why" ]; then
  sweep_tsv=$("$work/sweep" -tsv)
fi

{
  printf '### Go harness\n\n'
  # shellcheck disable=SC2016 # the backticks are markdown, not command substitution
  printf 'Parsing `mise %s` against mise'"'"'s committed spec, through the tables in\n' "$ARGV"
  # shellcheck disable=SC2016
  printf '`benches/go/mise` and the same spec declared in each of the other three frameworks by\n'
  # shellcheck disable=SC2016
  printf '`xtask gen-shadow`. Reproduce with `mise run perf:go`.\n\n'

  printf '#### What a parse costs\n\n'
  if [ -n "$shadows_why" ]; then
    printf 'Not measured this run — %s.\n\n' "$shadows_why"
  else
    printf 'In-process parse throughput: every parser run repeatedly in one process, the\n'
    printf 'fastest of many short rounds reported. The same measurement, and the same\n'
    # shellcheck disable=SC2016
    printf 'estimator, that `benches/gate/src/bin/time-sweep.rs` takes for the Rust four.\n\n'
    printf '| | one parse | median | vs usage-go |\n|---|---:|---:|---:|\n'
    base=$(printf '%s\n' "$sweep_tsv" | awk -F'\t' '/^usage-go, argv -> struct/ { print $2 }')
    printf '%s\n' "$sweep_tsv" | while IFS=$'\t' read -r label min _p01 _p10 median; do
      ratio=""
      case $label in
      "usage-go, argv -> struct") ratio="" ;;
      "usage-go, argv -> events") ratio="" ;;
      *) ratio=$(awk -v a="$min" -v b="$base" 'BEGIN { printf "%.0fx", a / b }') ;;
      esac
      printf '| %s | %s | %s | %s |\n' \
        "$label" "$(ns_cell "$min")" "$(ns_cell "$median")" "$ratio"
    done
    printf '\n'
    printf 'The minimum is the estimator to want on a shared machine — noise from other\n'
    printf 'tenants is additive, and nothing another process does can make this one faster —\n'
    printf 'and short rounds are the ones an interruption can only spoil individually. The\n'
    printf 'median is beside it because that is where garbage collection shows up: the three\n'
    printf 'frameworks build a model per parse and the collector eventually charges for it,\n'
    printf 'which a minimum over short rounds mostly steps around.\n\n'
    printf 'Two rows for usage-go because it has two answers. `argv -> struct` is the front\n'
    printf 'door, and the row comparable to the other three: bind, apply the post-binding\n'
    printf 'rules, fill the typed structs. `argv -> events` is the binder alone, which no\n'
    printf 'other framework here has as a separate stage.\n\n'
    # Rendered from the same TSV the table above reads, rather than by running the sweep a
    # second time: two runs are two measurements, and printing one beside the other as
    # though they were the same one is how a table starts disagreeing with itself.
    printf '```\n'
    printf '%s\n' "$sweep_tsv" | awk -F'\t' '
      BEGIN { printf "%-40s%9s %9s %9s %9s\n", "", "min", "p01", "p10", "median" }
      { printf "%-40s%9d %9d %9d %9d  ns\n", $1, $2, $3, $4, $5 }'
    printf '```\n\n'
  fi

  printf '#### What a whole process costs\n\n'
  printf 'The number an adopter feels, and mostly not the parser: a Go process is ~0.95 ms\n'
  printf 'and ~950,000 instructions old by the time `main` runs, whatever it then parses.\n'
  printf 'Reported beside the parse rather than subtracted from it, because the subtraction\n'
  printf 'hides how much of a Go CLI'"'"'s latency no parser can win back.\n\n'

  if ! command -v valgrind >/dev/null 2>&1; then
    printf 'valgrind is not installed, so there are no instruction counts — wall clock only.\n\n'
  fi

  printf '| | instructions | whole process | binary |\n|---|---:|---:|---:|\n'
  floor_wall=$(wall_ms "$bin" 0)
  floor_instr="not measured"
  if command -v valgrind >/dev/null 2>&1; then
    floor_instr="$(printf "%'d" "$(instructions "$bin" 0)")"
  fi
  printf '| the Go runtime, parsing nothing | %s | %s | — |\n' \
    "$floor_instr" "$(wall_cell "$floor_wall")"
  for framework in "${frameworks[@]}"; do
    IFS='|' read -r label binary iters <<<"$framework"
    path=$work/$binary
    if [ ! -x "$path" ]; then
      printf '| %s | not measured — %s | | |\n' "$label" "$shadows_why"
      continue
    fi
    instr="not measured"
    if command -v valgrind >/dev/null 2>&1; then
      many=$(instructions "$path" "$iters")
      floor=$(instructions "$path" 0)
      instr="$(printf "%'d" "$(((many - floor) / iters))")"
    fi
    printf '| %s | %s | %s | %s MB |\n' \
      "$label" "$instr" "$(wall_cell "$(wall_ms "$path" 1)")" "$(size_mb "$path")"
  done
  printf '\n'
  printf 'The wall column is the fastest of %s whole processes. The floor and the row\n' "$runs"
  printf 'under it are one binary asked for nought parses and for one, and they differ by\n'
  printf 'less than a process launch varies: a usage-go parse is below the resolution of\n'
  printf 'this column, which is the honest thing for it to say rather than an ordering to\n'
  printf 'read. The same caution applies between cobra and urfave, which land a tenth of a\n'
  printf 'millisecond apart here and swap places between runs; the table above is where\n'
  printf 'those two are separated.\n\n'
  printf 'Instruction counts are cachegrind, amortized over 1,000 parses for usage-go and\n'
  printf 'fewer for the frameworks that cost three to five orders of magnitude more — 20\n'
  printf 'each for cobra and urfave, 2 for kong, because a thousand kong parses under\n'
  printf 'cachegrind would take minutes. They are amortized rather than differenced from a\n'
  printf 'single parse because Go'"'"'s startup varies run to run by ±50,000 instructions,\n'
  printf 'which is twenty times what a usage-go bind costs.\n'
} >"$out"
