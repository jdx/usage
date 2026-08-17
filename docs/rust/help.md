# Help, Version, and Errors

## Help

`-h` and `--help` are supplied by the parser — you never declare them. They aren't written into
the spec either, so the help page never disagrees with the spec about what exists. If your CLI
declares its own `--help`, your declaration wins for that spelling.

`-h` renders the short page, `--help` the long page: the first paragraph of each doc comment
versus the whole comment, `long_help` over `help`, `long_about` over `about`.

With `parse()`, help is handled for you — printed to stdout, exit `0`. With `parse_from`, a help
request comes back as an _error_, because a parse that stopped to print help has not produced a
value (clap models it the same way):

```rust
use usage::{help, Error};

match Ex::parse_from(&argv) {
    Ok(cli) => run(cli),
    Err(Error::Help { cmd, long }) => {
        print!("{}", help::render(Ex::spec(), cmd, long).unwrap());
    }
    Err(Error::Version) => {
        println!("ex {}", env!("CARGO_PKG_VERSION"));
    }
    Err(err) => {
        eprint!("{}", usage::render_failure(Ex::spec(), &argv, &err));
        std::process::exit(2);
    }
}
```

`Error` is `#[non_exhaustive]` — always keep a fallback arm.

### Customizing the page

- `usage = "…"` on the root replaces the generated synopsis line(s) verbatim.
- `before_help`, `after_help`, `before_long_help`, `after_long_help` add text around the page —
  `after_long_help` is the conventional home for an Examples section.
- `help_heading` on a field groups it under a heading.
- `hide` removes an entry from help, docs, and completions while still parsing.

The rendered output matches what usage-lib renders from the same spec — the two renderers are
held to identical output over mise's 211 command pages in CI.

## Version

Declaring `version` (or bare `version`, which reads `CARGO_PKG_VERSION`) gives the root command
`--version` and `-V`. Neither is listed in help. If your CLI declares its own `--version` or
`-V`, your spelling wins and the other still answers — where clap panics at startup for the
same collision.

`parse()` prints `{bin} {version}` and exits `0`.

## Errors

`parse_from` returns `usage::Error`, which distinguishes every failure the grammar can produce:
`UnknownFlag`, `MissingFlagValue`, `UnexpectedArg`, `MissingRequired`, `DuplicateFlag`,
`InvalidChoice`, `InvalidValue`, `VarTooFew`/`VarTooMany`, `ConflictingFlags`, `MissingGroup`,
`MissingSubcommand`, `ArgRequiresDoubleDash`, and more — plus `Help` and `Version` as described
above.

`render_failure(spec, argv, &err)` turns any of them into the message users see. With the
`diagnostics` feature enabled the message is clap-shaped:

```
error: unexpected argument '--wat' found

Usage: ex [OPTIONS] <FILE>

For more information, try '--help'.
```

Without `diagnostics`, it falls back to the `Debug` form of the error — fine for internal tools,
not what you want to ship. `parse()` prints the rendered failure to **stderr** and exits **2**,
clap's status, so scripts that check for it keep working.
