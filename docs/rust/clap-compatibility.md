# clap compatibility

This is the audited compatibility matrix for `clap` 4.6.6 and `clap_derive` 4.6.4,
the versions in this workspace's lockfile. The audit covers clap's public derive
attributes and the corresponding `Command`, `Arg`, `ArgGroup`, and `PossibleValue`
builder settings that affect parsing or generated output. Builder operations for
constructing and mutating a command graph are listed separately as architectural
non-goals.

Updating either clap package requires updating the versions above and auditing this
matrix in the same pull request.

The columns distinguish every layer a migration crosses:

- **derive** — `usage-rs` / `usage-derive` can declare it and has typed coverage.
- **argv** — the compiled `usage-argv` parser enforces it.
- **KDL** — the portable spec can represent and round-trip it.
- **lib** — `usage-lib` enforces it when interpreting KDL.
- **output** — help, diagnostics, docs, or completions preserve the relevant behavior.
- **bridge** — `clap_usage` can recover it from a `clap::Command`.

| Mark           | Meaning                                                                                                |
| -------------- | ------------------------------------------------------------------------------------------------------ |
| **yes**        | Supported and covered at this layer.                                                                   |
| **usage-only** | Direct usage declarations work, but clap exposes no getter or the bridge cannot preserve the behavior. |
| **lossy**      | Some common forms work; the note names what is lost.                                                   |
| **different**  | Intentionally differs from clap.                                                                       |
| **no**         | Unsupported.                                                                                           |
| **n/a**        | The behavior does not belong at this layer.                                                            |
| **non-goal**   | Deliberately outside the static typed-parser API.                                                      |

The bridge is not a lossless migration verifier. clap exposes some settings only
through setters, so a generated spec cannot report that it lost them. Migrate from
the Rust declaration, not only from generated KDL, wherever the bridge column says
**usage-only** or **lossy**.

## Types and declarations

| clap surface                                     | derive   | argv     | KDL | lib | output | bridge | Notes                                                                                              |
| ------------------------------------------------ | -------- | -------- | --- | --- | ------ | ------ | -------------------------------------------------------------------------------------------------- |
| `Parser` / `Command` metadata                    | yes      | yes      | yes | yes | yes    | yes    | `#[derive(usage::Cli)]`; name, bin, about, long about, before/after help, and version are carried. |
| `Args`                                           | yes      | yes      | yes | yes | yes    | yes    | Dedicated, unit, reused, nested, and flattened Args types are covered.                             |
| `Subcommand`                                     | yes      | yes      | yes | yes | yes    | yes    | Bare, tuple, inline-struct, nested, boxed, aliases, and hidden aliases are covered.                |
| `ValueEnum` / `PossibleValue`                    | yes      | yes      | yes | yes | yes    | yes    | Names, help, hide, visible/hidden aliases, cfg, and case-insensitive matching are preserved.       |
| `flatten`                                        | yes      | yes      | yes | yes | yes    | yes    | Parsing and flattened `next_help_heading` topology are composed.                                   |
| `skip`                                           | yes      | yes      | n/a | n/a | n/a    | n/a    | `#[usage(skip)]` fills the field from `Default` and emits no argument.                             |
| `from_global`                                    | no       | no       | no  | no  | no     | no     | A global flag is parsed on its declaring root type; copying it into another field is unsupported.  |
| arbitrary `Command` / `Arg` builder code         | non-goal | non-goal | n/a | yes | yes    | lossy  | `usage-lib` is the dynamic API; the typed derive does not reproduce clap's builder API.            |
| `ArgMatches`, `FromArgMatches`, `CommandFactory` | non-goal | non-goal | n/a | n/a | n/a    | n/a    | Typed structs and borrowed static metadata replace these APIs.                                     |
| `update_from` / `try_update_from`                | no       | no       | n/a | n/a | n/a    | n/a    | Parsing currently constructs a new value.                                                          |

## Arguments and values

