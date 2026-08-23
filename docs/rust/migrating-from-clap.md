# Migrating from clap

If your CLI uses clap's derive API, most of the migration is a rename: swap the derives, change
`#[command]`/`#[arg]`/`#[value]` to `#[usage(...)]`, and let the compiler walk you through the
rest. usage is a typed parser with static metadata, not a compatibility layer around clap, so the
places where it diverges are deliberate — builder `Command`s and `ArgMatches` don't come along,
and a couple of parsing defaults are looser. This page covers the gaps worth knowing about up
front, then the rewrite itself.

## Compatibility gaps

Skim this list before you start. Everything else on the page is mechanical; these are the parts
that need a decision.

- **Runtime builders.** `Command`, `ArgMatches`, and `CommandFactory` are not part of the typed
  API. Move the declaration into derives and keep clap at whatever boundary still needs it — or
  reach for `usage-lib` if your CLI genuinely constructs itself at runtime.
- **Permissive defaults.** Unknown flags parse as values, and repeating a scalar flag is
  last-one-wins. Where clap's strictness matters, add `unknown_flags = "error"` and
  `args_override_self = false`.
- **`from_global`.** Unsupported. Read the global field from the type that declares it and pass
  it to the command as context.
- **`value_parser` callbacks.** A Rust closure can't travel in a portable spec. Use the field
  type's `FromStr`, a `ValueEnum`, literal choices, or a portable `validate` expression.
- **`value_optional`.** `#[usage(value_optional)]` changes help and spec presentation only. To
  actually accept a bare flag, bind it with `default_missing` or an `Option<Option<T>>` field.
- **Relationships across `flatten` and on positionals.** A few aren't available at binding time;
  keep a post-parse check for the cases described under
  [Subcommands and shared arguments](#subcommands-and-shared-arguments).
- **Prefix inference.** Intentionally unsupported. Long flags and subcommands must be spelled
  with their full name or a declared alias.
- **Help templates and styles.** clap templates and style palettes don't port as-is. Rewrite
  them against usage's ten [help sections](/rust/help#laying-a-page-out); usage chooses terminal
  styles automatically.

If you're migrating from a `clap::Command` value rather than from the Rust declaration,
`clap_usage::spec_with_report` detects recoverable losses. It cannot see state for which clap
exposes a setter but no getter — the `requires` family, `default_value_if`, and
`default_missing_value` — so audit those declarations by hand.

## Dependencies

Depend on the facade, not on `usage-derive` or `usage-argv` separately:

```toml
[dependencies]
usage = { package = "usage-rs", version = "6", features = ["completions"] }
```

The defaults include the derive, help rendering, and clap-shaped diagnostics. Add `completions`
if the binary generates or answers completion requests, and `validation` for portable validation
expressions.

During a prerelease migration, pin every producer and consumer to one revision — in particular,
the `usage-rs` that emits KDL and any installed `usage-cli` that renders it must match.

The swap also shrinks the build. On a minimal binary, clap 4.6.6 with `derive` compiles 8
third-party crates into the binary; usage's defaults plus `completions` compile 0. The whole
graph is 17 crates against 7, and usage's four non-usage crates — proc-macro2, quote, syn,
unicode-ident — are build-time only, already compiled by any project using serde's derive or
clap_derive itself. The one exception is the opt-in `validation` feature, which adds `expr-lang`.

## Derive mapping

The renames are one-to-one:

| clap                    | usage                           |
| ----------------------- | ------------------------------- |
| `#[derive(Parser)]`     | `#[derive(usage::Cli)]`         |
| `#[derive(Args)]`       | `#[derive(usage::Args)]`        |
| `#[derive(Subcommand)]` | `#[derive(usage::Subcommands)]` |
| `#[derive(ValueEnum)]`  | `#[derive(usage::ValueEnum)]`   |
| `#[command(...)]`       | `#[usage(...)]`                 |
| `#[arg(...)]`           | `#[usage(...)]`                 |
| `#[value(...)]`         | `#[usage(...)]`                 |

Rename every helper attribute when you replace the derive — usage rejects clap's helper
namespaces, and the compile error points at `#[usage(...)]`. A typical root migrates like this:

```rust
// before
use clap::{Parser, Subcommand};

#[derive(Parser)]
#[command(name = "tak", version)]
struct Cli {
    #[command(subcommand)]
    command: Command,
}

#[derive(Subcommand)]
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

```rust
// after
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
        #[usage(long)]
        bench: Option<String>,
        #[usage(long)]
        runs: Option<u32>,
    },
    Version,
}
```

The one addition is `unknown_flags = "error"`. usage treats unknown flag-like words as values by
default — useful for wrapper CLIs — so a command that should reject them, as clap does, says so.
Likewise, a repeated scalar flag is last-one-wins unless the command opts out with
`#[usage(args_override_self = false)]`. The clap bridge records clap's setting, so generated
specs keep the source command's policy either way.

