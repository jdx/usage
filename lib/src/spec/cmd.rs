use std::collections::HashMap;
use std::sync::OnceLock;

use crate::error::UsageErr;
use crate::sh::sh;
use crate::spec::builder::SpecCommandBuilder;
use crate::spec::context::ParsingContext;
use crate::spec::effect::{SpecCommandEffect, EFFECT_VALUES};
use crate::spec::helpers::{string_entry, NodeHelper};
use crate::spec::is_false;
use crate::spec::mount::SpecMount;
use crate::spec::unknown_flags::UnknownFlags;
use crate::{Spec, SpecArg, SpecComplete, SpecFlag};
use indexmap::IndexMap;
use itertools::Itertools;
use kdl::{KdlDocument, KdlEntry, KdlNode};
use serde::Serialize;

/// A CLI command or subcommand specification.
///
/// Commands define the structure of a CLI, including their flags, arguments,
/// and nested subcommands. The root command represents the main CLI entry point.
///
/// # Example
///
/// ```
/// use usage::{SpecCommand, SpecFlag, SpecArg};
///
/// let cmd = SpecCommand::builder()
///     .name("install")
///     .help("Install a package")
///     .alias("i")
///     .flag(SpecFlag::builder().short('f').long("force").build())
///     .arg(SpecArg::builder().name("package").required(true).build())
///     .build();
/// ```
#[derive(Debug, Serialize, Clone)]
pub struct SpecCommand {
    /// Full command path from root (e.g., ["git", "remote", "add"])
    pub full_cmd: Vec<String>,
    /// Generated usage string
    pub usage: String,
    /// Nested subcommands indexed by name
    pub subcommands: IndexMap<String, SpecCommand>,
    /// Positional arguments for this command
    pub args: Vec<SpecArg>,
    /// Flags/options for this command
    pub flags: Vec<SpecFlag>,
    /// Mounted external specs
    pub mounts: Vec<SpecMount>,
    /// Deprecation message if this command is deprecated
    #[serde(skip_serializing_if = "Option::is_none")]
    pub deprecated: Option<String>,
    /// What running this command does to the world: read, write or destructive.
    /// Not inherited by subcommands.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub effect: Option<SpecCommandEffect>,
    /// What to do here with a flag-like token that names no declared flag.
    ///
    /// Unset means "whatever encloses this command decided" — the nearest command
    /// above that set one, or failing that the spec, or failing that
    /// [`UnknownFlags::Value`]. Unlike [`SpecCommandEffect`] this *is* inherited,
    /// because it describes how a command line is read rather than what a command
    /// does, and a CLI that forwards options generally forwards them everywhere.
    pub unknown_flags: Option<UnknownFlags>,
    /// Whether to hide this command from help output
    pub hide: bool,
    /// True when this command came from a [`SpecMount`], i.e. it describes another
    /// program's CLI that was merged in at parse time.
    ///
    /// The flags of the commands *above* a mounted command belong to the mounting CLI,
    /// not to the mounted program, so they are not offered in completions once a mounted
    /// command has been reached. They stay recognized by the parser, since they may
    /// legitimately appear *before* the mounted command on the command line.
    ///
    /// Runtime-only: it is derived from `mount` nodes and is not part of the spec syntax.
    #[serde(skip)]
    pub mounted: bool,
    /// True when a [`SpecMount`] brought flags of its own onto this command. A mounted spec's
    /// root flags are merged into the command the mount sits on, *replacing* that command's
    /// flags (see [`SpecCommand::merge`]), so when this is set every flag here describes the
    /// mounted program and is offered inside the mounted commands accordingly.
    ///
    /// Runtime-only, like [`SpecCommand::mounted`].
    #[serde(skip)]
    pub flags_from_mount: bool,
    /// Whether a subcommand must be provided
    #[serde(skip_serializing_if = "is_false")]
    pub subcommand_required: bool,
    /// Token that resets argument parsing, allowing multiple command invocations.
    /// e.g., `mise run lint ::: test ::: check` with restart_token=":::"
    #[serde(skip_serializing_if = "Option::is_none")]
    pub restart_token: Option<String>,
    /// Short help text shown in command listings
    #[serde(skip_serializing_if = "Option::is_none")]
    pub help: Option<String>,
    /// Extended help text shown with --help
    #[serde(skip_serializing_if = "Option::is_none")]
    pub help_long: Option<String>,
    /// Markdown-formatted help text
    #[serde(skip_serializing_if = "Option::is_none")]
    pub help_md: Option<String>,
    /// Command name (e.g., "install")
    pub name: String,
    /// Alternative names for this command
    pub aliases: Vec<String>,
    /// Hidden alternative names (not shown in help)
    pub hidden_aliases: Vec<String>,
    /// Text displayed before the help content
    #[serde(skip_serializing_if = "Option::is_none")]
    pub before_help: Option<String>,
    /// Extended text displayed before help content
    #[serde(skip_serializing_if = "Option::is_none")]
    pub before_help_long: Option<String>,
    /// Markdown text displayed before help content
    #[serde(skip_serializing_if = "Option::is_none")]
    pub before_help_md: Option<String>,
    /// Text displayed after the help content
    #[serde(skip_serializing_if = "Option::is_none")]
    pub after_help: Option<String>,
    /// Extended text displayed after help content
    #[serde(skip_serializing_if = "Option::is_none")]
    pub after_help_long: Option<String>,
    /// Markdown text displayed after help content
    #[serde(skip_serializing_if = "Option::is_none")]
    pub after_help_md: Option<String>,
    /// Usage examples for this command
    pub examples: Vec<SpecExample>,
    /// Custom completers for arguments
    #[serde(skip_serializing_if = "IndexMap::is_empty")]
    pub complete: IndexMap<String, SpecComplete>,

