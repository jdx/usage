use kdl::{KdlDocument, KdlEntry, KdlNode};
use serde::Serialize;
use std::fmt::Display;
use std::hash::Hash;
use std::str::FromStr;

use crate::error::UsageErr;
use crate::spec::builder::SpecArgBuilder;
use crate::spec::context::ParsingContext;
use crate::spec::effect::{SpecCommandEffect, EFFECT_VALUES};
use crate::spec::helpers::{string_entry, NodeHelper};
use crate::spec::is_false;
use crate::{string, SpecChoices};
#[cfg(feature = "clap")]
use crate::{SpecChoice, SpecChoiceAlias};

/// A value comparison that can make another argument required.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct SpecRequiredIfEq {
    pub selector: String,
    pub value: String,
}

#[derive(Debug, Default, Clone, Serialize, PartialEq, Eq, strum::EnumString, strum::Display)]
#[strum(serialize_all = "snake_case")]
pub enum SpecDoubleDashChoices {
    /// Once an arg is entered, behave as if "--" was passed
    Automatic,
    /// Allow "--" to be passed
    #[default]
    Optional,
    /// Require "--" to be passed
    Required,
    /// Preserve "--" tokens as values (only for variadic args)
    Preserve,
}

/// A positional argument specification.
///
/// Arguments are positional values passed to a command without a flag prefix.
/// They can be required or optional, and can accept multiple values (variadic).
///
/// # Example
///
/// ```
/// use usage::SpecArg;
///
/// let arg = SpecArg::builder()
///     .name("file")
///     .required(true)
///     .help("Input file to process")
///     .build();
/// ```
#[derive(Debug, Default, Clone, Serialize)]
#[non_exhaustive]
pub struct SpecArg {
    /// Name of the argument (used in help text)
    pub name: String,
    /// Ordered placeholders for a fixed-arity value, such as `START` and `END`.
    /// Empty means the argument's `name` is the sole placeholder.
    #[serde(skip_serializing_if = "Vec::is_empty")]
    pub value_names: Vec<String>,
    /// Generated usage string (e.g., "<file>" or "[file]")
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
    /// Whether this argument must be provided
    pub required: bool,
    /// How to handle the "--" separator
    pub double_dash: SpecDoubleDashChoices,
    /// Whether this argument accepts multiple values
    #[serde(skip_serializing_if = "is_false")]
    pub var: bool,
    /// Minimum number of values for variadic arguments
    #[serde(skip_serializing_if = "Option::is_none")]
    pub var_min: Option<usize>,
    /// Maximum number of values for variadic arguments
    #[serde(skip_serializing_if = "Option::is_none")]
    pub var_max: Option<usize>,
    /// The character a single word is split on to produce several values.
    ///
    /// `--tags a,b,c` as three values rather than one, which is clap's
    /// `value_delimiter`. Only meaningful where several values can land, so it goes with
    /// [`SpecArg::var`]; declaring it anywhere else is refused rather than silently
    /// dropping everything after the first separator.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub delimiter: Option<char>,
    /// Accept negative numeric tokens without accepting arbitrary dash-prefixed words.
    #[serde(skip_serializing_if = "is_false")]
    pub allow_negative_numbers: bool,
    /// End this variadic argument when this token is seen, without binding it.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub value_terminator: Option<String>,
    /// Whether to hide this argument from help output
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
    /// Hide this argument only from short help.
    #[serde(skip_serializing_if = "is_false")]
    pub hide_short_help: bool,
    /// Hide this argument only from long help.
    #[serde(skip_serializing_if = "is_false")]
    pub hide_long_help: bool,
    /// Arguments and flags that cannot be given alongside this positional.
    ///
    /// A bare selector names another positional by name; flag selectors keep their
    /// `--long` or `-s` spelling.
    #[serde(skip_serializing_if = "Vec::is_empty")]
    pub conflicts: Vec<String>,
    /// Arguments that must also be present when this positional is present.
    #[serde(skip_serializing_if = "Vec::is_empty")]
    pub requires: Vec<String>,
    /// Presence conditions, any one of which makes this positional required.
    #[serde(skip_serializing_if = "Vec::is_empty")]
    pub required_if: Vec<String>,
    /// Value conditions, any one of which makes this positional required.
    #[serde(skip_serializing_if = "Vec::is_empty")]
    pub required_if_eq: Vec<SpecRequiredIfEq>,
    /// Value conditions which must all match to make this positional required.
    #[serde(skip_serializing_if = "Vec::is_empty")]
    pub required_if_eq_all: Vec<SpecRequiredIfEq>,
    /// Any present selector waives this positional's requirement.
    #[serde(skip_serializing_if = "Vec::is_empty")]
    pub required_unless: Vec<String>,
    /// Only the presence of every selector waives this positional's requirement.
    #[serde(skip_serializing_if = "Vec::is_empty")]
    pub required_unless_all: Vec<String>,
    /// Default value(s) if the argument is not provided
    #[serde(skip_serializing_if = "Vec::is_empty")]
    pub default: Vec<String>,
    /// Valid choices for this argument
    #[serde(skip_serializing_if = "Option::is_none")]
    pub choices: Option<SpecChoices>,
    /// A portable expr expression that must return true for each raw value.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub validate: Option<String>,
    /// Message reported when [`SpecArg::validate`] returns false.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub validate_error: Option<String>,
    /// Raises the effect of the command when this argument is supplied.
    /// See [`crate::spec::effect::SpecCommandEffect`]; never lowers it.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub effect: Option<SpecCommandEffect>,
    /// Environment variable that can provide this argument's value
    #[serde(skip_serializing_if = "Option::is_none")]
    pub env: Option<String>,
    /// Heading this argument is listed under in help output. Presentational only,
    /// like the flag field of the same name.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub help_heading: Option<String>,
}

impl SpecArg {
    /// Create a new builder for SpecArg
    pub fn builder() -> SpecArgBuilder {
        SpecArgBuilder::new()
    }

