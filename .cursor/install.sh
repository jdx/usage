#!/usr/bin/env bash
set -euo pipefail

# Make mise and the Rust toolchain discoverable even in a non-login shell. The
# Dockerfile installs mise into ~/.local/bin and Rust (via rustup) into ~/.cargo;
# rustup's own defaults (~/.rustup, ~/.cargo) match, so no RUSTUP_HOME/CARGO_HOME
# overrides are needed.
export PATH="$HOME/.local/bin:$HOME/.cargo/bin:$PATH"

# Trust the repo's mise config and install every pinned tool from mise.toml
# (go, node, python, prettier, actionlint, insta, shellcheck, ...). Idempotent:
# already-present tools are a fast no-op.
mise trust --yes
mise install

# Build the workspace so target/debug/usage is on PATH (mise adds ./target/debug).
# The Go corpus tests (`mise run test:go`) and the shell-completion integration
# tests both invoke `usage` from PATH.
mise exec -- cargo build --all
