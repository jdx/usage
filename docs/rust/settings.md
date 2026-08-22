# Settings

::: warning Draft
This page is a draft and has not yet been human reviewed. Details may change.
:::

CLIs that resolve configuration from several places — flags, environment variables, config
files — have historically kept three descriptions of every setting in step by hand: a registry
file, a code generator, and the struct the program reads. `#[derive(usage::Config)]` collapses
them to one. The struct the CLI already holds its settings in becomes the declaration; the
derive generates the [usage-config](https://docs.rs/usage-config) registry, the reader that
fills the struct from a resolution, and the spec `config` block that documents it.

Enable the `config` feature:

```toml
[dependencies]
usage = { package = "usage-rs", version = "5.1", features = ["config"] }
```

Reading config files is opt-in per format on `usage-config` itself — the facade's `config`
feature does not pull TOML, JSON, or YAML. Add the format you need when you use `FileLayer`:

```toml
[dependencies]
usage-config = { version = "5.1", features = ["toml"] }
```

## Declaring

```rust
use usage::Config;

/// How this tool behaves, resolved from flags, the environment, and files.
#[derive(Config)]
struct Settings {
    /// How many jobs to run at once
    #[usage(env = "EX_JOBS", default = 4, cli("--jobs", "-j"))]
    jobs: u64,

    /// Paths to leave alone
    #[usage(env = "EX_EXCLUDE", merge = "union", parse = "list_by_comma")]
    exclude: Option<Vec<String>>,

    /// Where the cache lives
    #[usage(env = "EX_CACHE_DIR", default_fn = default_cache_dir,
            default_note = "under the user cache directory")]
    cache_dir: std::path::PathBuf,

    #[usage(flatten)]
    task: TaskSettings,
}

/// The `task.*` settings.
#[derive(Config)]
#[usage(prefix = "task")]
struct TaskSettings {
    /// How task output is interleaved
    #[usage(default = "prefix", choices("prefix", "interleave"))]
    output: String,
}

fn default_cache_dir() -> std::path::PathBuf {
    dirs::cache_dir().unwrap_or_default().join("ex")
}
```

The field's type is the setting's type. `bool`, integers, `f64`, `String`, `PathBuf`,
`Vec<T>`, and `BTreeMap<String, T>` name their spec types on their own; `Option<T>` says
absence is a legitimate state. A type outside that table says what the spec should call it
with `ty = "…"` — a `String` field with `ty = "duration"` holds a span of time as its text,
which is how the fleet's registries store one. Doc comments become `help` and `long_help`,
exactly as they do for flags.

Struct-level attributes declare the `config` block around those settings. They are
documentation for resolvers the CLI already owns, not extra runtime layers:

| Attribute                                                                   | Effect                                                                                                       |
| --------------------------------------------------------------------------- | ------------------------------------------------------------------------------------------------------------ |
| `prefix = "task"`                                                           | Prefix every field's key in this struct                                                                      |
| `source(kind = "git", name = "git config", doc_hint = "…", set_hint = "…")` | A custom source kind's display metadata. Repeatable; kinds are written in sorted order                       |
| `file(path = "ex.toml", findup, scope = "project", format = "toml")`        | A config file in the documented precedence chain. Repeatable; **declaration order is precedence**, last wins |

`source` and `file` belong to the struct they are written on. Flattening another `Config`
type splices its _settings_, not its source/file declarations — a nested group that also
documents `task.toml` still does so when emitted on its own, and does not rewrite the parent
CLI's file chain.

Field attributes mirror the spec's [`prop` vocabulary](/spec/reference/config):

| Attribute                                    | Effect                                                                                                              |
| -------------------------------------------- | ------------------------------------------------------------------------------------------------------------------- |
| `env = "X"` / `env("A", "B")`                | Environment variables, highest precedence first                                                                     |
| `deprecated_env("OLD")`                      | Deprecated aliases, consulted afterwards and warned about                                                           |
| `default = 4` / `default(80, 443)`           | The value when no layer supplies one                                                                                |
| `default_fn = path`                          | A computed default (`fn() -> T`), applied after the resolution                                                      |
| `default_note = "…"`                         | Prose beside the default, for docs                                                                                  |
| `cli("--jobs", "-j")`                        | The flags that set it — what `Registry::drift` holds bindings against                                               |
| `source("git", "hk.jobs")`                   | Its keys in sources usage does not know about                                                                       |
| `choices("a", "b")`                          | The only values it accepts                                                                                          |
| `merge = "union"` / `"deep"`                 | How a collection combines across layers                                                                             |
| `scope = "global"` / `"env"`                 | Where a value is accepted from                                                                                      |
| `parse = "list_by_comma"`                    | How one string becomes several values                                                                               |
| `alias("other")`                             | Equivalent keys accepted without a warning — written in full, so a group's `prefix` is repeated rather than implied |
| `key = "match"`                              | The dotted key, when the field name is not it                                                                       |
| `hide`, `since = "…"`, `examples(…)`         | Documentation and lifecycle metadata                                                                                |
| `deprecated = "…"` and its `*_at` milestones | Warn, then ignore configured values when explicit CLI version context reaches removal                               |
| `help_heading = "Performance"`               | The section to list this setting under in generated docs                                                            |
| `writes_to = "git"`                          | Where `config set` should write this, when it is not the usual file                                                 |
| `x("tool.key", value)`                       | Tool-private `x` metadata. Spec-only: resolution does not interpret it. Repeatable; order is preserved              |
| `flatten`                                    | Splice another `Config` struct's settings in at this position                                                       |

`help_heading`, `writes_to`, and `x` are spec metadata. They ride in `SETTINGS_SPEC` beside
the runtime registry rather than on `PropMeta`, because the merge never reads them. The
emitted `config` block still carries them, so docs and round-trips match a KDL authoring of
the same CLI.

`ty` renames what the spec calls a setting; it cannot change what the field holds. The merge
coerces to the _declared_ type, so that is what decides the shape the field is handed — a
`ty = "uint"` on a `String` field would be given an integer the field cannot read, whatever
anyone configured, and is refused. A pairing that can read is left alone: `ty = "int"` on a
`u8` reads whenever the value fits, which is the author's call to make.

An `alias` is written in full, including a group's `prefix` — `alias("task.out")` inside a
`#[usage(prefix = "task")]` group, not `alias("out")`. An alias is usually a name a setting
used to have, and one that moved into a group often wants its old unprefixed spelling, so the
full form is the one that can say either.

Pass the running program version explicitly when resolving lifecycle gates:

```rust
let context = usage::config::ResolutionContext::for_cli_version(env!("CARGO_PKG_VERSION"));
let resolved =
    usage::config::resolve_with_context(Settings::SETTINGS_REGISTRY, layers, context)?;
```

The caller chooses that string because an adopting CLI's version is not `usage-config`'s package
version and may be computed at runtime. `resolve(...)` remains available for compatibility when
there is no version context; it warns about a deprecated setting but does not remove its value.

A default and a choice are written as the type the field holds. Nothing coerces a declared
default — the resolver seeds it as written and hands it to the field — so `default = 1` on a
`String` field is a compile error rather than the text `1`, and `default(80)` is how a list of
one is spelled apart from a bare `default = 80`. Choices follow the same spelling so the two
can be compared, with one exception that is not a coercion: on a list setting the choices name
what a single _item_ may be, so `choices("a", "b")` beside `default("a", "b")` is a `Vec<String>`
whose every value is one of them.

A flattened group declares its own dotted keys under its own `#[usage(prefix = "…")]`, and
the parent joins the slices at compile time — two groups declaring the same key are a compile
error, not a shadowed setting.

## Resolving

The derive generates `SETTINGS_PROPS`, `SETTINGS_REGISTRY`, `read`, and `spec_kdl` on the
struct. The CLI names the layers it has — that stays its own business — and the registry
decides what every value means:

```rust
use usage::config::{resolve, EnvLayer, FileLayer, FileScope, Layers};

let (cli, cli_layer) = Ex::parse_from_with_settings(&argv)?;
let env = EnvLayer::from_process();
// FileLayer needs a format feature on usage-config (`toml`, `json`, or `yaml`).
let project = FileLayer::find_up("ex.toml", &cwd, None, FileScope::Project);
let resolved = resolve(
    Settings::SETTINGS_REGISTRY,
    Layers::new().then(&cli_layer).then(&env).then(&project),
)?;
let settings = Settings::read(&resolved)?;
for warning in usage::config::explain::warnings(&resolved) {
    eprintln!("{warning}");
}
```

`read` visits every field before returning, so the error is the whole list of what is wrong
rather than the first thing found. Provenance is the merge's own output: `explain`, `list`,
and per-setting `origin` come free, without a second merge to drift from the first.

### When a value will not read

`read` is all or nothing, which leaves a CLI two moves when one field is bad: refuse to start,
or fall back to a struct of declared defaults and lose the environment and every config file
along with the offending value. Neither is a choice a library should be making for you, so
there is `read_lossy`:

```rust
let (settings, errors) = Settings::read_lossy(&resolved);
for error in &errors.0 {
    warn!("{error}");   // or bail, or ignore — the policy is yours
}
let settings = settings.expect("every setting declares a default");
```

A field that will not read falls back to its own declared default; every other field keeps what
the merge gave it. The failures come back alongside as the same `ReadError`s `read` returns, so
deciding a bad value is fatal after all costs nothing.

The struct is `None` only where a setting has no value _and_ no declared default — a hole in the
declaration rather than a bad value, and nothing to fall back to. A settings type where every
field declares a default can `expect` it.

Most of this cannot happen: the merge already coerces every value to the type its setting
declares. What is left is a post-merge hook writing through
`Resolved::coerced`, which is unchecked by design, a type only the tool understands, and a field
that narrows further than the setting does — a `uint` setting held as a `u16` port.

## The spec carries the settings

A root deriving [`Cli`](/rust/args-and-flags) names its settings type, and its emitted spec
carries the `config` block — so docs, JSON schema, and the reserved `config_keys` /
`config_values` completers read declarations made in Rust exactly as they read ones made in
KDL:

```rust
#[derive(usage::Cli)]
#[usage(bin = "ex", config = Settings)]
struct Ex {
    /// How many jobs to run at once
    #[usage(long, short = 'j', setting = "jobs")]
    jobs: Option<u64>,
}
```

`setting = "key"` on a flag is the executable binding; `cli("--jobs")` on the field is the
documented one. The adopter's whole drift test is one line:

```rust
assert_eq!(Settings::SETTINGS_REGISTRY.drift(Ex::SETTINGS_BINDINGS), Vec::<String>::new());
```

## The spec block

The struct is the only declaration. `Settings::spec_kdl()` renders it as the spec's
`config { source …; file …; prop … }` block, and `#[usage(config = Settings)]` on the `Cli`
root puts that block in the emitted spec — so docs, the JSON schema, and the `config_keys` /
`config_values` completers read settings declared in Rust exactly as they read ones written
in KDL.

```rust
#[derive(usage::Config)]
#[usage(source(kind = "git", name = "git config", doc_hint = "git config `{key}`"))]
#[usage(file(path = "/etc/ex.toml", scope = "system"))]
#[usage(file(path = "ex.toml", findup))]
struct Settings {
    #[usage(env = "EX_JOBS", default = 4, help_heading = "Performance", writes_to = "git",
            x("ex.restart_required", true))]
    jobs: u64,
}
```

There is no second, KDL-first backend to choose between: a `build.rs` that generated the
registry from the spec was a third description of every setting, which is the drift this
derive exists to remove.
