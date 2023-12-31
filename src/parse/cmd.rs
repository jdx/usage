use crate::error::UsageErr;
use crate::{Arg, Flag};
use indexmap::IndexMap;
use kdl::{KdlDocument, KdlEntry, KdlNode};

#[derive(Debug, Default)]
pub struct SchemaCmd {
    pub subcommands: IndexMap<String, SchemaCmd>,
    pub args: Vec<Arg>,
    pub flags: Vec<Flag>,
    pub hide: bool,
    pub subcommand_required: bool,
    pub help: Option<String>,
    pub long_help: Option<String>,
    pub name: String,
    pub aliases: Vec<String>,
    pub hidden_aliases: Vec<String>,
    pub before_help: Option<String>,
    pub before_long_help: Option<String>,
    pub after_help: Option<String>,
    pub after_long_help: Option<String>,
}

impl From<&SchemaCmd> for KdlNode {
    fn from(cmd: &SchemaCmd) -> Self {
        let mut node = Self::new("cmd");
        node.entries_mut().push(cmd.name.clone().into());
        if cmd.hide {
            node.entries_mut().push(KdlEntry::new_prop("hide", true));
        }
        if cmd.subcommand_required {
            node.entries_mut()
                .push(KdlEntry::new_prop("subcommand_required", true));
        }
        if !cmd.aliases.is_empty() {
            let mut aliases = KdlNode::new("alias");
            for alias in &cmd.aliases {
                aliases.entries_mut().push(alias.clone().into());
            }
            let children = node.children_mut().get_or_insert_with(KdlDocument::new);
            children.nodes_mut().push(aliases);
        }
        if !cmd.hidden_aliases.is_empty() {
            let mut aliases = KdlNode::new("alias");
            for alias in &cmd.hidden_aliases {
                aliases.entries_mut().push(alias.clone().into());
            }
            aliases.entries_mut().push(KdlEntry::new_prop("hide", true));
            let children = node.children_mut().get_or_insert_with(KdlDocument::new);
            children.nodes_mut().push(aliases);
        }
        for flag in &cmd.flags {
            let children = node.children_mut().get_or_insert_with(KdlDocument::new);
            children.nodes_mut().push(flag.into());
        }
        for arg in &cmd.args {
            let children = node.children_mut().get_or_insert_with(KdlDocument::new);
            children.nodes_mut().push(arg.into());
        }
        for cmd in cmd.subcommands.values() {
            let children = node.children_mut().get_or_insert_with(KdlDocument::new);
            children.nodes_mut().push(cmd.into());
        }
        node
    }
}

impl TryFrom<&KdlNode> for SchemaCmd {
    type Error = UsageErr;
    fn try_from(node: &KdlNode) -> Result<Self, UsageErr> {
        let mut cmd = Self {
            name: node
                .entries()
                .first()
                .expect("no name provided")
                .value()
                .as_string()
                .unwrap()
                .to_string(),
            ..Default::default()
        };
        for entry in node.entries().iter().skip(1) {
            match entry.name().unwrap().to_string().as_str() {
                "help" => cmd.help = entry.value().as_string().map(|s| s.to_string()),
                "long_help" => cmd.long_help = entry.value().as_string().map(|s| s.to_string()),
                "before_help" => cmd.before_help = entry.value().as_string().map(|s| s.to_string()),
                "before_long_help" => {
                    cmd.before_long_help = entry.value().as_string().map(|s| s.to_string())
                }
                "after_help" => cmd.after_help = entry.value().as_string().map(|s| s.to_string()),
                "after_long_help" => {
                    cmd.after_long_help = entry.value().as_string().map(|s| s.to_string())
                }
                "subcommand_required" => cmd.subcommand_required = entry.value().as_bool().unwrap(),
                "hide" => cmd.hide = entry.value().as_bool().unwrap(),
                _ => Err(UsageErr::InvalidInput(
                    entry.to_string(),
                    *entry.span(),
                    node.to_string(),
                ))?,
            }
        }
        for child in node.children().map(|c| c.nodes()).unwrap_or_default() {
            match child.name().to_string().as_str() {
                "flag" => cmd.flags.push(child.try_into()?),
                "arg" => cmd.args.push(child.try_into()?),
                "cmd" => {
                    let node: SchemaCmd = child.try_into()?;
                    cmd.subcommands.insert(node.name.to_string(), node);
                }
                "alias" => {
                    let alias = child
                        .entries()
                        .iter()
                        .filter_map(|e| e.value().as_string().map(|v| v.to_string()))
                        .collect::<Vec<_>>();
                    if child
                        .get("hide")
                        .is_some_and(|n| n.value().as_bool().unwrap())
                    {
                        cmd.hidden_aliases.extend(alias);
                    } else {
                        cmd.aliases.extend(alias);
                    }
                }
                _ => Err(UsageErr::InvalidInput(
                    child.to_string(),
                    *child.span(),
                    node.to_string(),
                ))?,
            }
        }
        Ok(cmd)
    }
}

