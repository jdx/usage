use std::collections::HashMap;
use std::sync::OnceLock;

use crate::error::UsageErr;
use crate::sh::sh;
use crate::spec::context::ParsingContext;
use crate::spec::helpers::NodeHelper;
use crate::spec::is_false;
use crate::spec::mount::SpecMount;
use crate::{Spec, SpecArg, SpecComplete, SpecFlag};
use indexmap::IndexMap;
use itertools::Itertools;
use kdl::{KdlDocument, KdlEntry, KdlNode, KdlValue};
use serde::Serialize;

#[derive(Debug, Serialize, Clone)]
pub struct SpecCommand {
    pub full_cmd: Vec<String>,
    pub usage: String,
    pub subcommands: IndexMap<String, SpecCommand>,
    pub args: Vec<SpecArg>,
    pub flags: Vec<SpecFlag>,
    pub mounts: Vec<SpecMount>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub deprecated: Option<String>,
    pub hide: bool,
    #[serde(skip_serializing_if = "is_false")]
    pub subcommand_required: bool,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub help: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub help_long: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub help_md: Option<String>,
    pub name: String,
    pub aliases: Vec<String>,
    pub hidden_aliases: Vec<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub before_help: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub before_help_long: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub before_help_md: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub after_help: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub after_help_long: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub after_help_md: Option<String>,
    pub examples: Vec<SpecExample>,
    #[serde(skip_serializing_if = "IndexMap::is_empty")]
    pub complete: IndexMap<String, SpecComplete>,

    // TODO: make this non-public
    #[serde(skip)]
    subcommand_lookup: OnceLock<HashMap<String, String>>,
}

impl Default for SpecCommand {
    fn default() -> Self {
        Self {
            full_cmd: vec![],
            usage: "".to_string(),
            subcommands: IndexMap::new(),
            args: vec![],
            flags: vec![],
            mounts: vec![],
            deprecated: None,
            hide: false,
            subcommand_required: false,
            help: None,
            help_long: None,
            help_md: None,
            name: "".to_string(),
            aliases: vec![],
            hidden_aliases: vec![],
            before_help: None,
            before_help_long: None,
            before_help_md: None,
            after_help: None,
            after_help_long: None,
            after_help_md: None,
            examples: vec![],
            subcommand_lookup: OnceLock::new(),
            complete: IndexMap::new(),
        }
    }
}

#[derive(Debug, Default, Serialize, Clone)]
pub struct SpecExample {
    pub code: String,
    pub header: Option<String>,
    pub help: Option<String>,
    pub lang: String,
}

impl SpecExample {
    pub(crate) fn new(code: String) -> Self {
        Self {
            code,
            ..Default::default()
        }
    }
}

