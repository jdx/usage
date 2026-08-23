# Dynamic Commands

::: warning Experimental
`usage-dynamic` is new. Its API may change ahead of the rest of the framework.
:::

A plugin manager can't compile tables for commands it hasn't met. If `ex format` is a plugin
the user installed yesterday, `ex --help` won't list it, `ex format <TAB>` completes nothing,
and when it runs, the application receives raw words to interpret by hand.

`usage-dynamic` adds the three missing answers — help, completion, and parsing for
runtime-discovered commands — without touching the static tables. The derive's parse tables,
the KDL it emits, and normal parse performance stay exactly as they were.

```toml
[dependencies]
usage = { package = "usage-rs", version = "6", features = ["completions"] }
usage-dynamic = "6"
```

The `completions` feature is what the completion half builds on; help and parsing work
without it.

Two responsibilities stay with the application, deliberately. It finds its plugins — a
directory scan, a lockfile, a registry — and it decides when to look. `usage-dynamic` runs no
subprocesses, reads no files, and calls nothing back.

## Built-in commands never pay for plugins

Discovery costs real work: directory scans, file reads, spec parses. Most invocations are
built-in commands, so the design keeps that work off their path entirely:

1. `Ex::parse_from(argv)` runs against the static tables alone. A built-in command parses and
   runs with no plugin discovered, loaded, or parsed.
2. Words the tables don't recognize land in an `external_subcommand` catch-all. That is the
   signal to go find plugins — after the parse, only on the invocations that need them.
3. Help and completion do want plugin specs up front, but both are interactive: reading a
   directory of KDL files is cheap next to rendering a page.

The API's costs line up with that ordering:

| Call                          | Cost                                             | Needs                 |
| ----------------------------- | ------------------------------------------------ | --------------------- |
| `Catalog::builder(…).build()` | validates names and parents; microseconds        | the specs you pass it |
| `catalog.parse_external(…)`   | one parse of the captured argv, against one spec | the matched spec      |
| `catalog.app()`               | merges the full command tree; once, then kept    | every catalogued spec |

`app()` is the only expensive call, and only help and completion use it. A catalog built with a
single plugin's spec is a complete dispatcher for that plugin — nothing requires loading the
rest.

## A plugin describes itself with a spec

A plugin's half of the contract is a [usage spec](/spec/) in KDL — the same format everything
else in usage reads. Where the text comes from is the application's convention: a file next to
the plugin, a `--usage` flag on its binary, a field in a manifest. Parsing it gives a `Spec`:

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

Everything the catalog does for a plugin comes from this spec: `about` is its line in the
parent's help, `cmd` and `flag` and `choices` are what completion descends into, and the whole
thing is what its argv parses against. A plugin that declares little gets little — a name in
the list — which also means a cheap stub spec (`name` and `about` only) is enough to make a
plugin _visible_ before its real spec has ever been loaded.

## The host declares where plugins may appear

Plugins attach to a command that opted in, by declaring an
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

The catch-all matters because it means the parser was already going to accept these words; the
catalog explains what was accepted rather than changing what parses. Attaching to a command
without one is an error at `build()`.

Each plugin spec is attached with one of two builder calls, repeated per plugin:

| Method                    | Result                                            |
| ------------------------- | ------------------------------------------------- |
| `.root(spec)`             | `ex formatter` — a top-level command              |
| `.under("plugins", spec)` | `ex plugins formatter` — beneath a static command |

The command's name is the spec's `name`. A path given to `under` may use a static command's
alias, visible or hidden; the catalog stores the canonical spelling.

## A complete host

This is `usage-dynamic/examples/host.rs`, compiled in CI. Built-ins run before any plugin
exists; the three paths that need plugins — the catch-all, help, completion — each load them
at the moment of use.

