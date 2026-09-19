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
mise exec -- hk install --mise
```

`--mise` runs the hooks through `mise x`, so the project's pinned tools are on
`PATH` even when Git is invoked from an editor. The `pre-commit` hook fixes what
it can and stages the result. The same steps run on demand as `mise run lint`
and `mise run lint-fix`.

## mbx build cache

mise wraps `cargo` with [mbx](https://mr-boxington.jdx.dev), so compiled work
is shared across checkouts. `mise run` tasks and `mise exec -- cargo …` always
use the wrapper; plain `cargo` does too once mise is activated in your shell
(`mise activate`).
