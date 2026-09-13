# Contributing

The [contributing guide](https://usage.jdx.dev/contributing) covers project scope,
review expectations, and checks. Start from the repository root:

```sh
mise install
mise run build
mise run test
```

For website changes, run `mise run docs:dev` for a live preview and
`mise exec -- aube run docs:build` for the production build. Guides live in
`docs/`; `docs/cli/reference/` is generated from command help in the Rust source.

## Git hooks

Linting runs through [hk](https://hk.jdx.dev), configured in `hk.pkl`.
`mise install` provides the binary; installing the hooks themselves is per-clone:

```sh
hk install --mise
```

`--mise` runs the hooks through `mise x`, so the project's pinned tools are on
`PATH` even when Git is invoked from an editor rather than an activated shell.
The `pre-commit` hook fixes what it can, stages the result, and stashes
unstaged work while it runs. The same steps are available on demand as
`mise run lint` and `mise run lint-fix`, which wrap `hk check --all` and
`hk fix --all`.

## mbx build cache

`mise install` installs [mbx](https://mr-boxington.jdx.dev) 1.8. The normal
`mise run build`, `mise run test`, and `mise run lint` workflows activate its
transparent Cargo wrapper and therefore use the cache while invoking Cargo
normally, including the Cargo steps hk runs. Standalone Cargo commands require an activated mise shell. To bypass
mbx without skipping or weakening a check, prefix the
equivalent Cargo command with `MBX_DISABLE=1` and keep the mise tool environment:

```sh
MBX_DISABLE=1 mise exec -- cargo build --all
MBX_DISABLE=1 mise exec -- cargo test --all --all-features
MBX_DISABLE=1 mise exec -- cargo clippy --all --all-features --all-targets -- -D warnings
```

If bypassed Cargo succeeds where the wrapper fails, or mbx introduces a papercut, please start a
[mr-boxington Discussion](https://github.com/jdx/mr-boxington/discussions).
Include the repository and commit, operating system, `mbx --version`,
`mbx doctor`, and both commands and their output. Before posting, redact
secrets, absolute cache paths, remote URLs, namespaces, and other sensitive or
identifying details.
