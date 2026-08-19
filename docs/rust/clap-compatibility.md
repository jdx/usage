# clap compatibility

This matrix describes compatibility with `clap` 4.6.6 and `clap_derive` 4.6.4, the
versions audited in this workspace. It distinguishes what a CLI can declare directly with
`usage` from what [`clap_usage`](/spec/integrations/clap) can recover from an existing
`clap::Command`.

The bridge is not a lossless migration verifier. clap exposes some settings only through
setters, so `clap_usage` cannot detect that they were present. Migrate from the Rust declaration,
not only from generated KDL, when a row says **usage only**.

## Status

| Status          | Meaning                                                                                    |
| --------------- | ------------------------------------------------------------------------------------------ |
| **Supported**   | The derive, compiled parser, spec, and reference parser carry the behavior.                |
| **Usage only**  | A usage declaration carries it, but clap exposes no getter or the bridge loses part of it. |
| **Partial**     | The common form works; the note names the unsupported part.                                |
| **Different**   | usage intentionally has different parsing behavior.                                        |
| **Unsupported** | No equivalent is available yet.                                                            |
| **Non-goal**    | The API shape is outside the static-parser architecture.                                   |

## Types and declarations

| clap                                             | usage       | clap → spec    | Notes                                                                                                                                               |
| ------------------------------------------------ | ----------- | -------------- | --------------------------------------------------------------------------------------------------------------------------------------------------- |
| `Parser`                                         | Supported   | Supported      | `#[derive(usage::Cli)]`                                                                                                                             |
| `Args`                                           | Supported   | Supported      | `#[derive(usage::Args)]`, including `flatten`                                                                                                       |
| `Subcommand`                                     | Supported   | Supported      | Nested and boxed variants are supported.                                                                                                            |
| `ValueEnum`                                      | Supported   | Supported      | Names, per-value help, hidden values, visible and hidden aliases, cfg-gated variants, and case-insensitive matching are carried in static metadata. |
| `#[arg(skip)]`                                   | Supported   | Not applicable | `#[usage(skip)]`; skipped fields do not belong in a spec.                                                                                           |
| arbitrary `Command` / `Arg` builder code         | Non-goal    | Partial        | `usage-lib` is the dynamic spec interpreter; usage derive does not reproduce clap's builder API.                                                    |
| `ArgMatches`, `FromArgMatches`, `CommandFactory` | Non-goal    | Not applicable | Typed structs are built directly from static tables.                                                                                                |
| `update_from` / `try_update_from`                | Unsupported | Not applicable | A parse currently constructs a new value.                                                                                                           |

## Arguments and values

| clap                                      | usage       | clap → spec    | Notes                                                                                                                                             |
| ----------------------------------------- | ----------- | -------------- | ------------------------------------------------------------------------------------------------------------------------------------------------- |
| long and short flags                      | Supported   | Partial        | Multiple visible and hidden aliases are supported directly; clap spelling and alias visibility are not all bridge-lossless.                       |
| positional arguments                      | Supported   | Supported      | Required, optional, and variadic positionals are supported.                                                                                       |
| `Option<T>`, `Vec<T>`, `Option<Vec<T>>`   | Supported   | Not applicable | Values use `FromStr`; Unix `PathBuf` and `OsString` also accept non-UTF-8 bytes.                                                                  |
| `ArgAction::SetTrue`, `SetFalse`, `Count` | Supported   | Supported      | `SetFalse` is `#[usage(negate)]`.                                                                                                                 |
| `default_value`                           | Supported   | Supported      | Defaults are applied after argv and environment values.                                                                                           |
| `default_missing_value`                   | Usage only  | Unsupported    | `#[usage(default_missing = "…")]`; clap has no getter.                                                                                            |
| `default_value_if(s)`                     | Usage only  | Unsupported    | `#[usage(default_if(...))]`; clap has no getter.                                                                                                  |
| `env`                                     | Supported   | Partial        | Environment fallback works, but the current bridge can lose the binding.                                                                          |
| `value_delimiter`                         | Supported   | Supported      | ASCII delimiters round-trip.                                                                                                                      |
| `num_args`                                | Partial     | Partial        | `var_min` / `var_max` cover ranges; fixed arity, distinct value names, and bridge preservation remain incomplete.                                 |
| `allow_hyphen_values`                     | Supported   | Supported      | Available on value-taking flags; trailing positionals use `double_dash = "automatic"`.                                                            |
| `allow_negative_numbers`                  | Unsupported | Unsupported    | `allow_hyphen_values` is broader, not equivalent.                                                                                                 |
| `require_equals`                          | Supported   | Supported      | Detached values are refused.                                                                                                                      |
| `value_terminator`                        | Unsupported | Unsupported    | No spec spelling yet.                                                                                                                             |
| `dont_delimit_trailing_values`            | Unsupported | Unsupported    | No spec spelling yet.                                                                                                                             |
| possible-values parser                    | Supported   | Supported      | Use `ValueEnum` or `choices`.                                                                                                                     |
| arbitrary `value_parser` / numeric ranges | Usage only  | Unsupported    | `FromStr` validates conversion; portable `validate` expressions cover declarative rules, but Rust callbacks cannot enter a language-neutral spec. |
| `ValueHint`                               | Partial     | Partial        | Path hints used by completions work; the full clap vocabulary does not.                                                                           |

