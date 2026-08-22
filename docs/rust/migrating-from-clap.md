# Migrating from clap

The Rust framework is a typed parser with static metadata, not a compatibility layer around
clap. Most derive-based CLIs migrate mechanically. Builder APIs and `ArgMatches` are intentional
API breaks: move their behavior into typed declarations or keep clap at that boundary.

## Compatibility gaps

Check these before starting a migration:

| Difference                                                                                    | What to do                                                                                                                      |
| --------------------------------------------------------------------------------------------- | ------------------------------------------------------------------------------------------------------------------------------- |
| Runtime `Command` builders, `ArgMatches`, and `CommandFactory` are not part of the typed API. | Move the declaration to derives, keep clap at that boundary, or use `usage-lib` for a CLI that is genuinely dynamic.            |
| Unknown flags and repeated scalar flags are permissive by default.                            | Add `unknown_flags = "error"` and `args_override_self = false` where clap's strict behavior matters.                            |
| `from_global` is unsupported.                                                                 | Read the global field from its declaring type and pass it to the command as context.                                            |
| Typed `value_parser` callbacks cannot enter a portable spec.                                  | Use the field type's `FromStr`, a `ValueEnum`, literal choices, or a portable `validate` expression.                            |
| `#[usage(value_optional)]` changes help and spec presentation only.                           | Bind a bare flag with `default_missing` or an `Option<Option<T>>` field.                                                        |
| Some relationships through `flatten` or on positionals are not available at binding time.     | Keep a post-parse check for the cases described under [Subcommands and shared arguments](#subcommands-and-shared-arguments).    |
| Prefix inference is intentionally unsupported.                                                | Long flags and subcommands must use a full name or declared alias.                                                              |
| clap help templates and style palettes are not portable as-is.                                | Rewrite templates using usage's six [help sections](/rust/help#laying-a-page-out); usage chooses terminal styles automatically. |
| Completion generation does not target Elvish.                                                 | Keep `clap_complete` or another generator for that artifact.                                                                    |

If a migration begins from a `clap::Command` rather than the Rust declaration,
`clap_usage::spec_with_report` detects recoverable losses. It cannot report state for which clap
exposes a setter but no getter, including the `requires` family, `default_value_if`, and
`default_missing_value`; audit those declarations directly.

## Dependencies

Depend on the facade rather than on `usage-derive` or `usage-argv` separately:

```toml
[dependencies]
usage = { package = "usage-rs", version = "6", features = ["completions"] }
```

The defaults include the derive, help, and clap-shaped diagnostics. Add `validation` for portable
validation expressions and `completions` when the binary generates or answers completion
requests.

During a prerelease migration, pin every producer and consumer to one revision. In particular,
the `usage-rs` dependency that emits KDL and any installed `usage-cli` that renders that KDL must
use the same revision.

The migration also shrinks the dependency graph (`cargo tree` on a minimal binary, clap 4.6.6
with `derive` against the defaults plus `completions`):

|                                              | clap | usage |
| -------------------------------------------- | ---: | ----: |
| third-party crates compiled into your binary |    8 |     0 |

The whole graph is 17 crates against 7: usage's four non-usage crates — proc-macro2, quote, syn,
unicode-ident — are build-time only and already compiled by any project using serde's derive or
clap_derive itself. The opt-in `validation` feature is the exception; it adds `expr-lang`.

## Derive mapping

| clap                    | usage                                                   |
| ----------------------- | ------------------------------------------------------- |
| `#[derive(Parser)]`     | `#[derive(usage::Cli)]`                                 |
| `#[derive(Args)]`       | `#[derive(usage::Args)]`                                |
| `#[derive(Subcommand)]` | `#[derive(usage::Subcommands)]`                         |
| `#[derive(ValueEnum)]`  | `#[derive(usage::ValueEnum)]`                           |
| `#[command(...)]`       | `#[usage(...)]`                                         |
| `#[arg(...)]`           | `#[usage(...)]`, or keep a supported migration spelling |
| `#[value(...)]`         | `#[usage(...)]`, or keep supported names and aliases    |

`#[arg(long)]` is accepted on fields and inline subcommand variants, so the first pass can keep
small diffs. Prefer `#[usage(...)]` for usage-only behavior and for the final declaration.

```rust
use usage::{Cli, Subcommands};

#[derive(Cli)]
#[usage(bin = "tak", version, unknown_flags = "error")]
struct Cli {
    #[usage(subcommand)]
    command: Command,
}

#[derive(Subcommands)]
enum Command {
    /// Benchmark a command.
    Run {
        #[arg(long)]
        bench: Option<String>,
        #[arg(long)]
        runs: Option<u32>,
    },
    Version,
}
```

Unknown flags are values by default, which is useful for wrapper CLIs. Add
`unknown_flags = "error"` on each command where unknown flag-like words must be rejected.

### Familiar field attributes

Many derive attributes migrate unchanged; `#[arg(...)]` remains accepted while a CLI is being
converted:

| clap declaration                                      | usage behavior or spelling                                       |
| ----------------------------------------------------- | ---------------------------------------------------------------- |
| `visible_alias`, `hide_*`, `last`, `trailing_var_arg` | Accepted with their existing meanings                            |
| `requires_if` and the other relationship attributes   | Accepted; see the cross-boundary limits below                    |
| `skip`                                                | Accepted; the field is filled from `Default`                     |
| `num_args = n` / `num_args = a..=b`                   | Exact or ranged `Vec` cardinality                                |
| `value_hint = ValueHint::…`                           | The full stable `ValueHint` vocabulary is accepted               |
| `allow_hyphen_values`, `allow_negative_numbers`       | Accepted with their existing token policies                      |
| `value_terminator`, `require_equals`                  | Accepted with their existing parsing behavior                    |
| `global`                                              | May appear once per command level; the innermost occurrence wins |
| `default_missing_value = "…"`                         | Write `default_missing = "…"`                                    |
| `default_value_if(…, ArgPredicate::IsPresent, value)` | Write `default_if(other, value)`                                 |
| `default_value_if(…, value, default)`                 | Write `default_if(other, value, default)`                        |

The native forms and their exact runtime behavior are documented under
[Args and flags](/rust/args-and-flags).

Repeated scalar flags also use permissive last-one-wins behavior by default. Add
`#[command(args_override_self = false)]` on commands that should reject a second occurrence.
The clap bridge records clap's setting, so generated specs retain the source command's policy.

`#[command(arg_required_else_help)]` migrates in place. usage checks whether the selected
command received an argv token; environment and default fallbacks do not count.

Help and version entry points migrate in place too. `disable_help_flag`,
`disable_help_subcommand`, and `disable_version_flag` remove the synthesized entries, while
`#[arg(action = usage::ArgAction::HelpShort)]` (or `Help`, `HelpLong`, `HelpAll`, and `Version`) can put
the action on any declared flag and keep that flag's own help text.

`#[command(subcommand_negates_reqs)]` also migrates in place. Selecting a child suppresses
the parent's positive requirements while leaving conflicts and the child's requirements active.

`#[command(args_conflicts_with_subcommands)]` migrates in place as well. A flag
or positional bound on the parent prevents selecting a later child command.

`#[command(subcommand_precedence_over_arg)]` retains clap's opt-in rule that a
known child ends an in-progress variadic value owner.

`#[command(allow_missing_positional)]` also migrates in place. When only enough
words remain for later required positionals, earlier optional fields stay empty.

Granular help visibility attributes migrate in place as well:
`hide_default_value`, `hide_env`, `hide_env_values`, `hide_possible_values`,
`hide_short_help`, and `hide_long_help`. They change presentation without changing
defaults, environment fallback, or accepted values.

Container casing also migrates in place. `#[command(rename_all = "snake_case")]` controls
inferred field or subcommand names, and `rename_all_env` controls names generated by bare
`#[arg(env)]`; an explicit `long`, `name`, or `env = "NAME"` still wins.

## Fields

Common mappings retain their meaning:

```rust
#[derive(usage::Args)]
struct Options {
    #[usage(short = 'v', long, count)]
    verbose: u8,

    #[usage(long, env = "APP_COLOR", default = "auto")]
    color: String,

    #[usage(long, delimiter = ',')]
    tags: Vec<String>,

    #[usage(long, value_enum)]
    shell: Shell,

    #[usage(skip)]
    computed: bool,
}
```

Defaults make a field optional in the generated grammar. `Option<T>` is also optional; `T`
without a default is required. A bare optional-value flag needs an explicit meaning:

```rust
#[usage(long, default_missing = "always", require_equals)]
color: Option<String>,
```

That accepts `--color` and `--color=never`, while refusing a detached value. usage does not
guess clap's per-occurrence `num_args(0..=1)` semantics.

For validation that must survive KDL emission, use a portable expression instead of a Rust
`value_parser` callback:

```rust
#[usage(
    long,
    validate = "int(value) >= 1 && int(value) <= 65535",
    validate_error = "must be a valid port"
)]
port: Option<u16>,
```

## Subcommands and shared arguments

Unit commands, inline struct variants, nested enums, flattened groups, and one `Args` type mounted
under more than one command are supported:

```rust
#[derive(usage::Args)]
struct RemoteArgs {
    #[usage(long, default = "origin")]
    remote: String,
}

#[derive(usage::Subcommands)]
enum Command {
    Push(RemoteArgs),
    Init(RemoteArgs),
    Doctor,
}
```

Tuple `Cli` and `Args` structs are not inferred. Name the field and say whether it is flattened:

```compile_fail
#[derive(usage::Args)]
struct Ambiguous(CommonArgs);
```

```rust
#[derive(usage::Args)]
struct Explicit {
    #[usage(flatten)]
    common: CommonArgs,
}
```

Relationships that cross a flattened boundary are **lossy**: common forms work, but a declaring
type cannot yet validate a selector supplied by a flattened sibling. Positional relationships
are **partial**: conflicts, requires, and conditional requiredness work; binding-time `overrides`
and value-source `requires_if` remain flag-only. Keep a post-parse check for those cases; the
derive rejects selectors it can prove invalid instead of silently weakening them.

## Parse entry points

clap tests usually include argv0. Choose the matching entry point explicitly:

| Need                                       | Entry point                         |
| ------------------------------------------ | ----------------------------------- |
| process argv, including help/version exits | `Cli::parse()`                      |
| words after argv0                          | `Cli::parse_from(&[&OsStr])`        |
| full argv with argv0, returning errors     | `Cli::parse_from_argv(&[OsString])` |
| clap-shaped call sites                     | `Cli::try_parse_from(iter)`         |
| merging into a value you already have      | `cli.try_update_from(&[&OsStr])`    |

`parse_from` is the allocation-free primitive. `parse_from_argv` also applies multicall basename
routing. Handle `usage::Error::Help` and `usage::Error::Version` before dispatch when an embedder
must intercept those built-ins. Render those with `Cli::render_help` and `Cli::render_failure` so
computed `name` / `bin` appear in the page; `Cli::spec()` is the portable identity.

`update_from` and `try_update_from` carry clap's names but state their merge rules explicitly,
because a parse cannot be run backwards to seed itself from a value: a standing field satisfies a
relationship, the environment and defaults fill only what is empty, and a subcommand word naming a
different variant replaces it. See [Updating an existing value](/rust/update-from).

The `match cli.command { … }` a clap CLI writes after parsing can go too: implement
`usage::Run` on each command struct, say `#[usage(run)]` on the enum, and the routing is
generated. Commands that need shared state implement `usage::RunWith<Ctx>` and the enum says
`#[usage(run_with)]`; async commands implement `usage::RunAsync` or `usage::RunAsyncWith<Ctx>`
under `#[usage(run_async)]` / `#[usage(run_async_with)]`. A clap unit or inline-struct variant
is dispatched through the `{Enum}{Variant}` struct the derive writes for it. A catch-all
`external_subcommand` names `external = fallback` on the enum. See [Dispatch](/rust/dispatch).

## Help, specs, and completions

Doc comments remain the source of short and long help. `Cli::to_kdl()` emits the portable spec;
`Cli::spec().view()` provides cold-path identity and metadata overlays without moving normal
parsing onto a dynamic command graph.

Command-level presentation settings keep their clap spellings:

```rust
#[derive(usage::Cli)]
#[command(
    subcommand_help_heading = "Actions",
    subcommand_value_name = "ACTION",
    next_line_help,
    flatten_help
)]
struct Cli {
    #[command(subcommand)]
    command: Option<Command>,
}
```

Package metadata is declared on the root and travels with the generated spec and references:

```rust
#[derive(usage::Cli)]
#[command(
    author = "Example Maintainers",
    license = "MIT OR Apache-2.0",
    repository = "https://example.com/tool"
)]
struct Cli;
```

For an embedded CLI whose program name is computed, pair `name = expression` with a portable
`name_spec = "literal"`, and `bin = expression` with `bin_spec = "literal"`. The expression is
used only by process output; the literal keeps generated specs reproducible.

With the `completions` feature, prefer the built-in completion surface:

```rust
let script = Cli::completion_script(usage::complete::Shell::Zsh);
```

Use `Cli::app().completion_app()` for projections and sync or async runtime candidates. Async
callbacks return a future and run on the application's executor; usage does not bundle one.

## Intentional non-goals

usage does not reproduce `Command::new`, `augment_args`, `ArgMatches`, `FromArgMatches`, or the
complete public `CommandFactory` builder surface. A library that publicly returns
`clap::Command` must make a major-version API change, retain a clap-specific adapter, or expose a
separately named usage spec/view API.

These are architectural boundaries, not temporarily undocumented compatibility promises. The
static typed path is what keeps normal parsing allocation-free; `usage-lib` remains the dynamic
interpreter for applications that genuinely construct a CLI at runtime.