impl SpecCommand {
    pub(crate) fn parse(ctx: &ParsingContext, node: &NodeHelper) -> Result<Self, UsageErr> {
        node.ensure_arg_len(1..=1)?;
        let mut cmd = Self {
            name: node.arg(0)?.ensure_string()?.to_string(),
            ..Default::default()
        };
        for (k, v) in node.props() {
            match k {
                "help" => cmd.help = Some(v.ensure_string()?),
                "long_help" => cmd.help_long = Some(v.ensure_string()?),
                "help_long" => cmd.help_long = Some(v.ensure_string()?),
                "help_md" => cmd.help_md = Some(v.ensure_string()?),
                "before_help" => cmd.before_help = Some(v.ensure_string()?),
                "before_long_help" => cmd.before_help_long = Some(v.ensure_string()?),
                "before_help_long" => cmd.before_help_long = Some(v.ensure_string()?),
                "before_help_md" => cmd.before_help_md = Some(v.ensure_string()?),
                "after_help" => cmd.after_help = Some(v.ensure_string()?),
                "after_long_help" => {
                    cmd.after_help_long = Some(v.ensure_string()?);
                }
                "after_help_long" => {
                    cmd.after_help_long = Some(v.ensure_string()?);
                }
                "after_help_md" => cmd.after_help_md = Some(v.ensure_string()?),
                "subcommand_required" => cmd.subcommand_required = v.ensure_bool()?,
                "hide" => cmd.hide = v.ensure_bool()?,
                "deprecated" => {
                    cmd.deprecated = match v.value.as_bool() {
                        Some(true) => Some("deprecated".to_string()),
                        Some(false) => None,
                        None => Some(v.ensure_string()?),
                    }
                }
                k => bail_parse!(ctx, v.entry.span(), "unsupported cmd prop {k}"),
            }
        }
        for child in node.children() {
            match child.name() {
                "flag" => cmd.flags.push(SpecFlag::parse(ctx, &child)?),
                "arg" => cmd.args.push(SpecArg::parse(ctx, &child)?),
                "mount" => cmd.mounts.push(SpecMount::parse(ctx, &child)?),
                "cmd" => {
                    let node = SpecCommand::parse(ctx, &child)?;
                    cmd.subcommands.insert(node.name.to_string(), node);
                }
                "alias" => {
                    let alias = child
                        .ensure_arg_len(1..)?
                        .args()
                        .map(|e| e.ensure_string())
                        .collect::<Result<Vec<_>, _>>()?;
                    let hide = child
                        .get("hide")
                        .map(|n| n.ensure_bool())
                        .unwrap_or(Ok(false))?;
                    if hide {
                        cmd.hidden_aliases.extend(alias);
                    } else {
                        cmd.aliases.extend(alias);
                    }
                }
                "example" => {
                    let code = child.ensure_arg_len(1..=1)?.arg(0)?.ensure_string()?;
                    let mut example = SpecExample::new(code.trim().to_string());
                    for (k, v) in child.props() {
                        match k {
                            "header" => example.header = Some(v.ensure_string()?),
                            "help" => example.help = Some(v.ensure_string()?),
                            "lang" => example.lang = v.ensure_string()?,
                            k => bail_parse!(ctx, v.entry.span(), "unsupported example key {k}"),
                        }
                    }
                    cmd.examples.push(example);
                }
                "help" => {
                    cmd.help = Some(child.ensure_arg_len(1..=1)?.arg(0)?.ensure_string()?);
                }
                "long_help" => {
                    cmd.help_long = Some(child.ensure_arg_len(1..=1)?.arg(0)?.ensure_string()?);
                }
                "before_help" => {
                    cmd.before_help = Some(child.ensure_arg_len(1..=1)?.arg(0)?.ensure_string()?);
                }
                "before_long_help" => {
                    cmd.before_help_long =
                        Some(child.ensure_arg_len(1..=1)?.arg(0)?.ensure_string()?);
                }
                "after_help" => {
                    cmd.after_help = Some(child.ensure_arg_len(1..=1)?.arg(0)?.ensure_string()?);
                }
                "after_long_help" => {
                    cmd.after_help_long =
                        Some(child.ensure_arg_len(1..=1)?.arg(0)?.ensure_string()?);
                }
                "subcommand_required" => {
                    cmd.subcommand_required = child.ensure_arg_len(1..=1)?.arg(0)?.ensure_bool()?
                }
                "hide" => cmd.hide = child.ensure_arg_len(1..=1)?.arg(0)?.ensure_bool()?,
                "deprecated" => {
                    cmd.deprecated = match child.arg(0)?.value.as_bool() {
                        Some(true) => Some("deprecated".to_string()),
                        Some(false) => None,
                        None => Some(child.arg(0)?.ensure_string()?),
                    }
                }
                "complete" => {
                    let complete = SpecComplete::parse(ctx, &child)?;
                    cmd.complete.insert(complete.name.clone(), complete);
                }
                k => bail_parse!(ctx, child.node.name().span(), "unsupported cmd key {k}"),
            }
        }
        Ok(cmd)
    }
    pub(crate) fn is_empty(&self) -> bool {
        self.args.is_empty()
            && self.flags.is_empty()
            && self.mounts.is_empty()
            && self.subcommands.is_empty()
    }
    pub fn usage(&self) -> String {
        let mut usage = self.full_cmd.join(" ");
        let flags = self.flags.iter().filter(|f| !f.hide).collect_vec();
        let args = self.args.iter().filter(|a| !a.hide).collect_vec();
        if !flags.is_empty() {
            if flags.len() <= 2 {
                let inlines = flags
                    .iter()
                    .map(|f| {
                        if f.required {
                            format!("<{}>", f.usage())
                        } else {
                            format!("[{}]", f.usage())
                        }
                    })
                    .join(" ");
                usage = format!("{usage} {inlines}").trim().to_string();
            } else if flags.iter().any(|f| f.required) {
                usage = format!("{usage} <FLAGS>");
            } else {
                usage = format!("{usage} [FLAGS]");
            }
        }
        if !args.is_empty() {
            if args.len() <= 2 {
                let inlines = args.iter().map(|a| a.usage()).join(" ");
                usage = format!("{usage} {inlines}").trim().to_string();
            } else if args.iter().any(|a| a.required) {
                usage = format!("{usage} <ARGS>…");
            } else {
                usage = format!("{usage} [ARGS]…");
            }
        }
        // TODO: mounts?
        // if !self.mounts.is_empty() {
        //     name = format!("{name} [mounts]");
        // }
        if !self.subcommands.is_empty() {
            usage = format!("{usage} <SUBCOMMAND>");
        }
        usage.trim().to_string()
    }
    pub(crate) fn merge(&mut self, other: Self) {
        if !other.name.is_empty() {
            self.name = other.name;
        }
        if other.help.is_some() {
            self.help = other.help;
        }
        if other.help_long.is_some() {
            self.help_long = other.help_long;
        }
        if other.help_md.is_some() {
            self.help_md = other.help_md;
        }
        if other.before_help.is_some() {
            self.before_help = other.before_help;
        }
        if other.before_help_long.is_some() {
            self.before_help_long = other.before_help_long;
        }
        if other.before_help_md.is_some() {
            self.before_help_md = other.before_help_md;
        }
        if other.after_help.is_some() {
            self.after_help = other.after_help;
        }
        if other.after_help_long.is_some() {
            self.after_help_long = other.after_help_long;
        }
        if other.after_help_md.is_some() {
            self.after_help_md = other.after_help_md;
        }
        if !other.args.is_empty() {
            self.args = other.args;
        }
        if !other.flags.is_empty() {
            self.flags = other.flags;
        }
        if !other.mounts.is_empty() {
            self.mounts = other.mounts;
        }
        if !other.aliases.is_empty() {
            self.aliases = other.aliases;
        }
        if !other.hidden_aliases.is_empty() {
            self.hidden_aliases = other.hidden_aliases;
        }
        if !other.examples.is_empty() {
            self.examples = other.examples;
        }
        self.hide = other.hide;
        self.subcommand_required = other.subcommand_required;
        for (name, cmd) in other.subcommands {
            self.subcommands.insert(name, cmd);
        }
        for (name, complete) in other.complete {
            self.complete.insert(name, complete);
        }
    }

