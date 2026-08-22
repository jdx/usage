# Dispatch

A parse ends with a value: an enum whose selected variant holds the command's own struct. What
every CLI then writes is the same `match` — one arm per command, each calling the one function
that command exists for. At mise's size that is 210 arms of pure routing, and nothing checks
that an arm calls the right thing, because every arm has the same shape.

`#[usage(run)]` generates it. A command implements `Run`, the enum says it dispatches, and the
match comes from the same declaration the parser and the spec come from. Four traits, differing
only in whether a command is handed a context and whether it is awaited:

|           | no context                         | a context                                        |
| --------- | ---------------------------------- | ------------------------------------------------ |
| **sync**  | `Run` — `#[usage(run)]`            | `RunWith<Ctx>` — `#[usage(run_with)]`            |
| **async** | `RunAsync` — `#[usage(run_async)]` | `RunAsyncWith<Ctx>` — `#[usage(run_async_with)]` |

One type may implement several, and one enum may dispatch several, which is what a CLI part-way
through adopting a context — or a runtime — needs. The sync, context-free case:

```rust
use usage::{Args, Cli, Run, Subcommands};

#[derive(Cli)]
#[usage(bin = "ex")]
struct Cli {
    #[usage(subcommand)]
    command: Commands,
}

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

impl RunWith<&mut App> for Sponsors {
    type Output = miette::Result<()>;
    fn run_with(self, app: &mut App) -> Self::Output {
        app.print_sponsors()
    }
}

fn main() -> miette::Result<()> {
    let cli = Cli::parse();
    let mut app = App::new(cli.verbose)?;
    cli.command.run_with(&mut app)
}
```

Every variant, since the dispatch is a `match`: a command left unimplemented is a compile error
naming it.

The generated implementation is generic over the context, so `RunWith<&Config>`,
`RunWith<&mut App>` and `RunWith<Arc<Ctx>>` are all ordinary implementations rather than shapes
this crate has to anticipate. An enum may say both `run` and `run_with`, which is what a CLI
part-way through adopting a context needs.

Two traits rather than one with a defaulted context, because otherwise the noise lands on the
wrong side: a hundred commands that need nothing shared would each carry `fn run(self, _: ())`.

## Async commands

`RunAsync` and `RunAsyncWith<Ctx>` are the async pair, under `#[usage(run_async)]` and
`#[usage(run_async_with)]`. An implementation writes `async fn`, and the generated dispatch is an
`async fn` that awaits the selected command:

```rust
use usage::{RunAsync, Subcommands};

#[derive(Subcommands)]
#[usage(run_async)]
enum Commands {
    Install(Install),
    Sponsors(Sponsors),
}

impl RunAsync for Install {
    type Output = miette::Result<()>;
    async fn run_async(self) -> Self::Output {
        install(&self.tools, self.force).await
    }
}

impl RunAsync for Sponsors {
    type Output = miette::Result<()>;
    async fn run_async(self) -> Self::Output {
        fetch_sponsors().await
    }
}

#[tokio::main]
async fn main() -> miette::Result<()> {
    Cli::parse().command.run_async().await
}
```

A context works the same way, and the future borrows it for as long as it runs:

```rust
impl RunAsyncWith<&App> for Install {
    type Output = miette::Result<()>;
    async fn run_async_with(self, app: &App) -> Self::Output {
        app.install(&self.tools, self.force).await
    }
}
```

### `Send`

Neither async trait imposes it. They declare `-> impl Future<Output = Self::Output>` rather than
`async fn`, which is the same signature to implement against and leaves `Send` to the commands
themselves:

- A CLI that spawns gets `Send` **by inference** — it leaks out of the concrete commands the
  dispatch reaches, so `tokio::spawn(cli.command.run_async())` compiles when every command's
  future is `Send`.
- A CLI on a single-threaded runtime keeps futures that are not, such as one holding an `Rc`
  across an await.

What no design can add is the third thing: _demanding_ `Send` in generic code, which
`-> impl Future + Send` in the trait would buy at the cost of the second bullet. The traits take
the side that refuses nothing.

### The other way

The sync traits can carry a future too, since `Output` is whatever the command produces:

```rust
type Task<'a, T> = Pin<Box<dyn Future<Output = T> + Send + 'a>>;

impl Run for Install {
    type Output = Task<'static, miette::Result<()>>;
    fn run(self) -> Self::Output {
        Box::pin(async move { install(&self.tools, self.force).await })
    }
}
```

