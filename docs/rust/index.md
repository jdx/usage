# Rust Framework

::: warning Experimental
Used by some of jdx's CLIs, but point releases may break. These docs have not been human reviewed.
:::

The Rust framework builds your CLI from Rust types. You declare commands, flags, and args as
structs and enums; a derive macro compiles that declaration into the parse tables the binary runs
on and a [usage spec](/spec/) it can print. Docs, manpages, and completions are generated from
that spec.

```rust
use usage::Cli;

/// A tool that does things
#[derive(Cli)]
#[usage(bin = "ex", version = "1.0")]
struct Cli {
    /// How many jobs to run at once
    #[usage(short = 'j', long, env = "EX_JOBS", default = "4")]
    jobs: Option<String>,

    /// Print more
    #[usage(short = 'v', long, count)]
    verbose: u8,

    /// Colorize output
    #[usage(long, negate = "--no-color", default = "true")]
    color: bool,

    /// Files to process
    files: Vec<String>,
}

fn main() {
    let cli = Cli::parse();
    // cli.jobs, cli.verbose, cli.color, cli.files are ready to use
}
```

Doc comments are the help text: the first paragraph becomes the short help shown by `-h`, the
whole comment becomes the long help shown by `--help`.

## Parser overhead

<UsageBenches lang="rust" embedded />

The derive emits the command tree as static tables at compile time. At runtime the parser walks
only the selected command path, scans that command's flags plus inherited globals, and writes
bindings into the result in place. It does not build a command tree, allocate a lookup map, or
touch help and spec metadata on a successful parse. A bare parse allocates nothing; an owned
value allocates only when argv actually supplies it. See [Parser performance](/rust/performance)
for the instruction counts, allocation tests, and benchmark limits.

## Installation

One dependency. Add `usage-rs` to your `Cargo.toml`, aliased to `usage`:

```toml
[dependencies]
usage = { package = "usage-rs", version = "5.1" }
```

That is the whole install: derives, the argv runtime, help, and clap-shaped errors come with the
defaults. The alias is supported directly — the derive resolves its runtime through the package
name, so depending on `usage-rs` under any name works.

Nothing third-party links into your binary — the only non-usage crates in the graph are the
derive's compiler, which runs at build time ([comparison with clap](/rust/migrating-from-clap#dependencies)).

`usage-rs` is a facade. Applications should depend on it alone. The split underneath stays
available for low-level adopters that want a thinner surface:

| Crate          | Role                                                                                   |
| -------------- | -------------------------------------------------------------------------------------- |
| `usage-rs`     | The one package an application depends on; re-exports the whole runtime                |
| `usage-derive` | The derive macros: `Cli`, `Args`, `Subcommands`, `ValueEnum`, `ArgGroup`               |
| `usage-argv`   | The zero-allocation, zero-dependency runtime the derive emits code against             |
| `usage-test`   | Test helpers: what a command line parses to, what a page says, what a shell is offered |
| `usage-config` | Layered settings resolution with provenance ([Settings](/rust/settings))               |

### Cargo features