| clap surface                                                 | derive     | argv | KDL   | lib | output | bridge     | Notes                                                                                                                                                   |
| ------------------------------------------------------------ | ---------- | ---- | ----- | --- | ------ | ---------- | ------------------------------------------------------------------------------------------------------------------------------------------------------- |
| `long`, `short`, visible aliases                             | yes        | yes  | yes   | yes | yes    | yes        | Multiple forms are accepted and advertised by parsing, help, completion, and generated tables.                                                          |
| hidden flag `alias` / `aliases`                              | yes        | yes  | yes   | yes | yes    | yes        | Hidden aliases bind and round-trip through KDL and generated Rust/Go tables without appearing in help or completion.                                    |
| explicit `id`                                                | yes        | yes  | yes   | yes | yes    | yes        | `#[arg(id = "…")]` supplies the stable field identity used by relationships and generated specs.                                                        |
| positional arguments                                         | yes        | yes  | yes   | yes | yes    | yes        | Required, optional, and variadic positionals are supported.                                                                                             |
| `Option<T>`, `Vec<T>`, `Option<Vec<T>>`, `Option<Option<T>>` | yes        | yes  | yes   | yes | yes    | n/a        | Nested `Option` preserves absent, bare, and explicitly valued flags; values use the declared `FromStr` or `ValueEnum` conversion.                       |
| derived `ValueEnum`                                          | yes        | yes  | yes   | yes | yes    | n/a        | Canonical values, aliases, case policy, help, and direct enum binding come from one derive; no separate `FromStr` is required.                          |
| `ArgAction::Set`, `SetTrue`, `SetFalse`, `Append`, `Count`   | yes        | yes  | yes   | yes | yes    | lossy      | Common typed shapes are covered; arbitrary action/type combinations are not.                                                                            |
| `default_value`                                              | yes        | yes  | yes   | yes | yes    | yes        | Defaults apply after argv and environment values and clear token-required metadata.                                                                     |
| `default_missing_value`                                      | yes        | yes  | yes   | yes | yes    | usage-only | `#[usage(default_missing = "…")]`; clap has no getter.                                                                                                  |
| `default_value_if(s)`                                        | yes        | yes  | yes   | yes | yes    | usage-only | Presence and equality predicates are portable; clap has no getter.                                                                                      |
| `env`                                                        | yes        | yes  | yes   | yes | yes    | lossy      | Environment fallback works; the current bridge can lose the binding.                                                                                    |
| `value_delimiter`                                            | yes        | yes  | yes   | yes | yes    | yes        | ASCII delimiters round-trip and are applied before arity checks.                                                                                        |
| `num_args` ranges                                            | yes        | yes  | yes   | yes | yes    | lossy      | Nested value `var_min` / `var_max` preserve per-occurrence ranges separately from flag occurrence bounds; zero-minimum flag ranges can be bridge-lossy. |
| fixed `num_args` with distinct `value_names`                 | yes        | yes  | yes   | yes | yes    | yes        | `#[arg(num_args = 2, value_names = ["START", "END"])]` preserves the exact bound and both placeholders.                                                 |
| `allow_hyphen_values`                                        | yes        | yes  | yes   | yes | yes    | yes        | Supported on value-taking flags; forwarded positionals use `double_dash = "automatic"`.                                                                 |
| `allow_negative_numbers`                                     | yes        | yes  | yes   | yes | yes    | yes        | Accepts negative numeric tokens without accepting arbitrary dash-prefixed values.                                                                       |
| `require_equals`                                             | yes        | yes  | yes   | yes | yes    | yes        | Detached values are refused.                                                                                                                            |
| explicit boolean `--flag=false`                              | usage-only | yes  | yes   | yes | yes    | no         | `#[usage(bool_value)]` opts a switch into exact, attached `true`/`false` values; detached words remain positional.                                      |
| `value_terminator`                                           | yes        | yes  | yes   | yes | yes    | yes        | Ends a variadic value owner without binding the terminator token.                                                                                       |
| `trailing_var_arg` / `last`                                  | yes        | yes  | yes   | yes | yes    | lossy      | `double_dash` carries automatic/required/optional; clap shadow generation still drops automatic mode.                                                   |
| `dont_delimit_trailing_values`                               | yes        | yes  | yes   | yes | yes    | yes        | Preserves delimiters after `--` and on automatic trailing positionals while ordinary values still split.                                                |
| possible-values parser                                       | yes        | yes  | yes   | yes | yes    | yes        | Use `ValueEnum` or `choices`.                                                                                                                           |
| non-strict suggested values                                  | usage-only | yes  | yes   | yes | yes    | no         | `choices_strict = false` keeps choices presentational while accepting other values.                                                                     |
| arbitrary `value_parser` callbacks                           | usage-only | yes  | lossy | yes | yes    | no         | `FromStr` handles typed conversion and portable `validate` expressions handle declarative rules; Rust callbacks cannot enter KDL.                       |
| `ValueHint` completion vocabulary                            | yes        | yes  | yes   | yes | yes    | yes        | Every stable clap hint lowers to a portable completion type; open-ended URL/email hints suppress path fallback.                                         |