### Command settings

Most command-level options keep their clap names and meanings, so this section is mostly
confirmation. The ones with a nuance worth knowing:

- `arg_required_else_help` checks whether the selected command received an argv token —
  environment and default fallbacks don't count.
- `disable_help_flag`, `disable_help_subcommand`, and `disable_version_flag` remove the
  synthesized entries. To put the built-in behavior on a flag you declare yourself — keeping that
  flag's own help text — set `#[usage(action = usage::ArgAction::HelpShort)]` (or `Help`,
  `HelpLong`, `HelpAll`, `Version`).
- `subcommand_negates_reqs` suppresses the parent's positive requirements while leaving conflicts
  and the child's own requirements active.
- `args_conflicts_with_subcommands` means a flag or positional bound on the parent prevents
  selecting a child command later in argv.
- `subcommand_precedence_over_arg` keeps clap's opt-in rule that a known child ends an
  in-progress variadic value owner.
- `allow_missing_positional` leaves earlier optional fields empty when only enough words remain
  for later required positionals.
- `no_binary_name` keeps clap's meaning for `try_parse_from`: every supplied word is an
  argument, with no argv0 to strip.
- `dont_delimit_trailing_values` stops `delimiter` splitting once parsing crosses the trailing
  boundary.
- `term_width` and `max_term_width` fix or cap the width help pages wrap to.
- Field-level `trailing_var_arg` is accepted as clap's spelling for a final greedy positional;
  it lowers to `double_dash = "automatic"`.
- The granular help-visibility options — `hide_default_value`, `hide_env`, `hide_env_values`,
  `hide_possible_values`, `hide_short_help`, `hide_long_help` — change presentation only, never
  defaults, environment fallback, or accepted values.
- `#[usage(rename_all = "snake_case")]` controls inferred field and subcommand names, and
  `rename_all_env` controls names generated by bare `#[usage(env)]`. An explicit `long`, `name`,
  or `env = "NAME"` still wins.

## Fields

Field attributes rename the same way. The common clap spellings and their usage equivalents,
side by side:

```rust
// before
use clap::ArgAction;

#[derive(clap::Args)]
struct Options {
    #[arg(short = 'v', long, action = ArgAction::Count)]
    verbose: u8,

    #[arg(long, env = "APP_COLOR", default_value = "auto")]
    color: String,

    #[arg(long, value_delimiter = ',')]
    tags: Vec<String>,

    #[arg(long, value_enum)]
    shell: Shell,

    #[arg(skip)]
    computed: bool,
}
```