    pub(crate) fn parse(ctx: &ParsingContext, node: &NodeHelper) -> Result<Self, UsageErr> {
        let mut arg: SpecArg = node.arg(0)?.ensure_string()?.parse()?;
        for (k, v) in node.props() {
            match k {
                "help" => arg.help = Some(v.ensure_string()?),
                "long_help" => arg.help_long = Some(v.ensure_string()?),
                "help_long" => arg.help_long = Some(v.ensure_string()?),
                "help_md" => arg.help_md = Some(v.ensure_string()?),
                "required" => arg.required = v.ensure_bool()?,
                "double_dash" => arg.double_dash = v.ensure_string()?.parse()?,
                "var" => arg.var = v.ensure_bool()?,
                "delimiter" => {
                    let raw = v.ensure_string()?;
                    let mut chars = raw.chars();
                    match (chars.next(), chars.next()) {
                        // ASCII, not merely one character. Splitting is by byte everywhere
                        // below this — the derive says so where it reads the same property —
                        // and a non-ASCII separator has no single byte to be. Worse than
                        // having none: its bytes are continuation bytes, which appear inside
                        // unrelated characters, so it would split words nobody separated.
                        (Some(c), None) if c.is_ascii() => arg.delimiter = Some(c),
                        (Some(c), None) => bail_parse!(
                            ctx,
                            v.entry.span(),
                            "a delimiter is one byte, and {c:?} is more than one; use an \
                             ASCII separator"
                        ),
                        _ => bail_parse!(
                            ctx,
                            v.entry.span(),
                            "a delimiter is one character, and {raw:?} is not"
                        ),
                    }
                }
                "allow_negative_numbers" => arg.allow_negative_numbers = v.ensure_bool()?,
                "value_terminator" => arg.value_terminator = v.ensure_string().map(Some)?,
                "hide" => arg.hide = v.ensure_bool()?,
                "hide_default_value" => arg.hide_default_value = v.ensure_bool()?,
                "hide_env" => arg.hide_env = v.ensure_bool()?,
                "hide_env_values" => arg.hide_env_values = v.ensure_bool()?,
                "hide_possible_values" => arg.hide_possible_values = v.ensure_bool()?,
                "hide_short_help" => arg.hide_short_help = v.ensure_bool()?,
                "hide_long_help" => arg.hide_long_help = v.ensure_bool()?,
                "conflicts" => arg.conflicts = vec![v.ensure_string()?],
                "requires" => arg.requires = vec![v.ensure_string()?],
                "required_if" => arg.required_if = vec![v.ensure_string()?],
                "required_unless" => arg.required_unless = vec![v.ensure_string()?],
                "required_unless_all" => arg.required_unless_all = vec![v.ensure_string()?],
                "var_min" => arg.var_min = v.ensure_usize().map(Some)?,
                "var_max" => arg.var_max = v.ensure_usize().map(Some)?,
                "default" => arg.default = vec![v.ensure_string()?],
                "effect" => {
                    let raw = v.ensure_string()?;
                    match raw.parse() {
                        Ok(effect) => arg.effect = Some(effect),
                        Err(_) => bail_parse!(
                            ctx,
                            v.entry.span(),
                            "unsupported effect {raw}, expected one of: {EFFECT_VALUES}"
                        ),
                    }
                }
                "env" => arg.env = v.ensure_string().map(Some)?,
                "validate" => arg.validate = v.ensure_string().map(Some)?,
                "validate_error" => arg.validate_error = v.ensure_string().map(Some)?,
                "help_heading" => arg.help_heading = v.ensure_string().map(Some)?,
                k => bail_parse!(ctx, v.entry.span(), "unsupported arg key {k}"),
            }
        }
        if !arg.default.is_empty() {
            arg.required = false;
        }
        for child in node.children() {
            match child.name() {
                "choices" => arg.choices = Some(SpecChoices::parse(ctx, &child)?),
                "effect" => {
                    let a = child.arg(0)?;
                    let raw = a.ensure_string()?;
                    match raw.parse() {
                        Ok(effect) => arg.effect = Some(effect),
                        Err(_) => bail_parse!(
                            ctx,
                            a.entry.span(),
                            "unsupported effect {raw}, expected one of: {EFFECT_VALUES}"
                        ),
                    }
                }
                "env" => arg.env = child.arg(0)?.ensure_string().map(Some)?,
                "validate" => arg.validate = child.arg(0)?.ensure_string().map(Some)?,
                "validate_error" => {
                    arg.validate_error = child.arg(0)?.ensure_string().map(Some)?;
                }
                "help_heading" => {
                    arg.help_heading = child.arg(0)?.ensure_string().map(Some)?;
                }
                "default" => {
                    // Support both single value and multiple values
                    // default "bar"            -> vec!["bar"]
                    // default { "xyz"; "bar" } -> vec!["xyz", "bar"]
                    let children = child.children();
                    if children.is_empty() {
                        // Single value: default "bar"
                        arg.default = vec![child.arg(0)?.ensure_string()?];
                    } else {
                        // Multiple values from children: default { "xyz"; "bar" }
                        // In KDL, these are child nodes where the string is the node name
                        arg.default = children.iter().map(|c| c.name().to_string()).collect();
                    }
                }
                "help" => arg.help = Some(child.arg(0)?.ensure_string()?),
                "long_help" => arg.help_long = Some(child.arg(0)?.ensure_string()?),
                "help_long" => arg.help_long = Some(child.arg(0)?.ensure_string()?),
                "help_md" => arg.help_md = Some(child.arg(0)?.ensure_string()?),
                "required" => arg.required = child.arg(0)?.ensure_bool()?,
                "var" => arg.var = child.arg(0)?.ensure_bool()?,
                "var_min" => arg.var_min = child.arg(0)?.ensure_usize().map(Some)?,
                "var_max" => arg.var_max = child.arg(0)?.ensure_usize().map(Some)?,
                "value_names" => {
                    arg.value_names = child
                        .ensure_arg_len(1..)?
                        .args()
                        .map(|entry| entry.ensure_string())
                        .collect::<Result<Vec<_>, _>>()?;
                }
                "allow_negative_numbers" => {
                    arg.allow_negative_numbers = child.arg(0)?.ensure_bool()?;
                }
                "value_terminator" => {
                    arg.value_terminator = child.arg(0)?.ensure_string().map(Some)?;
                }
                "hide" => arg.hide = child.arg(0)?.ensure_bool()?,
                "hide_default_value" => arg.hide_default_value = child.arg(0)?.ensure_bool()?,
                "hide_env" => arg.hide_env = child.arg(0)?.ensure_bool()?,
                "hide_env_values" => arg.hide_env_values = child.arg(0)?.ensure_bool()?,
                "hide_possible_values" => arg.hide_possible_values = child.arg(0)?.ensure_bool()?,
                "hide_short_help" => arg.hide_short_help = child.arg(0)?.ensure_bool()?,
                "hide_long_help" => arg.hide_long_help = child.arg(0)?.ensure_bool()?,
                "conflicts" => {
                    arg.conflicts = child
                        .ensure_arg_len(1..)?
                        .args()
                        .map(|entry| entry.ensure_string())
                        .collect::<Result<Vec<_>, _>>()?;
                }
                "requires" => arg.requires = string_args(&child)?,
                "required_if" => arg.required_if = string_args(&child)?,
                "required_if_eq" => arg.required_if_eq.push(required_if_eq(&child)?),
                "required_if_eq_all" => {
                    let len = child.args().count();
                    if len < 2 || len % 2 != 0 {
                        bail_parse!(
                            ctx,
                            child.node.name().span(),
                            "required_if_eq_all needs selector/value pairs"
                        );
                    }
                    arg.required_if_eq_all = required_if_eq_pairs(&child)?;
                }
                "required_unless" => arg.required_unless = string_args(&child)?,
                "required_unless_all" => arg.required_unless_all = string_args(&child)?,
                "double_dash" => arg.double_dash = child.arg(0)?.ensure_string()?.parse()?,
                k => bail_parse!(ctx, child.node.name().span(), "unsupported arg child {k}"),
            }
        }
        if let Some(first) = arg.value_names.first() {
            arg.name.clone_from(first);
        }
        if arg.value_names.len() > 1 {
            let arity = arg.value_names.len();
            match (arg.var_min, arg.var_max) {
                (None, None) => {
                    arg.var_min = Some(arity);
                    arg.var_max = Some(arity);
                }
                (Some(min), Some(max)) if min == arity && max == arity => {}
                _ => bail_parse!(
                    ctx,
                    node.node.name().span(),
                    "{arity} value names require var_min={arity} and var_max={arity}"
                ),
            }
            arg.var = true;
        }
        if arg.validate_error.is_some() && arg.validate.is_none() {
            bail_parse!(
                ctx,
                node.node.name().span(),
                "validate_error requires a validate expression"
            );
        }
        if arg.value_terminator.as_deref() == Some("") {
            bail_parse!(
                ctx,
                node.node.name().span(),
                "value_terminator cannot be empty"
            );
        }
        if arg.value_terminator.is_some() && !arg.var {
            bail_parse!(
                ctx,
                node.node.name().span(),
                "value_terminator requires a variadic argument"
            );
        }
        #[cfg(feature = "validation")]
        if let Some(expression) = &arg.validate {
            if let Err(error) = usage_validation::check(expression) {
                bail_parse!(
                    ctx,
                    node.node.name().span(),
                    "invalid validation expression: {error}"
                );
            }
        }
        arg.usage = arg.usage();
        if let Some(help) = &arg.help {
            arg.help_first_line = Some(string::first_line(help));
        }
        Ok(arg)
    }
}