    pub fn all_subcommands(&self) -> Vec<&SpecCommand> {
        let mut cmds = vec![];
        for cmd in self.subcommands.values() {
            cmds.push(cmd);
            cmds.extend(cmd.all_subcommands());
        }
        cmds
    }

    pub fn find_subcommand(&self, name: &str) -> Option<&SpecCommand> {
        let sl = self.subcommand_lookup.get_or_init(|| {
            let mut map = HashMap::new();
            for (name, cmd) in &self.subcommands {
                map.insert(name.clone(), name.clone());
                for alias in &cmd.aliases {
                    map.insert(alias.clone(), name.clone());
                }
                for alias in &cmd.hidden_aliases {
                    map.insert(alias.clone(), name.clone());
                }
            }
            map
        });
        let name = sl.get(name)?;
        self.subcommands.get(name)
    }

    pub(crate) fn mount(&mut self, global_flag_args: &[String]) -> Result<(), UsageErr> {
        for mount in self.mounts.iter().cloned().collect_vec() {
            let cmd = if global_flag_args.is_empty() {
                mount.run.clone()
            } else {
                // Parse the mount command into tokens, insert global flags after the first token
                // e.g., "mise tasks ls" becomes "mise --cd dir2 tasks ls"
                // Handles quoted arguments correctly: bash -c "mise tasks ls" stays correct
                match shell_words::split(&mount.run) {
                    Ok(mut tokens) => {
                        if !tokens.is_empty() {
                            // Insert global flags after the first token (the command name)
                            tokens.splice(1..1, global_flag_args.iter().cloned());
                        }
                        // Join tokens back into a properly quoted command string
                        shell_words::join(tokens)
                    }
                    Err(_) => {
                        // If parsing fails, use simple whitespace split
                        // Extract first word (command name), insert flags after it
                        let parts: Vec<&str> = mount.run.splitn(2, ' ').collect();
                        if parts.len() == 2 {
                            format!("{} {} {}", parts[0], global_flag_args.join(" "), parts[1])
                        } else {
                            // No space, just append flags after command
                            format!("{} {}", mount.run, global_flag_args.join(" "))
                        }
                    }
                }
            };
            let output = sh(&cmd)?;
            let spec: Spec = output.parse()?;
            self.merge(spec.cmd);
        }
        Ok(())
    }
}

