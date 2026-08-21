# Settings

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

## Declaring

```rust
use usage_rs as usage;

/// How this tool behaves, resolved from flags, the environment, and files.
#[derive(usage::Config)]
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
#[derive(usage::Config)]
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

Field attributes mirror the spec's [`prop` vocabulary](/spec/reference/config):

| Attribute                                                | Effect                                                                |
| -------------------------------------------------------- | --------------------------------------------------------------------- |
| `env = "X"` / `env("A", "B")`                            | Environment variables, highest precedence first                       |
| `deprecated_env("OLD")`                                  | Deprecated aliases, consulted afterwards and warned about             |
| `default = 4` / `default(80, 443)`                       | The value when no layer supplies one                                  |
| `default_fn = path`                                      | A computed default (`fn() -> T`), applied after the resolution        |
| `default_note = "…"`                                     | Prose beside the default, for docs                                    |
| `cli("--jobs", "-j")`                                    | The flags that set it — what `Registry::drift` holds bindings against |
| `source("git", "hk.jobs")`                               | Its keys in sources usage does not know about                         |
| `choices("a", "b")`                                      | The only values it accepts                                            |
| `merge = "union"` / `"deep"`                             | How a collection combines across layers                               |
| `scope = "global"` / `"env"`                             | Where a value is accepted from                                        |
| `parse = "list_by_comma"`                                | How one string becomes several values                                 |
| `alias("other")`                                         | Equivalent keys accepted without a warning                            |
| `key = "match"`                                          | The dotted key, when the field name is not it                         |
| `hide`, `deprecated = "…"`, `since = "…"`, `examples(…)` | Documentation and lifecycle metadata                                  |
| `flatten`                                                | Splice another `Config` struct's settings in at this position         |

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
`config { prop … }` block, and `#[usage(config = Settings)]` on the `Cli` root puts that
block in the emitted spec — so docs, the JSON schema, and the `config_keys` / `config_values`
completers read settings declared in Rust exactly as they read ones written in KDL.

There is no second, KDL-first backend to choose between: a `build.rs` that generated the
registry from the spec was a third description of every setting, which is the drift this
derive exists to remove.
