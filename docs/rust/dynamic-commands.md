# Dynamic Commands

::: warning Experimental
`usage-dynamic` is new. Its API may change ahead of the rest of the framework.
:::

A plugin manager does not know what `ex format` is until it has read its own configuration,
which happens long after the tables describing the rest of its CLI were compiled. So `ex --help`
never lists it, `ex format <TAB>` completes nothing, and `ex format --check` is a bag of
unparsed words the application has to pick through itself.

The `usage-dynamic` crate closes that gap without giving up the static tables. The application
discovers plugins its own way and hands their specs to a **catalog**, which grafts them onto the
compiled tree for the three things it cannot answer alone: help, completion, and parsing.

```toml
[dependencies]
usage = { package = "usage-rs", version = "6", features = ["completions"] }
usage-dynamic = "6"
```

The `completions` feature is what gives the host a completion surface for the catalog to extend;
without it you still get help and parsing.

Everything else stays where it was. The derive's parse tables are not touched, `Cli::to_kdl()`
emits what it emitted before, and the ordinary parse path costs what it cost before. The
catalog is consulted on cold paths only.

## What the application still owns

`usage-dynamic` performs no callbacks, no filesystem reads, and no subprocess execution. Finding
plugins, caching what they said, deciding when that cache is stale, and running the command
afterwards are all the application's — which is the point, because only it knows whether a
plugin lives in `~/.local/share`, what a stale cache costs, and whether a discovery pass is
worth doing before rendering a help page.

## A spec is a plugin's half of the contract

A plugin describes itself in [KDL](/spec/reference/), the same format `usage` reads everywhere
else. Whatever produces that text — a file next to the plugin, a `--usage` flag on it, a
manifest — the application parses it into a `Spec`:

```rust
use usage_dynamic::Spec;

let formatter: Spec = std::fs::read_to_string("plugins/formatter.usage.kdl")?.parse()?;
```

```kdl
name "formatter"
bin "formatter"
about "Format a project"
flag "--color <WHEN>" {
    choices "always" "never"
}
arg "[path]"
cmd "check" help="Check formatting" {
    flag "--fix" help="Apply fixes"
}
```

That spec is what the catalog renders help from, completes against, and parses with. A plugin
that declares choices gets those choices completed; one that declares nothing gets a name in the
list and no more.

## Declaring where runtime commands may appear

