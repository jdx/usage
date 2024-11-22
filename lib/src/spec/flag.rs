use itertools::Itertools;
use kdl::{KdlDocument, KdlEntry, KdlNode};
use serde::Serialize;
use std::fmt::Display;
use std::hash::Hash;
use std::str::FromStr;

use crate::error::UsageErr::InvalidFlag;
use crate::error::{Result, UsageErr};
use crate::spec::context::ParsingContext;
use crate::spec::helpers::NodeHelper;
use crate::{string, SpecArg, SpecChoices};

#[derive(Debug, Default, Clone, Serialize)]
pub struct SpecFlag {
    pub name: String,
    pub usage: String,
    pub help: Option<String>,
    pub help_long: Option<String>,
    pub help_md: Option<String>,
    pub help_first_line: Option<String>,
    pub short: Vec<char>,
    pub long: Vec<String>,
    pub required: bool,
    pub deprecated: Option<String>,
    pub var: bool,
    pub hide: bool,
    pub global: bool,
    pub count: bool,
    pub arg: Option<SpecArg>,
    pub default: Option<String>,
    pub negate: Option<String>,
}

impl SpecFlag {
    pub(crate) fn parse(ctx: &ParsingContext, node: &NodeHelper) -> Result<Self> {
        let mut flag: Self = node.arg(0)?.ensure_string()?.parse()?;
        for (k, v) in node.props() {
            match k {
                "help" => flag.help = Some(v.ensure_string()?),
                "long_help" => flag.help_long = Some(v.ensure_string()?),
                "help_long" => flag.help_long = Some(v.ensure_string()?),
                "help_md" => flag.help_md = Some(v.ensure_string()?),
                "required" => flag.required = v.ensure_bool()?,
                "var" => flag.var = v.ensure_bool()?,
                "hide" => flag.hide = v.ensure_bool()?,
                "deprecated" => {
                    flag.deprecated = match v.value.as_bool() {
                        Some(true) => Some("deprecated".into()),
                        Some(false) => None,
                        None => Some(v.ensure_string()?),
                    }
                }
                "global" => flag.global = v.ensure_bool()?,
                "count" => flag.count = v.ensure_bool()?,
                "default" => flag.default = v.ensure_string().map(Some)?,
                "negate" => flag.negate = v.ensure_string().map(Some)?,
                k => bail_parse!(ctx, *v.entry.span(), "unsupported flag key {k}"),
            }
        }
        if flag.default.is_some() {
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
                "var" => flag.var = child.arg(0)?.ensure_bool()?,
                "hide" => flag.hide = child.arg(0)?.ensure_bool()?,
                "deprecated" => {
                    flag.deprecated = match child.arg(0)?.ensure_bool() {
                        Ok(true) => Some("deprecated".into()),
                        Ok(false) => None,
                        _ => Some(child.arg(0)?.ensure_string()?),
                    }
                }
                "global" => flag.global = child.arg(0)?.ensure_bool()?,
                "count" => flag.count = child.arg(0)?.ensure_bool()?,
                "default" => flag.default = child.arg(0)?.ensure_string().map(Some)?,
                "choices" => {
                    if let Some(arg) = &mut flag.arg {
                        arg.choices = Some(SpecChoices::parse(ctx, &child)?);
                    } else {
                        bail_parse!(
                            ctx,
                            *child.node.name().span(),
                            "flag must have value to have choices"
                        )
                    }
                }
                k => bail_parse!(ctx, *child.node.name().span(), "unsupported flag child {k}"),
            }
        }
        flag.usage = flag.usage();
        flag.help_first_line = flag.help.as_ref().map(|s| string::first_line(s));
        Ok(flag)
    }
    pub fn usage(&self) -> String {
        let mut parts = vec![];
        let name = get_name_from_short_and_long(&self.short, &self.long).unwrap_or_default();
        if name != self.name {
            parts.push(format!("{}:", self.name));
        }
        if let Some(short) = self.short.first() {
            parts.push(format!("-{}", short));
        }
        if let Some(long) = self.long.first() {
            parts.push(format!("--{}", long));
        }
        let mut out = parts.join(" ");
        if self.var {
            out = format!("{}...", out);
        }
        if let Some(arg) = &self.arg {
            out = format!("{} {}", out, arg.usage());
        }
        out
    }
}

