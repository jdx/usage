use itertools::Itertools;
use kdl::{KdlDocument, KdlEntry, KdlNode};
use serde::Serialize;
use std::fmt::Display;
use std::hash::Hash;
use std::str::FromStr;

use crate::error::UsageErr::InvalidFlag;
use crate::error::{Result, UsageErr};
use crate::spec::arg::SpecDoubleDashChoices;
use crate::spec::builder::SpecFlagBuilder;
use crate::spec::context::ParsingContext;
use crate::spec::effect::{SpecCommandEffect, EFFECT_VALUES};
use crate::spec::helpers::{string_entry, NodeHelper};
use crate::spec::is_false;
use crate::{string, SpecArg, SpecChoices, SpecRequiredIfEq};

/// A non-binding action performed when a flag is supplied.
#[derive(Debug, Default, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum SpecFlagAction {
    #[default]
    Set,
    Help,
    HelpShort,
    HelpLong,
    HelpAll,
    Version,
}

impl SpecFlagAction {
    fn parse(value: &str) -> Option<Self> {
        Some(match value {
            "set" => Self::Set,
            "help" => Self::Help,
            "help_short" => Self::HelpShort,
            "help_long" => Self::HelpLong,
            "help_all" => Self::HelpAll,
            "version" => Self::Version,
            _ => return None,
        })
    }

    pub fn as_str(self) -> &'static str {
        match self {
            Self::Set => "set",
            Self::Help => "help",
            Self::HelpShort => "help_short",
            Self::HelpLong => "help_long",
            Self::HelpAll => "help_all",
            Self::Version => "version",
        }
    }
}

/// A requirement activated by one of a flag's values.
///
/// `flag "--config <file>" { requires_if "special.toml" "--key" }`
/// means `--key` is required only when `--config` was explicitly given the
/// value `special.toml`.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct SpecRequiresIf {
    /// The declaring flag's value that activates the requirement.
    pub value: String,
    /// The flag selector that must then be satisfied.
    pub requires: String,
}

/// A default that applies when another flag is given.
///
/// Lives on the *target* flag, the inverse of [`SpecRequiresIf`]:
/// `flag "--bin-names" { default_if "--json" "true" }` binds `true` on
/// `--bin-names` when `--json` was given. Two arguments are clap's
/// `ArgPredicate::IsPresent`; three (`default_if "--output" "json" "pretty"`)
/// are `Equals`. First match wins. Command-line and environment values on
/// this flag suppress it; a `default_if` value is a default, not an explicit
/// value, so it does not activate `requires_if`.
///
/// clap 4 has `Arg::default_value_if` as a setter with no getter, so a spec
/// generated from a clap command never carries this — same hole as
/// [`SpecFlag::requires`].
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct SpecDefaultIf {
    /// The other flag that decides whether this default applies (`"--json"`).
    pub selector: String,
    /// When set, the selector must have this explicit value (`Equals`).
    /// When `None`, the selector only has to be present (`IsPresent`).
    #[serde(skip_serializing_if = "Option::is_none")]
    pub when: Option<String>,
    /// The value to bind on this flag when the condition matches.
    pub value: String,
}

/// A CLI flag/option specification.
///
/// Flags are optional arguments that start with `-` (short) or `--` (long).
/// They can be boolean switches or accept values.
///
/// # Example
///
/// ```
/// use usage::SpecFlag;
///
/// let flag = SpecFlag::builder()
///     .short('v')
///     .long("verbose")
///     .help("Enable verbose output")
///     .build();
/// ```
#[derive(Debug, Default, Clone, Serialize)]
#[non_exhaustive]
pub struct SpecFlag {
    /// Internal name for the flag (derived from long/short if not set)
    pub name: String,
    /// Generated usage string (e.g., "-v, --verbose")
    pub usage: String,
    /// Short help text shown in command listings
    #[serde(skip_serializing_if = "Option::is_none")]
    pub help: Option<String>,
    /// Extended help text shown with --help
    #[serde(skip_serializing_if = "Option::is_none")]
    pub help_long: Option<String>,
    /// Markdown-formatted help text
    #[serde(skip_serializing_if = "Option::is_none")]
    pub help_md: Option<String>,
    /// First line of help text (auto-generated)
    #[serde(skip_serializing_if = "Option::is_none")]
    pub help_first_line: Option<String>,
    /// Short flag characters (e.g., 'v' for -v)
    pub short: Vec<char>,
    /// Short aliases accepted by parsing but omitted from help and completion.
    #[serde(skip_serializing_if = "Vec::is_empty")]
    pub hidden_short_aliases: Vec<char>,
    /// Long flag names (e.g., "verbose" for --verbose)
    pub long: Vec<String>,
    /// Long aliases accepted by parsing but omitted from help and completion.
    #[serde(skip_serializing_if = "Vec::is_empty")]
    pub hidden_aliases: Vec<String>,
    /// Whether this flag must be provided
    #[serde(skip_serializing_if = "is_false")]
    pub required: bool,
    /// Flags whose presence makes this flag required
    #[serde(skip_serializing_if = "Vec::is_empty")]
    pub required_if: Vec<String>,
    /// Value conditions, any one of which makes this flag required.
    #[serde(skip_serializing_if = "Vec::is_empty")]
    pub required_if_eq: Vec<SpecRequiredIfEq>,
    /// Value conditions which must all match to make this flag required.
    #[serde(skip_serializing_if = "Vec::is_empty")]
    pub required_if_eq_all: Vec<SpecRequiredIfEq>,
    /// Flags whose absence makes this flag required
    #[serde(skip_serializing_if = "Vec::is_empty")]
    pub required_unless: Vec<String>,
    /// Only the presence of every selector waives this flag's requirement.
    #[serde(skip_serializing_if = "Vec::is_empty")]
    pub required_unless_all: Vec<String>,
    /// Deprecation message if this flag is deprecated
    #[serde(skip_serializing_if = "Option::is_none")]
    pub deprecated: Option<String>,
    /// Version at which consumers should begin warning about this flag.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub deprecated_warn_at: Option<String>,
    /// Version at which consumers expect this flag to be removed.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub deprecated_remove_at: Option<String>,
    /// Whether this flag can be specified multiple times
    #[serde(skip_serializing_if = "is_false")]
    pub var: bool,
    /// Minimum number of times this flag must appear (for var flags)
    #[serde(skip_serializing_if = "Option::is_none")]
    pub var_min: Option<usize>,
    /// Maximum number of times this flag can appear (for var flags)
    #[serde(skip_serializing_if = "Option::is_none")]
    pub var_max: Option<usize>,
    /// Whether to hide this flag from help output
    pub hide: bool,
    /// Hide the default annotation while keeping the default behavior.
    #[serde(skip_serializing_if = "is_false")]
    pub hide_default_value: bool,
    /// Hide the environment annotation entirely.
    #[serde(skip_serializing_if = "is_false")]
    pub hide_env: bool,
    /// Hide an environment value while retaining its variable name.
    #[serde(skip_serializing_if = "is_false")]
    pub hide_env_values: bool,
    /// Hide possible values from help without changing validation.
    #[serde(skip_serializing_if = "is_false")]
    pub hide_possible_values: bool,
    /// Hide this flag only from short help.
    #[serde(skip_serializing_if = "is_false")]
    pub hide_short_help: bool,
    /// Hide this flag only from long help.
    #[serde(skip_serializing_if = "is_false")]
    pub hide_long_help: bool,
    /// Whether this flag is available to all subcommands
    pub global: bool,
    /// Whether this is a count flag (e.g., -vvv counts as 3)
    #[serde(skip_serializing_if = "is_false")]
    pub count: bool,
    /// Argument specification if this flag takes a value
    #[serde(skip_serializing_if = "Option::is_none")]
    pub arg: Option<SpecArg>,
    /// Default value(s) if the flag is not provided
    #[serde(skip_serializing_if = "Vec::is_empty")]
    pub default: Vec<String>,
    /// Negation prefix (e.g., "no-" for --no-verbose)
    #[serde(skip_serializing_if = "Option::is_none")]
    pub negate: Option<String>,
    /// Flags that this flag mutually overrides; the last one provided wins
    #[serde(skip_serializing_if = "Vec::is_empty")]
    pub overrides: Vec<String>,
    /// Flags that cannot be given alongside this one.
    ///
    /// Distinct from [`SpecFlag::overrides`], which is about the *last* one winning:
    /// conflicting flags are a mistake to report, not an order to resolve. clap has
    /// had `conflicts_with` for years and mise uses it forty times, so a spec
    /// generated from a clap command was losing it.
    #[serde(skip_serializing_if = "Vec::is_empty")]
    pub conflicts: Vec<String>,
    /// Flags that must also be given when this one is.
    ///
    /// The positive form of [`SpecFlag::conflicts`], and not the same statement as
    /// [`SpecFlag::required_if`] read backwards: `required_if` lives on the flag that
    /// becomes required, so declaring `--out` needs `--format` means editing `--format`,
    /// away from the flag the rule is about. `requires` lives on the flag that imposes
    /// the rule, which is where clap puts it and where a reader looks for it.
    ///
    /// Nothing generated from a clap command can carry this: clap 4.6 has `Arg::requires`
    /// and its variants as setters with no getter, so a `Command` cannot be asked what it
    /// requires. A CLI that declares it here gains a constraint its generated spec never
    /// had.
    #[serde(skip_serializing_if = "Vec::is_empty")]
    pub requires: Vec<String>,
    /// Flags required when this flag is explicitly given a particular value.
    ///
    /// Defaults do not activate the condition; command-line and environment
    /// values do. This matches clap's `requires_if`/`requires_ifs` semantics.
    #[serde(skip_serializing_if = "Vec::is_empty")]
    pub requires_if: Vec<SpecRequiresIf>,
    /// Defaults that apply when another flag is given.
    ///
    /// First match wins. Only considered when this flag was not on the command
    /// line and has no environment value. An applied `default_if` is a default,
    /// not an explicit value: it satisfies `requires` and does not activate
    /// `requires_if`.
    #[serde(skip_serializing_if = "Vec::is_empty")]
    pub default_if: Vec<SpecDefaultIf>,
    /// Whether this flag must be given on its own.
    ///
    /// The whole-command form of [`SpecFlag::conflicts`]: `--version` and `--help` are
    /// the shape — asking for one means the rest of the command line has nothing to act
    /// on. Everything the command declares counts, positionals included, which is what
    /// makes this different from being in a group with every other flag.
    #[serde(skip_serializing_if = "is_false")]
    pub exclusive: bool,
    /// Whether the value must be attached with `=`: `--flag=value` is accepted
    /// and `--flag value` is not. clap's `require_equals`. Aube's `--inspect`
    /// is the fleet case.
    #[serde(skip_serializing_if = "is_false")]
    pub require_equals: bool,
    /// Whether a value-taking flag may be present without a value.
    ///
    /// This is executable parser policy, distinct from the nested argument's
    /// `required` bit, which controls whether help renders `<VALUE>` or `[VALUE]`.
    #[serde(skip_serializing_if = "is_false")]
    pub value_optional: bool,
    /// Whether a boolean switch accepts an explicit attached value.
    ///
    /// Only `--flag=true` and `--flag=false` are values; a detached word remains
    /// a positional and the flag still renders without a value placeholder.
    #[serde(skip_serializing_if = "is_false")]
    pub bool_value: bool,
    /// Value used when the flag is present but no value is given.
    ///
    /// clap's `default_missing_value`: `--color` binds this string, `--color=never`
    /// binds `never`, and an absent flag stays absent (or takes [`Self::default`]).
    /// Combined with [`Self::require_equals`], a following word is still refused
    /// (`--inspect 9229`) while a bare `--inspect` binds this.
    ///
    /// clap 4 exposes this as a setter with no getter, so a spec generated from a
    /// clap command never carries it — same hole as [`Self::requires`].
    #[serde(skip_serializing_if = "Option::is_none")]
    pub default_missing: Option<String>,
    /// Raises the effect of the command when this flag is supplied.
    /// See [`crate::spec::effect::SpecCommandEffect`]; never lowers it.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub effect: Option<SpecCommandEffect>,
    /// Environment variable that can set this flag's value
    #[serde(skip_serializing_if = "Option::is_none")]
    pub env: Option<String>,
    /// Ordered environment variables consulted after [`Self::env`].
    #[serde(skip_serializing_if = "Vec::is_empty")]
    pub env_fallback: Vec<String>,
    /// Ordered compatibility aliases consulted last and advertised as deprecated.
    #[serde(skip_serializing_if = "Vec::is_empty")]
    pub deprecated_env: Vec<String>,
    /// Heading this flag is listed under in help output.
    ///
    /// Purely presentational: it groups a long flag list into sections rather
    /// than changing how anything parses. A CLI with dozens of flags — mise
    /// groups its `watch` passthrough arguments this way — is unreadable without
    /// it.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub help_heading: Option<String>,
    /// Explicit placement within its help section.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub display_order: Option<usize>,
    /// Whether this flag binds a value or requests help/version output.
    #[serde(skip_serializing_if = "is_set_action")]
    pub action: SpecFlagAction,
}

