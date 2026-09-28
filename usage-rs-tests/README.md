# Workspace integration tests for usage-rs

This unpublished crate runs tests that need more than the published `usage-rs`
source archive provides:

- `facade` compares derived parsers and emitted specifications with `usage-lib`.
- `external` launches the nested Cargo projects under `usage-rs/tests/fixtures`
  to check dependency aliases, workspace inheritance, and runtime identity.

They run as part of `cargo test --all --all-features`, or directly with:

```sh
cargo test -p usage-rs-workspace-tests --all-features
```

Keep tests that only need published dependencies in `usage-rs` itself. Cargo
omits path-only dev-dependencies and nested packages from published archives;
putting either suite back there makes a crates.io source archive fail to test.
