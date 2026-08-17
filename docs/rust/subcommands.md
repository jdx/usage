# Subcommands

::: warning Draft
This page is a draft. Some of what it documents is still in open pull requests, and details may
change before release.
:::

Subcommands are an enum. Each variant wraps a `#[derive(Args)]` struct (or nothing), and the
enum derives `Subcommands`:

```rust
/// A tool that does things
#[derive(Cli)]
#[usage(bin = "ex", version = "1.0")]
struct Ex {
    /// Say more
    #[usage(short = 'v', long, global)]
    verbose: bool,

    /// What to do
    #[usage(subcommand)]
    command: Option<Commands>,
}

#[derive(Subcommands)]
enum Commands {
    /// Install a tool
    Install(Install),
    /// Run a task
    #[usage(name = "run")]
    RunTask(Run),
}

#[derive(Args)]
struct Install {
    /// Overwrite an existing install
    #[usage(short = 'f', long)]
    force: bool,
    /// What to install
    tools: Vec<String>,
}
```

- `Option<Commands>` makes the subcommand optional; a bare `Commands` field makes it
  **required** (`subcommand_required` in the emitted spec).
- Variant names kebab-case into command names; override with `#[usage(name = "…")]`.
- A variant may box its struct — `Install(Box<Install>)` — with no semantic change.
- A **unit variant** is a command with nothing of its own; `name`, `alias`, `hide`, and
  `effect` go directly on the variant.
- Nesting is unbounded in practice: an `Args` struct can carry its own
  `#[usage(subcommand)]` field, up to a maximum depth of 16.

Variant attributes: `name`, `alias`, `alias_hidden`, `hide`, `effect`, `help`, `long_help`,
`verbatim_doc_comment`. Aliases declared on the variant and on the `Args` struct are joined.

Two variants wrapping the _same_ struct is a compile error — each command needs its own
declaration (two byte-identical structs in different modules are fine).

## Default subcommand

```rust
#[derive(Cli)]
#[usage(bin = "ex", default_subcommand = "run")]
struct Ex { /* … */ }
```

When argv selects no command, `run` is assumed. Naming a command that doesn't exist fails the
**build**, not the run.

## Sharing declarations with `flatten`

`#[usage(flatten)]` splices another struct's flags and args into a command, so two commands can
share a set of declarations:

```rust
#[derive(Args)]
struct Listing {
    /// Do not print a header
    #[usage(long)]
    no_header: bool,
    /// Output format
    #[usage(long, choices("json", "table"))]
    format: Option<String>,
}

#[derive(Args)]
struct Config {
    #[usage(long, short = 'f')]
    file: Option<String>,

    #[usage(flatten)]
    listing: Listing,   // config gets --no-header and --format too
}
```

The tables are joined at compile time and the emitted KDL lists the flags inline — a consumer of
the spec can't tell a flattened flag from a declared one. Groups and `exclusive` flags declared
on the flattened struct are enforced (and emitted) on the command that flattens them.

A flattened struct may not declare subcommands; that's a compile-time error with an explanation.

## Value enums

For a flag or arg whose values are a fixed set of words, derive `ValueEnum` instead of listing
`choices` by hand:

```rust
#[derive(usage::ValueEnum)]
enum Shell {
    Bash,
    Zsh,
    #[usage(name = "pwsh")]
    PowerShell,
}

#[derive(Args)]
struct Completion {
    /// Which shell to generate for
    #[usage(long, value_enum)]
    shell: Option<Shell>,
}
```

Variant names kebab-case into the accepted words. The derive also implements `FromStr`, whose
error lists the valid words. One limitation: a single variant cannot be `cfg`-ed out (the word
list is a `const`) — put the `cfg` on the whole enum.

## Mounts and restart tokens

Two spec features for wrapper-style CLIs are declared on the `Args` struct:

```rust
#[derive(Args)]
#[usage(mount = "ex tasks --usage", restart_token = ":::")]
struct Run {
    /// Arguments passed through to the task
    #[usage(double_dash = "preserve")]
    args: Vec<String>,
}
```

`mount` names a command that prints a spec for dynamically-defined subcommands (like mise
tasks); it is only consulted during completion — the cold path where running a subprocess is
affordable. `restart_token` lets one invocation contain several command lines
(`ex run build ::: test`). See the [spec reference](/spec/reference/cmd) for semantics.

## Command effects

A variant or `Args` struct can declare what running the command does to the world:

```rust
#[derive(Args)]
#[usage(effect = "destructive")]
struct Uninstall { /* … */ }
```

See [command effects](/spec/#command-effects) for what consumers do with this.
