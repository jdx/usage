# Configuration

A `config` block describes a CLI's settings: what they are called, what they hold, where a
value can come from, and what to say about them in documentation. It is what
`usage g markdown` renders as a settings reference, what a JSON schema for the config file
is generated from, and what a runtime resolves values against.

```kdl
config {
    prop "jobs" type="uint" default=0 help="Number of parallel jobs" {
        cli "--jobs" "-j"
        env "MYCLI_JOBS"
    }
}
```

Keys are dotted paths. `prop` blocks do not nest — write `prop "status.missing_tools"`
rather than a `status` block containing a `missing_tools` one — because one spelling keeps
merging, ordering and round-tripping unambiguous, and a schema generator can always
re-nest on the dots.

## Where values come from

Highest precedence first, and only the layers a CLI actually has:

- the command line
- the environment
- config files, nearest first, with a `.local` variant outranking its base
- the user's own configuration
- files installed for the whole machine
- the default a `prop` declares

A property can name its sources explicitly; the order they are written in is the order they
are consulted.

```kdl
prop "check" type="bool" {
    cli "--check"              // flags that set it, as declared elsewhere in this spec
    env "HK_CHECK" "HK_LINT"   // several: aliases, highest precedence first
    source "git" "hk.check"    // a source kind declared below
}
```

## `source` — kinds usage does not know about

usage reads the command line, the environment, and config files. Everything else — a git
config, a pkl file, an `.npmrc` — is a _kind_ the CLI reads itself and declares here so
documentation can describe it. `{key}` and `{value}` are substituted.

```kdl
config {
    source "git" name="git config" doc_hint="git config `{key}`" \
        set_hint="git config {key} {value}"
    source "pkl" name="hk.pkl"
}
```

Docs then render a property bound to that kind as "settable with `git config hk.check`",
without usage having any idea what git is.

## `file` — where config files live

In ascending precedence: the last one named wins. This is the chain that rc-style merging
walks, and writing it down is what lets documentation describe it accurately.

```kdl
config {
    file "/etc/mycli/config.toml" scope="system"
    file "~/.config/mycli/config.toml" scope="global"
    file "mycli.toml" findup=#true
    file "mycli.local.toml" findup=#true
    file ".myclirc" format="ini"
}
```

| property | meaning                                                      |
| -------- | ------------------------------------------------------------ |
| `findup` | look for this name in the current directory and every parent |
| `scope`  | `project` (default), `global`, or `system`                   |
| `format` | when the extension does not say: `toml`, `json`, `ini`       |

`scope` is not decoration: a `prop` marked `scope="global"` refuses values from `project`
files, so a setting a repository must not be able to change can say so.

## `prop` — the settings

| property                                     | meaning                                                                                                      |
| -------------------------------------------- | ------------------------------------------------------------------------------------------------------------ |
| `type`                                       | the type, see [below](#types). Defaults to `string`                                                          |
| `default`                                    | the value when nothing supplies one — a typed KDL value: `4`, `#true`, `"x"`                                 |
| `default_note`                               | what to print instead of the default, when the real one needs explaining                                     |
| `help`, `long_help`                          | one line, and the whole markdown story                                                                       |
| `help_heading`                               | the section to list it under in generated docs                                                               |
| `merge`                                      | `replace` (default), `union` for collections, `deep` for maps                                                |
| `scope`                                      | `any` (default), `global` (never from a project file), `env` (never from a file)                             |
| `deprecated`                                 | why not to use it any more                                                                                   |
| `deprecated_warn_at`, `deprecated_remove_at` | versions, for a tool that warns then removes                                                                 |
| `renamed_to`                                 | the property that replaces this one, so an old key folds into the new                                        |
| `hide`                                       | keep it out of docs and completions                                                                          |
| `since`                                      | the version that introduced it                                                                               |
| `parse`                                      | a named parser for one string: `list_by_comma`, `list_by_colon`, `list_by_os_path_separator`, `set_by_comma` |
| `writes_to`                                  | where `config set` should write it, when that is not the usual file                                          |

And as child nodes, for anything multi-valued or long:

| node                              | meaning                                                       |
| --------------------------------- | ------------------------------------------------------------- |
| `cli "--jobs" "-j"`               | flags that set it                                             |
| `env "A" "B"`                     | environment variables, highest precedence first               |
| `source "git" "a.b"`              | its keys in a declared source kind                            |
| `default "a" "b"`                 | a list default, values typed as written (`default 80 443`)    |
| `long_help "…"`                   | the long form, when a raw string reads better than a property |
| `example "…"`                     | one invocation worth showing                                  |
| `choices { choice "a" help="…" }` | the values it accepts, each with its own help                 |
| `x "ns.key" value`                | see [extensions](#extensions)                                 |

## Types

```
base  := bool | string | int | uint | float | path | url | duration | object
type  := base | list<type> | set<type> | map<base, type> | option<type> | type "|" type
```

`option<T>` means absent is a legitimate state with no default standing in. A union —
`bool|string` — records that a setting takes either; nothing here validates which.

A name this version of usage does not know is kept as written rather than refused, so a
spec can name a type only its own tool understands and still load everywhere else. Anything
consuming a type it cannot interpret treats it as a string.

For compatibility, `data_type` is still read as a spelling of `type`, and the older names
(`boolean`, `integer`, `number`, `usize`, `array<…>`, `optional<…>`) are accepted.

## Extensions

A tool often needs to carry something usage has no opinion about — which Rust type a
setting deserializes into, which file a write should be routed to, an enterprise policy.
`x` nodes hold it: preserved in order, written back out unchanged, present in
`usage g json`, and interpreted by nothing in usage.

```kdl
prop "python.uv_venv_auto" type="bool|string" {
    x "mise.rust_type" "PythonUvVenvAuto"
    x "mise.parse_env" "bool_string"
}
```

This is the seam that lets a CLI with special rules describe its settings here without
usage having to model those rules.

## Completing settings

A `config get`/`config set` pair completes from the block without a `run` of its own — see
[`complete`](./complete.md#completing-settings):

```kdl
complete "key" type="config_keys"
complete "value" type="config_values"
```

## Splitting it out

A CLI with many settings does not want them inline. `include` reads another file, resolved
relative to the one naming it:

```kdl
name "mise"
bin "mise"
include file="./settings.usage.kdl"
```

## Compatibility

The parser refuses vocabulary it does not know, so a spec using anything on this page must
say which version of usage it needs:

```kdl
min_usage_version "2.0"
```

Without it, an older `usage` fails to read the whole spec rather than quietly ignoring the
part it cannot understand.
