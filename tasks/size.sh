#!/usr/bin/env bash
# Binary size: this checkout against the commit it branched from, stripped.
#
# Size is a property of the build, not of the machine, so unlike instruction counts it needs
# no dedicated runner and no noise margin to be trusted: two builds of the same source with
# the same toolchain come out the same size. The 1% gate is for what an adopter pays, not
# for noise.
#
# Never loosen GATE_PCT, drop a binary, or mark one ungated to get a pull request through —
# see "Performance Gate" in AGENTS.md, which applies here the same way. A change that
# genuinely costs bytes says so with these numbers, and the maintainer decides.
#
# Usage: tasks/size.sh [BASE_REF] [OUT]
#   BASE_REF  what to branch-point against (default: origin/main)
#   OUT       where to write the markdown report (default: stdout)
# Exits 1 when a gated binary grew by more than GATE_PCT.
set -euo pipefail

GATE_PCT=1
base_ref=${1:-origin/main}
out=${2:-/dev/stdout}

base=$(git merge-base "$base_ref" HEAD)
tmp=$(mktemp -d)
trap 'git worktree remove --force "$tmp/src" 2>/dev/null || true; rm -rf "$tmp"' EXIT

# One target directory for both builds, so crates.io dependencies compile once. Workspace
# crates still build twice: cargo keys them by path, and the base lives in a worktree.
export CARGO_TARGET_DIR=${CARGO_TARGET_DIR:-$PWD/target/size}

# `parse-usage` is mise's whole CLI through the derive, which is the size an adopter's binary
# pays for usage. `parse-none` is the same program without the parse, subtracted so the gate
# measures usage's share rather than std's. `parse-clap` is context and never gated.
BINS=(parse-none parse-usage parse-clap usage)

build() {
  local src=$1 dest=$2
  (cd "$src" && cargo build --release --locked -q -p gate -p usage-cli \
    --bin parse-none --bin parse-usage --bin parse-clap --bin usage)
  mkdir -p "$dest"
  for bin in "${BINS[@]}"; do
    cp "$CARGO_TARGET_DIR/release/$bin" "$dest/$bin"
    strip "$dest/$bin"
  done
}

bytes() { wc -c <"$1" | tr -d ' '; }

git worktree add -q --detach "$tmp/src" "$base"
build "$tmp/src" "$tmp/base"
build . "$tmp/head"

# The shadow is a fixture that grows on purpose (see tak.toml): a pull request that
# regenerates it changes what `parse-usage` is, so its size is reported but not gated.
fixture_changed=false
if ! git diff --quiet "$base" -- benches/mise.usage.kdl benches/shadows/mise; then
  fixture_changed=true
fi

failed=false
row() {
  local label=$1 base_bytes=$2 head_bytes=$3 gated=$4
  local delta=$((head_bytes - base_bytes)) pct verdict
  pct=$(awk -v b="$base_bytes" -v d="$delta" 'BEGIN { printf "%+.2f", b ? 100 * d / b : 0 }')
  verdict=reported
  if [ "$gated" = true ]; then
    verdict=ok
    if awk -v p="$pct" -v g="$GATE_PCT" 'BEGIN { exit !(p > g) }'; then
      verdict="**over ${GATE_PCT}%**"
      failed=true
    fi
  fi
  printf '| %s | %s | %s | %+'"'"'d | %s%% | %s |\n' \
    "$label" "$(printf "%'d" "$base_bytes")" "$(printf "%'d" "$head_bytes")" "$delta" "$pct" "$verdict"
}

size_of() { bytes "$tmp/$1/$2"; }
share() { echo $(($(size_of "$1" parse-usage) - $(size_of "$1" parse-none))); }

shadow_gated=true
[ "$fixture_changed" = true ] && shadow_gated=false

report=$tmp/report.md
# The backticks below are markdown, not command substitution.
# shellcheck disable=SC2016
{
  printf '### Binary size\n\n'
  printf 'Stripped release builds, `%s` against the merge base `%s`. ' "$(git rev-parse --short HEAD)" "$(git rev-parse --short "$base")"
  printf 'Gated rows fail above +%s%%.\n\n' "$GATE_PCT"
  printf '| binary | base | head | change | %% | gate |\n'
  printf '|---|---:|---:|---:|---:|---|\n'
  row '`usage` CLI' "$(size_of base usage)" "$(size_of head usage)" true
  row 'derived mise CLI, usage'"'"'s share' "$(share base)" "$(share head)" "$shadow_gated"
  row 'derived mise CLI, whole binary' "$(size_of base parse-usage)" "$(size_of head parse-usage)" false
  row 'clap mise CLI, whole binary' "$(size_of base parse-clap)" "$(size_of head parse-clap)" false
  printf '\n'
  printf '"usage'"'"'s share" is `parse-usage` minus `parse-none`: the same program with and without the parse.\n'
  if [ "$fixture_changed" = true ]; then
    printf '\nThis change regenerates the mise shadow, so the derived CLI is reported rather than gated: its size moved with the fixture.\n'
  fi
} >"$report"

cat "$report" >"$out"
[ "$failed" = false ]
