use std::str::FromStr;

use itertools::Itertools;
use kdl::{KdlDocument, KdlEntry, KdlNode};

use crate::error::UsageErr;
use crate::error::UsageErr::InvalidFlag;
use crate::parse::helpers::NodeHelper;
use crate::{bail_parse, Arg};

#[derive(Debug, Default)]
pub struct Flag {
    pub name: String,
    pub help: Option<String>,
    pub long_help: Option<String>,
    pub short: Vec<char>,
    pub long: Vec<String>,
    pub required: bool,
    pub var: bool,
    pub hide: bool,
    pub global: bool,
    pub count: bool,
    pub arg: Option<Arg>,
}

impl Flag {
    pub fn usage(&self) -> String {
        let mut name = self
            .short
            .iter()
            .map(|c| format!("-{c}"))
            .chain(self.long.iter().map(|s| format!("--{s}")))
            .collect::<Vec<_>>()
            .join(",");
        if let Some(arg) = &self.arg {
            name = format!("{} {}", name, arg.usage());
        }
        name
    }
}

impl From<&Flag> for KdlNode {
    fn from(flag: &Flag) -> KdlNode {
        let mut node = KdlNode::new("flag");
        let name = flag
            .short
            .iter()
            .map(|c| format!("-{c}"))
            .chain(flag.long.iter().map(|s| format!("--{s}")))
            .collect::<Vec<_>>()
            .join(",");
        node.push(KdlEntry::new(name));
        if let Some(desc) = &flag.help {
            node.push(KdlEntry::new_prop("help", desc.clone()));
        }
        if let Some(desc) = &flag.long_help {
            node.push(KdlEntry::new_prop("long_help", desc.clone()));
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
        if let Some(arg) = &flag.arg {
            let children = node.children_mut().get_or_insert_with(KdlDocument::new);
            children.nodes_mut().push(arg.into());
        }
        node
    }
}

impl TryFrom<&KdlNode> for Flag {
    type Error = UsageErr;
    fn try_from(node: &KdlNode) -> Result<Self, UsageErr> {
        let hnode: NodeHelper = node.into();
        let mut flag: Self = hnode.arg(0)?.ensure_string()?.parse()?;
        for (k, v) in hnode.props() {
            match k {
                "help" => flag.help = Some(v.ensure_string()?),
                "long_help" => flag.long_help = Some(v.ensure_string()?),
                "required" => flag.required = v.ensure_bool()?,
                "var" => flag.var = v.ensure_bool()?,
                "hide" => flag.hide = v.ensure_bool()?,
                "global" => flag.global = v.ensure_bool()?,
                "count" => flag.count = v.ensure_bool()?,
                k => bail_parse!(v.entry, "unsupported key {k}"),
            }
        }
        let children = node.children().map(|c| c.nodes()).unwrap_or_default();
        for child in children {
            let child: NodeHelper = child.into();
            match child.name() {
                "arg" => flag.arg = Some(child.node.try_into()?),
                k => bail_parse!(child.node, "unsupported key {k}"),
            }
        }
        Ok(flag)
    }
}

impl FromStr for Flag {
    type Err = UsageErr;
    fn from_str(input: &str) -> std::result::Result<Self, UsageErr> {
        let mut flag = Self::default();
        let (names, val) = input.split_once(' ').unwrap_or((input, ""));
        for (i, n) in names.split(',').enumerate() {
            if i == 0 {
                flag.name = n.trim_start_matches('-').to_string();
            }
            if n.starts_with("--") {
                flag.long.push(n.trim_start_matches('-').to_string());
            } else if n.starts_with('-') {
                flag.short.extend(n.trim_start_matches('-').chars());
            } else {
                let span = (0, names.len());
                return Err(InvalidFlag(n.to_string(), span.into(), input.to_string()));
            }
        }
        if !val.is_empty() {
            flag.arg = Some(val.parse()?);
        }
        Ok(flag)
    }
}

#[cfg(feature = "clap")]
impl From<&clap::Arg> for Flag {
    fn from(c: &clap::Arg) -> Self {
        let required = c.is_required_set();
        let help = c.get_help().map(|s| s.to_string());
        let long_help = c.get_long_help().map(|s| s.to_string());
        let hide = c.is_hide_set();
        let var = matches!(
            c.get_action(),
            clap::ArgAction::Count | clap::ArgAction::Append
        );
        let short = c.get_short_and_visible_aliases().unwrap_or_default();
        let long = c
            .get_long_and_visible_aliases()
            .unwrap_or_default()
            .into_iter()
            .map(|s| s.to_string())
            .collect::<Vec<_>>();
        let name = long
            .first()
            .map(|s| s.to_string())
            .unwrap_or(short.first().cloned().unwrap_or_default().to_string());
        let arg = if let clap::ArgAction::Set | clap::ArgAction::Append = c.get_action() {
            let arg = c
                .get_value_names()
                .map(|s| s.iter().map(|s| s.to_string()).join(","))
                .unwrap_or(name.clone())
                .as_str()
                .into();
            Some(arg)
        } else {
            None
        };
        Self {
            name,
            short,
            long,
            required,
            help,
            long_help,
            var,
            hide,
            global: c.is_global_set(),
            arg,
            count: matches!(c.get_action(), clap::ArgAction::Count),
        }
    }
}

#[cfg(feature = "clap")]
impl From<&Flag> for clap::Arg {
    fn from(flag: &Flag) -> Self {
        let mut a = clap::Arg::new(&flag.name);
        if let Some(desc) = &flag.help {
            a = a.help(desc);
        }
        if flag.required {
            a = a.required(true);
        }
        if let Some(arg) = &flag.arg {
            a = a.value_name(&arg.name);
            if arg.var {
                a = a.action(clap::ArgAction::Append)
            } else {
                a = a.action(clap::ArgAction::Set)
            }
        } else {
            a = a.action(clap::ArgAction::SetTrue)
        }
        // let mut a = clap::Arg::new(&flag.name)
        //     .required(flag.required)
        //     .action(clap::ArgAction::SetTrue);
        if let Some(short) = flag.short.first() {
            a = a.short(*short);
        }
        if let Some(long) = flag.long.first() {
            a = a.long(long);
        }
        for short in flag.short.iter().skip(1) {
            a = a.visible_short_alias(*short);
        }
        for long in flag.long.iter().skip(1) {
            a = a.visible_alias(long);
        }
        // cmd = cmd.arg(a);
        // if flag.multiple {
        //     a = a.multiple(true);
        // }
        // if flag.hide {
        //     a = a.hide_possible_values(true);
        // }
        a
    }
}
