# Rust Framework

::: warning Experimental
Used by some of jdx's CLIs, but point releases may break.
:::

`usage-rs` is a fast, typed framework for building complete command-line applications in Rust.
Declare commands, flags, arguments, and settings with familiar structs and enums, and get
first-class environment and config-file resolution, advanced shell completions, portable
validation, negation flags, typed argument groups, categorized subcommands, and more.

In the mise-scale benchmark it parses hundreds of times faster than clap, with no third-party
runtime crates and a 1.6 MB stripped binary versus clap's 3.1 MB. See the
[performance results](/rust/performance) and [clap migration guide](/rust/migrating-from-clap).

The same declaration also becomes a portable [usage spec](/spec/) that the binary can print.
`usage-cli` turns it into documentation, manpages, and completions—the same toolchain used across
jdx's CLIs.

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

## Installation

One dependency. Add `usage-rs` to your `Cargo.toml`, aliased to `usage`:

```toml
[dependencies]
usage = { package = "usage-rs", version = "6" }
```

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
| `usage-config` | Layered settings resolution with provenance ([Configuration](/rust/configuration))     |

### Cargo features

| Feature          | Default | What it enables                                                                                          |
| ---------------- | :-----: | -------------------------------------------------------------------------------------------------------- |
| `spec`           |   ✅    | Spec metadata and `to_kdl()`; gates the derives                                                          |
| `help`           |   ✅    | `-h` / `--help` page rendering                                                                           |
| `diagnostics`    |   ✅    | clap-shaped error messages from `render_failure`                                                         |
| `completions`    |         | Shell completion scripts and the runtime completion protocol (`complete` is an alias of this feature)    |
| `validation`     |         | Portable `validate` / `validate_error` expressions ([Validation](/rust/validation#portable-expressions)) |
| `test`           |         | `usage::test`: command output, parse, and help assertions (completion assertions want `completions` too) |
| `config`         |         | The `usage::Config` derive and the resolver as `usage::config` ([Configuration](/rust/configuration))    |
| `response-files` |         | Explicit `@file` argument expansion as `usage::response` ([Response files](/rust/response-files))        |

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

For programs that parse more than once, see
[Updating an existing value](/rust/update-from).

## Generated dispatch

`#[usage(run)]` generates the `match` that routes parsed subcommands to their `Run`
implementations. Context and async variants are covered in [Dispatch](/rust/dispatch).

## One declaration, every artifact

Because the derive also emits a usage spec, [usage-cli](/cli/) can generate documentation,
manpages, and shell completions from your CLI. Every binary answers `__usage_spec__` with its
own spec:

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
- [Updating values](/rust/update-from) — merge another command line into an existing value
- [Subcommands](/rust/subcommands) — command enums, nesting, `flatten`, value enums
- [Dispatch](/rust/dispatch) — `Run`, `RunWith`, the async pair, and the generated `match`
- [Validation](/rust/validation) — choices, groups, `exclusive`, `delimiter`, conflicts, portable `validate`
- [Help, version, and errors](/rust/help) — what the parser renders and how to hook it
- [Completions](/rust/completions) — static scripts and runtime completion
- [Configuration](/rust/configuration) — settings declared in code: `usage::Config` and layered resolution
- [Response files](/rust/response-files) — opt-in, nested `@file` argument expansion
- [Testing](/rust/testing) — run commands or assert directly on parsing, help, and completions
- [Spec output](/rust/spec) — the emitted KDL and usage-cli integration
- [Migrating from clap](/rust/migrating-from-clap) — mechanical rewrites and intentional API breaks
- [Performance](/rust/performance) — what a parse costs, measured at mise's scale