impl SpecArg {
    pub fn usage(&self) -> String {
        let exact_arity = self.var.then_some(()).and_then(|()| {
            self.var_min
                .zip(self.var_max)
                .filter(|(min, max)| min == max && *min > 1)
                .map(|(arity, _)| arity)
        });
        if self.value_names.len() > 1 || exact_arity.is_some() {
            let labels = if self.value_names.len() > 1 {
                self.value_names.clone()
            } else {
                vec![
                    self.value_names
                        .first()
                        .cloned()
                        .unwrap_or_else(|| self.name.clone());
                    exact_arity.expect("branch checked")
                ]
            };
            let placeholders = labels
                .iter()
                .map(|name| {
                    if self.required {
                        format!("<{name}>")
                    } else {
                        format!("[{name}]")
                    }
                })
                .collect::<Vec<_>>()
                .join(" ");
            return if self.double_dash == SpecDoubleDashChoices::Required {
                format!("-- {placeholders}")
            } else {
                placeholders
            };
        }
        let name = if self.double_dash == SpecDoubleDashChoices::Required {
            format!("-- {}", self.name)
        } else {
            self.name.clone()
        };
        let mut name = if self.required {
            format!("<{name}>")
        } else {
            format!("[{name}]")
        };
        if self.var {
            name = format!("{name}…");
        }
        name
    }
}

impl From<&SpecArg> for KdlNode {
    fn from(arg: &SpecArg) -> Self {
        let mut node = KdlNode::new("arg");
        node.push(KdlEntry::new(arg.usage()));
        if let Some(desc) = &arg.help {
            node.push(string_entry(Some("help"), desc));
        }
        if let Some(desc) = &arg.help_long {
            node.push(string_entry(Some("help_long"), desc));
        }
        if let Some(desc) = &arg.help_md {
            node.push(string_entry(Some("help_md"), desc));
        }
        if !arg.required {
            node.push(KdlEntry::new_prop("required", false));
        }
        if arg.double_dash == SpecDoubleDashChoices::Automatic
            || arg.double_dash == SpecDoubleDashChoices::Preserve
        {
            node.push(KdlEntry::new_prop(
                "double_dash",
                arg.double_dash.to_string(),
            ));
        }
        if arg.var {
            node.push(KdlEntry::new_prop("var", true));
        }
        if let Some(min) = arg.var_min {
            node.push(KdlEntry::new_prop("var_min", min as i128));
        }
        if let Some(max) = arg.var_max {
            node.push(KdlEntry::new_prop("var_max", max as i128));
        }
        if let Some(delimiter) = arg.delimiter {
            node.push(string_entry(Some("delimiter"), &delimiter.to_string()));
        }
        if arg.allow_negative_numbers {
            node.push(KdlEntry::new_prop("allow_negative_numbers", true));
        }
        if let Some(terminator) = &arg.value_terminator {
            node.push(string_entry(Some("value_terminator"), terminator));
        }
        if arg.hide {
            node.push(KdlEntry::new_prop("hide", true));
        }
        for (name, hidden) in [
            ("hide_default_value", arg.hide_default_value),
            ("hide_env", arg.hide_env),
            ("hide_env_values", arg.hide_env_values),
            ("hide_possible_values", arg.hide_possible_values),
            ("hide_short_help", arg.hide_short_help),
            ("hide_long_help", arg.hide_long_help),
        ] {
            if hidden {
                node.push(KdlEntry::new_prop(name, true));
            }
        }
        if arg.conflicts.len() == 1 {
            node.push(string_entry(Some("conflicts"), &arg.conflicts[0]));
        } else if !arg.conflicts.is_empty() {
            let children = node.children_mut().get_or_insert_with(KdlDocument::new);
            let mut conflicts = KdlNode::new("conflicts");
            for target in &arg.conflicts {
                conflicts.push(string_entry(None, target));
            }
            children.nodes_mut().push(conflicts);
        }
        serialize_selector_list(&mut node, "requires", &arg.requires);
        serialize_selector_list(&mut node, "required_if", &arg.required_if);
        serialize_required_if_eq(&mut node, "required_if_eq", &arg.required_if_eq);
        if !arg.required_if_eq_all.is_empty() {
            serialize_required_if_eq(&mut node, "required_if_eq_all", &arg.required_if_eq_all);
        }
        serialize_selector_list(&mut node, "required_unless", &arg.required_unless);
        serialize_selector_list(&mut node, "required_unless_all", &arg.required_unless_all);
        // Serialize default values
        if !arg.default.is_empty() {
            if arg.default.len() == 1 {
                // Single value: use property default="bar"
                node.push(string_entry(Some("default"), &arg.default[0]));
            } else {
                // Multiple values: use child node default { "xyz"; "bar" }
                let children = node.children_mut().get_or_insert_with(KdlDocument::new);
                let mut default_node = KdlNode::new("default");
                let default_children = default_node
                    .children_mut()
                    .get_or_insert_with(KdlDocument::new);
                for val in &arg.default {
                    default_children
                        .nodes_mut()
                        .push(KdlNode::new(val.as_str()));
                }
                children.nodes_mut().push(default_node);
            }
        }
        if let Some(env) = &arg.env {
            node.push(string_entry(Some("env"), env));
        }
        if let Some(validate) = &arg.validate {
            node.push(string_entry(Some("validate"), validate));
        }
        if arg.validate.is_some() {
            if let Some(error) = &arg.validate_error {
                node.push(string_entry(Some("validate_error"), error));
            }
        }
        if let Some(help_heading) = &arg.help_heading {
            node.push(string_entry(Some("help_heading"), help_heading));
        }
        if let Some(effect) = &arg.effect {
            node.push(string_entry(Some("effect"), effect.as_str()));
        }
        if let Some(choices) = &arg.choices {
            let children = node.children_mut().get_or_insert_with(KdlDocument::new);
            children.nodes_mut().push(choices.into());
        }
        node
    }
}