    /// Cache for subcommand name lookups (including aliases).
    ///
    /// `pub(crate)` only so that other modules can destructure `SpecCommand`
    /// exhaustively; it stays private to the crate.
    #[serde(skip)]
    pub(crate) subcommand_lookup: OnceLock<HashMap<String, String>>,
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
            effect: None,
            unknown_flags: None,
            hide: false,
            mounted: false,
            flags_from_mount: false,
            subcommand_required: false,
            restart_token: None,
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
#[non_exhaustive]
pub struct SpecExample {
    pub code: String,
    pub header: Option<String>,
    pub help: Option<String>,
    pub lang: String,
}

impl SpecExample {
    /// An example invocation shown in generated docs and help.
    pub fn new(code: impl Into<String>) -> Self {
        Self {
            code: code.into(),
            ..Default::default()
        }
    }

    /// Heading shown above the example.
    pub fn header(mut self, header: impl Into<String>) -> Self {
        self.header = Some(header.into());
        self
    }

    /// Prose shown with the example.
    pub fn help(mut self, help: impl Into<String>) -> Self {
        self.help = Some(help.into());
        self
    }

    /// Language used for syntax highlighting.
    pub fn lang(mut self, lang: impl Into<String>) -> Self {
        self.lang = lang.into();
        self
    }
}

impl From<&SpecExample> for KdlNode {
    fn from(example: &SpecExample) -> KdlNode {
        let mut node = KdlNode::new("example");
        node.push(string_entry(None, &example.code));
        if let Some(header) = &example.header {
            node.push(string_entry(Some("header"), header));
        }
        if let Some(help) = &example.help {
            node.push(string_entry(Some("help"), help));
        }
        if !example.lang.is_empty() {
            node.push(string_entry(Some("lang"), &example.lang));
        }
        node
    }
}

impl SpecCommand {
    /// Create a new builder for SpecCommand
    pub fn builder() -> SpecCommandBuilder {
        SpecCommandBuilder::new()
    }

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
                "unknown_flags" => {
                    let raw = v.ensure_string()?;
                    match raw.parse() {
                        Ok(mode) => cmd.unknown_flags = Some(mode),
                        Err(_) => bail_parse!(
                            ctx,
                            v.entry.span(),
                            "unsupported unknown_flags {raw}, expected one of: {}",
                            crate::spec::unknown_flags::UNKNOWN_FLAGS_VALUES
                        ),
                    }
                }
                "effect" => {
                    let raw = v.ensure_string()?;
                    match raw.parse() {
                        Ok(effect) => cmd.effect = Some(effect),
                        Err(_) => bail_parse!(
                            ctx,
                            v.entry.span(),
                            "unsupported effect {raw}, expected one of: {EFFECT_VALUES}"
                        ),
                    }
                }
                "restart_token" => cmd.restart_token = Some(v.ensure_string()?),
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
                "help_md" => {
                    cmd.help_md = Some(child.ensure_arg_len(1..=1)?.arg(0)?.ensure_string()?);
                }
                "before_help" => {
                    cmd.before_help = Some(child.ensure_arg_len(1..=1)?.arg(0)?.ensure_string()?);
                }
                "before_long_help" => {
                    cmd.before_help_long =
                        Some(child.ensure_arg_len(1..=1)?.arg(0)?.ensure_string()?);
                }
                "before_help_md" => {
                    cmd.before_help_md =
                        Some(child.ensure_arg_len(1..=1)?.arg(0)?.ensure_string()?);
                }
                "after_help" => {
                    cmd.after_help = Some(child.ensure_arg_len(1..=1)?.arg(0)?.ensure_string()?);
                }
                "after_long_help" => {
                    cmd.after_help_long =
                        Some(child.ensure_arg_len(1..=1)?.arg(0)?.ensure_string()?);
                }
                "after_help_md" => {
                    cmd.after_help_md = Some(child.ensure_arg_len(1..=1)?.arg(0)?.ensure_string()?);
                }
                "subcommand_required" => {
                    cmd.subcommand_required = child.ensure_arg_len(1..=1)?.arg(0)?.ensure_bool()?
                }
                "hide" => cmd.hide = child.ensure_arg_len(1..=1)?.arg(0)?.ensure_bool()?,
                "effect" => {
                    let arg = child.ensure_arg_len(1..=1)?.arg(0)?;
                    let raw = arg.ensure_string()?;
                    match raw.parse() {
                        Ok(effect) => cmd.effect = Some(effect),
                        Err(_) => bail_parse!(
                            ctx,
                            arg.entry.span(),
                            "unsupported effect {raw}, expected one of: {EFFECT_VALUES}"
                        ),
                    }
                }
                "restart_token" => {
                    cmd.restart_token = Some(child.ensure_arg_len(1..=1)?.arg(0)?.ensure_string()?)
                }
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
        // Merging can add subcommands and aliases, and `find_subcommand` memoizes
        // its lookup into a OnceLock — so the cache has to go, or a name that
        // arrived here would not be findable. This worked before only because
        // mounting happened to precede the first lookup on a given command.
        self.subcommand_lookup = OnceLock::new();
        // Destructured exhaustively (no `..`) so that adding a field to
        // SpecCommand fails to compile until this decides what merging it means.
        // Runtime-derived fields are explicitly ignored rather than skipped.
        let Self {
            name,
            help,
            help_long,
            help_md,
            before_help,
            before_help_long,
            before_help_md,
            after_help,
            after_help_long,
            after_help_md,
            args,
            flags,
            mounts,
            aliases,
            hidden_aliases,
            examples,
            hide,
            subcommand_required,
            restart_token,
            subcommands,
            complete,
            deprecated,
            effect,
            unknown_flags,
            // Recomputed from the merged command, never carried over.
            full_cmd: _,
            usage: _,
            mounted: _,
            flags_from_mount: _,
            subcommand_lookup: _,
        } = other;
        if !name.is_empty() {
            self.name = name;
        }
        if help.is_some() {
            self.help = help;
        }
        if help_long.is_some() {
            self.help_long = help_long;
        }
        if help_md.is_some() {
            self.help_md = help_md;
        }
        if before_help.is_some() {
            self.before_help = before_help;
        }
        if before_help_long.is_some() {
            self.before_help_long = before_help_long;
        }
        if before_help_md.is_some() {
            self.before_help_md = before_help_md;
        }
        if after_help.is_some() {
            self.after_help = after_help;
        }
        if after_help_long.is_some() {
            self.after_help_long = after_help_long;
        }
        if after_help_md.is_some() {
            self.after_help_md = after_help_md;
        }
        if !args.is_empty() {
            self.args = args;
        }
        if !flags.is_empty() {
            self.flags = flags;
        }
        if !mounts.is_empty() {
            self.mounts = mounts;
        }
        if !aliases.is_empty() {
            self.aliases = aliases;
        }
        if !hidden_aliases.is_empty() {
            self.hidden_aliases = hidden_aliases;
        }
        if !examples.is_empty() {
            self.examples = examples;
        }
        self.hide = hide;
        self.subcommand_required = subcommand_required;
        if effect.is_some() {
            self.effect = effect;
        }
        if unknown_flags.is_some() {
            self.unknown_flags = unknown_flags;
        }
        if deprecated.is_some() {
            self.deprecated = deprecated;
        }
        if restart_token.is_some() {
            self.restart_token = restart_token;
        }
        for (name, cmd) in subcommands {
            self.subcommands.insert(name, cmd);
        }
        for (name, complete) in complete {
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
                // Handles quoted arguments correctly: "cmd 'arg with spaces'" stays correct
                let mut tokens = shell_words::split(&mount.run)
                    .expect("mount command should be valid shell syntax");
                if !tokens.is_empty() {
                    // Insert global flags after the first token (the command name)
                    tokens.splice(1..1, global_flag_args.iter().cloned());
                }
                // Join tokens back into a properly quoted command string
                shell_words::join(tokens)
            };
            let output = sh(&cmd)?;
            let mut spec: Spec = output.parse()?;
            // The subcommands emitted by a mount describe another program, so mark them (and
            // everything below them) as mounted. See `SpecCommand::mounted`.
            for cmd in spec.cmd.subcommands.values_mut() {
                cmd.mark_mounted();
            }
            // `merge` folds the mounted spec's root flags into this command; remember that they
            // came from the mount. See `SpecCommand::flags_from_mount`.
            self.flags_from_mount |= !spec.cmd.flags.is_empty();
            self.merge(spec.cmd);
        }
        Ok(())
    }

    /// Mark this command and all of its subcommands as coming from a mount.
    pub(crate) fn mark_mounted(&mut self) {
        self.mounted = true;
        for cmd in self.subcommands.values_mut() {
            cmd.mark_mounted();
        }
    }
}

