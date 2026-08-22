# Subcommands

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
- `#[usage(run)]` on the enum generates the `match` that hands the selected command to the code
  that carries it out — with `run_with`, `run_async` and `run_async_with` for a dispatch that
  carries a context, is awaited, or both; unit and inline variants, catch-alls, and mixed
  sync/async commands are covered in [Dispatch](/rust/dispatch).

Variant attributes: `name`, `alias`, `alias_hidden`, `hide`, `effect`, `help`, `long_help`,
`verbatim_doc_comment`, `external_subcommand`, `arg_required_else_help`. Aliases declared on the variant and on the `Args`
struct are joined.

The same `Args` type may be mounted under more than one variant — useful for shared option
groups across sibling commands.

## Default subcommand

```rust
#[derive(Cli)]
#[usage(bin = "ex", default_subcommand = "run")]
struct Ex { /* … */ }
```

When argv selects no command, `run` is assumed. Naming a command that doesn't exist fails the
**build**, not the run.

## Multicall

clap's `#[command(multicall = true)]` is busybox-style applets: argv[0]'s basename
selects a subcommand. `parse()` rewrites the process's argv[0]; `parse_from` is
unchanged, because the caller already decided the words.

```rust
#[derive(Cli)]
#[usage(bin = "busybox", multicall)]
struct Busybox {
    #[usage(subcommand)]
    command: Commands,
}
```

A symlink `ls -> busybox` runs the `ls` variant. `busybox ls` still does too: the
dispatcher name is skipped. Path components and a trailing `.exe` are stripped.

## Executable views

Use a view when an installed executable should expose one command as its root rather than merely
selecting a multicall subcommand:

```rust
#[derive(Cli)]
#[usage(
    bin = "aube",
    completion,
    view("aubr", root = "run", globals),
    view("aubx", root = "dlx", global = "--config")
)]
struct Aube { /* … */ }
```

`parse()` and `parse_from_argv()` select the view from argv0 and route directly to its command.
`to_kdl()` emits portable `view` nodes. With completion support enabled,
`completion_script_for("aubr", shell)` emits the script registered for that executable, and its
requests are completed from the promoted command. `global = "--flag"` may be repeated; bare
`globals` carries every root global.

## External subcommands

clap's `#[command(external_subcommand)]` is a catch-all variant that holds the unmatched
name plus the rest of argv:

```rust
#[derive(Cli)]
#[usage(bin = "ex", unknown_flags = "error")]
struct Ex {
    #[usage(subcommand)]
    command: Option<Commands>,
}

#[derive(Subcommands)]
enum Commands {
    Install(Install),
    #[usage(external_subcommand)]
    External(Vec<String>),
}
```

The variant must hold `Vec<String>` or `Vec<OsString>`. Only one such variant is allowed.
Known subcommands still win; a `default_subcommand` still catches first. `ex git --help`
becomes `Commands::External(vec!["git", "--help".into()])`. `ex --wat` is still an unknown
flag. The emitted spec carries `external_subcommand #true`.

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

The tables are joined at compile time, so the parser walks one flat slice and `flatten` costs
nothing at run time. Groups and `exclusive` flags declared on the flattened struct are enforced
(and emitted) on the command that flattens them.

The emitted KDL says the declarations are shared, rather than repeating them under each command:
the struct becomes a [`flagset`](/spec/reference/flagset) named after it, and every command that
flattens it gets a `use`.

```kdl
flagset listing {
    flag --no-header help="Do not print a header"
    flag --format help="Output format" {
        arg <FORMAT> {
            choices json table
        }
    }
}
cmd config {
    flag "-f --file" {
        arg <FILE>
    }
    use listing
}
```

A `use` is resolved while the spec is read, so what a command accepts is exactly what it was
when the flags were written out: anything reading the spec — docs, manpages, completions, another
implementation — sees `config` with all three flags. A struct that flattens another becomes a set
that uses a set. Positionals stay on each command, since a set holds flags only, and two flattened
structs whose names end in the same word get no set at all — one name cannot stand for both, so
both are written inline.

A flattened struct may not declare subcommands; that's a compile-time error with an explanation.

## Value enums

For a flag or arg whose values are a fixed set of words, derive `ValueEnum` instead of listing
`choices` by hand:

```rust
#[derive(usage::ValueEnum)]
enum Shell {
    /// Bourne Again shell
    #[value(visible_alias = "b")]
    Bash,
    #[value(alias = "shell-z", hide = true)]
    Zsh,
    #[value(name = "pwsh", help = "PowerShell")]
    PowerShell,
}

#[derive(Args)]
struct Completion {
    /// Which shell to generate for
    #[usage(long, value_enum)]
    shell: Option<Shell>,
}
```

Variant names kebab-case into the accepted words. A doc comment supplies per-value help;
`help` overrides it, `hide` keeps a value accepted without advertising it, `alias` adds a
hidden spelling, and `visible_alias` advertises the alternate spelling. Plural alias attributes
accept lists. `#[usage(ignore_case)]` applies to the whole enum, and `cfg`-gated variants remain
gated in every generated metadata table.

`ValueEnum` also binds those words directly to their variants, so the type needs no separate
`FromStr`. An existing domain `FromStr` implementation may still coexist; value-enum fields
use the derive's canonical values and aliases.

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