fn string_args(node: &NodeHelper<'_>) -> Result<Vec<String>, UsageErr> {
    node.ensure_arg_len(1..)?
        .args()
        .map(|entry| entry.ensure_string())
        .collect()
}

fn required_if_eq(node: &NodeHelper<'_>) -> Result<SpecRequiredIfEq, UsageErr> {
    node.ensure_arg_len(2..=2)?;
    Ok(SpecRequiredIfEq {
        selector: node.arg(0)?.ensure_string()?,
        value: node.arg(1)?.ensure_string()?,
    })
}

fn required_if_eq_pairs(node: &NodeHelper<'_>) -> Result<Vec<SpecRequiredIfEq>, UsageErr> {
    let entries = node.args().collect::<Vec<_>>();
    entries
        .chunks_exact(2)
        .map(|pair| {
            Ok(SpecRequiredIfEq {
                selector: pair[0].ensure_string()?,
                value: pair[1].ensure_string()?,
            })
        })
        .collect()
}

fn serialize_selector_list(node: &mut KdlNode, name: &str, selectors: &[String]) {
    if selectors.len() == 1 {
        node.push(string_entry(Some(name), &selectors[0]));
    } else if !selectors.is_empty() {
        let children = node.children_mut().get_or_insert_with(KdlDocument::new);
        let mut relation = KdlNode::new(name);
        for selector in selectors {
            relation.push(string_entry(None, selector));
        }
        children.nodes_mut().push(relation);
    }
}

fn serialize_required_if_eq(node: &mut KdlNode, name: &str, conditions: &[SpecRequiredIfEq]) {
    if conditions.is_empty() {
        return;
    }
    let children = node.children_mut().get_or_insert_with(KdlDocument::new);
    if name == "required_if_eq_all" {
        let mut relation = KdlNode::new(name);
        for condition in conditions {
            relation.push(string_entry(None, &condition.selector));
            relation.push(string_entry(None, &condition.value));
        }
        children.nodes_mut().push(relation);
    } else {
        for condition in conditions {
            let mut relation = KdlNode::new(name);
            relation.push(string_entry(None, &condition.selector));
            relation.push(string_entry(None, &condition.value));
            children.nodes_mut().push(relation);
        }
    }
}

impl From<&str> for SpecArg {
    fn from(input: &str) -> Self {
        let (input, after_double_dash) = input
            .strip_prefix("-- ")
            .map_or((input, false), |rest| (rest, true));
        if let Some(placeholders) = fixed_placeholders(input) {
            let required = placeholders
                .iter()
                .all(|placeholder| placeholder.starts_with('<'));
            let value_names = placeholders
                .iter()
                .map(|placeholder| placeholder[1..placeholder.len() - 1].to_string())
                .collect::<Vec<_>>();
            return SpecArg {
                name: value_names[0].clone(),
                value_names,
                required,
                var: true,
                var_min: Some(placeholders.len()),
                var_max: Some(placeholders.len()),
                double_dash: if after_double_dash {
                    SpecDoubleDashChoices::Required
                } else {
                    SpecDoubleDashChoices::Optional
                },
                ..Default::default()
            };
        }
        let mut arg = SpecArg {
            name: input.to_string(),
            required: true,
            double_dash: if after_double_dash {
                SpecDoubleDashChoices::Required
            } else {
                SpecDoubleDashChoices::Optional
            },
            ..Default::default()
        };
        // Handle trailing ellipsis: "foo..." or "foo…" or "<foo>..." or "[foo]..."
        if let Some(name) = arg
            .name
            .strip_suffix("...")
            .or_else(|| arg.name.strip_suffix("…"))
        {
            arg.var = true;
            arg.name = name.to_string();
        }
        let first = arg.name.chars().next().unwrap_or_default();
        let last = arg.name.chars().last().unwrap_or_default();
        match (first, last) {
            ('[', ']') => {
                arg.name = arg.name[1..arg.name.len() - 1].to_string();
                arg.required = false;
            }
            ('<', '>') => {
                arg.name = arg.name[1..arg.name.len() - 1].to_string();
            }
            _ => {}
        }
        // The single-placeholder shorthand encloses the separator with the value:
        // `[-- target]`. Multi-placeholder canonical output puts it before the
        // placeholders (`-- [START] [END]`) and was handled above.
        if let Some(name) = arg.name.strip_prefix("-- ") {
            arg.double_dash = SpecDoubleDashChoices::Required;
            arg.name = name.to_string();
        }
        // Also handle ellipsis inside brackets: "[args...]" or "<args...>"
        if !arg.var {
            if let Some(name) = arg
                .name
                .strip_suffix("...")
                .or_else(|| arg.name.strip_suffix("…"))
            {
                arg.var = true;
                arg.name = name.to_string();
            }
        }
        arg
    }
}
impl FromStr for SpecArg {
    type Err = UsageErr;
    fn from_str(input: &str) -> std::result::Result<Self, UsageErr> {
        if fixed_placeholders(input.strip_prefix("-- ").unwrap_or(input)).is_some_and(
            |placeholders| {
                placeholders
                    .windows(2)
                    .any(|pair| pair[0].starts_with('<') != pair[1].starts_with('<'))
            },
        ) {
            let message =
                "fixed-arity placeholders must be either all required or all optional".to_string();
            return Err(UsageErr::InvalidInput(
                message,
                (0, input.len()).into(),
                miette::NamedSource::new("argument", input.to_string()),
            ));
        }
        Ok(input.into())
    }
}

/// Return a multi-placeholder declaration without allocating for the overwhelmingly common
/// single-placeholder case.
fn fixed_placeholders(input: &str) -> Option<Vec<&str>> {
    if !input.bytes().any(|byte| byte.is_ascii_whitespace()) {
        return None;
    }
    let placeholders: Vec<_> = input.split_whitespace().collect();
    (placeholders.len() > 1
        && placeholders.iter().all(|placeholder| {
            matches!(
                (placeholder.chars().next(), placeholder.chars().last()),
                (Some('<'), Some('>')) | (Some('['), Some(']'))
            )
        }))
    .then_some(placeholders)
}

