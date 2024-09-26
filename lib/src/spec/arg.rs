use std::fmt::Display;
use std::hash::Hash;
use std::str::FromStr;

#[cfg(feature = "clap")]
use itertools::Itertools;
use kdl::{KdlEntry, KdlNode};
use serde::Serialize;

use crate::error::UsageErr;
use crate::spec::context::ParsingContext;
use crate::spec::helpers::NodeHelper;

#[derive(Debug, Default, Clone, Serialize)]
pub struct SpecArg {
    pub name: String,
    pub usage: String,
    pub help: Option<String>,
    pub long_help: Option<String>,
    pub required: bool,
    pub var: bool,
    pub var_min: Option<usize>,
    pub var_max: Option<usize>,
    pub hide: bool,
    pub default: Option<String>,
}

impl SpecArg {
    pub(crate) fn parse(ctx: &ParsingContext, node: &NodeHelper) -> Result<Self, UsageErr> {
        let mut arg: SpecArg = node.arg(0)?.ensure_string()?.parse()?;
        for (k, v) in node.props() {
            match k {
                "help" => arg.help = Some(v.ensure_string()?),
                "long_help" => arg.long_help = Some(v.ensure_string()?),
                "required" => arg.required = v.ensure_bool()?,
                "var" => arg.var = v.ensure_bool()?,
                "hide" => arg.hide = v.ensure_bool()?,
                "var_min" => arg.var_min = v.ensure_usize().map(Some)?,
                "var_max" => arg.var_max = v.ensure_usize().map(Some)?,
                "default" => arg.default = v.ensure_string().map(Some)?,
                k => bail_parse!(ctx, *v.entry.span(), "unsupported arg key {k}"),
            }
        }
        arg.usage = arg.usage();
        Ok(arg)
    }
}

impl SpecArg {
    pub(crate) fn usage(&self) -> String {
        let mut name = if self.required {
            format!("<{}>", self.name)
        } else {
            format!("[{}]", self.name)
        };
        if self.var {
            name = format!("{}...", name);
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
        if let Some(desc) = &arg.long_help {
            node.push(KdlEntry::new_prop("long_help", desc.clone()));
        }
        if arg.var {
            node.push(KdlEntry::new_prop("var", true));
        }
        if let Some(min) = arg.var_min {
            node.push(KdlEntry::new_prop("var_min", min as i64));
        }
        if let Some(max) = arg.var_max {
            node.push(KdlEntry::new_prop("var_max", max as i64));
        }
        if arg.hide {
            node.push(KdlEntry::new_prop("hide", true));
        }
        if let Some(default) = &arg.default {
            node.push(KdlEntry::new_prop("default", default.clone()));
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
        if input.strip_suffix("...").is_some() {
            arg.var = true;
            arg.name = arg.name[..arg.name.len() - 3].to_string();
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
        let long_help = arg.get_long_help().map(|s| s.to_string());
        let hide = arg.is_hide_set();
        let var = matches!(
            arg.get_action(),
            clap::ArgAction::Count | clap::ArgAction::Append
        );
        Self {
            name: arg
                .get_value_names()
                .unwrap_or_default()
                .first()
                .cloned()
                .unwrap_or_default()
                .to_string(),
            usage: "".into(),
            required,
            help,
            long_help,
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
        }
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