impl From<&SpecFlag> for KdlNode {
    fn from(flag: &SpecFlag) -> KdlNode {
        let mut node = KdlNode::new("flag");
        let name = flag
            .short
            .iter()
            .map(|c| format!("-{c}"))
            .chain(flag.long.iter().map(|s| format!("--{s}")))
            .collect_vec()
            .join(" ");
        node.push(KdlEntry::new(name));
        if let Some(desc) = &flag.help {
            node.push(KdlEntry::new_prop("help", desc.clone()));
        }
        if let Some(desc) = &flag.help_long {
            let children = node.children_mut().get_or_insert_with(KdlDocument::new);
            let mut node = KdlNode::new("long_help");
            node.entries_mut().push(KdlEntry::new(desc.clone()));
            children.nodes_mut().push(node);
        }
        if let Some(desc) = &flag.help_md {
            let children = node.children_mut().get_or_insert_with(KdlDocument::new);
            let mut node = KdlNode::new("help_md");
            node.entries_mut().push(KdlEntry::new(desc.clone()));
            children.nodes_mut().push(node);
        }
        if flag.required {
            node.push(KdlEntry::new_prop("required", true));
        }
        if flag.var {
            node.push(KdlEntry::new_prop("var", true));
        }
        if flag.hide {
            node.push(KdlEntry::new_prop("hide", true));
        }
        if flag.global {
            node.push(KdlEntry::new_prop("global", true));
        }
        if flag.count {
            node.push(KdlEntry::new_prop("count", true));
        }
        if let Some(negate) = &flag.negate {
            node.push(KdlEntry::new_prop("negate", negate.clone()));
        }
        if let Some(deprecated) = &flag.deprecated {
            node.push(KdlEntry::new_prop("deprecated", deprecated.clone()));
        }
        if let Some(arg) = &flag.arg {
            let children = node.children_mut().get_or_insert_with(KdlDocument::new);
            children.nodes_mut().push(arg.into());
        }
        node
    }
}

impl FromStr for SpecFlag {
    type Err = UsageErr;
    fn from_str(input: &str) -> Result<Self> {
        let mut flag = Self::default();
        let input = input.replace("...", " ... ");
        for part in input.split_whitespace() {
            if let Some(name) = part.strip_suffix(':') {
                flag.name = name.to_string();
            } else if let Some(long) = part.strip_prefix("--") {
                flag.long.push(long.to_string());
            } else if let Some(short) = part.strip_prefix('-') {
                if short.len() != 1 {
                    return Err(InvalidFlag(
                        short.to_string(),
                        (0, input.len()).into(),
                        input.to_string(),
                    ));
                }
                flag.short.push(short.chars().next().unwrap());
            } else if part == "..." {
                if let Some(arg) = &mut flag.arg {
                    arg.var = true;
                } else {
                    flag.var = true;
                }
            } else if part.starts_with('<') && part.ends_with('>')
                || part.starts_with('[') && part.ends_with(']')
            {
                flag.arg = Some(part.to_string().parse()?);
            } else {
                return Err(InvalidFlag(
                    part.to_string(),
                    (0, input.len()).into(),
                    input.to_string(),
                ));
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
        let default = c
            .get_default_values()
            .first()
            .map(|s| s.to_string_lossy().to_string());
        let short = c.get_short_and_visible_aliases().unwrap_or_default();
        let long = c
            .get_long_and_visible_aliases()
            .unwrap_or_default()
            .into_iter()
            .map(|s| s.to_string())
            .collect::<Vec<_>>();
        let name = get_name_from_short_and_long(&short, &long).unwrap_or_default();
        let arg = if let clap::ArgAction::Set | clap::ArgAction::Append = c.get_action() {
            let mut arg = SpecArg::from(
                c.get_value_names()
                    .map(|s| s.iter().map(|s| s.to_string()).join(" "))
                    .unwrap_or(name.clone())
                    .as_str(),
            );

            let choices = c
                .get_possible_values()
                .iter()
                .flat_map(|v| v.get_name_and_aliases().map(|s| s.to_string()))
                .collect::<Vec<_>>();
            if !choices.is_empty() {
                arg.choices = Some(SpecChoices { choices });
            }

            Some(arg)
        } else {
            None
        };
        Self {
            name,
            usage: "".into(),
            short,
            long,
            required,
            help,
            help_long,
            help_md: None,
            help_first_line,
            var,
            hide,
            global: c.is_global_set(),
            arg,
            count: matches!(c.get_action(), clap::ArgAction::Count),
            default,
            deprecated: None,
            negate: None,
        }
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
    use insta::assert_snapshot;

    #[test]
    fn from_str() {
        assert_snapshot!("-f".parse::<SpecFlag>().unwrap(), @"-f");
        assert_snapshot!("--flag".parse::<SpecFlag>().unwrap(), @"--flag");
        assert_snapshot!("-f --flag".parse::<SpecFlag>().unwrap(), @"-f --flag");
        assert_snapshot!("-f --flag...".parse::<SpecFlag>().unwrap(), @"-f --flag...");
        assert_snapshot!("-f --flag ...".parse::<SpecFlag>().unwrap(), @"-f --flag...");
        assert_snapshot!("--flag <arg>".parse::<SpecFlag>().unwrap(), @"--flag <arg>");
        assert_snapshot!("-f --flag <arg>".parse::<SpecFlag>().unwrap(), @"-f --flag <arg>");
        assert_snapshot!("-f --flag... <arg>".parse::<SpecFlag>().unwrap(), @"-f --flag... <arg>");
        assert_snapshot!("-f --flag <arg>...".parse::<SpecFlag>().unwrap(), @"-f --flag <arg>...");
        assert_snapshot!("myflag: -f".parse::<SpecFlag>().unwrap(), @"myflag: -f");
        assert_snapshot!("myflag: -f --flag <arg>".parse::<SpecFlag>().unwrap(), @"myflag: -f --flag <arg>");
    }
}
