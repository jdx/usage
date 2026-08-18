#!/usr/bin/env bash
set -euo pipefail

# The Rust toolchain is managed by rustup (not mise). rustup stores its default
# toolchain under RUSTUP_HOME, so pin these to the base image's locations in case
# the container-level env vars are not inherited. mise lives in ~/.local/bin.
export RUSTUP_HOME="${RUSTUP_HOME:-/usr/local/rustup}"
export CARGO_HOME="${CARGO_HOME:-/usr/local/cargo}"
export PATH="$HOME/.local/bin:$CARGO_HOME/bin:$PATH"

# Trust the repo's mise config and ensure every pinned tool (go, node, python,
# prettier, actionlint, insta, shellcheck, ...) is installed. Idempotent: when
# booting from the prebuilt environment the tools already exist and this is fast.
mise trust --yes
mise install

# Build the workspace so target/debug/usage exists. mise puts ./target/debug on
# PATH, and both the Go corpus tests (`mise run test:go`) and the shell
# completion integration tests invoke `usage` from PATH.
mise exec -- cargo build --all