A catalog does not graft commands anywhere it likes. It attaches them to a command that invited
them, by declaring an
[`external_subcommand`](/rust/subcommands#external-subcommands) catch-all — the variant that captures an unrecognized
word and everything after it:

```rust
use std::ffi::OsString;
use usage::{Cli, Subcommands};

#[derive(Cli)]
#[usage(bin = "ex")]
struct Ex {
    #[usage(subcommand)]
    command: Commands,
}

#[derive(Subcommands)]
enum Commands {
    /// Built into ex
    Build,
    #[usage(external_subcommand)]
    External(Vec<OsString>),
}
```

The catch-all is what makes the arrangement honest: the parser already had to accept these
words, and the catalog only explains what it accepted. A parent without one is rejected at
construction rather than silently doing nothing.

| Method                    | Where the command lands                                        |
| ------------------------- | -------------------------------------------------------------- |
| `.root(spec)`             | Top level. `ex format` runs it, and it appears in `ex --help`. |
| `.under("plugins", spec)` | Beneath a static command. `ex plugins format` runs it.         |

Both take the command's name from the spec — `name "formatter"` becomes `format`'s counterpart
`formatter` — and both may be called as many times as there are plugins. A path given to `under`
may be spelled with a static alias; the catalog stores the canonical one.

## Putting it together

`Cli::app()` is the derive-generated view of the static tables — the same thing that renders
help and completion for a CLI with no plugins at all. The catalog wraps it, and the host's
`main` routes three kinds of words: a completion request, the host's own commands, and whatever
the catch-all captured.

```rust
use std::ffi::{OsStr, OsString};
use usage_dynamic::{Catalog, Outcome};
use usage::{Cli, Error};

fn main() {
    let catalog = Catalog::builder(Ex::app())
        .root(load_plugin_spec("formatter"))
        .build()
        .unwrap();
    let app = catalog.app().unwrap();

    // A completion request is not a command anybody runs, so it is answered before the parse.
    let argv: Vec<OsString> = std::env::args_os().skip(1).collect();
    if let Some(answer) = futures::executor::block_on(app.completion_request(&argv)) {
        print!("{answer}");
        return;
    }

    let words: Vec<&OsStr> = argv.iter().map(OsString::as_os_str).collect();
    match Ex::parse_from(&words) {
        Ok(Ex { command: Commands::Build }) => build(),
        Ok(Ex { command: Commands::External(captured) }) => {
            match catalog.parse_external("", &captured) {
                Ok(Some(Outcome::Parsed(parsed))) => run_plugin(&parsed.name, &parsed.output),
                Ok(Some(Outcome::Help(help))) => print!("{}", help.page),
                Ok(Some(Outcome::Version(version))) => println!("{}", version.version),
                // `Outcome` is `#[non_exhaustive]`, so this arm is required — and it is where
                // the two cases the host answers for itself land: a name nobody catalogued,
                // and a token the spec model cannot represent. The argv is intact in both, so
                // dispatch it the way this host dispatched unrecognized words all along.
                _ => fallback(&captured),
            }
        }
        // The host's own help, rendered from the merged tree so that runtime commands appear
        // on the page.
        Err(Error::Help { cmd, long }) => {
            let path = usage::help::find(Ex::spec(), cmd)
                .map(|(path, _)| path[1..].join(" "))
                .unwrap_or_default();
            print!("{}", app.help(&path, long).unwrap());
        }
        Err(err) => {
            eprint!("{}", Ex::render_failure(&words, &err));
            std::process::exit(2);
        }
    }
}
```

Two things are deliberate. `parse_from` rather than the process-exiting `Cli::parse()`, because
the application needs the argv in hand to answer completion and to route the captured words. And
help rendered through `app` rather than `Ex::render_help`, because that is what makes a runtime
command appear on the host's page — `usage::help::find` turns the command the parser stopped at
into the path `help` takes.

This example is `usage-dynamic/examples/host.rs`, kept compiling in CI.

## Dispatch

`parse_external` takes the path of the command the catch-all sits on — the empty string for the
root — and the words it captured. The first word is the plugin's name, by any spelling it
answers to.

| Outcome                  | What happened                                                                                                                                                                                                                  |
| ------------------------ | ------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------ |
| `Some(Outcome::Parsed)`  | The words parsed against the plugin's spec. `parsed.output` is the usual `ParseOutput`, with defaults and environment fallbacks applied; `parsed.name` is the canonical name and `parsed.invoked_as` the alias actually typed. |
| `Some(Outcome::Help)`    | The words asked for `--help`. `help.page` is the rendered page.                                                                                                                                                                |
| `Some(Outcome::Version)` | The words asked for `--version`.                                                                                                                                                                                               |
| `None`                   | No catalogued command answers to that name.                                                                                                                                                                                    |

`None` is not a failure — it is the case that keeps whatever fallback the host had before it
grew a catalog. `Err(Error::NonUtf8)` deserves the same treatment: the portable spec model parses
`String`s, and a token that is not one cannot be represented. The derive handed over `OsString`s,
which lose nothing, so the words are still intact — dispatch them the way you dispatch `None`
rather than failing a command whose argument happens to be an unusual path.

## Help and completion

`catalog.app()` is the merged tree: the host's commands with the catalogued ones grafted in.

```rust
let app = catalog.app()?;
print!("{}", app.help("plugins formatter", false).unwrap());
```

`help` takes a command path, where the empty string is the root, and a `long` flag — `false` is
what `-h` renders, `true` what `--help` does. Runtime commands are reachable by path like any
other, nested pages included.

Completion is answered by two engines, and the seam is the catch-all. Up to it the words are the
host's, and the host's own tables answer for them — so registered completers, multicall
projections, and the `--candidates` half of the protocol all keep working, with catalogued names
added wherever a subcommand belongs. Past it the words are a plugin's, and that plugin's spec
answers alone: its subcommands, its flags, its declared choices.

If the host registered runtime completers or a multicall projection, tell the builder, exactly as
you would tell [`completion_app`](/rust/completions):

```rust
let catalog = Catalog::builder(Ex::app())
    .completions(&OVERLAYS)
    .root(spec)
    .build()?;
```

One thing the catalog will not do is run a spec's `run=` completer. That is a subprocess, and
this crate spawns none — an application that wants executable completers should answer those
requests itself.

## What construction rejects

Building a catalog validates against the static tables, so a mistake is a `Result` at startup
rather than a command that mysteriously does nothing:

| Rejected                                                                  | Because                                                           |
| ------------------------------------------------------------------------- | ----------------------------------------------------------------- |
| A parent that does not exist, or declares no catch-all                    | Nothing would ever reach the grafted command.                     |
| A name or alias colliding with a static command, another entry, or `help` | Two commands answering to one word is not resolvable.             |
| An empty name or alias                                                    | Not a word anybody can type.                                      |
| A supplied spec with an unresolved `mount`                                | Resolving one runs a subprocess, which is the application's call. |

## Compared with `mount`

[`mount`](/rust/subcommands#mounts-and-restart-tokens) solves a neighbouring problem: it names a command that prints
a spec, and the parser runs it during completion. Reach for it when one command's subcommands
come from a subprocess the CLI is happy to spawn on a cold path — mise tasks are the case it was
built for.

Reach for a catalog when the application already knows what its runtime commands are, or wants
to decide for itself when to find out. A catalog runs nothing, caches nothing, and covers help
and parsing as well as completion; `mount` covers completion and asks no questions.

## What stays static

Everything that was fast. The derive's parse tables are unchanged, so an ordinary command parses
in the same nanoseconds it did before. `Cli::to_kdl()` emits the same spec, so nothing
downstream of it learns about plugins it cannot see. The merged tree is assembled the first time
something asks for it and not before — a CLI that only ever dispatches a plugin command never
builds one at all.
