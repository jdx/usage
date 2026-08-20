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

| Attribute                                               | Effect                                                                                  |
| ------------------------------------------------------- | --------------------------------------------------------------------------------------- |
| `long` / `long = "name"`                                | `--name` flag (defaults to the kebab-cased field name)                                  |
| `short` / `short = 'x'`                                 | `-x` flag (defaults to the field name's first letter)                                   |
| `name = "…"`                                            | Override the arg/flag name used in help and the spec                                    |
| `arg`                                                   | Force the field to be a positional argument                                             |
| `env = "VAR"`                                           | Fall back to this environment variable when the flag/arg wasn't given                   |
| `default = "…"`                                         | Fall back to this value (repeatable for `Vec` fields)                                   |
| `negate = "--no-x"`                                     | A negation flag that sets a `bool` back to false                                        |
| `count`                                                 | Count occurrences into an integer field                                                 |
| `global`                                                | Usable on any subcommand below this one                                                 |
| `env = "X"`                                             | Read the value from `X` when argv does not supply it                                    |
| `env_fallback("A", "B")`                                | Try additional environment variables in declaration order                               |
| `deprecated_env("OLD_X")`                               | Try deprecated aliases last and label them in generated help/docs                       |
| `var` / `variadic`                                      | Repeatable / greedy multi-value (see above)                                             |
| `var_min = n` / `var_max = n`                           | Bounds on how many values a `Vec` may hold                                              |
| `num_args = n` / `num_args = a..=b`                     | clap-compatible spelling for exact or ranged `Vec` cardinality                          |
| `choices("a", "b")`                                     | Restrict values to a fixed set                                                          |
| `choices_strict = false`                                | Keep choices as suggestions while accepting other values                                |
| `value_enum`                                            | Take choices from a `#[derive(ValueEnum)]` type                                         |
| `delimiter = ','`                                       | Split one word into several values ([Validation](/rust/validation#delimiters))          |
| `allow_hyphen_values`                                   | Detached flag value may look like a flag, including `--`                                |
| `allow_negative_numbers`                                | Accept negative numeric tokens without accepting every dash-prefixed word               |
| `value_terminator = ";"`                                | End a variadic field at this token without storing the token                            |
| `require_equals`                                        | Accept `--flag=value` and refuse `--flag value`                                         |
| `bool_value`                                            | Let a boolean long flag accept attached `=true` or `=false`                             |
| `default_missing = "…"`                                 | Value when the flag is given with none (`--color` vs `--color=never`)                   |
| `default_if("--json", "true")`                          | Default when another flag is given (two args = present, three = equals)                 |
| `group = "name"`                                        | Join a flag group ([Validation](/rust/validation#groups))                               |
| `exclusive`                                             | Must be given alone ([Validation](/rust/validation#exclusive-flags))                    |
| `conflicts(…)` / `requires(…)`                          | Relations to other flags ([Validation](/rust/validation))                               |
| `overrides(…)`                                          | Later occurrence silently overrides the named flag                                      |
| `required_if(…)`, `required_if_eq…`, `required_unless…` | Conditional required-ness with single, any, and all forms                               |
| `complete = my_fn`                                      | Custom completion function ([Completions](/rust/completions))                           |
| `value_hint = ValueHint::FilePath`                      | Ask the shell for path completion (see below)                                           |
| `value_name = "…"`                                      | The placeholder shown in help (`--file <PATH>`)                                         |
| `value_names = ["A", "B"]`                              | Distinct placeholders for a fixed multi-value field                                     |
| `help = "…"` / `long_help = "…"`                        | Help text (doc comments are usually nicer)                                              |
| `help_heading = "…"`                                    | Group the entry under a heading in help output                                          |
| `display_order = n`                                     | Explicit help order; positional parsing still follows declaration order                 |
| `hide`                                                  | Omit from help, docs, and completions                                                   |
| `required`                                              | Explicit required-ness (for `Vec` fields)                                               |
| `value_optional`                                        | Mark the value optional in help (help-only; the parser still wants one)                 |
| `double_dash = "…"`                                     | `"optional"`, `"required"`, `"preserve"`, or `"automatic"` `--` handling                |
| `effect = "…"`                                          | `"read"`, `"write"`, or `"destructive"` — see [command effects](/spec/#command-effects) |
| `setting = "key"`                                       | Bind to a config setting (generates `parse_from_with_settings`)                         |
| `verbatim_doc_comment`                                  | Keep the doc comment's line breaks in help                                              |
| `skip`                                                  | Not an argument; filled from `Default` when the struct is built                         |

### Suggested values without strict validation

Use `choices_strict = false` when the declared choices should drive help and
completion but other values remain valid:

```rust
#[usage(long, choices("core", "git"), choices_strict = false)]
backend: Option<String>,
```

The portable form is `choices strict=#false core git`. Strict validation remains
the default.

`#[usage(skip)]` is clap's `#[arg(skip)]`: the field stays on the struct so a rewrite can keep
computed state beside parsed state, and nothing about it reaches the spec, the parse tables, or
help. Combining it with `long`, `arg`, or any other field option is a compile error. The type
has to implement `Default`.

`value_hint` accepts clap's full stable vocabulary: `Unknown`, `Other`, `FilePath`, `AnyPath`,
`DirPath`, `ExecutablePath`, `CommandName`, `CommandString`, `CommandWithArguments`,
`Username`, `Hostname`, `Url`, and `EmailAddress`. `CommandWithArguments` is for wrapper CLIs
and must be a positional `Vec` with `double_dash = "automatic"`: its first value completes
from the shell's commands, while later values fall back to ordinary argument paths. `Other`,
URL, and email values suppress filename fallback without pretending there is a finite list.

`#[usage(allow_hyphen_values)]` is clap's attribute of the same name: `--args -destroy` binds
`-destroy` instead of reading `-d` as a short. The flag has to take a value; a positional that
needs the same thing already has `double_dash = "automatic"`. Emitted KDL:
`flag "--args <ARGS>" allow_hyphen_values=#true`.

`#[usage(allow_negative_numbers)]` is the narrower clap policy: `--jobs -1` binds
`-1`, while `--jobs --force` still leaves a flag-like token for normal parsing.

`#[usage(value_terminator = ";")]` ends a `Vec` without storing the terminator.
It works on variadic flags and positionals and emits `value_terminator=";"` in KDL.

Fixed multi-value fields can retain clap's familiar attribute unchanged:

```rust
#[arg(long, num_args = 2, value_names = ["START", "END"])]
range: Vec<String>,
```

One `--range` occurrence consumes exactly two values and help prints
`--range <START> <END>`. The generated KDL uses the same two placeholders and
puts `var_min=2 var_max=2` on the flag's nested `arg`, not on the flag itself.
That distinction matters: bounds on the nested value apply to every occurrence,
while flag-level bounds count how many times a repeatable flag appears. A range
such as `num_args = 1..=3` sets the corresponding nested bounds; distinct
`value_names` require an exact bound matching their count.

`#[usage(require_equals)]` is clap's attribute of the same name: `--inspect=9229` binds
and `--inspect 9229` is a missing value. The flag has to take a value. Emitted KDL:
`flag "--inspect <PORT>" require_equals=#true`.

`#[usage(bool_value)]` is an opt-in for explicit boolean long-flag values:
`--color`, `--color=true`, and `--color=false` bind true, true, and false. A
detached `--color false` never consumes `false`; it remains a positional. The
portable form is `flag "--color" bool_value=#true`.

`#[usage(default_missing = "always")]` is clap's `default_missing_value`: `--color`
binds `always`, `--color=never` binds `never`, and an absent flag stays `None`.
The flag has to take a value. Help shows the value as optional. Emitted KDL:
`flag "--color <WHEN>" default_missing="always"`. Combined with `require_equals`,
a following word is still refused.

An `Option<Option<T>>` field preserves all three optional-value states: an absent
flag is `None`, a bare `--bump` is `Some(None)`, and `--bump=5` is
`Some(Some(5))`. It infers a zero-or-one value range and renders `[BUMP]` in help
and the portable spec.

`#[usage(default_if("--json", "true"))]` is clap's `default_value_if` with
`ArgPredicate::IsPresent`. Three arguments (`default_if("--output", "json", "pretty")`)
are `Equals`. First match wins. The target's own argv and env suppress it. An
applied `default_if` is a default: it does not set `__given_*`, so it does not
activate `requires_if`. Emitted KDL:

```kdl
flag "--bin-names" {
  default_if "--json" "true"
}
```

Argument relations (`conflicts`, `requires`, `required_if`, `required_if_eq`, `required_unless`) name
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
| `long_version = "…"`                | Extended `--version` text while `-V` stays concise             |
| `about` / `long_about`              | Description (doc comments work too)                            |
| `usage = "…"`                       | Verbatim synopsis line(s), replacing the generated one         |
| `before_help` / `after_help`        | Extra text around the help page (`*_long_help` variants too)   |
| `unknown_flags = "value"\|"error"`  | Treat unknown flags as values instead of errors                |
| `default_subcommand = "run"`        | Command to assume when argv names none                         |
| `multicall`                         | Treat argv[0]'s basename as a subcommand (busybox-style)       |
| `view("bin", root = "command")`     | Promote a command as another executable surface                |
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
