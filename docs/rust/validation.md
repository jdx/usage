# Validation

Everything on this page runs after argv is bound and env/default fallbacks are applied. Only the
command that actually ran is judged. Contradictory declarations — `choices` on a `bool`,
`var_min` greater than `var_max`, a default that isn't one of the choices — are compile errors,
not runtime surprises.

## Choices and bounds

```rust
/// Output format
#[usage(long, choices("json", "table"))]
format: Option<String>,

/// Patterns to include
#[usage(long, var_min = 1, var_max = 5)]
include: Vec<String>,
```

A value outside the set is `Error::InvalidChoice { name, choices }`; too few or too many values
are `VarTooFew`/`VarTooMany`. For enum-shaped values prefer
[`ValueEnum`](/rust/subcommands#value-enums).

## Flag relations

`conflicts`, `requires`, `required_if`, `required_if_eq`, and `required_unless` relate arguments to
another. Targets are named the way the KDL spec names them (`"--long"` or `"-s"` for flags, or a
bare positional name), and naming one that doesn't exist is a compile error:

```rust
/// Read from standard input
#[usage(long, conflicts("--file", "--url"))]
stdin: bool,

/// Retry count
#[usage(long, required_if("--retry"))]
max_retries: Option<u32>,
```

`overrides` is the quiet sibling of `conflicts`: a later occurrence of one flag discards an
earlier occurrence of the other instead of erroring — useful for `--json` / `--yaml` pairs where
the last one typed should win.

## Groups

A group relates several flags at once: membership is declared on each field, the group's
properties on the struct.

```rust
#[derive(Cli)]
#[usage(bin = "grp")]
#[usage(group("input", required))]
struct Grp {
    /// Read from a file
    #[usage(long, group = "input")]
    file: Option<String>,
    /// Read from a URL
    #[usage(long, group = "input")]
    url: Option<String>,
    /// Read from standard input
    #[usage(short = 's', long, group = "input")]
    stdin: bool,
}
```

The two properties compose the way clap's do:

| Declaration                         | Meaning      |
| ----------------------------------- | ------------ |
| `group("name")`                     | at most one  |
| `group("name", required)`           | exactly one  |
| `group("name", required, multiple)` | at least one |

An unsatisfied required group is `Error::MissingGroup { group, members }`, rendered as clap
renders it:

```
error: one of the following required arguments was not provided (input):
  --file <PATH>
  --url <URL>
  -s, --stdin
```

Two members of a non-`multiple` group produce `ConflictingFlags` — matched by flag, not
spelling, so giving one member as `-s` and another as `--file` still counts. A conflict is
reported before an unsatisfied group.

Groups are emitted into the KDL spec
(`group input --file --url --stdin required=#true`), and a group declared on a
[flattened](/rust/subcommands#sharing-declarations-with-flatten) struct is enforced on every
command that flattens it. Malformed groups — one member, no members, declared twice, a group on
a positional — are compile errors.

## A group as an enum

The group can be the type instead: an enum deriving `ArgGroup`, whose variants are the flags.
The code that reads it then matches on a variant rather than working out which of several
fields is set.

```rust
/// How to print the result
#[derive(ArgGroup)]
#[usage(name = "format")]
enum Format {
    /// Print JSON
    Json,
    /// Print YAML
    Yaml,
    /// Print one line per record
    #[usage(short = 'p', long = "plain")]
    PlainText,
}

#[derive(Cli)]
#[usage(bin = "fmt")]
struct Fmt {
    #[usage(arg_group)]
    format: Option<Format>,
}
```

Unit variants are switches. A tuple variant with one field is a value-taking flag:

```rust
#[derive(ArgGroup)]
enum Mode {
    Write,
    Check,
    #[usage(value_name = "SOURCE", value_enum)]
    Migrate(MigrationSource),
    #[usage(value_name = "PATH")]
    StdinFilepath(std::path::PathBuf),
}
```

Each member is named by its variant in kebab-case unless `long` or `name` says otherwise.
`short`, `hide`, `help` and `long_help` apply to both forms; `value_name` and `value_enum`
describe a tuple variant's payload. A doc comment is the help. Without
`#[usage(name = "…")]` the group is named after the type.

The field's type says whether the group is required, exactly as it does everywhere else:
`Option<Format>` is a group that may be left alone, and a bare `Format` is one that has to be
given. There is no default variant, so that distinction is the only spelling of required-ness a
group has.

Nothing new reaches the spec. The enum lowers to the same `group` node
(`group format --json --yaml --plain`), so `--json --yaml` is the same
`ConflictingFlags` a hand-written group produces, a required group with none of its members
given is the same `MissingGroup`, and help, docs and completions list the member flags without
knowing an enum was involved.

A payload converts through `FromStr`, preserving non-UTF-8 bytes for `PathBuf` and `OsString`.
With `value_enum`, its choices, aliases, help and case policy reach validation, help, generated
specs and completions from the same enum declaration.

## Exclusive flags

An `exclusive` flag has to be given alone — no other flag, no argument, no subcommand:

```rust
/// Dump the spec and leave
#[usage(long, exclusive)]
dump: bool,
```

This is stronger than `conflicts` with every other flag, because `conflicts` has nowhere to name
an _argument_. The details:

- `--dump -v` and `--dump somefile` both fail with `ConflictingFlags`.
- An exclusive flag **bypasses required-ness**: required siblings the flag's command declares are
  not demanded when the exclusive flag is given — the `--version`-style escape hatch.
- Declared defaults still apply; only values the user actually _supplied_ count as company —
  but a value supplied via `env` does count.
- Exclusivity crosses command boundaries in both directions: selecting a subcommand is company
  for a parent's exclusive flag, and a parent's flags are company for a child's.

`exclusive` on a positional is a compile error. Emitted KDL: `flag "--dump" exclusive=#true`.

## Delimiters

`delimiter` splits one word into several values, the way clap's `value_delimiter` does:

```rust
/// Tags to apply
#[usage(long, delimiter = ',', var_max = 3)]
tags: Vec<String>,
```

`--tags a,b,c` yields `["a", "b", "c"]`, and occurrences accumulate: `--tags a,b --tags c` is
`["a", "b", "c"]`. It works on positionals too (`#[usage(arg, delimiter = ';')]`).

The split runs after `env` fallback and **before every check**, so `choices` judges each split
value and `var_min`/`var_max` count values, not words — `--tags a,b,c,d` with `var_max = 3` is
`VarTooMany { got: 4 }`.

The field must be a `Vec`, and the delimiter must be a single ASCII character; both are enforced
at compile time. In KDL: `flag "--tags <tag>" var=#true delimiter=","`.

## Cross-field validation and typed finalization

Use `validate_with` for an invariant that needs the fully typed root command rather than one
field's text. It runs after field conversion and environment/default resolution on every parse
path:

```rust
use usage::{Cli, ValidationError};

#[derive(Cli)]
#[usage(bin = "copy", validate_with = validate_copy)]
struct CopyArgs {
    #[usage(long)]
    source: std::path::PathBuf,
    #[usage(long)]
    destination: std::path::PathBuf,
}

fn validate_copy(args: &CopyArgs) -> Result<(), ValidationError> {
    if args.source == args.destination {
        return Err(ValidationError::field("--destination")
            .value(args.destination.display().to_string())
            .reason("must differ from --source"));
    }
    Ok(())
}
```

The error is returned as the ordinary `Error::InvalidValue`, so renderers and embedding code do
not need a second diagnostic path. Derive `Clone` when using the update entry points: updates are
applied to a clone, validated, and committed only on success, so a failed update leaves the
standing value untouched.

For applications that keep their parser declaration separate from the type passed to the rest of
the program, `try_into` generates finalizing parse entry points:

```rust
#[derive(Cli)]
#[usage(bin = "copy", try_into = CopyCommand)]
struct CopyArgs { /* flags and arguments */ }

struct CopyCommand(CopyArgs);

impl TryFrom<CopyArgs> for CopyCommand {
    type Error = usage::ValidationError;

    fn try_from(args: CopyArgs) -> Result<Self, Self::Error> {
        // Resolve modes, normalize paths, or establish richer invariants here.
        Ok(Self(args))
    }
}

fn main() {
    let command: CopyCommand = CopyArgs::parse_into();
}
```

`parse_into_from`, `parse_into_from_with_warnings`, `parse_into_from_argv`, and
`try_parse_into_from` are the returning-error counterparts. The original `parse_*` methods remain
available when a caller wants the declaration type itself.

## Portable expressions

For a rule that must survive KDL emission — and that clap would have expressed with a
`value_parser` callback — enable the `validation` feature and declare an expression:

```toml
[dependencies]
usage = { package = "usage-rs", version = "6", features = ["validation"] }
```

```rust
#[usage(
    long,
    validate = "int(value) >= 1 && int(value) <= 65535",
    validate_error = "must be a valid port"
)]
port: Option<u16>,
```

`validate` is evaluated after binding. `validate_error` is the diagnostic users see when the
expression is false. Rust `value_parser` functions are not portable metadata — use `FromStr`
for typed conversion and these attributes for declarative rules that docs, manpages, and other
spec consumers can see.