impl From<&SpecCommand> for KdlNode {
    fn from(cmd: &SpecCommand) -> Self {
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
        if let Some(help) = &cmd.help {
            node.entries_mut()
                .push(KdlEntry::new_prop("help", help.clone()));
        }
        if let Some(help) = &cmd.help_long {
            let children = node.children_mut().get_or_insert_with(KdlDocument::new);
            let mut node = KdlNode::new("long_help");
            node.insert(0, KdlValue::String(help.clone()));
            children.nodes_mut().push(node);
        }
        if let Some(help) = &cmd.help_md {
            let children = node.children_mut().get_or_insert_with(KdlDocument::new);
            let mut node = KdlNode::new("help_md");
            node.insert(0, KdlValue::String(help.clone()));
            children.nodes_mut().push(node);
        }
        if let Some(help) = &cmd.before_help {
            node.entries_mut()
                .push(KdlEntry::new_prop("before_help", help.clone()));
        }
        if let Some(help) = &cmd.before_help_long {
            let children = node.children_mut().get_or_insert_with(KdlDocument::new);
            let mut node = KdlNode::new("before_long_help");
            node.insert(0, KdlValue::String(help.clone()));
            children.nodes_mut().push(node);
        }
        if let Some(help) = &cmd.before_help_md {
            let children = node.children_mut().get_or_insert_with(KdlDocument::new);
            let mut node = KdlNode::new("before_help_md");
            node.insert(0, KdlValue::String(help.clone()));
            children.nodes_mut().push(node);
        }
        if let Some(help) = &cmd.after_help {
            node.entries_mut()
                .push(KdlEntry::new_prop("after_help", help.clone()));
        }
        if let Some(help) = &cmd.after_help_long {
            let children = node.children_mut().get_or_insert_with(KdlDocument::new);
            let mut node = KdlNode::new("after_long_help");
            node.insert(0, KdlValue::String(help.clone()));
            children.nodes_mut().push(node);
        }
        if let Some(help) = &cmd.after_help_md {
            let children = node.children_mut().get_or_insert_with(KdlDocument::new);
            let mut node = KdlNode::new("after_help_md");
            node.insert(0, KdlValue::String(help.clone()));
            children.nodes_mut().push(node);
        }
        for flag in &cmd.flags {
            let children = node.children_mut().get_or_insert_with(KdlDocument::new);
            children.nodes_mut().push(flag.into());
        }
        for arg in &cmd.args {
            let children = node.children_mut().get_or_insert_with(KdlDocument::new);
            children.nodes_mut().push(arg.into());
        }
        for mount in &cmd.mounts {
            let children = node.children_mut().get_or_insert_with(KdlDocument::new);
            children.nodes_mut().push(mount.into());
        }
        for cmd in cmd.subcommands.values() {
            let children = node.children_mut().get_or_insert_with(KdlDocument::new);
            children.nodes_mut().push(cmd.into());
        }
        for complete in cmd.complete.values() {
            let children = node.children_mut().get_or_insert_with(KdlDocument::new);
            children.nodes_mut().push(complete.into());
        }
        node
    }
}

#[cfg(feature = "clap")]
impl From<&clap::Command> for SpecCommand {
    fn from(cmd: &clap::Command) -> Self {
        let mut spec = Self {
            name: cmd.get_name().to_string(),
            hide: cmd.is_hide_set(),
            help: cmd.get_about().map(|s| s.to_string()),
            help_long: cmd.get_long_about().map(|s| s.to_string()),
            before_help: cmd.get_before_help().map(|s| s.to_string()),
            before_help_long: cmd.get_before_long_help().map(|s| s.to_string()),
            after_help: cmd.get_after_help().map(|s| s.to_string()),
            after_help_long: cmd.get_after_long_help().map(|s| s.to_string()),
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
            let mut scmd: SpecCommand = subcmd.into();
            scmd.name = subcmd.get_name().to_string();
            spec.subcommands.insert(scmd.name.clone(), scmd);
        }
        spec
    }
}

#[cfg(feature = "clap")]
impl From<clap::Command> for Spec {
    fn from(cmd: clap::Command) -> Self {
        (&cmd).into()
    }
}
