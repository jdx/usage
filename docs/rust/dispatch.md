# Dispatch

::: warning Draft
This page is a draft. Some of what it documents is still in open pull requests, and details may
change before release.
:::

A parse ends with a value: an enum whose selected variant holds the command's own struct. What
every CLI then writes is the same `match` — one arm per command, each calling the one function
that command exists for. At mise's size that is 210 arms of pure routing, and nothing checks
that an arm calls the right thing, because every arm has the same shape.

`#[usage(run)]` generates it. A command implements `Run`, the enum says it dispatches, and the
match comes from the same declaration the parser and the spec come from:

```rust
use usage::{Args, Cli, Run, Subcommands};

#[derive(Subcommands)]
#[usage(run)]
enum Commands {
    /// Install a tool
    Install(Install),
    /// Show who pays for this
    Sponsors(Sponsors),
}

#[derive(Args)]
struct Install {
    /// Overwrite an existing install
    #[usage(short = 'f', long)]
    force: bool,
    /// What to install
    tools: Vec<String>,
}

/// Show who pays for this
#[derive(Args)]
struct Sponsors;

impl Run for Install {
    type Output = miette::Result<()>;
    fn run(self) -> Self::Output {
        install(&self.tools, self.force)
    }
}

impl Run for Sponsors {
    type Output = miette::Result<()>;
    fn run(self) -> Self::Output {
        print_sponsors();
        Ok(())
    }
}

fn main() -> miette::Result<()> {
    Cli::parse().command.run()
}
```

A command added to the enum and not implemented is a compile error naming the command. A
command whose output disagrees with its siblings is a compile error naming that command too: a
`match` has one type, and the dispatch takes its `Output` from the first variant and requires
the rest to agree.

`Output` is yours. `miette::Result<()>`, `anyhow::Result<()>`, `Result<(), MyError>`, `()`,
`std::process::ExitCode` — whatever the CLI's commands actually produce.

## A context

Most CLIs hand their commands something: a resolved config, an output handle, a client. That is
`RunWith<Ctx>`, and `#[usage(run_with)]` dispatches it:

```rust
use usage::{RunWith, Subcommands};

#[derive(Subcommands)]
#[usage(run_with)]
enum Commands {
    Install(Install),
    Sponsors(Sponsors),
}

impl RunWith<&mut App> for Install {
    type Output = miette::Result<()>;
    fn run_with(self, app: &mut App) -> Self::Output {
        app.install(&self.tools, self.force)
    }
}
```

```rust
fn main() -> miette::Result<()> {
    let cli = Cli::parse();
    let mut app = App::new(cli.verbose)?;
    cli.command.run_with(&mut app)
}
```

The generated implementation is generic over the context, so `RunWith<&Config>`,
`RunWith<&mut App>` and `RunWith<Arc<Ctx>>` are all ordinary implementations rather than shapes
this crate has to anticipate. An enum may say both `run` and `run_with`, which is what a CLI
part-way through adopting a context needs.

Two traits rather than one with a defaulted context, because otherwise the noise lands on the
wrong side: a hundred commands that need nothing shared would each carry `fn run(self, _: ())`.

## Groups in the middle

A command that exists only to hold other commands — `usage generate`, `mise config` — gets its
forward generated too. `#[usage(run)]` on a struct whose one field is its subcommands:

```rust
/// Generate completions, documentation, and other artifacts
#[derive(Args)]
#[usage(alias = "g", run)]
pub struct Generate {
    #[usage(subcommand)]
    pub command: Command,
}
```

That is all a generated `run` on a struct can do, so the struct holds one field: its
subcommands, not in an `Option`. Any other shape is a compile error, because forwarding past
arguments the struct declared would drop them:

- A struct with **flags or arguments of its own** has to decide what to do with them, so it
  implements `Run` by hand — usually reading them and then calling `self.command.run()`. That
  is the root's usual case: `--verbose` is set up before anything dispatches.
- An **`Option` subcommand** has a state — no command at all — that nothing generated can
  decide about. Whoever knows what an empty command line means writes the implementation.

## What cannot be dispatched

A dispatched arm hands the command's own value to the trait, so every variant has to hold a
type you can implement the trait for:

- A **unit variant** (`Sponsors,`) and a **variant declaring its fields inline**
  (`Add { path: String }`) are both served by a struct the derive writes for them, under a name
  nothing else can name. Hold an `Args` struct instead — `Sponsors(Sponsors)` — which is where
  the command's `effect` and description belong anyway.
- An **`external_subcommand`** variant holds the argv of a command that is not declared here.
  There is nothing to implement `Run` for and no exhaustive match to generate, so an enum with
  a catch-all keeps its hand-written match.

Both are compile errors on the variant rather than a missing implementation somewhere else,
which is the reason dispatch is opt-in rather than always generated. The other reason is that
the generated implementation is the only one the enum can have: a CLI that wants to do
something of its own between the parse and the dispatch leaves `run` off.

## What it says in the spec

Nothing. Which Rust function carries out a command is not part of what the CLI _is_, and a spec
recording it could be read by nothing but this program — so `run` and `run_with` reach the parse
tables, the help output and the emitted KDL exactly as much as `#[usage(skip)]` does, which is
not at all. `usage`'s own CLI moved to a generated dispatch without one byte of its spec, its
manpage or its completions changing.
