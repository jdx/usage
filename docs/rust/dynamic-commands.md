# Dynamic command catalogs

`usage-rs` normally compiles the entire command tree into static parse tables. That is the
fast path: it is a good fit when the commands are known while the application is compiled, but
it cannot discover plugin names from runtime configuration.

Applications with a derived host and runtime-discovered commands can add the optional
`usage-dynamic` companion crate. It keeps the host tables static and adds an owned catalog for
a fully merged command tree on cold paths:

- host help can navigate runtime commands and their nested pages;
- completion descends into runtime subcommands, flags, and declared choices;
- argv already captured by an `external_subcommand` variant can be parsed generically from the
  corresponding supplied usage spec.

The catalog does not mutate derived parse tables. The merged tree therefore does not change
normal parse performance or `Cli::to_kdl()` output. `catalog.app().spec()` exposes the separate
merged portable tree.

## Setup

```toml
[dependencies]
usage = { package = "usage-rs", version = "6", features = ["completions"] }
usage-dynamic = "6"
```

Declare an external catch-all where runtime commands may appear:

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
    Plugins {
        #[usage(subcommand)]
        command: PluginCommands,
    },
}

#[derive(Subcommands)]
enum PluginCommands {
    List,
    #[usage(external_subcommand)]
    External(Vec<OsString>),
}
```

Load and cache plugin specs in application code, then build the catalog. A parent path may use
a static alias; the catalog normalizes it to the canonical path.

```rust
use usage_dynamic::{Catalog, Outcome, Spec};

# fn example(formatter_spec: Spec) -> Result<(), usage_dynamic::Error> {
let catalog = Catalog::builder(Ex::app())
    .under("plugins", formatter_spec)
    .build()?;

let short_help = catalog.app().help("plugins", false).unwrap();
# Ok(())
# }
```

Use `Catalog::builder(Ex::app()).root(spec)` when the derived root itself has the external
catch-all. That makes the supplied command a real top-level runtime command: if the spec is
named `foo`, `ex foo` is captured for catalog parsing, `foo` appears in root help and completion,
and `catalog.app().help("foo", false)` renders its page. Multiple `root` and `under` calls add
multiple entries.

For example, a host that accepts top-level plugins declares the catch-all directly in its root
subcommand enum:

```rust
#[derive(Subcommands)]
enum Commands {
    Builtin,
    #[usage(external_subcommand)]
    External(Vec<OsString>),
}

let catalog = Catalog::builder(Ex::app()).root(foo_spec).build()?;
```

## Dispatch

Parse the host with `parse_from`, then hand the captured vector to the catalog. Do not use the
process-exiting `Cli::parse()` entry point: the application needs to route host help and
completion through `catalog.app()` and inspect the catalog's typed dynamic outcomes.

```rust
# use std::ffi::{OsStr, OsString};
# use usage_dynamic::{Catalog, Outcome};
# fn dispatch(catalog: &Catalog<'_>, external: &[OsString]) -> Result<(), usage_dynamic::Error> {
match catalog.parse_external("plugins", external)? {
    Some(Outcome::Parsed(parsed)) => {
        // Dispatch `parsed.name` using `parsed.output`.
    }
    Some(Outcome::Help(help)) => print!("{}", help.page),
    Some(Outcome::Version(version)) => println!("{}", version.version),
    None => {
        // The name was not catalogued. Preserve arbitrary fallback dispatch here.
    }
}
# Ok(())
# }
```

`Parsed` includes the canonical plugin name, the alias actually invoked, its canonical static
parent path, and `usage-lib`'s generic `ParseOutput`. Defaults and environment fallbacks are
applied by that parser. Input is accepted as `OsString`; non-UTF-8 input that the portable spec
model cannot represent returns a structured error containing the token position.

For completion requests, use `catalog.app().completion_app()`. Completion walks the fully merged
tree, including nested plugin commands, plugin flags, declared choices, and path hints. A
spec-declared `run=` completer is not executed: the catalog performs no subprocess execution, so
applications that want executable dynamic completers should delegate those requests explicitly.

## Catalog constraints

Catalog construction rejects ambiguous or unsafe trees:

- the static parent must exist and declare an external-subcommand catch-all;
- plugin names and aliases cannot collide with static commands, other catalog entries, or the
  synthetic `help` command;
- empty names and aliases are invalid;
- supplied specs must have every mount resolved already.

The application owns discovery, caching, refresh, and handler registration. `usage-dynamic`
performs no callbacks, filesystem reads, or subprocess execution. Plugin-specific parse help and
version requests are also available as `Outcome::Help` and `Outcome::Version` after selection.
