# Args and Flags

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
a conversion failure at runtime becomes `Error::InvalidValue(e)`, whose payload carries the
argument's name, the offending value, and the reason.

On Unix, `PathBuf` and `OsString` keep non-UTF-8 argv bytes unchanged. `String` fields report
invalid UTF-8 rather than replacing it. On Windows, values that cannot be converted safely are
reported instead of using an unchecked reconstruction.

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

The tables below cover the attributes most CLIs need.

**Naming and shape** — what the field is called and what kind of argument it is:

| Attribute                | Effect                                                          |
| ------------------------ | --------------------------------------------------------------- |
| `long` / `long = "name"` | `--name` flag (defaults to the kebab-cased field name)          |
| `short` / `short = 'x'`  | `-x` flag (defaults to the field name's first letter)           |
| `name = "…"`             | Override the arg/flag name used in help and the spec            |
| `arg`                    | Force the field to be a positional argument                     |
| `negate = "--no-x"`      | A negation flag that sets a `bool` back to false                |
| `count`                  | Count occurrences into an integer field                         |
| `global`                 | Usable on any subcommand below this one                         |
| `required`               | Explicit required-ness (for `Vec` fields)                       |
| `skip`                   | Not an argument; filled from `Default` when the struct is built |
| `surface = "…"`          | Descriptive audience or compatibility surface metadata          |
| `available_if("…", …)`   | Descriptive availability conditions; does not gate parsing      |

The metadata attributes also work on the root `Cli`, an `Args` command struct, and a
`Subcommands` variant. Their values are intentionally project-defined: `public`, `automation`,
and `internal` are common surfaces, while `unix`, `debug-build`, or `feature=json` are typical
availability conditions. Use `hide` or an application-level check when behavior should change.

**Values and cardinality** — what a field accepts and how many:

| Attribute                           | Effect                                                                         |
| ----------------------------------- | ------------------------------------------------------------------------------ |
| `var` / `variadic`                  | Repeatable / greedy multi-value (see above)                                    |
| `var_min = n` / `var_max = n`       | Bounds on how many values a `Vec` may hold                                     |
| `num_args = n` / `num_args = a..=b` | Exact or ranged `Vec` cardinality                                              |
| `choices("a", "b")`                 | Restrict values to a fixed set                                                 |
| `choices_strict = false`            | Keep choices as suggestions while accepting other values                       |
| `value_enum`                        | Take choices from a `#[derive(ValueEnum)]` type                                |
| `delimiter = ','`                   | Split one word into several values ([Validation](/rust/validation#delimiters)) |
| `value_terminator = ";"`            | End a variadic field at this token without storing the token                   |
| `bool_value`                        | Let a boolean long flag accept attached `=true` or `=false`                    |
| `value_optional`                    | Help/spec only; bind a bare flag with `default_missing` or `Option<Option<T>>` |

**Env vars and defaults** — where a value comes from when argv has none:

| Attribute                      | Effect                                                                           |
| ------------------------------ | -------------------------------------------------------------------------------- |
| `env = "X"` / `env`            | Read the value from `X` when argv does not supply it; bare `env` infers the name |
| `env_fallback("A", "B")`       | Try additional environment variables in declaration order                        |
| `deprecated_env("OLD_X")`      | Try deprecated aliases last, and report the one that supplied a value            |
| `default = "…"`                | Fall back to this value (repeatable for `Vec` fields)                            |
| `default_fn = function`        | Compute one typed Rust default at parse time without emitting a concrete value   |
| `default_note = "…"`           | Describe a `default_fn` in help without pretending the note is its value         |
| `default_missing = "…"`        | Value when the flag is given with none (`--color` vs `--color=never`)            |
| `default_if("--json", "true")` | Default when another flag is given (two args = present, three = equals)          |

Use `default_fn` when the answer depends on the current platform, environment, or another runtime
fact. The function returns the field's value type and is called for each parse:

```rust
use std::io::IsTerminal as _;

fn default_format() -> OutputFormat {
    if std::io::stdout().is_terminal() {
        OutputFormat::Pretty
    } else {
        OutputFormat::Json
    }
}

#[usage(
    long,
    default_fn = default_format,
    default_note = "pretty on a terminal, JSON otherwise"
)]
format: OutputFormat,
```

The emitted portable spec marks the field optional and carries the note as help prose, but emits
no `default`: another consumer cannot reproduce a Rust function and should not be told a guessed
value. Use `default_value_t = EXPR` beside `default = "literal"` when the computed Rust expression
does have one stable portable spelling.

**Parsing behavior** — how tokens on the line are read:

| Attribute                | Effect                                                                    |
| ------------------------ | ------------------------------------------------------------------------- |
| `allow_hyphen_values`    | Detached flag value may look like a flag, including `--`                  |
| `allow_negative_numbers` | Accept negative numeric tokens without accepting every dash-prefixed word |
| `require_equals`         | Accept `--flag=value` and refuse `--flag value`                           |
| `double_dash = "…"`      | `"optional"`, `"required"`, `"preserve"`, or `"automatic"` `--` handling  |
| `overrides(…)`           | Later occurrence silently overrides the named flag                        |

**Relationships** — constraints between arguments, checked after parsing:

| Attribute                                               | Effect                                                                                                         |
| ------------------------------------------------------- | -------------------------------------------------------------------------------------------------------------- |
| `conflicts(…)`, `requires(…)`, `overrides(…)`           | Relations to other args; `overrides` is flag-only ([Validation](/rust/validation))                             |
| `required_if(…)`, `required_if_eq…`, `required_unless…` | Conditional required-ness with single, any, and all forms                                                      |
| `validate` / `validate_error`                           | Portable expression rule ([Validation](/rust/validation#portable-expressions)); needs the `validation` feature |
| `group = "name"`                                        | Join a flag group ([Validation](/rust/validation#groups))                                                      |
| `arg_group`                                             | Take a whole group from an `ArgGroup` enum ([Validation](/rust/validation#a-group-as-an-enum))                 |
| `exclusive`                                             | Must be given alone ([Validation](/rust/validation#exclusive-flags))                                           |

**Deprecation** — a flag on its way out:

| Attribute                    | Effect                                                               |
| ---------------------------- | -------------------------------------------------------------------- |
| `deprecated = "…"`           | Still works, should not be used; reported when given                 |
| `deprecated_warn_at = "…"`   | Withhold that warning until this build's version reaches the release |
| `deprecated_remove_at = "…"` | The release it goes away in, which the warning mentions              |

**Help and presentation** — what the user reads:

| Attribute                        | Effect                                                                  |
| -------------------------------- | ----------------------------------------------------------------------- |
| `help = "…"` / `long_help = "…"` | Help text (doc comments are usually nicer)                              |
| `value_name = "…"`               | The placeholder shown in help (`--file <PATH>`)                         |
| `value_names = ["A", "B"]`       | Distinct placeholders for a fixed multi-value field                     |
| `help_heading = "…"`             | Group the entry under a heading in help output                          |
| `display_order = n`              | Explicit help order; positional parsing still follows declaration order |
| `hide`                           | Omit from help, docs, and completions                                   |
| `verbatim_doc_comment`           | Keep the doc comment's line breaks in help                              |

**Completion, effects, and settings**:

| Attribute                          | Effect                                                                                  |
| ---------------------------------- | --------------------------------------------------------------------------------------- |
| `complete = my_fn`                 | Custom completion function ([Completions](/rust/completions))                           |
| `value_hint = ValueHint::FilePath` | Ask the shell for path completion (see below)                                           |
| `extensions("toml", "yaml")`       | Limit a `FilePath` or `AnyPath` hint to these filename extensions                       |
| `effect = "…"`                     | `"read"`, `"write"`, or `"destructive"` — see [command effects](/spec/#command-effects) |
| `setting = "key"`                  | Bind to a config setting ([Configuration](/rust/configuration))                         |

The rest of this section expands the entries that need more than a table row.

### Skipped fields

`#[usage(skip)]` keeps a field on the struct for computed state without adding it to the spec,
parse tables, or help. Combining it with `long`, `arg`, or any other field option is a compile
error. The type has to implement `Default`.

### Suggested values without strict validation

Use `choices_strict = false` when the declared choices should drive help and
completion but other values remain valid:

```rust
#[usage(long, choices("core", "git"), choices_strict = false)]
backend: Option<String>,
```

The portable form is `choices strict=#false core git`. Strict validation remains
the default.

### Multiple values

Fixed multi-value fields declare their cardinality and placeholders together:

```rust
#[usage(long, num_args = 2, value_names = ["START", "END"])]
range: Vec<String>,
```

One `--range` occurrence consumes exactly two values and help prints
`--range <START> <END>`. The generated KDL uses the same two placeholders and
puts `var_min=2 var_max=2` on the flag's nested `arg`, not on the flag itself.
That distinction matters: bounds on the nested value apply to every occurrence,
while flag-level bounds count how many times a repeatable flag appears. A range
such as `num_args = 1..=3` sets the corresponding nested bounds; distinct
`value_names` require an exact bound matching their count.

`#[usage(value_terminator = ";")]` ends a `Vec` without storing the terminator.
It works on variadic flags and positionals and emits `value_terminator=";"` in KDL.

### Parsing behavior

`#[usage(allow_hyphen_values)]` makes `--args -destroy` bind `-destroy` instead of reading `-d`
as a short. The flag has to take a value; a positional that needs the same thing already has
`double_dash = "automatic"`. In KDL:
`flag "--args <ARGS>" allow_hyphen_values=#true`.

`#[usage(allow_negative_numbers)]` makes `--jobs -1` bind `-1`, while `--jobs --force` still
leaves a flag-like token for normal parsing.

`#[usage(require_equals)]` makes `--inspect=9229` bind while `--inspect 9229` is a missing
value. The flag has to take a value. In KDL:
`flag "--inspect <PORT>" require_equals=#true`.

`#[usage(bool_value)]` is an opt-in for explicit boolean long-flag values:
`--color`, `--color=true`, and `--color=false` bind true, true, and false. A
detached `--color false` never consumes `false`; it remains a positional. The
portable form is `flag "--color" bool_value=#true`.

### Optional values and conditional defaults

With `#[usage(default_missing = "always")]`, `--color` binds `always`, `--color=never` binds
`never`, and an absent flag stays `None`. The flag has to take a value. Help shows the value as
optional. In KDL:
`flag "--color <WHEN>" default_missing="always"`. Combined with `require_equals`,
a following word is still refused.

An `Option<Option<T>>` field preserves all three optional-value states: an absent
flag is `None`, a bare `--bump` is `Some(None)`, and `--bump=5` is
`Some(Some(5))`. It infers a zero-or-one value range and renders `[BUMP]` in help
and the portable spec.

`#[usage(default_if("--json", "true"))]` applies when `--json` is present. Three arguments
(`default_if("--output", "json", "pretty")`) apply when the other flag equals the middle value.
First match wins. The target's own argv and env suppress it. An applied `default_if` is a
default: it does not set `__given_*`, so it does not activate `requires_if`. Emitted KDL:

```kdl
flag "--bin-names" {
  default_if "--json" "true"
}
```

### Argument relations

Argument relations (`conflicts`, `requires`, `required_if`, `required_if_eq`, `required_unless`) name
their target the way the KDL spec does — `"--long"` or `"-s"`, one value or a list:

```rust
#[usage(long, conflicts("--file", "--url"))]
stdin: bool,
```

Naming a flag or positional that doesn't exist on the command is a **compile error**, not a
runtime surprise. Conflicts, requires, and conditional requiredness may name flags or
positionals; `overrides` and some `requires_if` forms stay flag-only.

### Completion hints

`value_hint` accepts `Unknown`, `Other`, `FilePath`, `AnyPath`, `DirPath`, `ExecutablePath`,
`CommandName`, `CommandString`, `CommandWithArguments`, `Username`, `Hostname`, `Url`, and
`EmailAddress`. `CommandWithArguments` is for wrapper CLIs and must be a positional `Vec` with
`double_dash = "automatic"`: its first value completes from the shell's commands, while later
values fall back to ordinary argument paths. `Other`, URL, and email values suppress filename
fallback without pretending there is a finite list.

## Resolution order

After argv is parsed, each field resolves in this order — matching
[config resolution](/spec/resolution) for the spec at large:

1. the value given on the command line
2. the `env` variable, if set
3. a matching `default_if`, if declared
4. the `default`, if declared

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

A global flag may be given **once per command level**, with the innermost occurrence winning:
`mycli -y install -y` works. Giving it twice at the _same_ level is still a `DuplicateFlag`
error: `mycli -y -y` is refused.

## Container attributes

On the root `#[derive(Cli)]` struct:

| Attribute                           | Effect                                                                                                                        |
| ----------------------------------- | ----------------------------------------------------------------------------------------------------------------------------- |
| `bin = "…"`                         | The binary name (used in help and the spec)                                                                                   |
| `name = "…"`                        | A friendly display name                                                                                                       |
| `version` / `version = "…"`         | Enable `--version`/`-V`; bare form uses `CARGO_PKG_VERSION`                                                                   |
| `long_version = "…"`                | Extended `--version` text while `-V` stays concise                                                                            |
| `about` / `long_about`              | Description (doc comments work too)                                                                                           |
| `usage = "…"`                       | Verbatim synopsis line(s), replacing the generated one                                                                        |
| `before_help` / `after_help`        | Extra text around the help page (`*_long_help` variants too)                                                                  |
| `unknown_flags = "value"\|"error"`  | Treat unknown flags as values instead of errors                                                                               |
| `default_subcommand = "run"`        | Command to assume when argv names none                                                                                        |
| `multicall`                         | Treat argv[0]'s basename as a subcommand (busybox-style)                                                                      |
| `view("bin", root = "command")`     | Promote a command as another executable surface                                                                               |
| `completion`                        | Generate completion support ([Completions](/rust/completions))                                                                |
| `settings`                          | Root resolves settings even when every `setting =` binding lives on flattened children ([Configuration](/rust/configuration)) |
| `config = Settings`                 | Emit the named type's `config` block ([Configuration](/rust/configuration))                                                   |
| `min_usage_version = "…"`           | Declare the minimum usage version the spec needs                                                                              |
| `group("name", required, multiple)` | Declare a flag group ([Validation](/rust/validation#groups))                                                                  |

On a `#[derive(Args)]` struct (refused on the root):

| Attribute               | Effect                                                                       |
| ----------------------- | ---------------------------------------------------------------------------- |
| `alias = "…"`           | Alternative command name (`alias_hidden` hides it from help)                 |
| `mount = "…"`           | Mount a subprocess-provided spec for completions ([Spec output](/rust/spec)) |
| `restart_token = ":::"` | Token that restarts parsing (for wrapper CLIs)                               |
| `effect = "…"`          | The command's [effect classification](/spec/#command-effects)                |

## Outputs and exit codes

A command can also declare what it writes to stdout and what its exit statuses mean. The
declarations travel into the spec, generated docs and manpages, `usage mcp`, and generated
SDKs:

```rust
/// Check the project
#[derive(usage::Args)]
#[usage(
    output("human", default, help = "A human-readable report"),
    output("json", framing = "json", schema_from = Report),
    exit_code(0, "all checks passed"),
    exit_code(1, "a check failed"),
)]
struct Check {
    /// Output format
    #[usage(long, select)]
    format: Option<String>,
}
```

`select` on a value-taking flag names it as the selector, and its choices are filled from the
output names. A boolean flag that picks one output is named from the output instead —
`output("json", framing = "json", select = "--json")` — and `select = "--format"` as a
container attribute is the same thing spelled the way the KDL node is. An output also takes
`hide`, and a schema comes from `schema = "…"`, `schema_from = Type` (via `schemars`), or
`schema_fn = path` where the function returns a `String`. Declared on the root `#[derive(Cli)]`
struct, outputs and exit codes apply CLI-wide, and a command can refine what it inherits. The
full model — framings, schema files, inheritance — is on the
[spec reference](/spec/reference/output).