```rust
use std::ffi::{OsStr, OsString};
use usage_dynamic::{Catalog, Outcome, Spec};
use usage::complete::{render, CompletionRequest};
use usage::{Cli, Error, Subcommands};

/// Discovery is the application's: scan a directory, read a lockfile, whatever fits.
fn plugin_catalog() -> Catalog<'static> {
    let mut builder = Catalog::builder(Ex::app());
    for spec in discover_plugin_specs() {
        builder = builder.root(spec);
    }
    builder.build().unwrap()
}

fn main() {
    let argv: Vec<OsString> = std::env::args_os().skip(1).collect();

    // A completion request is not a command anybody runs, so it is recognized before the
    // parse. It is also interactive: loading plugins here is affordable, and it is what puts
    // their names in the answer.
    if let Some(request) = CompletionRequest::parse(&argv) {
        let catalog = plugin_catalog();
        let answer = futures::executor::block_on(
            catalog.app().unwrap().complete_request(&request),
        );
        print!("{}", render(&answer, request.shell));
        return;
    }

    let words: Vec<&OsStr> = argv.iter().map(OsString::as_os_str).collect();
    match Ex::parse_from(&words) {
        // A built-in command runs with no plugin loaded. This is the hot path, and nothing on
        // it knows plugins exist.
        Ok(Ex { command: Commands::Build }) => build(),

        // The catch-all fired: now, and only now, load plugins.
        Ok(Ex { command: Commands::External(captured) }) => {
            let catalog = plugin_catalog();
            match catalog.parse_external("", &captured) {
                Ok(Some(Outcome::Parsed(parsed))) => run_plugin(&parsed.name, &parsed.output),
                Ok(Some(Outcome::Help(help))) => print!("{}", help.page),
                Ok(Some(Outcome::Version(version))) => println!("{}", version.version),
                // A name no loaded plugin answers to, or argv the spec model cannot
                // represent. The words are untouched either way: handle them however this
                // application handled unrecognized commands before it had plugins.
                _ => fallback(&captured),
            }
        }

        // Help is a cold path too. Render it through the catalog so plugins appear on the
        // page; `usage::help::find` turns the command the parser stopped at into the path
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

Note `parse_from`, not the process-exiting `Cli::parse()`: the application needs the argv in
hand, and it needs help to render through the catalog rather than from the static tables, or
plugins would vanish from the page.

An application that can map a command name to its spec file — `plugins/<name>.usage.kdl`, say
— can go further in the catch-all arm and load exactly one spec instead of all of them. Aliases
are the caveat: a plugin invoked by an alias won't be found by a filename lookup on the typed
word.

## Dispatch

`catalog.parse_external(parent, argv)` takes the path of the command the catch-all sits on
(`""` for the root) and the captured words. The first word is the plugin's name, by any
spelling its spec declares.

| Outcome                  | Meaning                                                                                                                                                            |
| ------------------------ | ------------------------------------------------------------------------------------------------------------------------------------------------------------------ |
| `Some(Outcome::Parsed)`  | Parsed against the plugin's spec. `output` is a normal `ParseOutput` with defaults and env fallbacks applied; `name` is canonical, `invoked_as` is what was typed. |
| `Some(Outcome::Help)`    | The words asked for `--help`; `page` is the rendered page.                                                                                                         |
| `Some(Outcome::Version)` | The words asked for `--version`.                                                                                                                                   |
| `None`                   | No catalogued command answers to that name.                                                                                                                        |

`None` is not an error; it's the case that preserves whatever the host did with unrecognized
words before it had plugins. Treat `Err(Error::NonUtf8)` the same way: the spec model parses
`String`s and can't represent a non-UTF-8 token, but the captured `OsString`s are intact, so
raw dispatch still works.

## Help and completion

`catalog.app()` merges the host's tree with every catalogued command. `app().help(path, long)`
renders any page by path — the empty path is the root, `long` picks between the `-h` summary
and the `--help` page — with plugin commands listed, ordered, and grouped under their
`help_heading` like anything static.

Completion splits each line in two at the catch-all:

- **Before it**, the words are the host's, and the host's own engine answers — registered
  completers (sync and async), multicall projections, and `--candidates` requests all behave
  exactly as they do without a catalog. Plugin names and visible aliases are added wherever a
  subcommand belongs.
- **After it**, the words are one plugin's, and that plugin's spec answers alone: its
  subcommands, flags, and declared choices. A name no plugin answers to completes nothing —
  not the host's flags, not the working directory.

If the host has runtime completers or a projection of its own, pass them to the builder — the
same values `completion_app` would take:

```rust
let catalog = Catalog::builder(Ex::app())
    .completions(&OVERLAYS)
    .root(spec)
    .build()?;
```

One deliberate hole: a `run=` completer declared in a plugin spec is a subprocess, and the
catalog spawns none. Those requests return nothing; an application that wants them answers
them itself.

## What `build()` rejects

| Rejected                                                                         | Why                                                     |
| -------------------------------------------------------------------------------- | ------------------------------------------------------- |
| A parent path that doesn't exist, or has no `external_subcommand` catch-all      | No words would ever reach the plugin                    |
| A name or alias that collides — with a static command, another plugin, or `help` | Two commands can't answer to one word                   |
| An empty name or alias                                                           | Nothing to type                                         |
| A spec containing an unresolved `mount`                                          | Resolving one runs a subprocess; do it before attaching |

All of it is checked against the static tables at `build()`, so a bad configuration is a
startup error instead of a command that silently does nothing.

## `mount` or a catalog?

[`mount`](/rust/subcommands#mounts-and-restart-tokens) covers a neighbouring case: one command
whose subcommands come from a subprocess the parser may run during completion — mise tasks.
If that's the shape, `mount` is less machinery.

A catalog is for the application that already knows its runtime commands, or wants control
over when to find out. It covers help and dispatch as well as completion, and it never runs
anything.