/// A clap argument's defaults, as the spec has to record them.
///
/// clap splits a value by the argument's `value_delimiter` before anyone sees it, defaults
/// included — so `default_value = "a,b,c"` with `value_delimiter = ','` is three values, not one.
/// The spec has no delimiter of its own; it has a list, which is the same statement. Recording the
/// joined string instead described a CLI whose default is a single value that its own `choices`
/// forbid, which is how mise's `--fs-events` reached the spec.
#[cfg(feature = "clap")]
pub(crate) fn default_values(arg: &clap::Arg) -> Vec<String> {
    let raw = arg
        .get_default_values()
        .iter()
        .map(|v| v.to_string_lossy().to_string());
    match arg.get_value_delimiter() {
        Some(delimiter) => raw
            .flat_map(|v| {
                v.split(delimiter)
                    .map(|part| part.to_string())
                    .collect::<Vec<_>>()
            })
            .collect(),
        None => raw.collect(),
    }
}

/// Carry clap's value-count range into the spec where the two parsers mean the same thing.
///
/// A positional may accept zero values through its ordinary optionality. A flag
/// with `num_args(0..)` additionally permits a bare occurrence; callers pass
/// `zero_values_supported` only when they also carry that executable policy on
/// the containing flag.
#[cfg(feature = "clap")]
pub(crate) fn value_bounds(source: &clap::Arg, target: &mut SpecArg, zero_values_supported: bool) {
    // clap verifies num_args against raw command-line tokens and only splits each token on the
    // delimiter afterward. Usage splits first and its bounds count the resulting values. Carrying
    // the range would therefore change the contract (for example, two comma-separated tokens can
    // become four Usage values), so leave it unmapped until the spec can distinguish both counts.
    if source.get_value_delimiter().is_some() {
        return;
    }

    let Some(range) = source.get_num_args() else {
        if target.value_names.len() > 1 {
            let arity = target.value_names.len();
            target.var = true;
            target.var_min = Some(arity);
            target.var_max = Some(arity);
        }
        return;
    };
    let min = range.min_values();
    let max = range.max_values();
    if max <= 1 || min == 0 && !zero_values_supported {
        return;
    }

    target.var = true;
    target.var_min = Some(min);
    target.var_max = (max != usize::MAX).then_some(max);
}

/// Value labels that can survive the spec's fixed-arity representation.
///
/// Clap permits several labels beside a ranged `num_args`; usage gives distinct labels only to
/// an exact number of slots. Keep the first display label for a range and let the fidelity report
/// name the loss instead of emitting KDL that cannot be parsed back.
#[cfg(feature = "clap")]
pub(crate) fn value_names_from_clap(source: &clap::Arg) -> Vec<String> {
    let names: Vec<String> = source
        .get_value_names()
        .unwrap_or_default()
        .iter()
        .map(ToString::to_string)
        .collect();
    if names.len() <= 1 {
        return names;
    }
    let mismatched_range = source.get_num_args().is_some_and(|range| {
        range.min_values() != names.len() || range.max_values() != names.len()
    });
    if source.get_value_delimiter().is_some() || mismatched_range {
        names.into_iter().take(1).collect()
    } else {
        names
    }
}

#[cfg(feature = "clap")]
pub(crate) fn choices_from_clap(arg: &clap::Arg) -> Option<SpecChoices> {
    let possible = arg.get_possible_values();
    if possible.is_empty() {
        return None;
    }
    let choices = possible
        .iter()
        .map(|value| value.get_name().to_string())
        .collect();
    let details = possible
        .iter()
        .filter_map(|value| {
            let aliases: Vec<_> = value
                .get_name_and_aliases()
                .skip(1)
                .map(|alias| SpecChoiceAlias {
                    value: alias.to_string(),
                    // clap PossibleValue aliases are always hidden.
                    hide: true,
                })
                .collect();
            let detail = SpecChoice {
                value: value.get_name().to_string(),
                help: value.get_help().map(ToString::to_string),
                hide: value.is_hide_set(),
                aliases,
            };
            (detail.help.is_some() || detail.hide || !detail.aliases.is_empty()).then_some(detail)
        })
        .collect();
    Some(SpecChoices {
        choices,
        details,
        ignore_case: arg.is_ignore_case_set(),
        ..Default::default()
    })
}

#[cfg(feature = "clap")]
impl From<&clap::Arg> for SpecArg {
    fn from(arg: &clap::Arg) -> Self {
        let source = arg;
        let required = arg.is_required_set();
        let help = arg.get_help().map(|s| s.to_string());
        let help_long = arg.get_long_help().map(|s| s.to_string());
        let help_first_line = help.as_ref().map(|s| string::first_line(s));
        let hide = arg.is_hide_set();
        // One byte only, for the reason given on the flag: a wider separator cannot be
        // written back out. `var` below still reads the original, since clap splits on it
        // either way and the field does collect several values.
        let delimiter = arg.get_value_delimiter();
        let recorded_delimiter = delimiter.filter(char::is_ascii);
        let value_terminator = arg.get_value_terminator().map(ToString::to_string);
        let var = matches!(
            arg.get_action(),
            clap::ArgAction::Count | clap::ArgAction::Append
        ) || delimiter.is_some();
        let choices = choices_from_clap(arg);
        let value_names = value_names_from_clap(arg);
        let mut arg = Self {
            name: value_names
                .first()
                .cloned()
                .unwrap_or_else(|| source.get_id().to_string()),
            value_names,
            usage: "".into(),
            required,
            double_dash: if arg.is_last_set() {
                SpecDoubleDashChoices::Required
            } else if arg.is_trailing_var_arg_set() {
                SpecDoubleDashChoices::Automatic
            } else {
                SpecDoubleDashChoices::Optional
            },
            help,
            help_long,
            help_md: None,
            help_first_line,
            var,
            var_max: None,
            var_min: None,
            // clap answers for this one, and the same getter `default_values` already
            // uses just above: a default is split by it, and so is a typed value.
            delimiter: recorded_delimiter,
            allow_negative_numbers: arg.is_allow_negative_numbers_set(),
            value_terminator: None,
            hide,
            hide_default_value: arg.is_hide_default_value_set(),
            hide_env: arg.is_hide_env_set(),
            hide_env_values: arg.is_hide_env_values_set(),
            hide_possible_values: arg.is_hide_possible_values_set(),
            hide_short_help: arg.is_hide_short_help_set(),
            hide_long_help: arg.is_hide_long_help_set(),
            conflicts: Vec::new(),
            requires: Vec::new(),
            required_if: Vec::new(),
            required_if_eq: Vec::new(),
            required_if_eq_all: Vec::new(),
            required_unless: Vec::new(),
            required_unless_all: Vec::new(),
            default: default_values(arg),
            choices: None,
            validate: None,
            validate_error: None,
            effect: None,
            env: None,
            help_heading: arg.get_help_heading().map(|s| s.to_string()),
        };
        arg.choices = choices;

        value_bounds(source, &mut arg, true);
        if arg.var {
            arg.value_terminator = value_terminator;
        }

        arg
    }
}