fn is_set_action(action: &SpecFlagAction) -> bool {
    *action == SpecFlagAction::Set
}

impl SpecFlag {
    /// Create a new builder for SpecFlag
    pub fn builder() -> SpecFlagBuilder {
        SpecFlagBuilder::new()
    }

    /// Environment sources in precedence order: canonical, fallbacks, deprecated aliases.
    pub fn env_names(&self) -> impl Iterator<Item = &str> {
        self.env
            .iter()
            .map(String::as_str)
            .chain(self.env_fallback.iter().map(String::as_str))
            .chain(self.deprecated_env.iter().map(String::as_str))
    }

    pub(crate) fn parse(ctx: &ParsingContext, node: &NodeHelper) -> Result<Self> {
        let mut flag: Self = node.arg(0)?.ensure_string()?.parse()?;
        let mut allow_hyphen_values = false;
        let mut allow_negative_numbers = false;
        let mut value_terminator: Option<String> = None;
        let mut delimiter: Option<String> = None;
        for (k, v) in node.props() {
            match k {
                "help" => flag.help = Some(v.ensure_string()?),
                "long_help" => flag.help_long = Some(v.ensure_string()?),
                "help_long" => flag.help_long = Some(v.ensure_string()?),
                "help_md" => flag.help_md = Some(v.ensure_string()?),
                "required" => flag.required = v.ensure_bool()?,
                "required_if" => flag.required_if = vec![v.ensure_string()?],
                "required_unless" => flag.required_unless = vec![v.ensure_string()?],
                "required_unless_all" => flag.required_unless_all = vec![v.ensure_string()?],
                "var" => flag.var = v.ensure_bool()?,
                "var_min" => flag.var_min = v.ensure_usize().map(Some)?,
                "var_max" => flag.var_max = v.ensure_usize().map(Some)?,
                "hide" => flag.hide = v.ensure_bool()?,
                "hide_default_value" => flag.hide_default_value = v.ensure_bool()?,
                "hide_env" => flag.hide_env = v.ensure_bool()?,
                "hide_env_values" => flag.hide_env_values = v.ensure_bool()?,
                "hide_possible_values" => flag.hide_possible_values = v.ensure_bool()?,
                "hide_short_help" => flag.hide_short_help = v.ensure_bool()?,
                "hide_long_help" => flag.hide_long_help = v.ensure_bool()?,
                "deprecated" => {
                    flag.deprecated = match v.value.as_bool() {
                        Some(true) => Some("deprecated".into()),
                        Some(false) => None,
                        None => Some(v.ensure_string()?),
                    }
                }
                "deprecated_warn_at" => flag.deprecated_warn_at = Some(v.ensure_string()?),
                "deprecated_remove_at" => flag.deprecated_remove_at = Some(v.ensure_string()?),
                "global" => flag.global = v.ensure_bool()?,
                "count" => flag.count = v.ensure_bool()?,
                "action" => {
                    let raw = v.ensure_string()?;
                    let Some(action) = SpecFlagAction::parse(&raw) else {
                        bail_parse!(ctx, v.entry.span(), "unsupported flag action {raw}");
                    };
                    flag.action = action;
                }
                "allow_hyphen_values" => allow_hyphen_values = v.ensure_bool()?,
                "allow_negative_numbers" => allow_negative_numbers = v.ensure_bool()?,
                "value_terminator" => value_terminator = Some(v.ensure_string()?),
                "default" => {
                    // Support both string and boolean defaults
                    let default_value = match v.value.as_bool() {
                        Some(b) => b.to_string(),
                        None => v.ensure_string()?,
                    };
                    flag.default = vec![default_value];
                }
                "negate" => flag.negate = v.ensure_string().map(Some)?,
                "overrides" => flag.overrides = vec![v.ensure_string()?],
                "conflicts" => flag.conflicts = vec![v.ensure_string()?],
                "requires" => flag.requires = vec![v.ensure_string()?],
                "exclusive" => flag.exclusive = v.ensure_bool()?,
                "require_equals" => flag.require_equals = v.ensure_bool()?,
                "value_optional" => flag.value_optional = v.ensure_bool()?,
                "bool_value" => flag.bool_value = v.ensure_bool()?,
                "default_missing" => flag.default_missing = Some(v.ensure_string()?),
                // Written on the flag and kept on its argument, as `allow_hyphen_values`
                // is: the value is what gets split, and `flag "--tags <tag>"` is where a
                // reader writes something about that value.
                "delimiter" => delimiter = Some(v.ensure_string()?),
                "effect" => {
                    let raw = v.ensure_string()?;
                    match raw.parse() {
                        Ok(effect) => flag.effect = Some(effect),
                        Err(_) => bail_parse!(
                            ctx,
                            v.entry.span(),
                            "unsupported effect {raw}, expected one of: {EFFECT_VALUES}"
                        ),
                    }
                }
                "env" => flag.env = v.ensure_string().map(Some)?,
                "env_fallback" => flag.env_fallback = vec![v.ensure_string()?],
                "deprecated_env" => flag.deprecated_env = vec![v.ensure_string()?],
                "help_heading" => flag.help_heading = v.ensure_string().map(Some)?,
                "display_order" => flag.display_order = v.ensure_usize().map(Some)?,
                k => bail_parse!(ctx, v.entry.span(), "unsupported flag key {k}"),
            }
        }
        if !flag.default.is_empty() {
            flag.required = false;
        }
        for child in node.children() {
            match child.name() {
                "arg" => flag.arg = Some(SpecArg::parse(ctx, &child)?),
                "help" => flag.help = Some(child.arg(0)?.ensure_string()?),
                "long_help" => flag.help_long = Some(child.arg(0)?.ensure_string()?),
                "help_long" => flag.help_long = Some(child.arg(0)?.ensure_string()?),
                "help_md" => flag.help_md = Some(child.arg(0)?.ensure_string()?),
                "required" => flag.required = child.arg(0)?.ensure_bool()?,
                "required_if" => {
                    flag.required_if = child
                        .ensure_arg_len(1..)?
                        .args()
                        .map(|arg| arg.ensure_string())
                        .collect::<Result<Vec<_>>>()?;
                }
                "required_if_eq" => {
                    child.ensure_arg_len(2..=2)?;
                    flag.required_if_eq.push(SpecRequiredIfEq {
                        selector: child.arg(0)?.ensure_string()?,
                        value: child.arg(1)?.ensure_string()?,
                    });
                }
                "required_if_eq_all" => {
                    let entries = child.args().collect::<Vec<_>>();
                    if entries.len() < 2 || entries.len() % 2 != 0 {
                        bail_parse!(
                            ctx,
                            child.node.name().span(),
                            "required_if_eq_all needs selector/value pairs"
                        );
                    }
                    flag.required_if_eq_all = entries
                        .chunks_exact(2)
                        .map(|pair| {
                            Ok(SpecRequiredIfEq {
                                selector: pair[0].ensure_string()?,
                                value: pair[1].ensure_string()?,
                            })
                        })
                        .collect::<Result<Vec<_>>>()?;
                }
                "required_unless" => {
                    flag.required_unless = child
                        .ensure_arg_len(1..)?
                        .args()
                        .map(|arg| arg.ensure_string())
                        .collect::<Result<Vec<_>>>()?;
                }
                "required_unless_all" => {
                    flag.required_unless_all = child
                        .ensure_arg_len(1..)?
                        .args()
                        .map(|arg| arg.ensure_string())
                        .collect::<Result<Vec<_>>>()?;
                }
                "var" => flag.var = child.arg(0)?.ensure_bool()?,
                "var_min" => flag.var_min = child.arg(0)?.ensure_usize().map(Some)?,
                "var_max" => flag.var_max = child.arg(0)?.ensure_usize().map(Some)?,
                "hide" => flag.hide = child.arg(0)?.ensure_bool()?,
                "hide_default_value" => flag.hide_default_value = child.arg(0)?.ensure_bool()?,
                "hide_env" => flag.hide_env = child.arg(0)?.ensure_bool()?,
                "hide_env_values" => flag.hide_env_values = child.arg(0)?.ensure_bool()?,
                "hide_possible_values" => {
                    flag.hide_possible_values = child.arg(0)?.ensure_bool()?
                }
                "hide_short_help" => flag.hide_short_help = child.arg(0)?.ensure_bool()?,
                "hide_long_help" => flag.hide_long_help = child.arg(0)?.ensure_bool()?,
                "deprecated" => {
                    flag.deprecated = match child.arg(0)?.ensure_bool() {
                        Ok(true) => Some("deprecated".into()),
                        Ok(false) => None,
                        _ => Some(child.arg(0)?.ensure_string()?),
                    }
                }
                "deprecated_warn_at" => {
                    flag.deprecated_warn_at = Some(child.arg(0)?.ensure_string()?)
                }
                "deprecated_remove_at" => {
                    flag.deprecated_remove_at = Some(child.arg(0)?.ensure_string()?)
                }
                "global" => flag.global = child.arg(0)?.ensure_bool()?,
                "count" => flag.count = child.arg(0)?.ensure_bool()?,
                "action" => {
                    let arg = child.arg(0)?;
                    let raw = arg.ensure_string()?;
                    let Some(action) = SpecFlagAction::parse(&raw) else {
                        bail_parse!(ctx, arg.entry.span(), "unsupported flag action {raw}");
                    };
                    flag.action = action;
                }
                "allow_hyphen_values" => {
                    allow_hyphen_values = child.arg(0)?.ensure_bool()?;
                }
                "allow_negative_numbers" => {
                    allow_negative_numbers = child.arg(0)?.ensure_bool()?;
                }
                "value_terminator" => {
                    value_terminator = Some(child.arg(0)?.ensure_string()?);
                }
                "default" => {
                    // Support both single value and multiple values
                    // default "bar"            -> vec!["bar"]
                    // default #true            -> vec!["true"]
                    // default { "xyz"; "bar" } -> vec!["xyz", "bar"]
                    let children = child.children();
                    if children.is_empty() {
                        // Single value: default "bar" or default #true
                        let arg = child.arg(0)?;
                        let default_value = match arg.value.as_bool() {
                            Some(b) => b.to_string(),
                            None => arg.ensure_string()?,
                        };
                        flag.default = vec![default_value];
                    } else {
                        // Multiple values from children: default { "xyz"; "bar" }
                        // In KDL, these are child nodes where the string is the node name
                        flag.default = children.iter().map(|c| c.name().to_string()).collect();
                    }
                }
                "effect" => {
                    let arg = child.arg(0)?;
                    let raw = arg.ensure_string()?;
                    match raw.parse() {
                        Ok(effect) => flag.effect = Some(effect),
                        Err(_) => bail_parse!(
                            ctx,
                            arg.entry.span(),
                            "unsupported effect {raw}, expected one of: {EFFECT_VALUES}"
                        ),
                    }
                }
                "env" => flag.env = child.arg(0)?.ensure_string().map(Some)?,
                "env_fallback" => {
                    flag.env_fallback = child
                        .ensure_arg_len(1..)?
                        .args()
                        .map(|entry| entry.ensure_string())
                        .collect::<Result<_>>()?;
                }
                "deprecated_env" => {
                    flag.deprecated_env = child
                        .ensure_arg_len(1..)?
                        .args()
                        .map(|entry| entry.ensure_string())
                        .collect::<Result<_>>()?;
                }
                "help_heading" => {
                    flag.help_heading = child.arg(0)?.ensure_string().map(Some)?;
                }
                "display_order" => {
                    flag.display_order = child.arg(0)?.ensure_usize().map(Some)?;
                }
                "alias" => {
                    let hide = child
                        .get("hide")
                        .map(|entry| entry.ensure_bool())
                        .unwrap_or(Ok(false))?;
                    for entry in child.ensure_arg_len(1..)?.args() {
                        let spelling = entry.ensure_string()?;
                        if let Some(long) = spelling.strip_prefix("--") {
                            if !flag.long.iter().any(|existing| existing == long) {
                                flag.long.push(long.to_string());
                            }
                            if hide && !flag.hidden_aliases.iter().any(|existing| existing == long)
                            {
                                flag.hidden_aliases.push(long.to_string());
                            }
                        } else if let Some(short) = spelling.strip_prefix('-') {
                            let mut chars = short.chars();
                            let Some(short) = chars.next().filter(|_| chars.next().is_none())
                            else {
                                bail_parse!(
                                    ctx,
                                    entry.entry.span(),
                                    "a short flag alias must be exactly one character"
                                );
                            };
                            if !flag.short.contains(&short) {
                                flag.short.push(short);
                            }
                            if hide && !flag.hidden_short_aliases.contains(&short) {
                                flag.hidden_short_aliases.push(short);
                            }
                        } else {
                            bail_parse!(
                                ctx,
                                entry.entry.span(),
                                "flag aliases must begin with - or --"
                            );
                        }
                    }
                }
                "conflicts" => {
                    flag.conflicts = child
                        .ensure_arg_len(1..)?
                        .args()
                        .map(|arg| arg.ensure_string())
                        .collect::<Result<Vec<_>>>()?;
                }
                "overrides" => {
                    flag.overrides = child
                        .ensure_arg_len(1..)?
                        .args()
                        .map(|arg| arg.ensure_string())
                        .collect::<Result<Vec<_>>>()?;
                }
                "exclusive" => flag.exclusive = child.arg(0)?.ensure_bool()?,
                "require_equals" => flag.require_equals = child.arg(0)?.ensure_bool()?,
                "value_optional" => flag.value_optional = child.arg(0)?.ensure_bool()?,
                "bool_value" => flag.bool_value = child.arg(0)?.ensure_bool()?,
                "default_missing" => {
                    flag.default_missing = Some(child.arg(0)?.ensure_string()?);
                }
                "requires" => {
                    flag.requires = child
                        .ensure_arg_len(1..)?
                        .args()
                        .map(|arg| arg.ensure_string())
                        .collect::<Result<Vec<_>>>()?;
                }
                "requires_if" => {
                    child.ensure_arg_len(2..=2)?;
                    flag.requires_if.push(SpecRequiresIf {
                        value: child.arg(0)?.ensure_string()?,
                        requires: child.arg(1)?.ensure_string()?,
                    });
                }
                "default_if" => {
                    child.ensure_arg_len(2..=3)?;
                    let count = child.args().count();
                    flag.default_if.push(if count == 2 {
                        SpecDefaultIf {
                            selector: child.arg(0)?.ensure_string()?,
                            when: None,
                            value: child.arg(1)?.ensure_string()?,
                        }
                    } else {
                        SpecDefaultIf {
                            selector: child.arg(0)?.ensure_string()?,
                            when: Some(child.arg(1)?.ensure_string()?),
                            value: child.arg(2)?.ensure_string()?,
                        }
                    });
                }
                "choices" => {
                    if let Some(arg) = &mut flag.arg {
                        arg.choices = Some(SpecChoices::parse(ctx, &child)?);
                    } else {
                        bail_parse!(
                            ctx,
                            child.node.name().span(),
                            "flag must have value to have choices"
                        )
                    }
                }
                k => bail_parse!(ctx, child.node.name().span(), "unsupported flag child {k}"),
            }
        }
        if allow_hyphen_values {
            flag.set_allow_hyphen_values(ctx, node.node.name().span(), true)?;
        }
        if allow_negative_numbers {
            let Some(arg) = flag.arg.as_mut() else {
                bail_parse!(
                    ctx,
                    node.node.name().span(),
                    "flag must have value to allow negative numbers"
                );
            };
            arg.allow_negative_numbers = true;
        }
        if let Some(terminator) = value_terminator {
            let Some(arg) = flag.arg.as_mut() else {
                bail_parse!(
                    ctx,
                    node.node.name().span(),
                    "flag must have a variadic value to have a value terminator"
                );
            };
            if !arg.var {
                bail_parse!(
                    ctx,
                    node.node.name().span(),
                    "value_terminator requires a variadic flag value"
                );
            }
            if terminator.is_empty() {
                bail_parse!(
                    ctx,
                    node.node.name().span(),
                    "value_terminator cannot be empty"
                );
            }
            arg.value_terminator = Some(terminator);
        }
        if flag.require_equals && flag.arg.is_none() {
            bail_parse!(
                ctx,
                node.node.name().span(),
                "flag must have value to require equals"
            );
        }
        if flag.value_optional && flag.arg.is_none() {
            bail_parse!(
                ctx,
                node.node.name().span(),
                "flag must have a value to make that value optional"
            );
        }
        if flag.bool_value
            && (flag.arg.is_some() || flag.count || flag.action != SpecFlagAction::Set)
        {
            bail_parse!(
                ctx,
                node.node.name().span(),
                "bool_value is only valid on a boolean switch"
            );
        }
        if flag.default_missing.is_some() && flag.arg.is_none() {
            bail_parse!(
                ctx,
                node.node.name().span(),
                "flag must have value to have a default when missing"
            );
        }
        // `--color` is a complete invocation, so help shows the value as optional.
        // The same folding a nested `default` already does for `required`.
        if flag.default_missing.is_some() {
            if let Some(arg) = flag.arg.as_mut() {
                arg.required = false;
            }
        }
        if let Some(raw) = delimiter {
            let mut chars = raw.chars();
            let Some(delimiter) = chars.next().filter(|_| chars.next().is_none()) else {
                bail_parse!(
                    ctx,
                    node.node.name().span(),
                    "a delimiter is one character, and {raw:?} is not"
                );
            };
            // And one *byte*, for the reason given where an argument reads the same
            // property: splitting is by byte below this, and a non-ASCII separator would
            // match the continuation bytes inside unrelated characters.
            if !delimiter.is_ascii() {
                bail_parse!(
                    ctx,
                    node.node.name().span(),
                    "a delimiter is one byte, and {delimiter:?} is more than one; use an \
                     ASCII separator"
                );
            }
            let Some(arg) = flag.arg.as_mut() else {
                bail_parse!(
                    ctx,
                    node.node.name().span(),
                    "`delimiter` splits a value, and flag --{} takes none",
                    flag.name
                );
            };
            arg.delimiter = Some(delimiter);
        }
        // A delimiter with nowhere to put the extra values would drop everything after
        // the first separator, silently. Refused where it is written instead — and `var`
        // on either the flag or its argument is somewhere for them to go, since both are
        // ways of saying the flag holds a list.
        if flag.arg.as_ref().is_some_and(|a| a.delimiter.is_some()) && !flag.var {
            let takes_several = flag.arg.as_ref().is_some_and(|a| a.var);
            if !takes_several {
                bail_parse!(
                    ctx,
                    node.node.name().span(),
                    "flag --{} has a delimiter and holds one value; add `var=#true` for \
                     the values it splits into",
                    flag.name
                );
            }
        }
        if flag.action != SpecFlagAction::Set && flag.arg.is_some() {
            bail_parse!(
                ctx,
                node.node.name().span(),
                "a help or version action does not take a value"
            );
        }
        flag.usage = flag.usage();
        flag.help_first_line = flag.help.as_ref().map(|s| string::first_line(s));
        Ok(flag)
    }
    pub fn allow_hyphen_values(&self) -> bool {
        self.arg
            .as_ref()
            .is_some_and(|arg| arg.double_dash == SpecDoubleDashChoices::Automatic)
    }