impl From<&SpecCommand> for KdlNode {
    fn from(cmd: &SpecCommand) -> Self {
        // Destructured exhaustively (no `..`) so that adding a field to
        // SpecCommand fails to compile until this decides how to serialize it.
        let SpecCommand {
            name,
            hide,
            subcommand_required,
            restart_token,
            unknown_flags,
            aliases,
            hidden_aliases,
            help,
            help_long,
            help_md,
            before_help,
            before_help_long,
            before_help_md,
            after_help,
            after_help_long,
            after_help_md,
            deprecated,
            effect,
            flags,
            args,
            mounts,
            subcommands,
            complete,
            examples,
            // Derived from the spec rather than written by it.
            full_cmd: _,
            usage: _,
            mounted: _,
            flags_from_mount: _,
            subcommand_lookup: _,
        } = cmd;
        let mut node = Self::new("cmd");
        node.entries_mut().push(name.clone().into());
        if *hide {
            node.entries_mut().push(KdlEntry::new_prop("hide", true));
        }
        if *subcommand_required {
            node.entries_mut()
                .push(KdlEntry::new_prop("subcommand_required", true));
        }
        if let Some(restart_token) = &restart_token {
            node.entries_mut()
                .push(KdlEntry::new_prop("restart_token", restart_token.clone()));
        }
        if !aliases.is_empty() {
            let mut alias_node = KdlNode::new("alias");
            for alias in aliases {
                alias_node.entries_mut().push(alias.clone().into());
            }
            let children = node.children_mut().get_or_insert_with(KdlDocument::new);
            children.nodes_mut().push(alias_node);
        }
        if !hidden_aliases.is_empty() {
            let mut alias_node = KdlNode::new("alias");
            for alias in hidden_aliases {
                alias_node.entries_mut().push(alias.clone().into());
            }
            alias_node
                .entries_mut()
                .push(KdlEntry::new_prop("hide", true));
            let children = node.children_mut().get_or_insert_with(KdlDocument::new);
            children.nodes_mut().push(alias_node);
        }
        if let Some(help) = &help {
            node.entries_mut().push(string_entry(Some("help"), help));
        }
        if let Some(help) = &help_long {
            let children = node.children_mut().get_or_insert_with(KdlDocument::new);
            let mut node = KdlNode::new("long_help");
            node.push(string_entry(None, help));
            children.nodes_mut().push(node);
        }
        if let Some(help) = &help_md {
            let children = node.children_mut().get_or_insert_with(KdlDocument::new);
            let mut node = KdlNode::new("help_md");
            node.push(string_entry(None, help));
            children.nodes_mut().push(node);
        }
        if let Some(help) = &before_help {
            node.entries_mut()
                .push(string_entry(Some("before_help"), help));
        }
        if let Some(help) = &before_help_long {
            let children = node.children_mut().get_or_insert_with(KdlDocument::new);
            let mut node = KdlNode::new("before_long_help");
            node.push(string_entry(None, help));
            children.nodes_mut().push(node);
        }
        if let Some(help) = &before_help_md {
            let children = node.children_mut().get_or_insert_with(KdlDocument::new);
            let mut node = KdlNode::new("before_help_md");
            node.push(string_entry(None, help));
            children.nodes_mut().push(node);
        }
        if let Some(help) = &after_help {
            node.entries_mut()
                .push(string_entry(Some("after_help"), help));
        }
        if let Some(help) = &after_help_long {
            let children = node.children_mut().get_or_insert_with(KdlDocument::new);
            let mut node = KdlNode::new("after_long_help");
            node.push(string_entry(None, help));
            children.nodes_mut().push(node);
        }
        if let Some(help) = &after_help_md {
            let children = node.children_mut().get_or_insert_with(KdlDocument::new);
            let mut node = KdlNode::new("after_help_md");
            node.push(string_entry(None, help));
            children.nodes_mut().push(node);
        }
        if let Some(deprecated) = &deprecated {
            node.entries_mut()
                .push(string_entry(Some("deprecated"), deprecated));
        }
        if let Some(effect) = effect {
            node.entries_mut()
                .push(string_entry(Some("effect"), effect.as_str()));
        }
        if let Some(unknown_flags) = unknown_flags {
            node.entries_mut()
                .push(string_entry(Some("unknown_flags"), unknown_flags.as_str()));
        }
        for flag in flags {
            let children = node.children_mut().get_or_insert_with(KdlDocument::new);
            children.nodes_mut().push(flag.into());
        }
        for arg in args {
            let children = node.children_mut().get_or_insert_with(KdlDocument::new);
            children.nodes_mut().push(arg.into());
        }
        for mount in mounts {
            let children = node.children_mut().get_or_insert_with(KdlDocument::new);
            children.nodes_mut().push(mount.into());
        }
        for cmd in subcommands.values() {
            let children = node.children_mut().get_or_insert_with(KdlDocument::new);
            children.nodes_mut().push(cmd.into());
        }
        for example in examples {
            let children = node.children_mut().get_or_insert_with(KdlDocument::new);
            children.nodes_mut().push(example.into());
        }
        for complete in complete.values() {
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
                let mut flag: SpecFlag = arg.into();
                // clap keeps conflicts on the command rather than on the argument, so
                // this is the only place both are in view. Written with dashes,
                // matching how the spec refers to a flag everywhere else; clap stores
                // an id, which for a long flag is its name.
                flag.conflicts = cmd
                    .get_arg_conflicts_with(arg)
                    .iter()
                    .filter_map(|other| other.get_long().map(|long| format!("--{long}")))
                    .collect();
                spec.flags.push(flag)
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

#[cfg(test)]
mod tests {
    use crate::spec::effect::SpecCommandEffect;
    use crate::Spec;
    use insta::assert_snapshot;

    #[test]
    fn test_effect_prop_and_child_node() {
        let spec = Spec::parse(
            &Default::default(),
            r#"
bin "mise"
cmd "ls" effect="read"
cmd "use" effect="write"
cmd "uninstall" {
    effect "destructive"
}
cmd "version"
            "#,
        )
        .unwrap();

        let cmds = &spec.cmd.subcommands;
        assert_eq!(cmds["ls"].effect, Some(SpecCommandEffect::Read));
        assert_eq!(cmds["use"].effect, Some(SpecCommandEffect::Write));
        assert_eq!(
            cmds["uninstall"].effect,
            Some(SpecCommandEffect::Destructive)
        );
        // Unspecified stays unknown rather than defaulting to anything.
        assert_eq!(cmds["version"].effect, None);
    }

    #[test]
    fn test_effect_is_not_inherited_by_subcommands() {
        let spec = Spec::parse(
            &Default::default(),
            r#"
bin "git"
cmd "remote" effect="read" {
    cmd "add" effect="write"
    cmd "show"
}
            "#,
        )
        .unwrap();

        let remote = &spec.cmd.subcommands["remote"];
        assert_eq!(remote.effect, Some(SpecCommandEffect::Read));
        assert_eq!(
            remote.subcommands["add"].effect,
            Some(SpecCommandEffect::Write)
        );
        assert_eq!(remote.subcommands["show"].effect, None);
    }

    #[test]
    fn test_effect_roundtrips_through_kdl() {
        let spec = Spec::parse(
            &Default::default(),
            r#"
bin "mise"
cmd "ls" effect="read"
cmd "uninstall" effect="destructive"
            "#,
        )
        .unwrap();

        assert_snapshot!(spec, @r#"
        name mise
        bin mise
        cmd ls effect=read
        cmd uninstall effect=destructive
        "#);
    }

    /// `merge` is how included and mounted specs are composed onto a command.
    /// It has to treat `effect` the way it treats every other optional field:
    /// an overlay that says nothing must not erase what is already declared.
    #[test]
    fn test_effect_survives_merge() {
        let cmd_with = |src: &str| {
            Spec::parse(&Default::default(), src)
                .unwrap()
                .cmd
                .subcommands["uninstall"]
                .clone()
        };

        let declared = cmd_with(r#"cmd "uninstall" effect="destructive""#);
        let silent = cmd_with(r#"cmd "uninstall" help="Remove a tool""#);
        let contradicting = cmd_with(r#"cmd "uninstall" effect="write""#);

        let mut cmd = declared.clone();
        cmd.merge(silent);
        assert_eq!(cmd.effect, Some(SpecCommandEffect::Destructive));

        let mut cmd = declared;
        cmd.merge(contradicting);
        assert_eq!(cmd.effect, Some(SpecCommandEffect::Write));
    }

    #[test]
    fn test_unknown_effect_is_an_error() {
        let err = Spec::parse(
            &Default::default(),
            r#"
bin "mise"
cmd "ls" effect="readonly"
            "#,
        )
        .unwrap_err();
        assert!(
            err.to_string().contains("Invalid usage config"),
            "unexpected error: {err}"
        );
    }
}

#[cfg(test)]
mod merge_tests {
    use crate::Spec;

    fn uninstall(src: &str) -> crate::SpecCommand {
        Spec::parse(&Default::default(), src)
            .unwrap()
            .cmd
            .subcommands["uninstall"]
            .clone()
    }

    /// An overlay that says nothing about deprecation must not un-deprecate a
    /// command that already declared it.
    #[test]
    fn test_deprecated_survives_merge() {
        let declared = uninstall(r#"cmd "uninstall" deprecated="use `remove`""#);
        let silent = uninstall(r#"cmd "uninstall" help="Remove a tool""#);
        let contradicting = uninstall(r#"cmd "uninstall" deprecated="gone in v3""#);

        let mut cmd = declared.clone();
        cmd.merge(silent);
        assert_eq!(cmd.deprecated.as_deref(), Some("use `remove`"));

        let mut cmd = declared;
        cmd.merge(contradicting);
        assert_eq!(cmd.deprecated.as_deref(), Some("gone in v3"));
    }
}

#[cfg(test)]
mod roundtrip_tests {
    use crate::Spec;

    /// Serializing a spec back to KDL and reparsing it must not lose anything.
    ///
    /// The parser is a match on node names, so exhaustive destructuring can't
    /// catch a field the serializer knows about but the parser doesn't, or the
    /// reverse. Comparing the serde representation covers every field without
    /// this test having to enumerate them, so a new field is covered the day it
    /// is added.
    #[test]
    fn test_spec_survives_a_kdl_roundtrip() {
        let src = r#"
name "My CLI"
bin "mycli"
about "does things"
version "1.0.0"
author "nobody"
license "MIT"

flag "-v --verbose" help="Verbose logging" global=#true count=#true
arg "<dir>" help="Directory to use"

cmd "install" help="Install a package" subcommand_required=#false {
    alias "i"
    alias "add" hide=#true
    long_help "The long help for install"
    help_md "The **markdown** help for install"
    before_help "before"
    before_long_help "The long before-help for install"
    before_help_md "The **markdown** before-help for install"
    after_help "after"
    after_long_help "The long after-help for install"
    after_help_md "The **markdown** after-help for install"
    arg "<pkg>" help="Package to install"
    arg "[dest]" effect="write"
    flag "-f --force" help="Overwrite"
    flag "--purge" effect="destructive" overrides="-f" required_unless="--keep"
    complete "pkg" run="mycli list --available" descriptions=#true
    example "mycli install foo" header="Install foo" help="Installs foo" lang="sh"
    example "mycli install bar"
    cmd "from" help="Install from a source" {
        arg "<src>"
    }
}
cmd "wrapped" help="Wraps another CLI" {
    mount run="mycli plugin usage-spec"
}
cmd "remove" help="Remove a package" deprecated="use `uninstall`" effect="destructive"
cmd "run" restart_token=":::" help="Run tasks"
cmd "hidden" hide=#true
        "#;

        let original = Spec::parse(&Default::default(), src).unwrap();
        let reparsed = Spec::parse(&Default::default(), &original.to_string()).unwrap();

        let original = serde_json::to_value(&original).unwrap();
        let reparsed = serde_json::to_value(&reparsed).unwrap();
        pretty_assertions::assert_eq!(original, reparsed);

        // Equality is only meaningful if the fixture actually populated the
        // fields, so guard against a future edit quietly emptying it out.
        let install = &original["cmd"]["subcommands"]["install"];
        let purge = install["flags"]
            .as_array()
            .unwrap()
            .iter()
            .find(|flag| flag["name"] == "purge")
            .unwrap();
        assert_eq!(purge["overrides"], serde_json::json!(["-f"]));
        assert_eq!(purge["required_unless"], serde_json::json!(["--keep"]));
        for key in [
            "help_long",
            "help_md",
            "before_help",
            "before_help_long",
            "before_help_md",
            "after_help",
            "after_help_long",
            "after_help_md",
            "deprecated",
            "effect",
            "restart_token",
            "examples",
            "complete",
            "mounts",
            "aliases",
            "hidden_aliases",
        ] {
            let populated = match key {
                // These sit on other commands in the fixture.
                "deprecated" | "effect" => {
                    original["cmd"]["subcommands"]["remove"].get(key).is_some()
                }
                "restart_token" => original["cmd"]["subcommands"]["run"].get(key).is_some(),
                "mounts" => !original["cmd"]["subcommands"]["wrapped"]["mounts"]
                    .as_array()
                    .unwrap()
                    .is_empty(),
                _ => match install.get(key) {
                    Some(serde_json::Value::Array(a)) => !a.is_empty(),
                    Some(serde_json::Value::Object(o)) => !o.is_empty(),
                    Some(_) => true,
                    None => false,
                },
            };
            assert!(populated, "fixture does not exercise `{key}`");
        }

        // Flag- and arg-level effects live one level down, so check them
        // explicitly rather than by name against the command object.
        assert!(
            install["flags"]
                .as_array()
                .unwrap()
                .iter()
                .any(|f| f.get("effect").is_some()),
            "fixture does not exercise a flag-level `effect`"
        );
        assert!(
            install["args"]
                .as_array()
                .unwrap()
                .iter()
                .any(|a| a.get("effect").is_some()),
            "fixture does not exercise an arg-level `effect`"
        );
    }
}
