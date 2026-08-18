# Args and Flags

::: warning Draft
This page is a draft. Some of what it documents is still in open pull requests, and details may
change before release.
:::

A field on a `#[derive(Cli)]` or `#[derive(Args)]` struct becomes a flag when it carries `long`
or `short`, and a positional argument otherwise (or explicitly with `#[usage(arg)]`).

```rust
#[derive(Cli)]
#[usage(bin = "ex")]
struct Cli {
    /// User to run as
    #[usage(short = 'u', long)]
    user: Option<String>,        // flag: -u, --user <user>

    /// The directory to use
    dir: String,                 // required positional: <dir>

    /// The files to read
    files: Vec<String>,          // variadic positional: [files]...
}
```

## Types drive cardinality

Whether something is required, optional, or repeatable is read off the field's type — the type
has nowhere to put "absent", so `T` means required:

| Field type             | Meaning                                 |
| ---------------------- | --------------------------------------- |
| `T`                    | one value, **required**                 |
| `Option<T>`            | one value or nothing                    |
| `Vec<T>`               | several values; empty when none arrived |
| `Option<Vec<T>>`       | several values; `None` when never given |
| `bool`                 | a switch                                |
| `u8`…`usize` + `count` | occurrence count (`-vvv` → `3`)         |

Values are built with `FromStr`, so `PathBuf`, `usize`, `IpAddr`, and your own types all work.
The `FromStr` error type must implement `Display` (a compile error names the type otherwise);
a conversion failure at runtime becomes `Error::InvalidValue { name, value, reason }`.

A `Vec` flag is repeatable (`var` in spec terms) automatically. Two related attributes cover the
other shapes:

- `var` — makes a _single-value_ flag repeatable where the last occurrence wins
- `variadic` — one occurrence greedily takes values: `--include a b c`

`var` and `variadic` together is a compile error. A **required** `Vec` is the one place
required-ness is declared rather than inferred: `#[usage(arg, required)]`.

## Field attributes

```rust
#[usage(short = 'j', long, env = "EX_JOBS", default = "4")]
jobs: Option<u32>,
```