## Relationships and command routing

| clap                                                      | usage       | clap → spec | Notes                                                                                                    |
| --------------------------------------------------------- | ----------- | ----------- | -------------------------------------------------------------------------------------------------------- |
| `required`, `conflicts_with`, `overrides_with`            | Supported   | Supported   | Environment values participate in post-binding checks.                                                   |
| `requires`                                                | Usage only  | Unsupported | clap exposes setters but no getter.                                                                      |
| `requires_if(s)`                                          | Usage only  | Unsupported | Presence and value-conditional forms are supported in usage.                                             |
| `required_if_eq`, `required_unless_present`               | Partial     | Partial     | Common single-selector forms work; the complete all/any families do not.                                 |
| `ArgGroup`, required groups, `exclusive`                  | Supported   | Supported   | Positional group members are not represented yet.                                                        |
| positional conflicts and relationships                    | Unsupported | Unsupported | Spec selectors currently name flags.                                                                     |
| global flags                                              | Supported   | Supported   | A global may occur once per selected command scope.                                                      |
| `allow_external_subcommands`                              | Supported   | Supported   | Use an `#[usage(external_subcommand)]` catch-all variant.                                                |
| `multicall`                                               | Supported   | Supported   | `parse()` routes on the executable basename.                                                             |
| `no_binary_name`                                          | Supported   | Supported   | `parse_from` is words-only; `try_parse_from` and `parse_from_argv` honor the clap-shaped command policy. |
| `infer_subcommands`, `infer_long_args`                    | Unsupported | Unsupported | Diagnostics suggest likely names but do not accept prefixes.                                             |
| `arg_required_else_help` and subcommand/argument policies | Unsupported | Unsupported | No command-policy vocabulary yet.                                                                        |
| unknown flags                                             | Different   | Different   | Parsing is permissive by default. Opt into clap-like rejection with `#[usage(unknown_flags = "error")]`. |

## Help, version, and generated artifacts

| clap                                              | usage       | clap → spec    | Notes                                                                                                               |
| ------------------------------------------------- | ----------- | -------------- | ------------------------------------------------------------------------------------------------------------------- |
| short and long help                               | Supported   | Supported      | The compiled renderer is checked against usage-lib across the jdx CLI fleet.                                        |
| help headings                                     | Supported   | Supported      | Flags and arguments are grouped by heading.                                                                         |
| hidden commands, flags, and arguments             | Supported   | Supported      | Granular hides for defaults, env, and possible values are unsupported.                                              |
| doc comments and long help                        | Supported   | Supported      | First paragraph is short help; the full block is long help.                                                         |
| `verbatim_doc_comment`                            | Unsupported | Not applicable | Doc comments are normalized.                                                                                        |
| `help_template`, `next_line_help`, `flatten_help` | Unsupported | Unsupported    | No equivalent yet.                                                                                                  |
| `term_width`, `max_term_width`                    | Unsupported | Unsupported    | Help wraps using `COLUMNS`.                                                                                         |
| help styles and color                             | Partial     | Unsupported    | Diagnostics are styled; help output is not.                                                                         |
| built-in help/version flag control                | Unsupported | Unsupported    | Custom help/version actions and disabling built-ins are not represented.                                            |
| `--version` / `-V`                                | Supported   | Supported      | Version propagation is supported.                                                                                   |
| completions                                       | Partial     | Supported      | Self-contained bash, fish, Nushell, PowerShell, and zsh scripts plus runtime overlays are available; Elvish is not. |
| KDL, markdown, and manpages                       | Supported   | Supported      | Emitted KDL feeds the existing generators without a runtime dependency.                                             |

## Usage extensions

These are not clap compatibility gaps. usage additionally supports `mount`, `restart_token`,
`default_subcommand`, command and flag `effect`, Nushell completions, and a language-neutral
conformance corpus. clap cannot express those properties, so a clap-generated spec cannot carry
them without an overlay.

This page is the compatibility baseline, not a claim that every clap builder method is covered.
When clap is updated, audit new public derive attributes and relevant `Command`, `Arg`, and
`PossibleValue` methods here before treating the update as migration-neutral.
