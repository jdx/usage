# Dynamic Commands

::: warning Experimental
`usage-dynamic` is new. Its API may change ahead of the rest of the framework.
:::

usage compiles a CLI's command tree at build time. A CLI that has plugins doesn't know its full
command tree until runtime: plugins add commands, and which plugins are installed is only known
by looking. Out of the box those commands don't appear in `--help`, don't complete, and reach
the application as unparsed words.

`usage-dynamic` handles this. The application loads a spec for each plugin and hands them to a
`Catalog`. The catalog merges them into the host's command tree for help and completion, and
parses the argv a plugin command was invoked with. The derived parse tables and the emitted KDL
are not modified, and normal parsing stays exactly as fast.

```toml
[dependencies]
usage = { package = "usage-rs", version = "6", features = ["completions"] }
usage-dynamic = "6"
```

The `completions` feature is only needed for the completion half; help and parsing work
without it.

`usage-dynamic` does not discover anything itself: it runs no subprocesses, reads no files, and
has no callbacks. The application finds its plugins — a directory scan, a lockfile, a registry
— and decides when to look.

## Plugins load only when needed

Because loading plugins is expensive relative to running a built-in command — directory scans,
file reads, spec parses — and most invocations never touch a plugin, the API is arranged so
that work only happens when something needs it:

1. Parse argv with the static tables first. A built-in command parses and runs without any
   plugin being loaded.
2. Words the tables don't recognize land in an `external_subcommand` catch-all. That's when to
   load plugins: after the parse, only on the invocations that involve one.
3. Help and completion do need the plugin list up front — but both are interactive, where
   reading a directory of KDL files is cheap next to rendering a page.

The API's costs match that ordering:

| Call                          | Cost                                             | Needs                 |
| ----------------------------- | ------------------------------------------------ | --------------------- |
| `Catalog::builder(…).build()` | validates names and parents; microseconds        | the specs you pass it |
| `catalog.parse_external(…)`   | one parse of the captured argv, against one spec | the matched spec      |
| `catalog.app()`               | merges the full command tree; once, then kept    | every catalogued spec |

`app()` is the only expensive call, and only help and completion use it. A catalog built from a
single plugin's spec is a complete dispatcher for that plugin — nothing requires loading the
rest.

## Plugin specs

Each plugin provides a [usage spec](/spec/) in KDL, the same format used everywhere else in
usage. How the text gets to the application is its own convention — a file next to the plugin,
a `--usage` flag on its binary, a field in a manifest. Parse it into a `Spec`:

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

The spec is everything the catalog knows about a plugin: `about` becomes its summary in help,
its commands, flags, and choices drive completion, and its argv parses against it. A minimal
spec — just `name` and `about` — is enough for the plugin to show up in help and completion by
name.

## Declaring where plugins attach