    pub(crate) fn set_allow_hyphen_values(
        &mut self,
        ctx: &ParsingContext,
        span: miette::SourceSpan,
        allow: bool,
    ) -> Result<()> {
        if let Some(arg) = &mut self.arg {
            arg.double_dash = if allow {
                SpecDoubleDashChoices::Automatic
            } else if arg.double_dash == SpecDoubleDashChoices::Automatic {
                SpecDoubleDashChoices::Optional
            } else {
                arg.double_dash.clone()
            };
            Ok(())
        } else if allow {
            bail_parse!(ctx, span, "flag must have value to allow hyphen values")
        } else {
            Ok(())
        }
    }

    pub fn usage(&self) -> String {
        let mut parts = vec![];
        let name = get_name_from_short_and_long(&self.short, &self.long).unwrap_or_default();
        if name != self.name {
            parts.push(format!("{}:", self.name));
        }
        if let Some(short) = self.short.first() {
            parts.push(format!("-{short}"));
        }
        if let Some(long) = self.long.first() {
            parts.push(format!("--{long}"));
        }
        let mut out = parts.join(" ");
        if self.var {
            out = format!("{out}…");
        }
        if let Some(arg) = &self.arg {
            let usage = arg.usage();
            if self.require_equals && (self.value_optional || !arg.required) {
                out = format!("{out}{}", optional_equals_usage(&usage));
            } else {
                let separator = if self.require_equals { "=" } else { " " };
                out = format!("{out}{separator}{usage}");
            }
        }
        out
    }
}

pub(crate) fn optional_equals_usage(usage: &str) -> String {
    let (value, closing) = if let Some(value) = usage.strip_prefix('[') {
        (value, ']')
    } else if let Some(value) = usage.strip_prefix('<') {
        (value, '>')
    } else {
        return format!("={usage}");
    };
    let Some(end) = value.find(closing) else {
        return format!("={usage}");
    };
    format!("[={}]{}", &value[..end], &value[end + 1..])
}