impl Display for SpecArg {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}", self.usage())
    }
}
impl PartialEq for SpecArg {
    fn eq(&self, other: &Self) -> bool {
        self.name == other.name
    }
}
impl Eq for SpecArg {}
impl Hash for SpecArg {
    fn hash<H: std::hash::Hasher>(&self, state: &mut H) {
        self.name.hash(state);
    }
}

#[cfg(all(test, feature = "validation"))]
mod validation_tests {
    use std::collections::HashMap;

    use crate::{parse, parse::Parser, Spec};

    fn spec() -> Spec {
        r#"
name "ex"
bin "ex"
arg "<port>" validate="int(value) >= 1 && int(value) <= 65535" validate_error="must be a valid port"
        "#
        .parse()
        .unwrap()
    }

    #[test]
    fn validation_round_trips_through_kdl() {
        let spec = spec();
        let kdl = spec.to_string();
        let reparsed: Spec = kdl.parse().unwrap();
        let arg = &reparsed.cmd.args[0];
        assert_eq!(
            arg.validate.as_deref(),
            Some("int(value) >= 1 && int(value) <= 65535")
        );
        assert_eq!(arg.validate_error.as_deref(), Some("must be a valid port"));
    }

    #[test]
    fn invalid_validation_declarations_are_rejected_with_the_spec() {
        let missing_expression = r#"name "demo"
bin "demo"
arg "<port>" validate_error="must be a port"
"#;
        assert!(missing_expression.parse::<Spec>().is_err());

        let invalid_expression = r#"name "demo"
bin "demo"
arg "<port>" validate="int(value) >"
"#;
        assert!(invalid_expression.parse::<Spec>().is_err());
    }

    #[test]
    fn reference_parser_validates_each_raw_value() {
        parse(&spec(), &["ex".to_string(), "9229".to_string()]).unwrap();

        let error = parse(&spec(), &["ex".to_string(), "0".to_string()]).unwrap_err();
        assert!(
            error.to_string().contains("must be a valid port"),
            "{error:?}"
        );

        let variadic: Spec = r#"
name "ex"
bin "ex"
arg "<port>" var=#true validate="int(value) > 0" validate_error="port must be positive"
        "#
        .parse()
        .unwrap();
        let error = parse(
            &variadic,
            &["ex".to_string(), "0".to_string(), "-1".to_string()],
        )
        .unwrap_err()
        .to_string();
        assert_eq!(error.matches("port must be positive").count(), 1, "{error}");
    }

    #[test]
    fn reference_parser_validates_environment_and_default_fallbacks() {
        let spec: Spec = r#"
name "ex"
bin "ex"
arg "[port]" env="PORT" validate="int(value) > 0" validate_error="port must be positive"
flag "--mode" default="bad" {
    arg "<mode>" validate="value == 'good'" validate_error="mode must be good"
}
arg "[ports]..." env="PORTS" var=#true var_max=1 delimiter="," validate="int(value) > 0" validate_error="all ports must be positive"
flag "--levels" env="LEVELS" {
    arg "<level>..." var=#true var_max=1 delimiter="," validate="value == 'good'" validate_error="all levels must be good"
}
flag "--modes" default="good,bad" {
    arg "<mode>..." var=#true var_max=1 delimiter="," validate="value == 'good'" validate_error="all modes must be good"
}
flag "--conditional" {
    default_if "--trigger" "good,bad"
    arg "<conditional>..." var=#true var_max=1 delimiter="," validate="value == 'good'" validate_error="all conditional values must be good"
}
flag "--repeats <repeat>" env="REPEATS" var=#true var_max=1 delimiter=","
flag "--trigger"
        "#
        .parse()
        .unwrap();
        let env = HashMap::from([
            ("PORT".to_string(), "0".to_string()),
            ("PORTS".to_string(), "1,0".to_string()),
            ("LEVELS".to_string(), "good,bad".to_string()),
            ("REPEATS".to_string(), "one,two".to_string()),
        ]);
        let error = Parser::new(&spec)
            .with_env(env)
            .parse(&["ex".to_string(), "--trigger".to_string()])
            .unwrap_err();
        let error = error.to_string();
        assert!(error.contains("port must be positive"), "{error}");
        assert!(error.contains("mode must be good"), "{error}");
        assert!(error.contains("all ports must be positive"), "{error}");
        assert!(error.contains("all levels must be good"), "{error}");
        assert!(error.contains("all modes must be good"), "{error}");
        assert!(
            error.contains("all conditional values must be good"),
            "{error}"
        );
        assert!(
            error.contains("Variadic argument <ports> accepts at most 1 value(s), got 2"),
            "{error}"
        );
        for flag in ["levels", "modes", "conditional", "repeats"] {
            assert!(
                error.contains(&format!(
                    "Variadic flag --{flag} accepts at most 1 value(s), got 2"
                )),
                "{error}"
            );
        }
    }
}

#[cfg(test)]
mod delimiter_tests {
    use crate::Spec;

    #[test]
    fn a_delimiter_has_to_be_one_byte() {
        // Splitting is by byte below the spec. A separator that is one *character* but
        // several bytes has no byte to be, and picking its low one would match the
        // continuation bytes inside unrelated characters — `§` would split `aЧb`. Refused
        // where it is written, which is the derive's rule too.
        for spec in [
            "flag \"--tags <tag>\" var=#true delimiter=\"§\"\n",
            "arg \"[tags]...\" var=#true delimiter=\"、\"\n",
        ] {
            let err = spec.parse::<Spec>().unwrap_err();
            assert!(format!("{err:?}").contains("one byte"), "{err:?}");
        }

        // A clap command may still declare one; clap splits on it by character. The spec
        // cannot say so, and drops it rather than recording a separator it could not write
        // back out — the values still arrive, since `var` is set either way.
        let cmd = clap::Command::new("ex").arg(
            clap::Arg::new("tags")
                .long("tags")
                .value_delimiter('、')
                .action(clap::ArgAction::Set),
        );
        let spec = Spec::from(&cmd);
        let arg = spec.cmd.flags[0].arg.as_ref().unwrap();
        assert_eq!(
            arg.delimiter, None,
            "a separator it cannot write is not recorded"
        );
        assert!(arg.var, "clap still splits, so the values still arrive");
        spec.to_string()
            .parse::<Spec>()
            .expect("what the bridge produces has to parse back");
    }