Plugins attach beneath a command that declares an
[`external_subcommand`](/rust/subcommands#external-subcommands) catch-all — the variant that
captures an unrecognized word and everything after it:

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

The catch-all is what makes the parser accept unknown words there in the first place; the
catalog gives those words meaning without changing what parses. Attaching to a command without
one is an error at `build()`.

Attach one spec per plugin, with either builder call:

| Method                    | Result                                            |
| ------------------------- | ------------------------------------------------- |
| `.root(spec)`             | `ex formatter` — a top-level command              |
| `.under("plugins", spec)` | `ex plugins formatter` — beneath a static command |

The command's name comes from the spec's `name`. Paths given to `under` may use a static
command's aliases; the catalog stores the canonical path.

## A complete host

This example is `usage-dynamic/examples/host.rs` and compiles in CI. Built-in commands run
without loading anything; the catch-all, help, and completion each load plugins at the moment
of use.

```rust
use std::ffi::{OsStr, OsString};
use usage_dynamic::{Catalog, Outcome, Spec};
use usage::complete::{render, CompletionRequest};
use usage::{Cli, Error, Subcommands};

/// How plugins are found is up to the application: a directory scan, a lockfile, a registry.
fn plugin_catalog() -> Catalog<'static> {
    let mut builder = Catalog::builder(Ex::app());
    for spec in discover_plugin_specs() {
        builder = builder.root(spec);
    }
    builder.build().unwrap()
}

fn main() {
    let argv: Vec<OsString> = std::env::args_os().skip(1).collect();

    // Completion requests are recognized before the parse. They are interactive, so loading
    // plugins here is affordable — and it is what puts plugin names in the answer.
    if let Some(request) = CompletionRequest::parse(&argv) {
        let catalog = plugin_catalog();
        let answer =
            futures::executor::block_on(catalog.app().unwrap().complete_request(&request));
        print!("{}", render(&answer, request.shell));
        return;
    }

    let words: Vec<&OsStr> = argv.iter().map(OsString::as_os_str).collect();
    match Ex::parse_from(&words) {
        // Built-in commands run without loading any plugin.
        Ok(Ex { command: Commands::Build }) => build(),

        // The words named something the static tables don't know. Load plugins now.
        Ok(Ex { command: Commands::External(captured) }) => {
            let catalog = plugin_catalog();
            match catalog.parse_external("", &captured) {
                Ok(Some(Outcome::Parsed(parsed))) => run_plugin(&parsed.name, &parsed.output),
                Ok(Some(Outcome::Help(help))) => print!("{}", help.page),
                Ok(Some(Outcome::Version(version))) => println!("{}", version.version),
                // No loaded plugin matches, or the argv is not UTF-8. The captured words are
                // untouched — handle them like any unknown command.
                _ => fallback(&captured),
            }
        }

        // Render help through the catalog so plugin commands appear on the page.
        // `usage::help::find` converts the command the parser stopped at into the path
        // `help` takes.
        Err(Error::Help { cmd, long }) => {
            let catalog = plugin_catalog();
            let path = usage::help::find(Ex::spec(), cmd)
                .map(|(path, _)| path[1..].join(" "))
                .unwrap_or_default();
            print!("{}", catalog.app().unwrap().help(&path, long).unwrap());
        }
        Err(Error::Version { .. }) => println!("ex {}", env!("CARGO_PKG_VERSION")),
        Err(err) => {
            eprint!("{}", Ex::render_failure(&words, &err));
            std::process::exit(2);
        }
    }
}
```

Use `parse_from`, not the process-exiting `Cli::parse()`: `parse()` renders help from the
static tables and exits, so plugin commands would never appear on the page. Rendering help
through the catalog is what adds them.

If the application can map a command name to its spec file — `plugins/<name>.usage.kdl` — the
catch-all arm can load just that one spec instead of all of them. Watch out for aliases: a
plugin invoked by an alias won't be found by a filename lookup on the typed word.

## Dispatch

`catalog.parse_external(parent, argv)` takes the path of the command with the catch-all (`""`
for the root) and the captured words. The first word selects the plugin, by its name or any
alias its spec declares.

| Outcome                  | Meaning                                                                                                                                                            |
| ------------------------ | ------------------------------------------------------------------------------------------------------------------------------------------------------------------ |
| `Some(Outcome::Parsed)`  | Parsed against the plugin's spec. `output` is a normal `ParseOutput` with defaults and env fallbacks applied; `name` is canonical, `invoked_as` is what was typed. |
| `Some(Outcome::Help)`    | The words asked for `--help`; `page` is the rendered page.                                                                                                         |
| `Some(Outcome::Version)` | The words asked for `--version`.                                                                                                                                   |
| `None`                   | No catalogued command matches that name.                                                                                                                           |

`None` is not an error: do whatever the application did with unknown commands before it had
plugins. Handle `Err(Error::NonUtf8)` the same way — the spec model is UTF-8 strings, but the
captured `OsString`s are intact, so raw dispatch still works.

## Help and completion

`catalog.app()` returns the merged tree. `app().help(path, long)` renders any command's page by
path — the empty path is the root, `long` selects the `--help` page over the `-h` summary —
with plugin commands sorted and grouped under their `help_heading` like static ones.

Completion splits the line at the catch-all:

- **Before it**, the host's own completion engine answers. Registered completers (sync and
  async), multicall projections, and `--candidates` requests work exactly as they do without a
  catalog, and plugin names and visible aliases are offered wherever a subcommand could go.
- **After it**, the matched plugin's spec answers: its subcommands, flags, and declared
  choices. An unknown name completes nothing — no host flags, no file fallback.

If the host has runtime completers or a projection of its own, pass them to the builder — the
same values `completion_app` takes:

```rust
let catalog = Catalog::builder(Ex::app())
    .completions(&OVERLAYS)
    .root(spec)
    .build()?;
```

The catalog never executes a `run=` completer from a plugin spec — that's a subprocess. Those
requests return nothing; answer them in the application if you want them.

## What `build()` rejects

| Rejected                                                                         | Why                                                     |
| -------------------------------------------------------------------------------- | ------------------------------------------------------- |
| A parent path that doesn't exist, or has no `external_subcommand` catch-all      | No words would ever reach the plugin                    |
| A name or alias that collides — with a static command, another plugin, or `help` | Two commands can't answer to one word                   |
| An empty name or alias                                                           | Nothing to type                                         |
| A spec containing an unresolved `mount`                                          | Resolving one runs a subprocess; do it before attaching |

Every check runs against the static tables at `build()`, so a bad configuration fails at
startup instead of producing a command that silently does nothing.

## `mount` or a catalog?

[`mount`](/rust/subcommands#mounts-and-restart-tokens) handles a related case: one command
whose subcommands come from a subprocess the parser is allowed to run during completion — mise
tasks. If that's the shape, `mount` is less machinery.

A catalog is for applications that manage the specs themselves, or want control over when
loading happens. It also covers help and dispatch, and it never runs anything.
