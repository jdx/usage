# Rust Framework

::: warning Experimental — draft docs
The Rust framework is experimental. It is complete enough that `usage-cli` itself is built with it,
but attribute names and APIs may still change between releases. These docs are a draft: some of
what they document is still in open pull requests, and details may change before release.
:::

The Rust framework builds your CLI from Rust types. You declare commands, flags, and args as
structs and enums; a derive macro compiles that declaration into parse tables **and** a usage
spec. The same declaration that parses argv is the spec that generates your docs, manpages, and
shell completions.

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

## Installation

One dependency. Add `usage-rs` to your `Cargo.toml`, aliased to `usage`:

```toml
[dependencies]
usage = { package = "usage-rs", version = "6" }
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
| `usage-derive` | The derive macros: `Cli`, `Args`, `Subcommands`, `ValueEnum`                           |
| `usage-argv`   | The zero-allocation, zero-dependency runtime the derive emits code against             |
| `usage-test`   | Test helpers: what a command line parses to, what a page says, what a shell is offered |

### Cargo features

| Feature       | Default | What it enables                                                                                                   |
| ------------- | :-----: | ----------------------------------------------------------------------------------------------------------------- |
| `spec`        |   ✅    | Spec metadata and `to_kdl()`; gates the derives                                                                   |
| `help`        |   ✅    | `-h` / `--help` page rendering                                                                                    |
| `diagnostics` |   ✅    | clap-shaped error messages from `render_failure`                                                                  |
| `completions` |         | Shell completion scripts and the runtime completion protocol                                                      |
| `test`        |         | `usage::test`: parse and help assertions (a dev-dependency feature; completion assertions want `completions` too) |

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

- [Args and flags](/rust/args-and-flags) — field types, attributes, env vars, defaults
- [Subcommands](/rust/subcommands) — command enums, nesting, `flatten`, value enums
- [Dispatch](/rust/dispatch) — `Run`, `RunWith`, the async pair, and the generated `match`
- [Validation](/rust/validation) — choices, groups, `exclusive`, `delimiter`, conflicts
- [Help, version, and errors](/rust/help) — what the parser renders and how to hook it
- [Completions](/rust/completions) — static scripts and runtime completion
- [Spec output](/rust/spec) — the emitted KDL and usage-cli integration
- [Migrating from clap](/rust/migrating-from-clap) — mechanical rewrites and intentional API breaks
- [clap compatibility](/rust/clap-compatibility) — supported behavior, bridge losses, and non-goals

## Current limitations

The framework intentionally targets standard GNU-style CLIs, and a few clap features have no
equivalent yet:

- A declared `value_optional` needs either `default_missing` or an
  `Option<Option<T>>` field to define what a bare flag binds.
- Rust `value_parser` functions are not portable metadata. Values use `FromStr`; use
  `validate` for a portable expression rule and `validate_error` for its diagnostic.
- Long flags and subcommands require exact spellings. Diagnostics can suggest a close match, but
  usage does not accept prefixes whose meaning could change when another declaration is added.
- On Unix, `PathBuf` and `OsString` fields accept non-UTF-8 argv without changing a byte. String
  fields still report invalid UTF-8 precisely rather than replacing it; on Windows, values that
  cannot be converted safely are reported instead of using an unchecked reconstruction.