## Relationships and command routing

| clap surface                                         | derive    | argv      | KDL       | lib       | output  | bridge     | Notes                                                                                                                   |
| ---------------------------------------------------- | --------- | --------- | --------- | --------- | ------- | ---------- | ----------------------------------------------------------------------------------------------------------------------- |
| flag `required`, `conflicts_with`, `overrides_with`  | yes       | yes       | yes       | yes       | yes     | yes        | Environment values participate in post-binding checks.                                                                  |
| `requires`                                           | yes       | yes       | yes       | yes       | yes     | usage-only | clap exposes setters but no getter.                                                                                     |
| `requires_if(s)`                                     | yes       | yes       | yes       | yes       | yes     | usage-only | Presence and value-conditional forms are supported.                                                                     |
| `required_if_eq`, `required_unless_present` families | exact     | exact     | exact     | exact     | exact   | usage-only | Single, any, and all truth tables work for flags and positionals; clap exposes setters but no getters.                  |
| `ArgGroup`, required groups, `exclusive`             | yes       | yes       | yes       | yes       | yes     | yes        | Bare selectors preserve positional members.                                                                             |
| positional conflicts                                 | yes       | yes       | yes       | yes       | yes     | yes        | Bare selectors name positionals; dashed selectors name flags.                                                           |
| other relationships declared on positionals          | partial   | partial   | partial   | partial   | partial | usage-only | `requires` and conditional requiredness work; binding-time `overrides` and value-source `requires_if` remain flag-only. |
| relationships through `flatten`                      | lossy     | lossy     | yes       | yes       | yes     | lossy      | A declaring type cannot yet validate a selector supplied by a flattened sibling.                                        |
| global flags                                         | yes       | yes       | yes       | yes       | yes     | yes        | Exact lookup preserves child shadowing.                                                                                 |
| `allow_external_subcommands`                         | yes       | yes       | yes       | yes       | yes     | yes        | Use an `#[usage(external_subcommand)]` catch-all variant.                                                               |
| `multicall`                                          | yes       | yes       | yes       | yes       | yes     | yes        | Process-level entry points route using the executable basename.                                                         |
| `no_binary_name`                                     | yes       | yes       | n/a       | yes       | n/a     | yes        | `parse_from` is words-only; full-argv helpers honor the command policy.                                                 |
| `infer_subcommands`, `infer_long_args`               | no        | no        | no        | no        | no      | no         | Intentional non-goal: usage requires exact flag and subcommand spellings.                                               |
| `arg_required_else_help`                             | yes       | yes       | yes       | yes       | yes     | yes        | Bare selected commands request short help; defaults and environment values do not count as argv.                        |
| `args_override_self`                                 | yes       | yes       | yes       | yes       | yes     | yes        | Usage defaults to permissive last-one-wins behavior; set false for strict duplicate checking.                           |
| `subcommand_negates_reqs`                            | yes       | yes       | yes       | yes       | yes     | yes        | A selected child suppresses its parent's positive requirements, not conflicts or the child's requirements.              |
| `args_conflicts_with_subcommands`                    | yes       | yes       | yes       | yes       | yes     | yes        | Parent flags or positionals exclude a later child subcommand.                                                           |
| `subcommand_precedence_over_arg`                     | yes       | yes       | yes       | yes       | yes     | yes        | A known child can end a variadic flag or positional value owner.                                                        |
| `allow_missing_positional`                           | yes       | yes       | yes       | yes       | yes     | yes        | Later required positionals can claim the remaining words while earlier optional fields stay empty.                      |
| unknown flags                                        | different | different | different | different | yes     | different  | usage is permissive by default; `unknown_flags = "error"` opts into strict parsing.                                     |

