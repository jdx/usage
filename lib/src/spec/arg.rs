#[cfg(feature = "clap")]
use itertools::Itertools;
use kdl::{KdlDocument, KdlEntry, KdlNode};
use serde::Serialize;
use std::fmt::Display;
use std::hash::Hash;
use std::str::FromStr;

use crate::error::UsageErr;
use crate::spec::context::ParsingContext;
use crate::spec::helpers::NodeHelper;
use crate::spec::is_false;
use crate::{string, SpecChoices};

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
}

#[derive(Debug, Default, Clone, Serialize)]
pub struct SpecArg {
    pub name: String,
    pub usage: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub help: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub help_long: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub help_md: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub help_first_line: Option<String>,
    pub required: bool,
    pub double_dash: SpecDoubleDashChoices,
    #[serde(skip_serializing_if = "is_false")]
    pub var: bool,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub var_min: Option<usize>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub var_max: Option<usize>,
    pub hide: bool,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub default: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub choices: Option<SpecChoices>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub env: Option<String>,
}

impl SpecArg {
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
                "hide" => arg.hide = v.ensure_bool()?,
                "var_min" => arg.var_min = v.ensure_usize().map(Some)?,
                "var_max" => arg.var_max = v.ensure_usize().map(Some)?,
                "default" => arg.default = v.ensure_string().map(Some)?,
                "env" => arg.env = v.ensure_string().map(Some)?,
                k => bail_parse!(ctx, v.entry.span(), "unsupported arg key {k}"),
            }
        }
        if arg.default.is_some() {
            arg.required = false;
        }
        for child in node.children() {
            match child.name() {
                "choices" => arg.choices = Some(SpecChoices::parse(ctx, &child)?),
                "env" => arg.env = child.arg(0)?.ensure_string().map(Some)?,
                k => bail_parse!(ctx, child.node.name().span(), "unsupported arg child {k}"),
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
            node.push(KdlEntry::new_prop("help", desc.clone()));
        }
        if let Some(desc) = &arg.help_long {
            node.push(KdlEntry::new_prop("help_long", desc.clone()));
        }
        if let Some(desc) = &arg.help_md {
            node.push(KdlEntry::new_prop("help_md", desc.clone()));
        }
        if !arg.required {
            node.push(KdlEntry::new_prop("required", false));
        }
        if arg.double_dash == SpecDoubleDashChoices::Automatic {
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
        if arg.hide {
            node.push(KdlEntry::new_prop("hide", true));
        }
        if let Some(default) = &arg.default {
            node.push(KdlEntry::new_prop("default", default.clone()));
        }
        if let Some(env) = &arg.env {
            node.push(KdlEntry::new_prop("env", env.clone()));
        }
        if let Some(choices) = &arg.choices {
            let children = node.children_mut().get_or_insert_with(KdlDocument::new);
            children.nodes_mut().push(choices.into());
        }
        node
    }
}

impl From<&str> for SpecArg {
    fn from(input: &str) -> Self {
        let mut arg = SpecArg {
            name: input.to_string(),
            required: true,
            ..Default::default()
        };
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
        if let Some(name) = arg.name.strip_prefix("-- ") {
            arg.double_dash = SpecDoubleDashChoices::Required;
            arg.name = name.to_string();
        }
        arg
    }
}
impl FromStr for SpecArg {
    type Err = UsageErr;
    fn from_str(input: &str) -> std::result::Result<Self, UsageErr> {
        Ok(input.into())
    }
}

#[cfg(feature = "clap")]
impl From<&clap::Arg> for SpecArg {
    fn from(arg: &clap::Arg) -> Self {
        let required = arg.is_required_set();
        let help = arg.get_help().map(|s| s.to_string());
        let help_long = arg.get_long_help().map(|s| s.to_string());
        let help_first_line = help.as_ref().map(|s| string::first_line(s));
        let hide = arg.is_hide_set();
        let var = matches!(
            arg.get_action(),
            clap::ArgAction::Count | clap::ArgAction::Append
        );
        let choices = arg
            .get_possible_values()
            .iter()
            .flat_map(|v| v.get_name_and_aliases().map(|s| s.to_string()))
            .collect::<Vec<_>>();
        let mut arg = Self {
            name: arg
                .get_value_names()
                .unwrap_or_default()
                .first()
                .cloned()
                .unwrap_or_default()
                .to_string(),
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
            hide,
            default: if arg.get_default_values().is_empty() {
                None
            } else {
                Some(
                    arg.get_default_values()
                        .iter()
                        .map(|v| v.to_string_lossy().to_string())
                        .join("|"),
                )
            },
            choices: None,
            env: None,
        };
        if !choices.is_empty() {
            arg.choices = Some(SpecChoices { choices });
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

#[cfg(test)]
mod tests {
    use crate::Spec;
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
}