impl From<&SpecFlag> for KdlNode {
    fn from(flag: &SpecFlag) -> KdlNode {
        let mut node = KdlNode::new("flag");
        let name = flag
            .short
            .iter()
            .filter(|short| !flag.hidden_short_aliases.contains(short))
            .map(|c| format!("-{c}"))
            .chain(
                flag.long
                    .iter()
                    .filter(|long| !flag.hidden_aliases.contains(long))
                    .map(|s| format!("--{s}")),
            )
            .collect_vec()
            .join(" ");
        node.push(KdlEntry::new(name));
        if let Some(desc) = &flag.help {
            node.push(string_entry(Some("help"), desc));
        }
        if !flag.hidden_aliases.is_empty() || !flag.hidden_short_aliases.is_empty() {
            let children = node.children_mut().get_or_insert_with(KdlDocument::new);
            let mut aliases = KdlNode::new("alias");
            for alias in &flag.hidden_short_aliases {
                aliases.push(string_entry(None, &format!("-{alias}")));
            }
            for alias in &flag.hidden_aliases {
                aliases.push(string_entry(None, &format!("--{alias}")));
            }
            aliases.push(KdlEntry::new_prop("hide", true));
            children.nodes_mut().push(aliases);
        }
        if let Some(desc) = &flag.help_long {
            let children = node.children_mut().get_or_insert_with(KdlDocument::new);
            let mut node = KdlNode::new("long_help");
            node.push(string_entry(None, desc));
            children.nodes_mut().push(node);
        }
        if let Some(desc) = &flag.help_md {
            let children = node.children_mut().get_or_insert_with(KdlDocument::new);
            let mut node = KdlNode::new("help_md");
            node.push(string_entry(None, desc));
            children.nodes_mut().push(node);
        }
        if flag.required {
            node.push(KdlEntry::new_prop("required", true));
        }
        serialize_flag_list(&mut node, "required_if", &flag.required_if);
        for condition in &flag.required_if_eq {
            let children = node.children_mut().get_or_insert_with(KdlDocument::new);
            let mut relation = KdlNode::new("required_if_eq");
            relation.push(string_entry(None, &condition.selector));
            relation.push(string_entry(None, &condition.value));
            children.nodes_mut().push(relation);
        }
        if !flag.required_if_eq_all.is_empty() {
            let children = node.children_mut().get_or_insert_with(KdlDocument::new);
            let mut relation = KdlNode::new("required_if_eq_all");
            for condition in &flag.required_if_eq_all {
                relation.push(string_entry(None, &condition.selector));
                relation.push(string_entry(None, &condition.value));
            }
            children.nodes_mut().push(relation);
        }
        serialize_flag_list(&mut node, "required_unless", &flag.required_unless);
        serialize_flag_list(&mut node, "required_unless_all", &flag.required_unless_all);
        if flag.var {
            node.push(KdlEntry::new_prop("var", true));
        }
        if let Some(var_min) = flag.var_min {
            node.push(KdlEntry::new_prop("var_min", var_min as i128));
        }
        if let Some(var_max) = flag.var_max {
            node.push(KdlEntry::new_prop("var_max", var_max as i128));
        }
        if flag.hide {
            node.push(KdlEntry::new_prop("hide", true));
        }
        for (name, hidden) in [
            ("hide_default_value", flag.hide_default_value),
            ("hide_env", flag.hide_env),
            ("hide_env_values", flag.hide_env_values),
            ("hide_possible_values", flag.hide_possible_values),
            ("hide_short_help", flag.hide_short_help),
            ("hide_long_help", flag.hide_long_help),
        ] {
            if hidden {
                node.push(KdlEntry::new_prop(name, true));
            }
        }
        if flag.global {
            node.push(KdlEntry::new_prop("global", true));
        }
        if flag.count {
            node.push(KdlEntry::new_prop("count", true));
        }
        if flag.action != SpecFlagAction::Set {
            node.push(string_entry(Some("action"), flag.action.as_str()));
        }
        if flag.allow_hyphen_values() {
            node.push(KdlEntry::new_prop("allow_hyphen_values", true));
        }
        if flag
            .arg
            .as_ref()
            .is_some_and(|arg| arg.allow_negative_numbers)
        {
            node.push(KdlEntry::new_prop("allow_negative_numbers", true));
        }
        if let Some(terminator) = flag
            .arg
            .as_ref()
            .and_then(|arg| arg.value_terminator.as_deref())
        {
            node.push(string_entry(Some("value_terminator"), terminator));
        }
        if let Some(negate) = &flag.negate {
            node.push(string_entry(Some("negate"), negate));
        }
        if flag.overrides.len() == 1 {
            node.push(string_entry(Some("overrides"), &flag.overrides[0]));
        } else if !flag.overrides.is_empty() {
            let children = node.children_mut().get_or_insert_with(KdlDocument::new);
            let mut overrides = KdlNode::new("overrides");
            for target in &flag.overrides {
                overrides.push(string_entry(None, target));
            }
            children.nodes_mut().push(overrides);
        }
        if flag.conflicts.len() == 1 {
            node.push(string_entry(Some("conflicts"), &flag.conflicts[0]));
        } else if !flag.conflicts.is_empty() {
            let children = node.children_mut().get_or_insert_with(KdlDocument::new);
            let mut conflicts = KdlNode::new("conflicts");
            for target in &flag.conflicts {
                conflicts.push(string_entry(None, target));
            }
            children.nodes_mut().push(conflicts);
        }
        if flag.requires.len() == 1 {
            node.push(string_entry(Some("requires"), &flag.requires[0]));
        } else if !flag.requires.is_empty() {
            let children = node.children_mut().get_or_insert_with(KdlDocument::new);
            let mut requires = KdlNode::new("requires");
            for target in &flag.requires {
                requires.push(string_entry(None, target));
            }
            children.nodes_mut().push(requires);
        }
        for condition in &flag.requires_if {
            let children = node.children_mut().get_or_insert_with(KdlDocument::new);
            let mut requires_if = KdlNode::new("requires_if");
            requires_if.push(string_entry(None, &condition.value));
            requires_if.push(string_entry(None, &condition.requires));
            children.nodes_mut().push(requires_if);
        }
        for condition in &flag.default_if {
            let children = node.children_mut().get_or_insert_with(KdlDocument::new);
            let mut default_if = KdlNode::new("default_if");
            default_if.push(string_entry(None, &condition.selector));
            if let Some(when) = &condition.when {
                default_if.push(string_entry(None, when));
            }
            default_if.push(string_entry(None, &condition.value));
            children.nodes_mut().push(default_if);
        }
        if flag.exclusive {
            node.push(KdlEntry::new_prop("exclusive", true));
        }
        if flag.require_equals {
            node.push(KdlEntry::new_prop("require_equals", true));
        }
        if flag.value_optional {
            node.push(KdlEntry::new_prop("value_optional", true));
        }
        if flag.bool_value {
            node.push(KdlEntry::new_prop("bool_value", true));
        }
        if let Some(missing) = &flag.default_missing {
            node.push(string_entry(Some("default_missing"), missing));
        }
        if let Some(env) = &flag.env {
            node.push(string_entry(Some("env"), env));
        }
        serialize_flag_list(&mut node, "env_fallback", &flag.env_fallback);
        serialize_flag_list(&mut node, "deprecated_env", &flag.deprecated_env);
        if let Some(help_heading) = &flag.help_heading {
            node.push(string_entry(Some("help_heading"), help_heading));
        }
        if let Some(order) = flag.display_order {
            node.push(KdlEntry::new_prop("display_order", order as i128));
        }
        if let Some(effect) = &flag.effect {
            node.push(string_entry(Some("effect"), effect.as_str()));
        }
        if let Some(deprecated) = &flag.deprecated {
            node.push(string_entry(Some("deprecated"), deprecated));
        }
        if let Some(at) = &flag.deprecated_warn_at {
            node.push(string_entry(Some("deprecated_warn_at"), at));
        }
        if let Some(at) = &flag.deprecated_remove_at {
            node.push(string_entry(Some("deprecated_remove_at"), at));
        }
        // Serialize default values
        if !flag.default.is_empty() {
            if flag.default.len() == 1 {
                // Single value: use property default="bar"
                node.push(KdlEntry::new_prop("default", flag.default[0].clone()));
            } else {
                // Multiple values: use child node default { "xyz"; "bar" }
                let children = node.children_mut().get_or_insert_with(KdlDocument::new);
                let mut default_node = KdlNode::new("default");
                let default_children = default_node
                    .children_mut()
                    .get_or_insert_with(KdlDocument::new);
                for val in &flag.default {
                    default_children
                        .nodes_mut()
                        .push(KdlNode::new(val.as_str()));
                }
                children.nodes_mut().push(default_node);
            }
        }
        if let Some(arg) = &flag.arg {
            let children = node.children_mut().get_or_insert_with(KdlDocument::new);
            if flag.allow_hyphen_values() {
                let mut arg = arg.clone();
                arg.double_dash = SpecDoubleDashChoices::Optional;
                children.nodes_mut().push((&arg).into());
            } else {
                children.nodes_mut().push(arg.into());
            }
        }
        node
    }
}

fn serialize_flag_list(node: &mut KdlNode, name: &str, flags: &[String]) {
    if flags.len() == 1 {
        node.push(string_entry(Some(name), &flags[0]));
    } else if !flags.is_empty() {
        let children = node.children_mut().get_or_insert_with(KdlDocument::new);
        let mut list = KdlNode::new(name);
        for flag in flags {
            list.push(string_entry(None, flag));
        }
        children.nodes_mut().push(list);
    }
}

impl FromStr for SpecFlag {
    type Err = UsageErr;
    fn from_str(input: &str) -> Result<Self> {
        let mut flag = Self::default();
        // Keep a flag-level repetition marker attached when an equals value follows it.
        // Every other ellipsis becomes its own token so its position still distinguishes
        // a repeatable flag (`--flag… <ARG>`) from a variadic value (`--flag <ARG>…`).
        let input = input
            .replace("...", "…")
            .replace("…[=", "\u{e000}[=")
            .replace("…=", "\u{e000}=")
            .replace("…", " … ")
            .replace('\u{e000}', "…");
        for part in input.split_whitespace() {
            if let Some((form, value)) = part
                .strip_suffix(']')
                .and_then(|part| part.split_once("[="))
            {
                let (form, repeatable) = form
                    .strip_suffix('…')
                    .map_or((form, false), |form| (form, true));
                let recognized = if let Some(long) = form.strip_prefix("--") {
                    if long.is_empty() {
                        false
                    } else {
                        flag.long.push(long.to_string());
                        true
                    }
                } else if let Some(short) = form.strip_prefix('-') {
                    if short.chars().count() != 1 {
                        return Err(InvalidFlag {
                            token: form.to_string(),
                            reason:
                                "short flags must be a single character (use -- for long flags)"
                                    .to_string(),
                            span: (0, input.len()).into(),
                            input: input.to_string(),
                        });
                    }
                    flag.short.push(short.chars().next().unwrap());
                    true
                } else {
                    false
                };
                if recognized && !value.is_empty() {
                    flag.var |= repeatable;
                    flag.require_equals = true;
                    flag.arg = Some(match flag.arg.take() {
                        Some(existing) => format!("{} [{value}]", existing.usage()).parse()?,
                        None => format!("[{value}]").parse()?,
                    });
                    continue;
                }
            }
            if let Some((form, value)) = part.split_once('=') {
                let (form, repeatable) = form
                    .strip_suffix('…')
                    .map_or((form, false), |form| (form, true));
                let recognized = if let Some(long) = form.strip_prefix("--") {
                    if long.is_empty() {
                        false
                    } else {
                        flag.long.push(long.to_string());
                        true
                    }
                } else if let Some(short) = form.strip_prefix('-') {
                    if short.chars().count() != 1 {
                        return Err(InvalidFlag {
                            token: form.to_string(),
                            reason:
                                "short flags must be a single character (use -- for long flags)"
                                    .to_string(),
                            span: (0, input.len()).into(),
                            input: input.to_string(),
                        });
                    }
                    flag.short.push(short.chars().next().unwrap());
                    true
                } else {
                    false
                };
                if recognized {
                    flag.var |= repeatable;
                    if !(value.starts_with('<') && value.ends_with('>')
                        || value.starts_with('[') && value.ends_with(']'))
                    {
                        return Err(InvalidFlag {
                            token: part.to_string(),
                            reason: "an equals sign must attach <arg> or [arg]".to_string(),
                            span: (0, input.len()).into(),
                            input: input.to_string(),
                        });
                    }
                    flag.require_equals = true;
                    flag.arg = Some(match flag.arg.take() {
                        Some(existing) => format!("{} {value}", existing.usage()).parse()?,
                        None => value.to_string().parse()?,
                    });
                    continue;
                }
            }
            if let Some(name) = part.strip_suffix(':') {
                flag.name = name.to_string();
            } else if let Some(long) = part.strip_prefix("--") {
                flag.long.push(long.to_string());
            } else if let Some(short) = part.strip_prefix('-') {
                if short.chars().count() != 1 {
                    return Err(InvalidFlag {
                        token: format!("-{short}"),
                        reason: "short flags must be a single character (use -- for long flags)"
                            .to_string(),
                        span: (0, input.len()).into(),
                        input: input.to_string(),
                    });
                }
                flag.short.push(short.chars().next().unwrap());
            } else if part == "…" {
                if let Some(arg) = &mut flag.arg {
                    arg.var = true;
                } else {
                    flag.var = true;
                }
            } else if part.starts_with('<') && part.ends_with('>')
                || part.starts_with('[') && part.ends_with(']')
            {
                flag.arg = Some(match flag.arg.take() {
                    Some(existing) => format!("{} {part}", existing.usage()).parse()?,
                    None => part.to_string().parse()?,
                });
            } else {
                return Err(InvalidFlag {
                    token: part.to_string(),
                    reason: "unexpected token (expected -x, --long, <arg>, or [arg])".to_string(),
                    span: (0, input.len()).into(),
                    input: input.to_string(),
                });
            }
        }
        if flag.name.is_empty() {
            flag.name = get_name_from_short_and_long(&flag.short, &flag.long).unwrap_or_default();
        }
        flag.usage = flag.usage();
        Ok(flag)
    }
}

