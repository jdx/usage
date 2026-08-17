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

`conflicts`, `requires`, `overrides`, `required_if`, and `required_unless` relate one flag to
another. Targets are named the way the KDL spec names them (`"--long"` or `"-s"`), and naming a
flag that doesn't exist is a compile error:

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
(`group "input" "--file" "--url" "--stdin" required=#true`), and a group declared on a
[flattened](/rust/subcommands#sharing-declarations-with-flatten) struct is enforced on every
command that flattens it. Malformed groups — one member, no members, declared twice, a group on
a positional — are compile errors.

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
at compile time. Emitted KDL: `flag "--tags <tag>" var=#true delimiter=","`.