```rust
// after
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

Requiredness follows the types, as in clap: a default makes a field optional in the generated
grammar, `Option<T>` is optional, and a bare `T` without a default is required.

One place usage asks for more than clap did: a flag whose value is optional. usage doesn't guess
clap's per-occurrence `num_args(0..=1)` semantics — say what a bare occurrence means:

```rust
#[usage(long, default_missing = "always", require_equals)]
color: Option<String>,
```

This accepts `--color` and `--color=never` while refusing a detached value.

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

Everything the derive can see comes over: unit variants, inline struct variants, nested enums,
flattened groups, and one `Args` type mounted under more than one command.

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

Tuple `Cli` and `Args` structs are not inferred, though — name the field and say whether it is
flattened:

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

Relationships that cross a flattened boundary are **lossy**: the common forms work, but a
declaring type cannot yet validate a selector supplied by a flattened sibling. Positional
relationships are **partial**: conflicts, requires, and conditional requiredness work, while
binding-time `overrides` and value-source `requires_if` remain flag-only. Keep a post-parse check
for those cases — the derive rejects selectors it can prove invalid rather than silently
weakening them.

## Parse entry points

clap tests usually include argv0; usage makes the choice explicit rather than guessing. Pick the
entry point that matches what you're handing it:

| Need                                       | Entry point                       |
| ------------------------------------------ | --------------------------------- |
| process argv, including help/version exits | `Cli::parse()`                    |
| words after argv0                          | `Cli::parse_from(&[&OsStr])`      |
| full argv with argv0, returning errors     | `Cli::parse_from_argv(&[&OsStr])` |
| clap-shaped call sites                     | `Cli::try_parse_from(&[&OsStr])`  |
| merging into a value you already have      | `cli.try_update_from(&[&OsStr])`  |

`parse_from` is the allocation-free primitive; `parse_from_argv` additionally applies multicall
basename routing. An embedder that must intercept the built-ins handles `usage::Error::Help` and
`usage::Error::Version` before dispatch, rendering them with `Cli::render_help` and
`Cli::render_failure` so computed `name` / `bin` appear in the page. `Cli::spec()` is the
portable identity.

`update_from` and `try_update_from` carry clap's names but state their merge rules explicitly,
because a parse can't be run backwards to seed itself from a value: a standing field satisfies a
relationship, the environment and defaults fill only what is empty, and a subcommand word naming
a different variant replaces it. See [Updating an existing value](/rust/update-from).

The `match cli.command { … }` a clap CLI writes after parsing can go too. Implement `usage::Run`
on each command struct, put `#[usage(run)]` on the enum, and the routing is generated. Commands
that need shared state implement `usage::RunWith<Ctx>` under `#[usage(run_with)]`; async commands
implement `usage::RunAsync` or `usage::RunAsyncWith<Ctx>` under `#[usage(run_async)]` /
`#[usage(run_async_with)]`. A clap unit or inline-struct variant is dispatched through the
`{Enum}{Variant}` struct the derive writes for it, and a catch-all `external_subcommand` becomes
`external = fallback` on the enum. See [Dispatch](/rust/dispatch).

## Help, specs, and completions

Doc comments stay the source of short and long help. `Cli::to_kdl()` emits the portable spec, and
`Cli::spec().view()` gives cold-path identity and metadata overlays without moving normal parsing
onto a dynamic command graph.

Command-level presentation settings keep their clap names in the usage namespace:

```rust
#[derive(usage::Cli)]
#[usage(
    subcommand_help_heading = "Actions",
    subcommand_value_name = "ACTION",
    next_line_help,
    flatten_help
)]
struct Cli {
    #[usage(subcommand)]
    command: Option<Command>,
}
```

Package metadata is declared on the root and travels with the generated spec and references:

```rust
#[derive(usage::Cli)]
#[usage(
    author = "Example Maintainers",
    license = "MIT OR Apache-2.0",
    repository = "https://example.com/tool"
)]
struct Cli;
```

For an embedded CLI whose program name is computed, pair `name = expression` with a portable
`name_spec = "literal"`, and `bin = expression` with `bin_spec = "literal"`. The expression is
used only by process output; the literal keeps generated specs reproducible.

With the `completions` feature, prefer the built-in completion surface over `clap_complete`:

```rust
let script = Cli::completion_script(usage::complete::Shell::Zsh);
```

`Cli::app().completion_app()` covers projections and sync or async runtime candidates. Async
callbacks return a future and run on the application's executor — usage does not bundle one.

## Intentional non-goals

usage does not reproduce `Command::new`, `augment_args`, `ArgMatches`, `FromArgMatches`, or the
complete public `CommandFactory` builder surface. A library that publicly returns `clap::Command`
must make a major-version API change, keep a clap-specific adapter, or expose a separately named
usage spec/view API.

These are architectural boundaries, not temporarily undocumented compatibility promises. The
static typed path is what keeps normal parsing allocation-free; `usage-lib` remains the dynamic
interpreter for applications that genuinely construct a CLI at runtime.