    #[test]
    fn a_delimiter_round_trips_and_comes_across_from_clap() {
        let spec: Spec = "flag \"--tags <tag>\" var=#true delimiter=\",\"\n"
            .parse()
            .unwrap();
        let arg = spec.cmd.flags[0].arg.as_ref().unwrap();
        assert_eq!(arg.delimiter, Some(','));

        let reparsed: Spec = spec.to_string().parse().unwrap();
        let arg = reparsed.cmd.flags[0].arg.as_ref().unwrap();
        assert_eq!(arg.delimiter, Some(','), "{spec}");

        // clap answers for this one, through the same getter the default splitting
        // already used.
        let cmd = clap::Command::new("ex").arg(
            clap::Arg::new("tags")
                .long("tags")
                .value_delimiter(',')
                .num_args(1..)
                .default_value("a,b"),
        );
        let spec = Spec::from(&cmd);
        let flag = &spec.cmd.flags[0];
        assert_eq!(flag.arg.as_ref().unwrap().delimiter, Some(','));
        // And the default is still recorded split, which is the same statement. On the
        // flag rather than on its argument, which is where the bridge puts a flag's.
        assert_eq!(flag.default, vec!["a", "b"]);
    }

    #[test]
    fn a_single_valued_clap_arg_keeps_its_delimiter() {
        // clap's parser splits whenever a delimiter is set, whatever `num_args` says, so
        // `ArgAction::Set` with `value_delimiter(',')` is one word becoming several — the
        // common spelling. Reading it as single-valued dropped the delimiter and left a
        // CLI whose defaults split and whose typed values did not.
        let cmd = clap::Command::new("ex").arg(
            clap::Arg::new("tags")
                .long("tags")
                .action(clap::ArgAction::Set)
                .value_delimiter(','),
        );
        let spec = Spec::from(&cmd);
        let arg = spec.cmd.flags[0].arg.as_ref().unwrap();
        assert_eq!(arg.delimiter, Some(','));
        // And it says so: a delimiter is the statement that several values can land, so
        // the emitted spec has somewhere to put them and parses back.
        assert!(arg.var, "a delimiter brings `var` with it");
        let _: Spec = spec.to_string().parse().expect("{spec}");
    }

    #[test]
    fn a_single_valued_clap_positional_splits_into_stored_values() {
        // The positional bridge uses `SpecArg::from(&clap::Arg)` directly, unlike a
        // flag. A delimiter therefore has to make that argument variadic here too or
        // parsing validates the split parts and then stores the original unsplit word.
        let cmd = clap::Command::new("ex").arg(
            clap::Arg::new("tags")
                .action(clap::ArgAction::Set)
                .value_delimiter(',')
                .value_parser(["a", "b"]),
        );
        let spec = Spec::from(&cmd);
        let arg = &spec.cmd.args[0];
        assert!(arg.var, "a positional delimiter brings `var` with it");
        assert_eq!(arg.delimiter, Some(','));

        let input = ["ex", "a,b"].map(str::to_string);
        let parsed = crate::parse(&spec, &input).expect("both split values are choices");
        let value = parsed
            .args
            .values()
            .next()
            .expect("the positional was stored");
        assert!(matches!(
            value,
            crate::parse::ParseValue::MultiString(values)
                if values == &["a".to_string(), "b".to_string()]
        ));
    }

    #[test]
    fn a_delimiter_needs_somewhere_to_put_what_it_splits() {
        // Without `var` everything after the first separator would be dropped, silently.
        let err = "flag \"--tags <tag>\" delimiter=\",\"\n"
            .parse::<Spec>()
            .unwrap_err();
        assert!(format!("{err:?}").contains("one value"), "{err:?}");

        let err = "arg \"[tags]\" delimiter=\",\"\n"
            .parse::<Spec>()
            .unwrap_err();
        assert!(format!("{err:?}").contains("one value"), "{err:?}");

        // A flag that takes no value has nothing to split at all.
        let err = "flag \"--quiet\" delimiter=\",\"\n"
            .parse::<Spec>()
            .unwrap_err();
        assert!(format!("{err:?}").contains("takes none"), "{err:?}");

        // One character, or it is not a delimiter.
        let err = "flag \"--tags <tag>\" var=#true delimiter=\"::\"\n"
            .parse::<Spec>()
            .unwrap_err();
        assert!(format!("{err:?}").contains("one character"), "{err:?}");
    }
}

#[cfg(test)]
mod possible_value_tests {
    use clap::builder::PossibleValue;

    #[test]
    fn clap_possible_value_metadata_survives_the_bridge() {
        let command = clap::Command::new("ex").arg(
            clap::Arg::new("color").ignore_case(true).value_parser([
                PossibleValue::new("always")
                    .help("Always use color")
                    .alias("yes"),
                PossibleValue::new("never").hide(true),
            ]),
        );
        let spec = crate::Spec::from(&command);
        let choices = spec.cmd.args[0].choices.as_ref().unwrap();
        assert_eq!(choices.choices, ["always", "never"]);
        assert!(choices.ignore_case);
        assert!(choices.matches("YES"));
        assert_eq!(choices.values(), ["always"]);
        assert_eq!(choices.details[0].help.as_deref(), Some("Always use color"));
        assert!(choices.details[0].aliases[0].hide);
        assert!(choices.details[1].hide);
    }
}

#[cfg(test)]
mod tests {
    use crate::{Spec, SpecArg};
    use insta::assert_snapshot;