| Feature       | Default | What it enables                                                                                                   |
| ------------- | :-----: | ----------------------------------------------------------------------------------------------------------------- |
| `spec`        |   ✅    | Spec metadata and `to_kdl()`; gates the derives                                                                   |
| `help`        |   ✅    | `-h` / `--help` page rendering                                                                                    |
| `diagnostics` |   ✅    | clap-shaped error messages from `render_failure`                                                                  |
| `completions` |         | Shell completion scripts and the runtime completion protocol (`complete` is an alias of this feature)             |
| `validation`  |         | Portable `validate` / `validate_error` expressions ([Validation](/rust/validation#portable-expressions))          |
| `test`        |         | `usage::test`: parse and help assertions (a dev-dependency feature; completion assertions want `completions` too) |
| `config`      |         | The `usage::Config` derive and the resolver as `usage::config` ([Settings](/rust/settings))                       |

`#[usage(completion)]` without the `completions` feature is a deliberate `compile_error!` that
tells you which feature to add. To drop diagnostics (or help) from a binary that does not want
them, turn defaults off and re-enable only what you need — or depend on `usage-argv` directly.

## Parse entry points

`#[derive(Cli)]` generates these on your struct:

```rust
// parse std::env::args; print help/version/errors and exit as appropriate
pub fn parse() -> Self;

// parse the given argv; hand errors (including help/version requests) back to you
pub fn parse_from<'v>(argv: &'v [&'v OsStr]) -> Result<Self, usage::Error<'static, 'v>>;

// the static parse tables and spec metadata
pub fn command() -> &'static usage::Command<'static>;
pub fn spec() -> &'static usage::spec::Spec<'static>;

// the usage spec as KDL
pub fn to_kdl() -> String;
```

`parse()` is the whole program shell: it prints the help page to stdout and exits `0` for
`-h`/`--help`, prints `{bin} {version}` and exits `0` for `--version`, and prints a rendered
failure to stderr and exits `2` — clap's exit status, so scripts that check for it keep working.
`parse_from` gives you the same machinery without the process control; see
[Help, version, and errors](/rust/help) for handling its `Err` variants.

## Updating a value you already have

A CLI parsed more than once — a REPL reading a line at a time, a daemon reconfigured while it
runs — merges a command line into the value it already holds rather than building a new one:

```rust
// merge argv into self; print help/version/errors and exit as `parse()` does
pub fn update_from<'v>(&mut self, argv: &[&'v OsStr]);

// the same, handing errors back
pub fn try_update_from<'v>(&mut self, argv: &[&'v OsStr])
    -> Result<(), usage::Error<'static, 'v>>;
```

`update_from_argv` and `try_update_from_argv` are the `parse_from_argv` counterparts: they strip
argv0 and apply multicall applet selection.

A parse cannot be run backwards — a `String` field says nothing about the word it was made from —
so what you already hold is read from the struct itself, and the rules are stated rather than
inherited from a fresh parse:

- **Relationships see the standing value.** `required`, `requires`, `conflicts` and the rest
  treat a field that already holds a value as present, so a required flag need not be repeated
  and a standing flag still conflicts with a new one. What is validated is the union of both
  inputs.
- **The environment and declared defaults fill only what is empty.** An update never clobbers a
  value you set deliberately, and an update whose argv says nothing cannot change anything.
- **A collection is replaced when this argv mentions it**, and left alone when it does not.
  Appending was rejected because it leaves no way to clear a field.
- **A different subcommand replaces the variant** whole, discarding the old variant's fields;
  the same subcommand merges field by field.

Nothing is merged until every check has passed, so a `try_update_from` that returns `Err` leaves
the value exactly as it was.

Two things a standing value cannot answer, because the bytes it was parsed from are gone: a check
about what a value _is_ — a choice list, a `validate` expression — is skipped for a field this
argv did not supply, and a `requires_if` or `default_value_if` comparing against a particular
value does not match one that merely stands. A field whose type has nowhere to put "absent", such
as a plain `String`, always counts as present.

What runs afterwards can be generated too. A command implements `Run`, its subcommand enum
says `#[usage(run)]`, and the `match` that routes argv to the code carrying it out is written
from the same declaration:

```rust
#[derive(Cli)]
#[usage(bin = "ex")]
struct Ex {
    #[usage(subcommand)]
    command: Commands,
}

#[derive(Subcommands)]
#[usage(run)]
enum Commands {
    Install(Install),
}

impl Run for Install {
    type Output = miette::Result<()>;
    fn run(self) -> Self::Output {
        install(&self.tools, self.force)
    }
}

fn main() -> miette::Result<()> {
    Ex::parse().command.run()
}
```

`RunWith<Ctx>` and `#[usage(run_with)]` are the same for a CLI that hands its commands shared
state, and `RunAsync` / `RunAsyncWith<Ctx>` with `#[usage(run_async)]` / `#[usage(run_async_with)]`
are the async pair. See [Dispatch](/rust/dispatch).

## One declaration, every artifact

Because the derive also emits a usage spec, everything on this site that consumes a spec works
with your CLI — and you do not have to wire anything up for it. Every binary answers
`__usage_spec__` with its own spec:

```bash
mycli __usage_spec__ > mycli.usage.kdl
usage g markdown -f mycli.usage.kdl --out-dir docs
usage g manpage -f mycli.usage.kdl > mycli.1
usage g completion bash mycli --file mycli.usage.kdl
```

`Cli::to_kdl()` is the same document in-process, for a build script or a checked-in artifact.

See [Spec output](/rust/spec) for the round-trip guarantees, what the emitted KDL looks like, and
how to opt out of the endpoint.

## Where to go next

- [Quickstart](/rust/quickstart) — a small CLI from declaration to generated docs, end to end
- [Args and flags](/rust/args-and-flags) — field types, attributes, env vars, defaults
- [Subcommands](/rust/subcommands) — command enums, nesting, `flatten`, value enums
- [Dispatch](/rust/dispatch) — `Run`, `RunWith`, the async pair, and the generated `match`
- [Validation](/rust/validation) — choices, groups, `exclusive`, `delimiter`, conflicts, portable `validate`
- [Help, version, and errors](/rust/help) — what the parser renders and how to hook it
- [Completions](/rust/completions) — static scripts and runtime completion
- [Settings](/rust/settings) — settings declared in code: `usage::Config` and layered resolution
- [Testing](/rust/testing) — assert parses, help pages, and completions with no process spawned
- [Spec output](/rust/spec) — the emitted KDL and usage-cli integration
- [Migrating from clap](/rust/migrating-from-clap) — mechanical rewrites and intentional API breaks
- [clap compatibility](/rust/clap-compatibility) — supported behavior, bridge losses, and non-goals
- [Performance](/rust/performance) — what a parse costs, measured at mise's scale

## Current limitations

The framework intentionally targets standard GNU-style CLIs, and a few clap features have no
equivalent yet:

- A declared `value_optional` needs either `default_missing` or an
  `Option<Option<T>>` field to define what a bare flag binds.
- Rust `value_parser` functions are not portable metadata. Values use `FromStr`; use
  `validate` / `validate_error` (the opt-in `validation` feature) for a portable expression
  rule and its diagnostic.
- Long flags and subcommands require exact spellings. Diagnostics can suggest a close match, but
  usage does not accept prefixes whose meaning could change when another declaration is added.
- Completion scripts cover bash, fish, Nushell, PowerShell, and zsh. Elvish is not supported; a
  clap application publishing an Elvish script must keep `clap_complete` for that one artifact.
- `help_template` has no equivalent yet; see the
  [compatibility matrix](/rust/clap-compatibility) for the full audited list.
- On Unix, `PathBuf` and `OsString` fields accept non-UTF-8 argv without changing a byte. String
  fields still report invalid UTF-8 precisely rather than replacing it; on Windows, values that
  cannot be converted safely are reported instead of using an unchecked reconstruction.