## Help, version, and generated artifacts

| clap surface                                       | derive | argv | KDL | lib | output | bridge  | Notes                                                                                                                |
| -------------------------------------------------- | ------ | ---- | --- | --- | ------ | ------- | -------------------------------------------------------------------------------------------------------------------- |
| short/long help and doc comments                   | yes    | yes  | yes | yes | yes    | yes     | First paragraph is short help; the full block is long help.                                                          |
| `help_heading` on flags and arguments              | yes    | n/a  | yes | yes | yes    | yes     | Flags and arguments are grouped and retain declaration order.                                                        |
| `help_heading` on subcommands                      | yes    | yes  | yes | yes | yes    | n/a     | Commands can be grouped into named sections in their parent's help.                                                  |
| whole-entry `hide`                                 | yes    | yes  | yes | yes | yes    | yes     | Hidden commands, flags, arguments, and values still parse.                                                           |
| granular hide settings                             | yes    | yes  | yes | yes | yes    | yes     | Default, environment, possible-value, short-help, and long-help visibility is independent.                           |
| `subcommand_help_heading`, `subcommand_value_name` | yes    | yes  | yes | yes | yes    | yes     | Customize the subcommand section label and the synopsis placeholder.                                                 |
| `verbatim_doc_comment`                             | yes    | n/a  | yes | yes | yes    | n/a     | Commands, fields, and variants preserve line breaks and indentation when requested.                                  |
| `rename_all`, `rename_all_env`                     | yes    | n/a  | yes | yes | yes    | n/a     | Full clap casing vocabulary; bare `env` uses the environment casing policy.                                          |
| `next_line_help`                                   | yes    | yes  | yes | yes | yes    | yes     | Put command, argument, and flag descriptions below their usage instead of beside it.                                 |
| `flatten_help`                                     | yes    | yes  | yes | yes | yes    | yes     | Expand visible subcommands into their parent's usage synopsis and help page.                                         |
| `display_order`                                    | yes    | yes  | yes | yes | yes    | yes     | Explicit field and subcommand presentation order is portable; parsing order is unchanged.                            |
| `help_template`                                    | no     | n/a  | no  | no  | no     | no      | No equivalent yet.                                                                                                   |
| `term_width`, `max_term_width`                     | yes    | yes  | yes | yes | yes    | no      | Fixed width overrides a detected-width cap; clap exposes no bridge getters for these settings.                       |
| help styles and color                              | n/a    | n/a  | n/a | yes | lossy  | no      | Help and diagnostics use automatic ANSI styles; clap's custom style palette is not portable.                         |
| built-in help/version action and flag control      | yes    | yes  | yes | yes | yes    | yes     | `Help`, `HelpShort`, `HelpLong`, and `Version` actions can relocate built-ins; each synthetic entry can be disabled. |
| `--version` / `-V`, dynamic and long versions      | yes    | yes  | yes | yes | yes    | yes     | `long_version` customizes `--version`; `-V` keeps the concise value.                                                 |
| `author`, `license`, `repository`                  | yes    | n/a  | yes | yes | yes    | partial | Package metadata is rendered in Markdown and manpages; clap exposes author but not license.                          |
| completion generation                              | yes    | yes  | yes | yes | lossy  | yes     | Bash, fish, Nushell, PowerShell, and zsh plus runtime overlays are supported; Elvish is not.                         |
| KDL, markdown, JSON, and manpages                  | yes    | n/a  | yes | yes | yes    | yes     | Direct derived KDL feeds the existing generators; broader canonicalization remains open.                             |

## Usage extensions

These are not clap compatibility gaps. usage additionally supports `mount`,
`restart_token`, `default_subcommand`, command and flag `effect`, Nushell completions,
and a language-neutral conformance corpus. clap cannot express those properties, so
a clap-generated spec cannot carry them without an overlay.

This matrix is the compatibility baseline, not a promise to reproduce clap's dynamic
builder and `ArgMatches` architecture. Setter-only clap state remains explicitly
**usage-only** until clap exposes a getter or the bridge gains another reliable source.