| Attribute                               | Effect                                                                                  |
| --------------------------------------- | --------------------------------------------------------------------------------------- |
| `long` / `long = "name"`                | `--name` flag (defaults to the kebab-cased field name)                                  |
| `short` / `short = 'x'`                 | `-x` flag (defaults to the field name's first letter)                                   |
| `name = "…"`                            | Override the arg/flag name used in help and the spec                                    |
| `arg`                                   | Force the field to be a positional argument                                             |
| `env = "VAR"`                           | Fall back to this environment variable when the flag/arg wasn't given                   |
| `default = "…"`                         | Fall back to this value (repeatable for `Vec` fields)                                   |
| `negate = "--no-x"`                     | A negation flag that sets a `bool` back to false                                        |
| `count`                                 | Count occurrences into an integer field                                                 |
| `global`                                | Usable on any subcommand below this one                                                 |
| `var` / `variadic`                      | Repeatable / greedy multi-value (see above)                                             |
| `var_min = n` / `var_max = n`           | Bounds on how many values a `Vec` may hold                                              |
| `choices("a", "b")`                     | Restrict values to a fixed set                                                          |
| `value_enum`                            | Take choices from a `#[derive(ValueEnum)]` type                                         |
| `delimiter = ','`                       | Split one word into several values ([Validation](/rust/validation#delimiters))          |
| `allow_hyphen_values`                   | Detached flag value may look like a flag, including `--`                                |
| `group = "name"`                        | Join a flag group ([Validation](/rust/validation#groups))                               |
| `exclusive`                             | Must be given alone ([Validation](/rust/validation#exclusive-flags))                    |
| `conflicts(…)` / `requires(…)`          | Relations to other flags ([Validation](/rust/validation))                               |
| `overrides(…)`                          | Later occurrence silently overrides the named flag                                      |
| `required_if(…)` / `required_unless(…)` | Conditional required-ness                                                               |
| `complete = my_fn`                      | Custom completion function ([Completions](/rust/completions))                           |
| `value_hint = ValueHint::FilePath`      | Complete values as paths (`FilePath`, `DirPath`, `AnyPath`)                             |
| `value_name = "…"`                      | The placeholder shown in help (`--file <PATH>`)                                         |
| `help = "…"` / `long_help = "…"`        | Help text (doc comments are usually nicer)                                              |
| `help_heading = "…"`                    | Group the entry under a heading in help output                                          |
| `hide`                                  | Omit from help, docs, and completions                                                   |
| `required`                              | Explicit required-ness (for `Vec` fields)                                               |
| `value_optional`                        | Mark the value optional in help (help-only; the parser still wants one)                 |
| `double_dash = "…"`                     | `"optional"`, `"required"`, `"preserve"`, or `"automatic"` `--` handling                |
| `effect = "…"`                          | `"read"`, `"write"`, or `"destructive"` — see [command effects](/spec/#command-effects) |
| `setting = "key"`                       | Bind to a config setting (generates `parse_from_with_settings`)                         |
| `verbatim_doc_comment`                  | Keep the doc comment's line breaks in help                                              |
| `skip`                                  | Not an argument; filled from `Default` when the struct is built                         |

`#[usage(skip)]` is clap's `#[arg(skip)]`: the field stays on the struct so a rewrite can keep
computed state beside parsed state, and nothing about it reaches the spec, the parse tables, or
help. Combining it with `long`, `arg`, or any other field option is a compile error. The type
has to implement `Default`.

`#[usage(allow_hyphen_values)]` is clap's attribute of the same name: `--args -destroy` binds
`-destroy` instead of reading `-d` as a short. The flag has to take a value; a positional that
needs the same thing already has `double_dash = "automatic"`. Emitted KDL:
`flag "--args <ARGS>" allow_hyphen_values=#true`.

Flag relations (`conflicts`, `requires`, `overrides`, `required_if`, `required_unless`) name
their target the way the KDL spec does — `"--long"` or `"-s"`, one value or a list:

```rust
#[usage(long, conflicts("--file", "--url"))]
stdin: bool,
```

Naming a flag that doesn't exist on the command is a **compile error**, not a runtime surprise.
Relations are flag-to-flag only; a positional cannot carry one.

## Resolution order

After argv is parsed, each field resolves in this order — matching
[config resolution](/spec/resolution) for the spec at large:

1. the value given on the command line
2. the `env` variable, if set
3. the `default`, if declared

Then validation runs: required-ness (skipped for anything a default or env var filled),
`choices`, and `var_min`/`var_max`. Only the command that actually ran is judged — a required
flag on a sibling subcommand you didn't invoke costs nothing.

## Global flags

A `global` flag declared on a parent is accepted anywhere below it:

```rust
/// Say yes to everything
#[usage(long, short = 'y', global)]
yes: bool,
```

A global flag may be given **once per command level**, with the innermost occurrence winning —
`mycli -y install -y` works, matching clap. Giving it twice at the _same_ level is still a
`DuplicateFlag` error: `mycli -y -y` is refused.

## Container attributes

On the root `#[derive(Cli)]` struct:

| Attribute                           | Effect                                                         |
| ----------------------------------- | -------------------------------------------------------------- |
| `bin = "…"`                         | The binary name (used in help and the spec)                    |
| `name = "…"`                        | A friendly display name                                        |
| `version` / `version = "…"`         | Enable `--version`/`-V`; bare form uses `CARGO_PKG_VERSION`    |
| `about` / `long_about`              | Description (doc comments work too)                            |
| `usage = "…"`                       | Verbatim synopsis line(s), replacing the generated one         |
| `before_help` / `after_help`        | Extra text around the help page (`*_long_help` variants too)   |
| `unknown_flags = "value"\|"error"`  | Treat unknown flags as values instead of errors                |
| `default_subcommand = "run"`        | Command to assume when argv names none                         |
| `completion`                        | Generate completion support ([Completions](/rust/completions)) |
| `settings`                          | Generate config-settings bindings                              |
| `min_usage_version = "…"`           | Declare the minimum usage version the spec needs               |
| `group("name", required, multiple)` | Declare a flag group ([Validation](/rust/validation#groups))   |

On a `#[derive(Args)]` struct (refused on the root):

| Attribute               | Effect                                                                       |
| ----------------------- | ---------------------------------------------------------------------------- |
| `alias = "…"`           | Alternative command name (`alias_hidden` hides it from help)                 |
| `mount = "…"`           | Mount a subprocess-provided spec for completions ([Spec output](/rust/spec)) |
| `restart_token = ":::"` | Token that restarts parsing (for wrapper CLIs)                               |
| `effect = "…"`          | The command's [effect classification](/spec/#command-effects)                |
