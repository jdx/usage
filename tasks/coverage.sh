#!/usr/bin/env bash

source <(cargo llvm-cov show-env --export-prefix)
set -euxo pipefail
cargo build
cargo test --all-features --all-targets
cargo llvm-cov report --codecov --output-path codecov.json