    #[test]
    fn test_arg_with_env() {
        let spec = Spec::parse(
            &Default::default(),
            r#"
arg "<input>" env="MY_INPUT" help="Input file"
arg "<output>" env="MY_OUTPUT"
            "#,
        )
        .unwrap();

        assert_snapshot!(spec, @r#"
        arg <input> help="Input file" env=MY_INPUT
        arg <output> env=MY_OUTPUT
        "#);

        let input_arg = spec.cmd.args.iter().find(|a| a.name == "input").unwrap();
        assert_eq!(input_arg.env, Some("MY_INPUT".to_string()));

        let output_arg = spec.cmd.args.iter().find(|a| a.name == "output").unwrap();
        assert_eq!(output_arg.env, Some("MY_OUTPUT".to_string()));
    }

    #[test]
    fn test_arg_with_env_child_node() {
        let spec = Spec::parse(
            &Default::default(),
            r#"
arg "<input>" help="Input file" {
    env "MY_INPUT"
}
arg "<output>" {
    env "MY_OUTPUT"
}
            "#,
        )
        .unwrap();

        assert_snapshot!(spec, @r#"
        arg <input> help="Input file" env=MY_INPUT
        arg <output> env=MY_OUTPUT
        "#);

        let input_arg = spec.cmd.args.iter().find(|a| a.name == "input").unwrap();
        assert_eq!(input_arg.env, Some("MY_INPUT".to_string()));

        let output_arg = spec.cmd.args.iter().find(|a| a.name == "output").unwrap();
        assert_eq!(output_arg.env, Some("MY_OUTPUT".to_string()));
    }

    #[test]
    fn test_arg_variadic_syntax() {
        use crate::SpecArg;

        // Trailing ellipsis with required brackets
        let arg: SpecArg = "<files>...".into();
        assert_eq!(arg.name, "files");
        assert!(arg.var);
        assert!(arg.required);

        // Trailing ellipsis with optional brackets
        let arg: SpecArg = "[files]...".into();
        assert_eq!(arg.name, "files");
        assert!(arg.var);
        assert!(!arg.required);

        // Unicode ellipsis
        let arg: SpecArg = "<files>…".into();
        assert_eq!(arg.name, "files");
        assert!(arg.var);

        let arg: SpecArg = "[files]…".into();
        assert_eq!(arg.name, "files");
        assert!(arg.var);
        assert!(!arg.required);

        // Ellipsis inside brackets: [args...] and <args...>
        let arg: SpecArg = "[args...]".into();
        assert_eq!(arg.name, "args");
        assert!(arg.var);
        assert!(!arg.required);

        let arg: SpecArg = "<args...>".into();
        assert_eq!(arg.name, "args");
        assert!(arg.var);
        assert!(arg.required);

        // Unicode ellipsis inside brackets
        let arg: SpecArg = "[args…]".into();
        assert_eq!(arg.name, "args");
        assert!(arg.var);
        assert!(!arg.required);
    }

    #[test]
    fn fixed_arity_placeholders_round_trip() {
        let spec: Spec = "arg \"<START> <END>\"\n".parse().unwrap();
        let arg = &spec.cmd.args[0];
        assert_eq!(arg.value_names, ["START", "END"]);
        assert_eq!((arg.var_min, arg.var_max), (Some(2), Some(2)));
        assert_eq!(arg.usage, "<START> <END>");

        let reparsed: Spec = spec.to_string().parse().unwrap();
        assert_eq!(reparsed.cmd.args[0].value_names, ["START", "END"]);
    }

    #[test]
    fn fixed_arity_placeholders_reject_mismatched_bounds() {
        let error = "arg \"<START> <END>\" var_min=1 var_max=2\n"
            .parse::<Spec>()
            .unwrap_err();
        assert!(
            format!("{error:?}").contains("require var_min=2 and var_max=2"),
            "{error:?}"
        );
    }

    #[test]
    fn a_single_value_name_replaces_the_display_name() {
        let spec: Spec = "arg \"<input>\" { value_names \"INPUT\" }\n"
            .parse()
            .unwrap();
        let arg = &spec.cmd.args[0];
        assert_eq!(arg.name, "INPUT");
        assert_eq!(arg.usage, "<INPUT>");

        let built = SpecArg::builder()
            .name("input")
            .required(true)
            .value_names(["INPUT"])
            .build();
        assert_eq!(built.name, "INPUT");
        assert_eq!(built.usage, "<INPUT>");
    }

    #[test]
    fn builder_fixed_arity_survives_later_bound_setters() {
        let after = SpecArg::builder()
            .value_names(["START", "END"])
            .var(false)
            .var_min(1)
            .var_max(4)
            .build();
        let before = SpecArg::builder()
            .var(false)
            .var_min(1)
            .var_max(4)
            .value_names(["START", "END"])
            .build();
        for arg in [after, before] {
            assert!(arg.var);
            assert_eq!((arg.var_min, arg.var_max), (Some(2), Some(2)));
            assert_eq!(arg.usage, "[START] [END]");
        }
    }

    #[test]
    fn one_label_with_exact_bounds_renders_each_value_slot() {
        let spec: Spec = "arg \"<item>…\" var_min=2 var_max=2 { value_names \"ITEM\" }\n"
            .parse()
            .unwrap();
        assert_eq!(spec.cmd.args[0].usage, "<ITEM> <ITEM>");
        let reparsed: Spec = spec.to_string().parse().unwrap();
        assert_eq!(reparsed.cmd.args[0].value_names, ["ITEM", "ITEM"]);
        assert_eq!(
            (reparsed.cmd.args[0].var_min, reparsed.cmd.args[0].var_max),
            (Some(2), Some(2))
        );

        let built = SpecArg::builder()
            .value_names(["ITEM"])
            .required(true)
            .var(true)
            .var_min(2)
            .var_max(2)
            .build();
        assert_eq!(built.usage, "<ITEM> <ITEM>");
    }

    #[test]
    fn fixed_arity_placeholders_reject_mixed_requiredness() {
        let error = "arg \"<START> [END]\"\n".parse::<Spec>().unwrap_err();
        assert!(
            format!("{error:?}")
                .contains("fixed-arity placeholders must be either all required or all optional"),
            "{error:?}"
        );
    }

    #[test]
    fn test_arg_child_nodes() {
        let spec = Spec::parse(
            &Default::default(),
            r#"
arg "<environment>" {
    help "Deployment environment"
    choices "dev" "staging" "prod"
}
arg "[services]" {
    help "Services to deploy"
    var #true
    var_min 0
}
            "#,
        )
        .unwrap();

        let env_arg = spec
            .cmd
            .args
            .iter()
            .find(|a| a.name == "environment")
            .unwrap();
        assert_eq!(env_arg.help, Some("Deployment environment".to_string()));
        assert!(env_arg.choices.is_some());

        let svc_arg = spec.cmd.args.iter().find(|a| a.name == "services").unwrap();
        assert_eq!(svc_arg.help, Some("Services to deploy".to_string()));
        assert!(svc_arg.var);
        assert_eq!(svc_arg.var_min, Some(0));
    }

    #[test]
    fn test_arg_long_help_child_node() {
        let spec = Spec::parse(
            &Default::default(),
            r#"
arg "<input>" {
    help "Input file"
    long_help "Extended help text for input"
}
            "#,
        )
        .unwrap();

        let input_arg = spec.cmd.args.iter().find(|a| a.name == "input").unwrap();
        assert_eq!(input_arg.help, Some("Input file".to_string()));
        assert_eq!(
            input_arg.help_long,
            Some("Extended help text for input".to_string())
        );
    }

    #[test]
    fn positional_conflicts_round_trip_without_dropping_members() {
        let spec: Spec = "arg \"[VALUE]\" { conflicts \"--from-file\" \"--stdin\" }\n"
            .parse()
            .unwrap();
        assert_eq!(
            spec.cmd.args[0].conflicts,
            vec!["--from-file".to_string(), "--stdin".to_string()]
        );

        let rendered = spec.to_string();
        let reparsed: Spec = rendered.parse().unwrap();
        assert_eq!(reparsed.cmd.args[0].conflicts, spec.cmd.args[0].conflicts);
    }
}
