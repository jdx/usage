# Contributing

See the [contributing guide](https://usage.jdx.dev/contributing).

## mbx build cache

The normal `mise run build`, `mise run test`, and `mise run lint:clippy`
workflows use [mbx](https://mr-boxington.jdx.dev) for compilation-heavy Cargo
work. If mbx appears to be the problem, use the equivalent Cargo commands to
unblock yourself without skipping or weakening the check:

```sh
cargo build --all
cargo test --all --all-features
cargo clippy --all --all-features --all-targets -- -D warnings
```

If Cargo succeeds where mbx fails, or mbx introduces a papercut, please start a
[mr-boxington Discussion](https://github.com/jdx/mr-boxington/discussions).
Include the repository and commit, operating system, `mbx --version`, both
commands and their output, the mbx cache summary, and an `MBX_BYPASS_LOG` when
relevant (for example, `MBX_BYPASS_LOG=mbx-bypasses.log mise run build`).
