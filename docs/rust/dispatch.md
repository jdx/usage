# Dispatch

::: warning Draft
This page is a draft and has not yet been human reviewed. Details may change.
:::

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
something of its own between the parse and the dispatch leaves the attribute off and writes the
match.