That costs an allocation and names a type — the box is needed because an `async` block's type
cannot be named and an associated type has to be — and it is where the `+ Send` goes if the CLI
wants one. Worth it only when the future has to be a value: stored, selected over, or returned
across an API boundary. Otherwise use `RunAsync`.

A CLI whose commands are mostly synchronous can also keep `Output = Result<()>` and hold a
runtime handle in its context, which is what `RunWith` is for.

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

That is all a generated `run` on a **container** can do — a struct whose one field is its
subcommands, not in an `Option`. A **root** that also declares flags gets `run_command` instead
of `impl Run`: the flags stay on `self`, the subcommand is moved out, and whoever parsed them
decides what `--verbose` means before calling it:

```rust
fn main() -> miette::Result<()> {
    let cli = Cli::parse();
    if cli.verbose {
        enable_tracing();
    }
    cli.run_command()
}
```

`run_with` / `run_async` / `run_async_with` on that root become `run_command_with`,
`run_command_async`, and `run_command_async_with`.

An **`Option` subcommand** still has a state — no command at all — that nothing generated can
decide about. Whoever knows what an empty command line means writes the implementation.

## Unit and inline variants

A dispatched arm still needs a type the command can implement the trait for. A **unit variant**
(`Sponsors,`) and a **variant declaring its fields inline** (`Add { path: String }`) get one:
the derive writes `{Enum}{Variant}` — `CommandSponsors`, `CommandAdd` — and the generated match
rebuilds that struct from the variant. Implement `Run` there:

```rust
#[derive(Subcommands)]
#[usage(run)]
enum Command {
    Sponsors,
    Add { path: String },
}

impl Run for CommandSponsors {
    type Output = miette::Result<()>;
    fn run(self) -> Self::Output {
        print_sponsors();
        Ok(())
    }
}

impl Run for CommandAdd {
    type Output = miette::Result<()>;
    fn run(self) -> Self::Output {
        add(&self.path)
    }
}
```

Holding a separately declared `Args` struct — `Sponsors(Sponsors)` — is still the usual shape
when the command has work and description of its own. The generated name is for CLIs that kept
clap's unit and inline layout.

## Mixed sync and async

One generated `async fn` can call a synchronous command and await another. The enum says
`run_async`; a variant that should not wait says `#[usage(run)]`:

```rust
#[derive(Subcommands)]
#[usage(run_async)]
enum Commands {
    #[usage(run)]
    Activate(Activate),
    Install(Install),
}
```

`Activate` implements `Run`; `Install` implements `RunAsync`. The match is still one type,
because both produce the same `Output`. Do not also put `run` on the enum: that would generate
a second, synchronous method the async commands cannot enter.

## Context some commands do not want

`RunWith<Ctx>` still takes a context by value, and a variant that should not see it says
`#[usage(no_ctx)]`. That arm implements `Run` (or `RunAsync`) instead. When at least one
command skips the context, the enum also gets `run_with_lazy` / `run_async_with_lazy`, which
take `FnOnce() -> Ctx` and only call it for an arm that needs it — so a missing config file is
not opened for `version`:

```rust
#[derive(Subcommands)]
#[usage(run_with)]
enum Commands {
    #[usage(no_ctx)]
    Version(Version),
    Get(Get),
}

cli.command.run_with_lazy(|| load_config(&cli)?)?;
```

## Catch-alls

An `external_subcommand` holds argv, not a command struct. Name the function that should
receive those words:

```rust
fn fallback(argv: Vec<OsString>) -> miette::Result<()> {
    start_from_argv(argv)
}

#[derive(Subcommands)]
#[usage(run, external = fallback)]
enum Commands {
    Start(Start),
    #[usage(external_subcommand)]
    Fallback(Vec<OsString>),
}
```

`run_with` passes `(argv, ctx)`; `run_async` awaits the function. If the first command is not
a reliable `Output` source — or the catch-all is the first variant — name the type with
`output = miette::Result<()>`.

## What it says in the spec

Nothing. Which Rust function carries out a command is not part of what the CLI _is_, and a spec
recording it could be read by nothing but this program — so `run` and `run_with` reach the parse
tables, the help output and the emitted KDL exactly as much as `#[usage(skip)]` does, which is
not at all. `usage`'s own CLI moved to a generated dispatch without one byte of its spec, its
manpage or its completions changing.
