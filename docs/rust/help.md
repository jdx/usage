# Help, Version, and Errors

::: warning Draft
This page is a draft and has not yet been human reviewed. Details may change.
:::

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
    Err(Error::Version { long }) => {
        let version = if long { LONG_VERSION } else { env!("CARGO_PKG_VERSION") };
        println!("ex {version}");
    }
    Err(err) => {
        eprint!("{}", usage::render_failure(Ex::spec(), &argv, &err));
        std::process::exit(2);
    }
}
```

`Error` is `#[non_exhaustive]` — always keep a fallback arm.

`version` supplies the concise `-V` response. `long_version` optionally supplies a richer
`--version` response, falling back to `version` when omitted. Computed expressions use matching
`version_spec` / `long_version_spec` literals so emitted KDL stays portable.

That match is also the post-parse interception point. An application that must
run an update notifier, rewrite output, or re-exec before answering help or
version does that work in the corresponding arm and then renders or returns.
Work that depends on a successfully built CLI value belongs after the `Ok(cli)`
arm and before command dispatch. There is no hidden callback lifecycle: the
embedding application owns the order explicitly, and `parse()` remains the
convenience entry point for CLIs that want immediate print-and-exit behavior.

### Deprecation warnings

A `deprecated` flag or command, or a value that arrived through a `deprecated_env` alias, is
something a parse has to say — and saying it is not the parser's decision to make about your
program's output. So the warnings come back as values:

```rust
let mut warnings = Vec::new();
let cli = Cli::parse_from_with_warnings(&argv, &mut warnings)?;
for warning in &warnings {
    // your logger, once it is up
    log::warn!("{}", usage::warn::render_warning(warning));
}
```

`parse_from_argv_with_warnings` and `try_parse_from_with_warnings` are the same thing beside
their own entry points. `parse()` — the one that already exits for `--help` — renders them to
stderr itself and carries on. The entry points without a sink collect nothing at all, so a
caller that does not want warnings pays for none of it.

Each `usage::warn::Warning` carries a `kind`, the `name` as the user spelled it, the author's
`message`, and the release milestones. A warning whose `deprecated_warn_at` this build's
`version` has not reached is not collected: declaring one is how an author says _not yet_. The
full rule, including what a default does not count as, is in
[the grammar](/spec/argv#warnings).

### Customizing the page

- `usage = "…"` on the root replaces the generated synopsis line(s) verbatim.
- `before_help`, `after_help`, `before_long_help`, `after_long_help` add text around the page.
- `example = "mycli deploy -e prod"` on a command declares a worked invocation, repeatable, and
  rendered as an Examples section. It takes a header and prose where the line needs them:
  `example("mycli deploy -e prod", header = "Basic deployment", help = "Deploy to production")`.
  Declared on a subcommand variant, it speaks for that command; declared on the `Args` type, it
  stands wherever the variant declares none. Unlike an Examples section written by hand into
  `after_long_help`, a declared example reaches the emitted spec, so docs, manpages and
  `usage lint` can all read it — the last of those checks that it still parses.
- `help_heading` on a field or subcommand variant groups it under a heading.
- `display_order = n` on a field or subcommand controls its position within a help section
  without changing positional parsing order.
- `next_line_help` on a command puts every argument, flag, and subcommand description beneath
  its usage instead of in an aligned column beside it.
- `flatten_help` replaces the command list with a synopsis and argument summary for every
  visible subcommand.
- `hide` removes an entry from help, docs, and completions while still parsing.

The rendered output matches what usage-lib renders from the same spec — the two renderers are
held to identical output over mise's 211 command pages in CI.

## Version

Declaring `version` (or bare `version`, which reads `CARGO_PKG_VERSION`) gives the root command
`--version` and `-V`, and lists them on its page. If your CLI declares its own `--version` or
`-V`, your spelling wins, the other still answers, and the page shows whichever is left — where
clap panics at startup for the same collision.

`parse()` prints `{bin} {version}` and exits `0`.

## Errors

`parse_from` returns `usage::Error`, which distinguishes every failure the grammar can produce:
`UnknownFlag`, `MissingFlagValue`, `UnexpectedArg`, `MissingRequired`, `DuplicateFlag`,
`InvalidChoice`, `InvalidValue`, `VarTooFew`/`VarTooMany`, `ConflictingFlags`, `MissingGroup`,
`MissingSubcommand`, `ArgRequiresDoubleDash`, `MissingArgsHelp`, `HelpAll`,
`SubcommandConflict`, and more — plus `Help` and `Version` as described above.

`render_failure(spec, argv, &err)` turns any of them into the message users see. Facade defaults
include `diagnostics`, so the message is clap-shaped out of the box:

```
error: unexpected argument '--wat' found

Usage: ex [OPTIONS] <FILE>

For more information, try '--help'.
```

Without `diagnostics` (for example after `default-features = false`, or when depending on
`usage-argv` alone), it falls back to the `Debug` form of the error — fine for internal tools,
not what you want to ship. `parse()` prints the rendered failure to **stderr** and exits **2**,
clap's status, so scripts that check for it keep working.