#[cfg(feature = "clap")]
impl From<&clap::Arg> for SpecFlag {
    fn from(c: &clap::Arg) -> Self {
        let required = c.is_required_set();
        let help = c.get_help().map(|s| s.to_string());
        let help_long = c.get_long_help().map(|s| s.to_string());
        let help_first_line = help.as_ref().map(|s| string::first_line(s));
        let hide = c.is_hide_set();
        let var = matches!(
            c.get_action(),
            clap::ArgAction::Count | clap::ArgAction::Append
        );
        let default: Vec<String> = crate::spec::arg::default_values(c);
        let mut short = c.get_short_and_visible_aliases().unwrap_or_default();
        let visible_short = short.clone();
        let hidden_short_aliases = c
            .get_all_short_aliases()
            .unwrap_or_default()
            .into_iter()
            .filter(|alias| !visible_short.contains(alias))
            .collect::<Vec<_>>();
        short.extend(hidden_short_aliases.iter().copied());
        let mut long = c
            .get_long_and_visible_aliases()
            .unwrap_or_default()
            .into_iter()
            .map(|s| s.to_string())
            .collect::<Vec<_>>();
        let visible_long = long.clone();
        let hidden_aliases = c
            .get_all_aliases()
            .unwrap_or_default()
            .into_iter()
            .filter(|alias| !visible_long.iter().any(|visible| visible == alias))
            .map(str::to_string)
            .collect::<Vec<_>>();
        long.extend(hidden_aliases.iter().cloned());
        let name = get_name_from_short_and_long(&short, &long).unwrap_or_default();
        let arg = if let clap::ArgAction::Set | clap::ArgAction::Append = c.get_action() {
            let value_names = crate::spec::arg::value_names_from_clap(c);
            let mut arg = SpecArg::from(
                value_names
                    .first()
                    .cloned()
                    .unwrap_or_else(|| name.clone())
                    .as_str(),
            );
            arg.value_names = value_names;

            arg.choices = crate::spec::arg::choices_from_clap(c);

            // The flag's argument is built from its value name rather than from the
            // clap `Arg`, so what the `Arg` says about the *value* has to be carried
            // here — the `From<&clap::Arg> for SpecArg` impl never sees this one.
            //
            // A delimiter *is* the statement that several values can land, so it brings
            // `var` with it rather than waiting for one.
            //
            // Gating this on the action or on `num_args` was wrong: clap's parser splits
            // whenever a delimiter is set — `parser.rs` reaches for
            // `arg.get_value_delimiter()` before it looks at anything else — so
            // `ArgAction::Set` with `value_delimiter(',')` is one word becoming several,
            // and that is the common spelling. Reading it as single-valued dropped the
            // delimiter and left a CLI whose defaults split and whose typed values did
            // not.
            if let Some(delimiter) = c.get_value_delimiter() {
                arg.var = true;
                // Only if it is one byte. Splitting is by byte everywhere below the spec,
                // and a spec carrying a wider separator could not be written back out —
                // `to_kdl` would emit what parsing then refuses. clap still splits on it,
                // so `var` stays: the values arrive, and only the spec's account of how
                // they were separated is lost.
                if delimiter.is_ascii() {
                    arg.delimiter = Some(delimiter);
                }
            } else if var || c.get_num_args().is_some_and(|n| n.max_values() > 1) {
                arg.var = true;
            }
            arg.allow_negative_numbers = c.is_allow_negative_numbers_set();
            if arg.var {
                if let Some(terminator) = c.get_value_terminator() {
                    arg.value_terminator = Some(terminator.to_string());
                }
            }

            // These bounds live on the nested value argument and are enforced per occurrence.
            // That preserves both a single `Set` and each repetition of `Append`.
            crate::spec::arg::value_bounds(c, &mut arg, true);

            Some(arg)
        } else {
            None
        };
        let mut flag = Self {
            name,
            usage: "".into(),
            short,
            hidden_short_aliases,
            long,
            hidden_aliases,
            required,
            required_if: vec![],
            required_if_eq: vec![],
            required_if_eq_all: vec![],
            required_unless: vec![],
            required_unless_all: vec![],
            deprecated_warn_at: None,
            deprecated_remove_at: None,
            conflicts: vec![],
            // clap 4.6 has `Arg::requires` and its variants as setters with no getter, so
            // there is nothing to read here however the `Arg` was built. Left empty rather
            // than guessed at, and counted by `gen-shadow` as a thing the clap dialect
            // cannot carry.
            requires: vec![],
            // The conditional forms are hidden behind the same clap API boundary.
            requires_if: vec![],
            // clap 4 has `Arg::default_value_if` as a setter with no getter.
            default_if: vec![],
            // This one clap does expose, unlike `requires` just above.
            exclusive: c.is_exclusive_set(),
            require_equals: c.is_require_equals_set(),
            value_optional: arg.is_some()
                && c.get_num_args()
                    .is_some_and(|n| n.min_values() == 0 && n.max_values() > 0),
            // clap has no attached-value boolean-switch policy.
            bool_value: false,
            // clap 4 has `Arg::default_missing_value` as a setter with no getter.
            default_missing: None,
            help,
            help_long,
            help_md: None,
            help_first_line,
            var,
            var_min: None,
            var_max: None,
            hide,
            hide_default_value: c.is_hide_default_value_set(),
            hide_env: c.is_hide_env_set(),
            hide_env_values: c.is_hide_env_values_set(),
            hide_possible_values: c.is_hide_possible_values_set(),
            hide_short_help: c.is_hide_short_help_set(),
            hide_long_help: c.is_hide_long_help_set(),
            global: c.is_global_set(),
            arg,
            count: matches!(c.get_action(), clap::ArgAction::Count),
            action: match c.get_action() {
                clap::ArgAction::Help => SpecFlagAction::Help,
                clap::ArgAction::HelpShort => SpecFlagAction::HelpShort,
                clap::ArgAction::HelpLong => SpecFlagAction::HelpLong,
                clap::ArgAction::Version => SpecFlagAction::Version,
                _ => SpecFlagAction::Set,
            },
            default,
            deprecated: None,
            negate: None,
            overrides: vec![],
            // Filled by the command conversion: clap keeps conflicts on the
            // `Command`, not the `Arg`, so an `Arg` alone cannot see them.
            // clap has no way to express this; consumers set it on the derived
            // spec (see the effect docs).
            effect: None,
            env: None,
            env_fallback: vec![],
            deprecated_env: vec![],
            help_heading: c.get_help_heading().map(|s| s.to_string()),
            display_order: Some(c.get_display_order()),
        };
        if c.is_allow_hyphen_values_set() {
            if let Some(arg) = &mut flag.arg {
                arg.double_dash = SpecDoubleDashChoices::Automatic;
            }
        }
        flag
    }
}

// #[cfg(feature = "clap")]
// impl From<&SpecFlag> for clap::Arg {
//     fn from(flag: &SpecFlag) -> Self {
//         let mut a = clap::Arg::new(&flag.name);
//         if let Some(desc) = &flag.help {
//             a = a.help(desc);
//         }
//         if flag.required {
//             a = a.required(true);
//         }
//         if let Some(arg) = &flag.arg {
//             a = a.value_name(&arg.name);
//             if arg.var {
//                 a = a.action(clap::ArgAction::Append)
//             } else {
//                 a = a.action(clap::ArgAction::Set)
//             }
//         } else {
//             a = a.action(clap::ArgAction::SetTrue)
//         }
//         // let mut a = clap::Arg::new(&flag.name)
//         //     .required(flag.required)
//         //     .action(clap::ArgAction::SetTrue);
//         if let Some(short) = flag.short.first() {
//             a = a.short(*short);
//         }
//         if let Some(long) = flag.long.first() {
//             a = a.long(long);
//         }
//         for short in flag.short.iter().skip(1) {
//             a = a.visible_short_alias(*short);
//         }
//         for long in flag.long.iter().skip(1) {
//             a = a.visible_alias(long);
//         }
//         // cmd = cmd.arg(a);
//         // if flag.multiple {
//         //     a = a.multiple(true);
//         // }
//         // if flag.hide {
//         //     a = a.hide_possible_values(true);
//         // }
//         a
//     }
// }

impl Display for SpecFlag {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}", self.usage())
    }
}
impl PartialEq for SpecFlag {
    fn eq(&self, other: &Self) -> bool {
        self.name == other.name
    }
}
impl Eq for SpecFlag {}
impl Hash for SpecFlag {
    fn hash<H: std::hash::Hasher>(&self, state: &mut H) {
        self.name.hash(state);
    }
}