#[cfg(feature = "clap")]
impl From<&clap::Command> for SchemaCmd {
    fn from(cmd: &clap::Command) -> Self {
        let mut spec = Self {
            name: cmd.get_name().to_string(),
            hide: cmd.is_hide_set(),
            help: cmd.get_about().map(|s| s.to_string()),
            long_help: cmd.get_long_about().map(|s| s.to_string()),
            before_help: cmd.get_before_help().map(|s| s.to_string()),
            before_long_help: cmd.get_before_long_help().map(|s| s.to_string()),
            after_help: cmd.get_after_help().map(|s| s.to_string()),
            after_long_help: cmd.get_after_long_help().map(|s| s.to_string()),
            ..Default::default()
        };
        for alias in cmd.get_visible_aliases() {
            spec.aliases.push(alias.to_string());
        }
        for alias in cmd.get_all_aliases() {
            if spec.aliases.contains(&alias.to_string()) {
                continue;
            }
            spec.hidden_aliases.push(alias.to_string());
        }
        for arg in cmd.get_arguments() {
            if arg.is_positional() {
                spec.args.push(arg.into())
            } else {
                spec.flags.push(arg.into())
            }
        }
        spec.subcommand_required = cmd.is_subcommand_required_set();
        for subcmd in cmd.get_subcommands() {
            let mut scmd: SchemaCmd = subcmd.into();
            scmd.name = subcmd.get_name().to_string();
            spec.subcommands.insert(scmd.name.clone(), scmd);
        }
        spec
    }
}

#[cfg(feature = "clap")]
impl From<&SchemaCmd> for clap::Command {
    fn from(cmd: &SchemaCmd) -> Self {
        let mut app = Self::new(cmd.name.to_string());
        if let Some(help) = &cmd.help {
            app = app.about(help);
        }
        if let Some(help) = &cmd.long_help {
            app = app.long_about(help);
        }
        if let Some(help) = &cmd.before_help {
            app = app.before_help(help);
        }
        if let Some(help) = &cmd.before_long_help {
            app = app.before_long_help(help);
        }
        if let Some(help) = &cmd.after_help {
            app = app.after_help(help);
        }
        if let Some(help) = &cmd.after_long_help {
            app = app.after_long_help(help);
        }
        if cmd.subcommand_required {
            app = app.subcommand_required(true);
        }
        if cmd.hide {
            app = app.hide(true);
        }
        for alias in &cmd.aliases {
            app = app.visible_alias(alias);
        }
        for alias in &cmd.hidden_aliases {
            app = app.alias(alias);
        }
        for arg in &cmd.args {
            app = app.arg(arg);
        }
        for flag in &cmd.flags {
            app = app.arg(flag);
        }
        for subcmd in cmd.subcommands.values() {
            app = app.subcommand(subcmd);
        }
        app
    }
}
