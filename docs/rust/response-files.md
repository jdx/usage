# Response files

Large generated command lines can be placed in a response file and expanded before parsing.
This is opt-in so ordinary parsing performs no filesystem access or allocation.

```toml
[dependencies]
usage = { package = "usage-rs", version = "6", features = ["response-files"] }
```

```rust
use std::ffi::OsStr;
use usage::Cli;

let expanded = usage::response::expand(std::env::args_os().skip(1))?;
let argv: Vec<&OsStr> = expanded.iter().map(|word| word.as_os_str()).collect();
let cli = MyCli::parse_from(&argv)?;
```

An argument beginning with `@` names a UTF-8 file of shell-style words:

```text
--format json
--output "release report.json"
@more-options.args
```

Nested paths are resolved relative to the file containing them. `@@value` produces the literal
argument `@value`; a lone `@` and non-UTF-8 arguments pass through unchanged. Includes are limited
to 16 levels by default and cycles are rejected. Use `expand_with` and `Options` to select another
depth limit.

Expansion deliberately happens before `Cli::parse_from` rather than inside the parser. This keeps
filesystem policy visible to the application, lets callers decide whether response files are
allowed in privileged contexts, and preserves usage-argv's normal zero-allocation path.