fn get_name_from_short_and_long(short: &[char], long: &[String]) -> Option<String> {
    long.first()
        .map(|s| s.to_string())
        .or_else(|| short.first().map(|c| c.to_string()))
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::Spec;
    use insta::assert_snapshot;

    #[test]
    fn from_str() {
        assert_snapshot!("-f".parse::<SpecFlag>().unwrap(), @"-f");
        assert_snapshot!("--flag".parse::<SpecFlag>().unwrap(), @"--flag");
        assert_snapshot!("-f --flag".parse::<SpecFlag>().unwrap(), @"-f --flag");
        assert_snapshot!("-f --flag…".parse::<SpecFlag>().unwrap(), @"-f --flag…");
        assert_snapshot!("-f --flag …".parse::<SpecFlag>().unwrap(), @"-f --flag…");
        assert_snapshot!("--flag <arg>".parse::<SpecFlag>().unwrap(), @"--flag <arg>");
        assert_snapshot!("-f --flag <arg>".parse::<SpecFlag>().unwrap(), @"-f --flag <arg>");
        assert_snapshot!("-f --flag… <arg>".parse::<SpecFlag>().unwrap(), @"-f --flag… <arg>");
        assert_snapshot!("-f --flag <arg>…".parse::<SpecFlag>().unwrap(), @"-f --flag <arg>…");
        let range = "--range <start> <end>".parse::<SpecFlag>().unwrap();
        let arg = range.arg.as_ref().unwrap();
        assert_eq!(arg.value_names, ["start", "end"]);
        assert_eq!((arg.var_min, arg.var_max), (Some(2), Some(2)));
        assert_snapshot!(range, @"--range <start> <end>");
        assert_snapshot!("myflag: -f".parse::<SpecFlag>().unwrap(), @"myflag: -f");
        assert_snapshot!("myflag: -f --flag <arg>".parse::<SpecFlag>().unwrap(), @"myflag: -f --flag <arg>");
    }

    #[test]
    fn clap_token_boundaries_survive_the_bridge() {
        let command = clap::Command::new("ex")
            .allow_negative_numbers(true)
            .arg(
                clap::Arg::new("item")
                    .long("item")
                    .action(clap::ArgAction::Append)
                    .value_terminator(";"),
            )
            .arg(clap::Arg::new("number"));
        let spec = Spec::from(&command);
        let item = spec.cmd.flags[0].arg.as_ref().unwrap();

        assert!(item.allow_negative_numbers);
        assert_eq!(item.value_terminator.as_deref(), Some(";"));
        assert!(spec.cmd.args[0].allow_negative_numbers);
    }

    #[test]
    fn hidden_aliases_parse_bind_and_round_trip_without_becoming_visible() {
        let spec: Spec =
            "flag \"-o --output <file>\" {\n  alias \"-q\" \"--quietly\" hide=#true\n}\n"
                .parse()
                .unwrap();
        let flag = &spec.cmd.flags[0];
        assert_eq!(flag.short, ['o', 'q']);
        assert_eq!(flag.long, ["output", "quietly"]);
        assert_eq!(flag.hidden_short_aliases, ['q']);
        assert_eq!(flag.hidden_aliases, ["quietly"]);

        let emitted = spec.to_string();
        assert!(emitted.contains("flag \"-o --output\""), "{emitted}");
        assert!(!emitted.contains("flag \"-o -q"), "{emitted}");
        assert!(
            emitted.contains("alias \"-q\" \"--quietly\" hide=#true"),
            "{emitted}"
        );
        let reparsed: Spec = emitted.parse().unwrap();
        assert_eq!(reparsed.cmd.flags[0].short, flag.short);
        assert_eq!(reparsed.cmd.flags[0].long, flag.long);
        assert_eq!(
            reparsed.cmd.flags[0].hidden_short_aliases,
            flag.hidden_short_aliases
        );
        assert_eq!(reparsed.cmd.flags[0].hidden_aliases, flag.hidden_aliases);

        let cmd = clap::Command::new("ex").arg(
            clap::Arg::new("output")
                .short('o')
                .short_alias('q')
                .long("output")
                .visible_alias("out")
                .alias("quietly"),
        );
        let bridged = Spec::from(&cmd);
        let flag = &bridged.cmd.flags[0];
        assert_eq!(flag.short, ['o', 'q']);
        assert_eq!(flag.hidden_short_aliases, ['q']);
        assert_eq!(flag.long, ["output", "out", "quietly"]);
        assert_eq!(flag.hidden_aliases, ["quietly"]);
    }

    #[test]
    fn conflicts_round_trip_and_come_across_from_clap() {
        // Both spellings, as `overrides` has: a property for one, a child node for
        // several.
        let spec: Spec = "flag \"--file <f>\" conflicts=\"--stdin\"\nflag \"--stdin\" {\n  conflicts \"--file\" \"--url\"\n}\nflag \"--url <u>\"\n"
            .parse()
            .unwrap();
        assert_eq!(spec.cmd.flags[0].conflicts, vec!["--stdin".to_string()]);
        assert_eq!(
            spec.cmd.flags[1].conflicts,
            vec!["--file".to_string(), "--url".to_string()]
        );

        let reparsed: Spec = spec.to_string().parse().unwrap();
        assert_eq!(reparsed.cmd.flags[1].conflicts.len(), 2, "{spec}");
    }

    #[test]
    fn requires_round_trips_in_both_spellings() {
        // The same two spellings `conflicts` has, because it is the same shape of
        // statement: a property for one selector, a child node for several.
        let spec: Spec = "flag \"--out <p>\" requires=\"--format\"\nflag \"--sign\" {\n  requires \"--key\" \"--identity\"\n}\nflag \"--format <f>\"\nflag \"--key <k>\"\nflag \"--identity <i>\"\n"
            .parse()
            .unwrap();
        assert_eq!(spec.cmd.flags[0].requires, vec!["--format".to_string()]);
        assert_eq!(
            spec.cmd.flags[1].requires,
            vec!["--key".to_string(), "--identity".to_string()]
        );

        let reparsed: Spec = spec.to_string().parse().unwrap();
        assert_eq!(reparsed.cmd.flags[0].requires, vec!["--format".to_string()]);
        assert_eq!(reparsed.cmd.flags[1].requires.len(), 2, "{spec}");
    }

    #[test]
    fn conditional_requirements_round_trip_in_order() {
        let spec: Spec = "flag \"--config <file>\" {\n  requires_if \"special.toml\" \"--key\"\n  requires_if \"remote.toml\" \"--token\"\n}\nflag \"--key <key>\"\nflag \"--token <token>\"\n"
            .parse()
            .unwrap();
        assert_eq!(
            spec.cmd.flags[0].requires_if,
            [
                SpecRequiresIf {
                    value: "special.toml".into(),
                    requires: "--key".into(),
                },
                SpecRequiresIf {
                    value: "remote.toml".into(),
                    requires: "--token".into(),
                },
            ]
        );

        let emitted = spec.to_string();
        let reparsed: Spec = emitted.parse().unwrap();
        assert_eq!(
            reparsed.cmd.flags[0].requires_if,
            spec.cmd.flags[0].requires_if
        );
    }

    #[test]
    fn conditional_defaults_round_trip_in_order_and_cannot_come_across_from_clap() {
        let spec: Spec = "flag \"--bin-names\" {\n  default_if \"--json\" \"true\"\n  default_if \"--output\" \"json\" \"pretty\"\n}\nflag \"--json\"\nflag \"--output <fmt>\"\n"
            .parse()
            .unwrap();
        assert_eq!(
            spec.cmd.flags[0].default_if,
            [
                SpecDefaultIf {
                    selector: "--json".into(),
                    when: None,
                    value: "true".into(),
                },
                SpecDefaultIf {
                    selector: "--output".into(),
                    when: Some("json".into()),
                    value: "pretty".into(),
                },
            ]
        );

        let emitted = spec.to_string();
        let reparsed: Spec = emitted.parse().unwrap();
        assert_eq!(
            reparsed.cmd.flags[0].default_if,
            spec.cmd.flags[0].default_if
        );

        // Same hole as `requires`: clap 4 has the setter and keeps the field private.
        let cmd = clap::Command::new("ex")
            .arg(
                clap::Arg::new("bin-names")
                    .long("bin-names")
                    .action(clap::ArgAction::SetTrue)
                    .default_value_if("json", clap::builder::ArgPredicate::IsPresent, "true"),
            )
            .arg(
                clap::Arg::new("json")
                    .long("json")
                    .action(clap::ArgAction::SetTrue),
            );
        let spec = Spec::from(&cmd);
        let bin_names = spec
            .cmd
            .flags
            .iter()
            .find(|f| f.name == "bin-names")
            .unwrap();
        assert!(
            bin_names.default_if.is_empty(),
            "clap exposes no getter for `default_value_if`; if this now fails, \
             the bridge can carry it and `SpecFlag::default_if` should say so"
        );
    }

    #[test]
    fn exclusive_round_trips_and_comes_across_from_clap() {
        let spec: Spec = "flag \"--dump\" exclusive=#true\nflag \"--verbose\"\n"
            .parse()
            .unwrap();
        assert!(spec.cmd.flags[0].exclusive);
        assert!(!spec.cmd.flags[1].exclusive);

        let reparsed: Spec = spec.to_string().parse().unwrap();
        assert!(reparsed.cmd.flags[0].exclusive, "{spec}");

        // Unlike `requires`, clap answers for this one — `Arg::is_exclusive_set` — so a
        // spec generated from a clap command carries it.
        let cmd = clap::Command::new("ex")
            .arg(clap::Arg::new("dump").long("dump").exclusive(true))
            .arg(clap::Arg::new("verbose").long("verbose"));
        let spec = Spec::from(&cmd);
        let dump = spec.cmd.flags.iter().find(|f| f.name == "dump").unwrap();
        assert!(dump.exclusive);
        let verbose = spec.cmd.flags.iter().find(|f| f.name == "verbose").unwrap();
        assert!(!verbose.exclusive);
    }

    #[test]
    fn require_equals_round_trips_and_comes_across_from_clap() {
        let spec: Spec = "flag \"--inspect <PORT>\" require_equals=#true\n"
            .parse()
            .unwrap();
        assert!(spec.cmd.flags[0].require_equals);

        let reparsed: Spec = spec.to_string().parse().unwrap();
        assert!(reparsed.cmd.flags[0].require_equals, "{spec}");

        let cmd = clap::Command::new("ex").arg(
            clap::Arg::new("inspect")
                .long("inspect")
                .action(clap::ArgAction::Set)
                .require_equals(true),
        );
        let spec = Spec::from(&cmd);
        let inspect = spec.cmd.flags.iter().find(|f| f.name == "inspect").unwrap();
        assert!(inspect.require_equals);
        assert_eq!(inspect.usage(), "--inspect=<inspect>");
        let usage_reparsed: SpecFlag = inspect.usage().parse().unwrap();
        assert!(usage_reparsed.require_equals);
        assert_eq!(usage_reparsed.long, ["inspect"]);
        assert_eq!(usage_reparsed.arg.unwrap().name, "inspect");
        assert_eq!(
            crate::docs::models::SpecFlag::from(inspect).usage,
            "--inspect=<inspect>"
        );

        let cmd = clap::Command::new("ex").arg(
            clap::Arg::new("color")
                .long("color")
                .action(clap::ArgAction::Set)
                .num_args(0..=1)
                .require_equals(true),
        );
        let spec = Spec::from(&cmd);
        let color = spec.cmd.flags.iter().find(|f| f.name == "color").unwrap();
        assert!(color.require_equals);
        assert!(color.value_optional);
        assert!(
            color.arg.as_ref().unwrap().required,
            "clap's optional arity is flag metadata, not positional presentation"
        );
        assert_eq!(color.usage(), "--color[=color]");
        assert_eq!(
            crate::docs::models::SpecFlag::from(color).usage,
            "--color[=color]"
        );

        let optional: SpecFlag = "--color [WHEN]".parse().unwrap();
        let optional = SpecFlag {
            require_equals: true,
            ..optional
        };
        assert_eq!(optional.usage(), "--color[=WHEN]");
        let reparsed: SpecFlag = optional.usage().parse().unwrap();
        assert!(reparsed.require_equals);
        assert!(!reparsed.arg.as_ref().unwrap().required);
        assert_eq!(reparsed.arg.as_ref().unwrap().name, "WHEN");
        assert_eq!(
            crate::docs::models::SpecFlag::from(&optional).usage,
            "--color[=WHEN]"
        );

        let variadic: SpecFlag = "--color [WHEN]…".parse().unwrap();
        let variadic = SpecFlag {
            require_equals: true,
            ..variadic
        };
        assert_eq!(variadic.usage(), "--color[=WHEN]…");
        assert_eq!(
            crate::docs::models::SpecFlag::from(&variadic).usage,
            "--color[=WHEN]…"
        );
        assert_eq!(
            variadic.usage().parse::<SpecFlag>().unwrap().usage(),
            "--color[=WHEN]…"
        );

        let pair: SpecFlag = "--range [START] [END]".parse().unwrap();
        let pair = SpecFlag {
            require_equals: true,
            ..pair
        };
        assert_eq!(pair.usage(), "--range[=START] [END]");
        assert_eq!(
            crate::docs::models::SpecFlag::from(&pair).usage,
            "--range[=START] [END]"
        );
        assert_eq!(
            pair.usage().parse::<SpecFlag>().unwrap().usage(),
            "--range[=START] [END]"
        );

        let repeatable: SpecFlag = "--tag <TAG>".parse().unwrap();
        let repeatable = SpecFlag {
            var: true,
            require_equals: true,
            ..repeatable
        };
        assert_eq!(repeatable.usage(), "--tag…=<TAG>");
        let reparsed: SpecFlag = repeatable.usage().parse().unwrap();
        assert!(reparsed.var);
        assert!(reparsed.require_equals);
        assert!(!reparsed.arg.as_ref().unwrap().var);

        let repeatable_optional: SpecFlag = "--color [WHEN]".parse().unwrap();
        let repeatable_optional = SpecFlag {
            var: true,
            require_equals: true,
            ..repeatable_optional
        };
        assert_eq!(repeatable_optional.usage(), "--color…[=WHEN]");
        let reparsed: SpecFlag = repeatable_optional.usage().parse().unwrap();
        assert!(reparsed.var);
        assert!(reparsed.require_equals);
        assert!(!reparsed.arg.as_ref().unwrap().var);
    }

    #[test]
    fn optional_flag_value_policy_round_trips_separately_from_help() {
        let spec: Spec = "flag \"--bump [LEVEL]\" value_optional=#true\n"
            .parse()
            .unwrap();
        let bump = &spec.cmd.flags[0];
        assert!(bump.value_optional);
        assert!(!bump.arg.as_ref().unwrap().required);

        let rendered = spec.to_string();
        assert!(rendered.contains("value_optional=#true"), "{rendered}");
        let reparsed: Spec = rendered.parse().unwrap();
        assert!(reparsed.cmd.flags[0].value_optional);

        let presentation_only: Spec = "flag \"--bump [LEVEL]\"\n".parse().unwrap();
        assert!(!presentation_only.cmd.flags[0].value_optional);

        let command = clap::Command::new("ex").arg(
            clap::Arg::new("bump")
                .long("bump")
                .action(clap::ArgAction::Set)
                .num_args(0..=1),
        );
        let bridged = Spec::from(&command);
        assert!(bridged.cmd.flags[0].value_optional);

        let zero_arity = clap::Command::new("ex").arg(
            clap::Arg::new("plain")
                .long("plain")
                .action(clap::ArgAction::Set)
                .num_args(0),
        );
        assert!(!Spec::from(&zero_arity).cmd.flags[0].value_optional);
    }

    #[test]
    fn explicit_boolean_values_round_trip() {
        let spec: Spec = "flag \"--color\" negate=\"--no-color\" bool_value=#true\n"
            .parse()
            .unwrap();
        assert!(spec.cmd.flags[0].bool_value);
        let rendered = spec.to_string();
        assert!(rendered.contains("bool_value=#true"), "{rendered}");
        assert!(rendered.parse::<Spec>().unwrap().cmd.flags[0].bool_value);

        for invalid in [
            "flag \"--jobs <N>\" bool_value=#true\n",
            "flag \"--verbose\" count=#true bool_value=#true\n",
        ] {
            assert!(invalid.parse::<Spec>().is_err(), "{invalid}");
        }
    }

    #[test]
    fn default_missing_round_trips_and_cannot_come_across_from_clap() {
        let spec: Spec = "flag \"--color <WHEN>\" default_missing=\"always\"\n"
            .parse()
            .unwrap();
        assert_eq!(spec.cmd.flags[0].default_missing.as_deref(), Some("always"));
        assert!(
            !spec.cmd.flags[0].arg.as_ref().unwrap().required,
            "a missing value is optional, so help should not demand it"
        );
        assert!(
            spec.cmd.flags[0].usage.contains("[WHEN]")
                && !spec.cmd.flags[0].usage.contains("<WHEN>"),
            "help should show an optional value: {}",
            spec.cmd.flags[0].usage
        );

        let reparsed: Spec = spec.to_string().parse().unwrap();
        assert_eq!(
            reparsed.cmd.flags[0].default_missing.as_deref(),
            Some("always"),
            "{spec}"
        );

        // Same hole as `requires`: clap 4 has the setter and keeps the field private.
        let cmd = clap::Command::new("ex").arg(
            clap::Arg::new("color")
                .long("color")
                .action(clap::ArgAction::Set)
                .num_args(0..=1)
                .default_missing_value("always"),
        );
        let spec = Spec::from(&cmd);
        let color = spec.cmd.flags.iter().find(|f| f.name == "color").unwrap();
        assert!(
            color.default_missing.is_none(),
            "clap exposes no getter for `default_missing_value`; if this now fails, \
             the bridge can carry it and `SpecFlag::default_missing` should say so"
        );
    }

    #[test]
    fn value_count_bounds_survive_the_clap_bridge() {
        let cmd = clap::Command::new("ex")
            .arg(
                clap::Arg::new("pair")
                    .long("pair")
                    .action(clap::ArgAction::Set)
                    .num_args(2),
            )
            .arg(
                clap::Arg::new("files")
                    .value_name("FILES")
                    .required(true)
                    .num_args(2..=4),
            );
        let spec = Spec::from(&cmd);

        let pair_flag = spec.cmd.flags.iter().find(|f| f.name == "pair").unwrap();
        assert!(!pair_flag.var, "the flag itself is not repeatable");
        assert_eq!(pair_flag.var_min, None);
        assert_eq!(pair_flag.var_max, None);
        let pair = pair_flag.arg.as_ref().unwrap();
        assert!(pair.var);
        assert_eq!(pair.var_min, Some(2));
        assert_eq!(pair.var_max, Some(2));

        let files = spec.cmd.args.iter().find(|a| a.name == "FILES").unwrap();
        assert!(files.var);
        assert_eq!(files.var_min, Some(2));
        assert_eq!(files.var_max, Some(4));

        let words = ["ex", "--pair", "a", "b", "one", "two"].map(str::to_string);
        crate::parse(&spec, &words).expect("both clap value-count ranges are satisfied");

        let words = ["ex", "--pair", "a", "--", "one", "two"].map(str::to_string);
        let err = crate::parse(&spec, &words).unwrap_err();
        assert!(
            format!("{err:?}").contains("requires at least 2 value(s), got 1"),
            "{err:?}"
        );

        let reparsed: Spec = spec.to_string().parse().unwrap();
        let pair = reparsed.cmd.flags[0].arg.as_ref().unwrap();
        assert_eq!((pair.var_min, pair.var_max), (Some(2), Some(2)));
        assert_eq!(
            (reparsed.cmd.args[0].var_min, reparsed.cmd.args[0].var_max),
            (Some(2), Some(4))
        );
    }

    #[test]
    fn append_value_count_bounds_are_per_occurrence() {
        let cmd = clap::Command::new("ex").arg(
            clap::Arg::new("pair")
                .long("pair")
                .action(clap::ArgAction::Append)
                .num_args(2),
        );
        let spec = Spec::from(&cmd);
        let flag = &spec.cmd.flags[0];
        let values = flag.arg.as_ref().unwrap();
        assert!(flag.var);
        assert_eq!((values.var_min, values.var_max), (Some(2), Some(2)));

        crate::parse(
            &spec,
            &["ex", "--pair", "a", "b", "--pair", "c", "d"].map(str::to_string),
        )
        .expect("each occurrence satisfies the fixed cardinality");

        let err = crate::parse(
            &spec,
            &["ex", "--pair", "a", "--pair", "c", "d"].map(str::to_string),
        )
        .unwrap_err();
        assert!(format!("{err:?}").contains("requires at least 2 value(s), got 1"));
    }

    #[test]
    fn ranged_value_names_do_not_emit_invalid_fixed_arity() {
        let cmd = clap::Command::new("ex")
            .arg(
                clap::Arg::new("range")
                    .long("range")
                    .action(clap::ArgAction::Set)
                    .num_args(2..=4)
                    .value_names(["START", "END"]),
            )
            .arg(
                clap::Arg::new("files")
                    .num_args(1..=3)
                    .value_names(["FIRST", "REST"]),
            );
        let spec = Spec::from(&cmd);
        assert_eq!(
            spec.cmd.flags[0].arg.as_ref().unwrap().value_names,
            ["START"]
        );
        assert_eq!(spec.cmd.args[0].value_names, ["FIRST"]);
        let rendered = spec.to_string();
        let _: Spec = rendered.parse().expect("the generated KDL must parse back");
    }

    #[test]
    fn delimiter_value_count_bounds_are_not_mapped() {
        let cmd = clap::Command::new("ex")
            .arg(
                clap::Arg::new("pairs")
                    .long("pairs")
                    .action(clap::ArgAction::Set)
                    .value_delimiter(',')
                    .num_args(2),
            )
            .arg(clap::Arg::new("items").value_delimiter(',').num_args(2..=3));
        let spec = Spec::from(&cmd);

        let pairs = spec.cmd.flags[0].arg.as_ref().unwrap();
        assert!(pairs.var);
        assert_eq!(pairs.delimiter, Some(','));
        assert_eq!((pairs.var_min, pairs.var_max), (None, None));

        let items = &spec.cmd.args[0];
        assert!(items.var);
        assert_eq!(items.delimiter, Some(','));
        assert_eq!((items.var_min, items.var_max), (None, None));
    }

    #[test]
    fn an_optional_flag_value_carries_its_policy_and_bound() {
        let cmd = clap::Command::new("ex").arg(
            clap::Arg::new("values")
                .long("values")
                .action(clap::ArgAction::Set)
                .num_args(0..=3),
        );
        let spec = Spec::from(&cmd);
        let values = spec.cmd.flags[0].arg.as_ref().unwrap();

        assert!(spec.cmd.flags[0].value_optional);
        assert_eq!(values.var_min, Some(0));
        assert_eq!(values.var_max, Some(3));
        assert_eq!(spec.cmd.flags[0].var_min, None);
        assert_eq!(spec.cmd.flags[0].var_max, None);
    }

    #[test]
    fn requires_cannot_come_across_from_clap() {
        // Not an oversight to be fixed later: clap 4 has `Arg::requires` as a setter
        // with no getter and keeps the field private, so there is nothing here to read.
        // Asserted rather than left implied, because an empty vector otherwise looks
        // like a bug in the bridge — and because a future clap that *does* expose it
        // should fail this test rather than pass silently.
        let cmd = clap::Command::new("ex")
            .arg(clap::Arg::new("out").long("out").requires("format"))
            .arg(clap::Arg::new("format").long("format"));
        let spec = Spec::from(&cmd);
        let out = spec.cmd.flags.iter().find(|f| f.name == "out").unwrap();
        assert!(
            out.requires.is_empty(),
            "clap exposes no getter for `requires`; if this now fails, the bridge can \
             carry it and `SpecFlag::requires` should say so"
        );
    }

    #[cfg(feature = "clap")]
    #[test]
    fn conflicts_survive_the_clap_bridge() {
        // clap has had `conflicts_with` for years and mise declares forty of them; the
        // bridge was dropping every one, because clap keeps conflicts on the command
        // rather than on the argument.
        let cmd = clap::Command::new("ex")
            .arg(clap::Arg::new("file").long("file").conflicts_with("stdin"))
            .arg(clap::Arg::new("stdin").long("stdin"));
        let spec: Spec = (&cmd).into();

        let file = spec.cmd.flags.iter().find(|f| f.name == "file").unwrap();
        assert_eq!(file.conflicts, vec!["--stdin".to_string()]);

        // Only the declared direction: clap validates a conflict both ways but reports
        // it only from the argument that declared it. Recording it once is enough,
        // because the check looks at every flag that was given — see the parser test
        // that rejects either order.
        let stdin = spec.cmd.flags.iter().find(|f| f.name == "stdin").unwrap();
        assert!(stdin.conflicts.is_empty());

        let positional = clap::Command::new("ex")
            .arg(
                clap::Arg::new("from-file")
                    .long("from-file")
                    .conflicts_with("value"),
            )
            .arg(clap::Arg::new("value"));
        let spec: Spec = (&positional).into();
        let from_file = spec
            .cmd
            .flags
            .iter()
            .find(|f| f.name == "from-file")
            .unwrap();
        assert_eq!(from_file.conflicts, vec!["value".to_string()]);

        // A short-only target is named `-q`, since that is the only name it has.
        // Taking only the long form dropped the conflict and left the spec accepting a
        // combination clap rejects.
        let shorts = clap::Command::new("ex")
            .arg(clap::Arg::new("loud").long("loud").conflicts_with("quiet"))
            .arg(clap::Arg::new("quiet").short('q'));
        let spec: Spec = (&shorts).into();
        let loud = spec.cmd.flags.iter().find(|f| f.name == "loud").unwrap();
        assert_eq!(loud.conflicts, vec!["-q".to_string()]);
    }

    #[test]
    fn a_serialized_spec_can_always_be_read_back() {
        // Both of these produced KDL that this crate could not reparse: a node
        // argument beginning with a dash was rendered bare, and a control character
        // was rendered literally. Help text carries the second whenever a CLI
        // colors its output.
        let spec: Spec = "flag \"--shell <s>\" {\n  required_unless \"--jobs\" \"--color\"\n  overrides \"--keep\" \"--dry-run\"\n  long_help \"Colored.\\u{1b}[0m Text.\"\n}\n"
            .parse()
            .unwrap();

        let serialized = spec.to_string();
        let reparsed: Spec = serialized
            .parse()
            .unwrap_or_else(|e| panic!("a serialized spec should reparse: {e}\n\n{serialized}"));

        let flag = &reparsed.cmd.flags[0];
        assert_eq!(
            flag.required_unless,
            vec!["--jobs".to_string(), "--color".to_string()]
        );
        assert_eq!(
            flag.overrides,
            vec!["--keep".to_string(), "--dry-run".to_string()]
        );
        assert_eq!(flag.help_long.as_deref(), Some("Colored.\u{1b}[0m Text."));
    }

    #[test]
    fn help_heading_round_trips() {
        // Both spellings: a property, and a child node for when the text is long.
        let spec: Spec = r#"
flag "--filter <pattern>" help_heading="Filtering"
flag "--exclude <pattern>" {
  help_heading "Filtering"
}
arg "<file>" help_heading="Input"
"#
        .parse()
        .unwrap();
        assert_eq!(spec.cmd.flags[0].help_heading.as_deref(), Some("Filtering"));
        assert_eq!(spec.cmd.flags[1].help_heading.as_deref(), Some("Filtering"));
        assert_eq!(spec.cmd.args[0].help_heading.as_deref(), Some("Input"));

        // And it survives being written back out.
        let reparsed: Spec = spec.to_string().parse().unwrap();
        assert_eq!(
            reparsed.cmd.flags[0].help_heading.as_deref(),
            Some("Filtering")
        );
        assert_eq!(reparsed.cmd.args[0].help_heading.as_deref(), Some("Input"));
    }

    #[cfg(feature = "clap")]
    #[test]
    fn help_heading_comes_across_from_clap() {
        // clap has had help_heading for years and the bridge was dropping it, so
        // a CLI that grouped its flags lost the grouping on the way into a spec.
        let cmd = clap::Command::new("ex")
            .arg(
                clap::Arg::new("filter")
                    .long("filter")
                    .help_heading("Filtering"),
            )
            .arg(clap::Arg::new("plain").long("plain"));
        let spec: Spec = (&cmd).into();

        let filter = spec
            .cmd
            .flags
            .iter()
            .find(|f| f.name == "filter")
            .expect("--filter should be in the spec");
        assert_eq!(filter.help_heading.as_deref(), Some("Filtering"));

        let plain = spec
            .cmd
            .flags
            .iter()
            .find(|f| f.name == "plain")
            .expect("--plain should be in the spec");
        assert_eq!(plain.help_heading, None);
    }

    #[test]
    fn test_flag_with_env() {
        let spec = Spec::parse(
            &Default::default(),
            r#"
flag "--color" env="MYCLI_COLOR" help="Enable color output"
flag "--verbose" env="MYCLI_VERBOSE"
            "#,
        )
        .unwrap();

        assert_snapshot!(spec, @r#"
        flag --color help="Enable color output" env=MYCLI_COLOR
        flag --verbose env=MYCLI_VERBOSE
        "#);

        let color_flag = spec.cmd.flags.iter().find(|f| f.name == "color").unwrap();
        assert_eq!(color_flag.env, Some("MYCLI_COLOR".to_string()));

        let verbose_flag = spec.cmd.flags.iter().find(|f| f.name == "verbose").unwrap();
        assert_eq!(verbose_flag.env, Some("MYCLI_VERBOSE".to_string()));
    }

    #[test]
    fn test_flag_with_env_child_node() {
        let spec = Spec::parse(
            &Default::default(),
            r#"
flag "--color" help="Enable color output" {
    env "MYCLI_COLOR"
}
flag "--verbose" {
    env "MYCLI_VERBOSE"
}
            "#,
        )
        .unwrap();

        assert_snapshot!(spec, @r#"
        flag --color help="Enable color output" env=MYCLI_COLOR
        flag --verbose env=MYCLI_VERBOSE
        "#);

        let color_flag = spec.cmd.flags.iter().find(|f| f.name == "color").unwrap();
        assert_eq!(color_flag.env, Some("MYCLI_COLOR".to_string()));

        let verbose_flag = spec.cmd.flags.iter().find(|f| f.name == "verbose").unwrap();
        assert_eq!(verbose_flag.env, Some("MYCLI_VERBOSE".to_string()));
    }

    #[test]
    fn test_flag_with_overrides() {
        let spec = Spec::parse(
            &Default::default(),
            r#"
flag "--file <file>" overrides="--stdin"
flag "--format <format>" {
    overrides "--json" "--yaml"
}
            "#,
        )
        .unwrap();

        assert_eq!(spec.cmd.flags[0].overrides, ["--stdin"]);
        assert_eq!(spec.cmd.flags[1].overrides, ["--json", "--yaml"]);

        let reparsed: Spec = spec.to_string().parse().unwrap();
        assert_eq!(reparsed.cmd.flags[0].overrides, ["--stdin"]);
        assert_eq!(reparsed.cmd.flags[1].overrides, ["--json", "--yaml"]);
    }

    #[test]
    fn test_flag_with_conditional_requirements() {
        let spec = Spec::parse(
            &Default::default(),
            r#"
flag "--file <file>" required_if="--dir"
flag "--output <output>" {
    required_unless "--stdout" "--check"
}
            "#,
        )
        .unwrap();

        assert_eq!(spec.cmd.flags[0].required_if, ["--dir"]);
        assert_eq!(spec.cmd.flags[1].required_unless, ["--stdout", "--check"]);

        let reparsed: Spec = spec.to_string().parse().unwrap();
        assert_eq!(reparsed.cmd.flags[0].required_if, ["--dir"]);
        assert_eq!(
            reparsed.cmd.flags[1].required_unless,
            ["--stdout", "--check"]
        );
    }

    #[test]
    fn test_flag_with_boolean_defaults() {
        let spec = Spec::parse(
            &Default::default(),
            r#"
flag "--color" default=#true
flag "--verbose" default=#false
flag "--debug" default="true"
flag "--quiet" default="false"
            "#,
        )
        .unwrap();

        let color_flag = spec.cmd.flags.iter().find(|f| f.name == "color").unwrap();
        assert_eq!(color_flag.default, vec!["true".to_string()]);

        let verbose_flag = spec.cmd.flags.iter().find(|f| f.name == "verbose").unwrap();
        assert_eq!(verbose_flag.default, vec!["false".to_string()]);

        let debug_flag = spec.cmd.flags.iter().find(|f| f.name == "debug").unwrap();
        assert_eq!(debug_flag.default, vec!["true".to_string()]);

        let quiet_flag = spec.cmd.flags.iter().find(|f| f.name == "quiet").unwrap();
        assert_eq!(quiet_flag.default, vec!["false".to_string()]);
    }

    #[test]
    fn test_flag_with_boolean_defaults_child_node() {
        let spec = Spec::parse(
            &Default::default(),
            r#"
flag "--color" {
    default #true
}
flag "--verbose" {
    default #false
}
            "#,
        )
        .unwrap();

        let color_flag = spec.cmd.flags.iter().find(|f| f.name == "color").unwrap();
        assert_eq!(color_flag.default, vec!["true".to_string()]);

        let verbose_flag = spec.cmd.flags.iter().find(|f| f.name == "verbose").unwrap();
        assert_eq!(verbose_flag.default, vec!["false".to_string()]);
    }

    #[test]
    fn test_flag_with_single_default() {
        let spec = Spec::parse(
            &Default::default(),
            r#"
flag "--foo <foo>" var=#true default="bar"
            "#,
        )
        .unwrap();

        let flag = spec.cmd.flags.iter().find(|f| f.name == "foo").unwrap();
        assert!(flag.var);
        assert_eq!(flag.default, vec!["bar".to_string()]);
    }

    #[test]
    fn test_flag_with_multiple_defaults_child_node() {
        let spec = Spec::parse(
            &Default::default(),
            r#"
flag "--foo <foo>" var=#true {
    default {
        "xyz"
        "bar"
    }
}
            "#,
        )
        .unwrap();

        let flag = spec.cmd.flags.iter().find(|f| f.name == "foo").unwrap();
        assert!(flag.var);
        assert_eq!(flag.default, vec!["xyz".to_string(), "bar".to_string()]);
    }

    #[test]
    fn test_flag_with_single_default_child_node() {
        let spec = Spec::parse(
            &Default::default(),
            r#"
flag "--foo <foo>" var=#true {
    default "bar"
}
            "#,
        )
        .unwrap();

        let flag = spec.cmd.flags.iter().find(|f| f.name == "foo").unwrap();
        assert!(flag.var);
        assert_eq!(flag.default, vec!["bar".to_string()]);
    }

    #[test]
    fn test_flag_default_serialization_single() {
        let spec = Spec::parse(
            &Default::default(),
            r#"
flag "--foo <foo>" default="bar"
            "#,
        )
        .unwrap();

        // When serialized, single default should use property format
        let output = spec.to_string();
        assert!(output.contains("default=bar") || output.contains(r#"default="bar""#));
    }

    #[test]
    fn test_flag_default_serialization_multiple() {
        let spec = Spec::parse(
            &Default::default(),
            r#"
flag "--foo <foo>" var=#true {
    default {
        "xyz"
        "bar"
    }
}
            "#,
        )
        .unwrap();

        // When serialized, multiple defaults should use child node format
        let output = spec.to_string();
        // The output should contain a default block with children
        assert!(output.contains("default {"));
    }
}
