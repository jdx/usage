pub mod arg;
pub mod builder;
pub mod choices;
pub mod cmd;
pub mod complete;
pub mod config;
pub mod config_type;
mod context;
pub mod data_types;
pub mod effect;
pub mod flag;
pub mod flagset;
pub mod group;
pub mod helpers;
pub mod mount;
pub mod policy;
pub mod unknown_flags;
pub mod view;

use indexmap::IndexMap;
use kdl::{KdlDocument, KdlEntry, KdlNode};
use log::{info, warn};
use regex::Regex;
use serde::Serialize;
use std::collections::HashMap;
use std::fmt::{Display, Formatter};
use std::iter::once;
use std::path::{Path, PathBuf};
use std::str::FromStr;
use std::sync::LazyLock;

use crate::error::UsageErr;
use crate::spec::cmd::{SpecCommand, SpecExample};
use crate::spec::config::SpecConfig;
use crate::spec::context::ParsingContext;
use crate::spec::flagset::{SpecFlagSet, SpecUse};
use crate::spec::helpers::{string_entry, NodeHelper};
use crate::{SpecArg, SpecComplete, SpecFlag};
use view::SpecView;

#[derive(Debug, Default, Clone, Serialize)]
#[non_exhaustive]
pub struct Spec {
    pub name: String,
    pub bin: String,
    pub cmd: SpecCommand,
    pub config: SpecConfig,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub version: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub long_version: Option<String>,
    pub usage: String,
    pub complete: IndexMap<String, SpecComplete>,
    /// Named executable surfaces promoted from commands in this canonical spec.
    #[serde(skip_serializing_if = "IndexMap::is_empty")]
    pub views: IndexMap<String, SpecView>,
    /// Reusable flag declarations, by name.
    ///
    /// Not serialized, and not re-emitted: a `use` is resolved while the file is read, so by
    /// the time anything reads this spec the flags are on the commands that use them and these
    /// entries only record where they came from.
    #[serde(skip)]
    pub flagsets: IndexMap<String, SpecFlagSet>,
    /// Every file this spec was read from: its own path, then each `include`, recursively.
    ///
    /// What a build script has to watch. A generator that watches only the file it was pointed at
    /// rebuilds nothing when an included file changes — and `include` is how a CLI with many
    /// settings keeps them in a file of their own, so that is the file most likely to be edited.
    ///
    /// Not serialized: it is where the spec came from rather than part of what it says, and `usage g
    /// json` describes the latter.
    #[serde(skip)]
    pub sources: Vec<PathBuf>,

    #[serde(skip_serializing_if = "Option::is_none")]
    pub source_code_link_template: Option<String>,
    /// Where the CLI's source lives, e.g. `https://github.com/jdx/mise`.
    ///
    /// Distinct from [`Self::source_code_link_template`], which is a per-command
    /// deep link with a `{{path}}` placeholder and is only usable for building
    /// "view source" links in generated docs. Scraping a repository out of it
    /// works for one forge and one URL layout and fails everywhere else.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub repository: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub author: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub about: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub about_long: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub about_md: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub license: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub before_help: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub after_help: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub before_help_long: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub after_help_long: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub disable_help: Option<bool>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub min_usage_version: Option<String>,
    #[serde(skip_serializing_if = "Vec::is_empty")]
    pub examples: Vec<SpecExample>,
    /// Default subcommand to use when first non-flag argument is not a known subcommand.
    /// This enables "naked" command syntax like `mise foo` instead of `mise run foo`.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub default_subcommand: Option<String>,
    /// Whether argv[0]'s basename selects a subcommand (busybox-style applets).
    ///
    /// clap's `multicall`. The dispatcher names ([`Self::name`] and [`Self::bin`])
    /// are skipped; any other basename is parsed as the first word, so a symlink
    /// `ls -> busybox` runs the `ls` applet. Path components and a trailing `.exe`
    /// are stripped.
    #[serde(default, skip_serializing_if = "is_false")]
    pub multicall: bool,
    /// Whether the source explicitly declared [`Self::multicall`].
    ///
    /// This distinguishes an omitted node from `multicall #false` while includes
    /// are merged. It is parsing bookkeeping rather than part of the JSON model.
    #[doc(hidden)]
    #[serde(skip)]
    pub multicall_set: bool,
    /// What to do with a flag-like token that names no declared flag, for the whole
    /// CLI. A command may override it; see [`SpecCommand::unknown_flags`].
    pub unknown_flags: Option<crate::spec::unknown_flags::UnknownFlags>,
}

impl Spec {
    /// Resolve every mount from supplied command outputs without spawning processes.
    ///
    /// This is intended for deterministic generators and conformance harnesses. The
    /// map is keyed by each mount's exact `run` declaration. Missing entries are an
    /// error, so injecting a partial view cannot silently execute the remainder.
    pub fn resolve_mount_outputs(
        &mut self,
        outputs: &HashMap<String, String>,
    ) -> Result<(), UsageErr> {
        self.resolve_mount_outputs_at_root(outputs, true)
    }

    pub(crate) fn resolve_mount_outputs_at_root(
        &mut self,
        outputs: &HashMap<String, String>,
        apply_default_subcommand: bool,
    ) -> Result<(), UsageErr> {
        fn resolve(
            cmd: &mut SpecCommand,
            outputs: &HashMap<String, String>,
            skip_mounts: bool,
        ) -> Result<(), UsageErr> {
            if !skip_mounts && !cmd.mounts.is_empty() {
                cmd.mount(&[], Some(outputs))?;
                cmd.mounts.clear();
            }
            for subcommand in cmd.subcommands.values_mut() {
                resolve(subcommand, outputs, false)?;
            }
            Ok(())
        }

        let default_outranks_root_mounts = apply_default_subcommand
            && self.default_subcommand.is_some()
            && !self.cmd.mounts.iter().any(|mount| mount.overrides_default);
        resolve(&mut self.cmd, outputs, default_outranks_root_mounts)
    }

    /// Parse a spec from a file.
    ///
    /// Automatically detects whether the file is:
    /// - A `.kdl` or `.usage.kdl` file containing a raw spec
    /// - A script file with embedded `#USAGE` comments
    ///
    /// If `bin` is not specified in the spec, it defaults to the filename.
    #[must_use = "parsing result should be used"]
    pub fn parse_file(file: &Path) -> Result<Spec, UsageErr> {
        Self::parse_file_with_metadata_inference(file, true)
    }

    fn parse_file_with_metadata_inference(
        file: &Path,
        infer_metadata_from_filename: bool,
    ) -> Result<Spec, UsageErr> {
        let spec = split_script(file)?;
        let ctx = ParsingContext::new(file, &spec);
        let mut schema = Self::parse(&ctx, &spec)?;
        if infer_metadata_from_filename && schema.bin.is_empty() {
            schema.bin = file
                .file_name()
                .and_then(|n| n.to_str())
                .ok_or_else(|| UsageErr::InvalidPath(file.display().to_string()))?
                .to_string();
        }
        if schema.name.is_empty() {
            schema.name.clone_from(&schema.bin);
        }
        Ok(schema)
    }
    /// Parse a spec from a script file's embedded USAGE comments.
    ///
    /// Extracts the spec from comment lines marked with `#USAGE`, `//USAGE`,
    /// `::USAGE`, or their `[USAGE]` variants.
    /// If `bin` is not specified in the spec, it defaults to the filename.
    #[must_use = "parsing result should be used"]
    pub fn parse_script(file: &Path) -> Result<Spec, UsageErr> {
        let mut spec = Self::parse_script_with_path(&read_to_string(file)?, file)?;
        if spec.bin.is_empty() {
            spec.bin = file
                .file_name()
                .and_then(|n| n.to_str())
                .ok_or_else(|| UsageErr::InvalidPath(file.display().to_string()))?
                .to_string();
        }
        if spec.name.is_empty() {
            spec.name.clone_from(&spec.bin);
        }
        Ok(spec)
    }

    /// Parse a spec from a script string's embedded USAGE comments.
    ///
    /// Extracts the spec from comment lines marked with `#USAGE`, `//USAGE`,
    /// `::USAGE`, or their `[USAGE]` variants. Unlike [`Self::parse_script`],
    /// this function cannot infer `bin` or `name` from a filename. Relative
    /// `include` paths are rejected because there is no source path to resolve
    /// them against; absolute `include` paths remain supported.
    #[must_use = "parsing result should be used"]
    pub fn parse_script_str(input: &str) -> Result<Spec, UsageErr> {
        Self::parse_script_with_path(input, Path::new(""))
    }

    fn parse_script_with_path(input: &str, file: &Path) -> Result<Spec, UsageErr> {
        let raw = extract_usage_from_comments(input);
        let ctx = ParsingContext::new(file, &raw);
        Self::parse(&ctx, &raw)
    }

    #[deprecated]
    pub fn parse_spec(input: &str) -> Result<Spec, UsageErr> {
        Self::parse(&Default::default(), input)
    }

    pub fn is_empty(&self) -> bool {
        self.name.is_empty()
            && self.bin.is_empty()
            && self.usage.is_empty()
            && self.cmd.is_empty()
            && self.config.is_empty()
            && self.complete.is_empty()
            && self.views.is_empty()
            && self.examples.is_empty()
    }

    /// Materialize one declared executable view.
    ///
    /// This is a cold-path operation for documentation and completion generation. The canonical
    /// spec remains unchanged; the returned spec promotes the view's command to the root and
    /// carries only the root globals the view declares.
    pub fn for_view(&self, id: &str) -> Result<Spec, UsageErr> {
        let view = self
            .views
            .get(id)
            .ok_or_else(|| UsageErr::InvalidView(format!("spec declares no view named `{id}`")))?;
        let mut command = &self.cmd;
        for segment in view.root.split_whitespace() {
            command = command.subcommands.get(segment).ok_or_else(|| {
                UsageErr::InvalidView(format!(
                    "view `{id}` promotes `{}`, but `{segment}` is not a command on that path",
                    view.root
                ))
            })?;
        }
        let mut promoted = command.clone();
        let matches_selector = |flag: &SpecFlag, selector: &str| {
            selector
                .strip_prefix("--")
                .is_some_and(|name| flag.long.iter().any(|long| long == name))
                || selector
                    .strip_prefix('-')
                    .filter(|short| short.len() == 1)
                    .and_then(|short| short.chars().next())
                    .is_some_and(|short| flag.short.contains(&short))
        };
        let carries = |flag: &SpecFlag| {
            if !flag.global {
                return false;
            }
            view.all_globals
                || view
                    .globals
                    .iter()
                    .any(|selector| matches_selector(flag, selector))
        };
        for selector in &view.globals {
            if !self
                .cmd
                .flags
                .iter()
                .any(|flag| flag.global && matches_selector(flag, selector))
            {
                return Err(UsageErr::InvalidView(format!(
                    "view `{id}` carries `{selector}`, but it is not a root global flag"
                )));
            }
        }
        let globals: Vec<SpecFlag> = self
            .cmd
            .flags
            .iter()
            // A view is another executable surface of this package. Keep the host's
            // version actions in addition to the globals explicitly carried by the view.
            .filter(|flag| carries(flag) || flag.action == crate::SpecFlagAction::Version)
            .cloned()
            .collect();
        // A promoted command may redeclare a global spelling. The nearer declaration owns it,
        // matching ordinary parsing, so do not create a duplicate root flag.
        let mut globals: Vec<SpecFlag> = globals
            .into_iter()
            .filter(|global| {
                !promoted
                    .flags
                    .iter()
                    .any(|local| spec_flag_forms_overlap(global, local))
            })
            .collect();
        // Root completers belong to the host's fields, not to every executable view. Carry the
        // ones for selected globals, then let the promoted command's own completers win on a
        // shared name. A promoted command is the new root, so its command-scoped entries become
        // the materialized spec's root entries rather than remaining in both places.
        let mut complete = IndexMap::new();
        for flag in &globals {
            if let Some(arg) = &flag.arg {
                let name = arg.name.to_lowercase();
                if let Some(completer) = self.complete.get(&name) {
                    complete.insert(name, completer.clone());
                }
            }
        }
        complete.extend(std::mem::take(&mut promoted.complete));
        // Root groups are relationships between the root fields, so project them along with
        // the carried globals. A group reduced to one required member is ordinary requiredness;
        // keeping it as a one-member group would emit KDL the spec reader deliberately refuses.
        let mut carried_groups = Vec::new();
        for group in &self.cmd.groups {
            let members: Vec<String> = group
                .members
                .iter()
                .filter(|selector| {
                    globals
                        .iter()
                        .any(|flag| flag_matches_selector(flag, selector))
                })
                .cloned()
                .collect();
            match members.as_slice() {
                [only] if group.required => {
                    if let Some(flag) = globals
                        .iter_mut()
                        .find(|flag| flag_matches_selector(flag, only))
                    {
                        flag.required = true;
                    }
                }
                [_, _, ..] => {
                    let mut projected = group.clone();
                    projected.members = members;
                    carried_groups.push(projected);
                }
                _ => {}
            }
        }
        promoted.flags.splice(0..0, globals);
        promoted.groups.splice(0..0, carried_groups);
        promoted.name.clone_from(&view.bin);
        promoted.full_cmd.clear();
        promoted.aliases.clear();
        promoted.hidden_aliases.clear();
        // A view is another executable surface of the host package. Keep the host policy that
        // governs its synthesized version entry along with the host version strings retained on
        // `spec`; otherwise materializing a promoted command silently re-enables `--version`.
        promoted.disable_version_flag = self.cmd.disable_version_flag;
        set_subcommand_ancestors(&mut promoted, &[]);
        promoted.usage = promoted.usage();

        let mut spec = self.clone();
        spec.name.clone_from(&view.name);
        spec.bin.clone_from(&view.bin);
        spec.about = promoted.help.clone();
        spec.about_long = promoted.help_long.clone();
        spec.about_md = promoted.help_md.clone();
        spec.before_help = promoted.before_help.clone();
        spec.before_help_long = promoted.before_help_long.clone();
        spec.after_help = promoted.after_help.clone();
        spec.after_help_long = promoted.after_help_long.clone();
        spec.examples.clone_from(&promoted.examples);
        spec.usage = promoted.usage.clone();
        spec.complete = complete;
        spec.cmd = promoted;
        spec.default_subcommand = None;
        spec.multicall = false;
        spec.multicall_set = false;
        spec.views.clear();
        Ok(spec)
    }

    /// The stable identifier of the executable view selected by a program name.
    pub fn view_for_program(&self, program: &str) -> Option<&str> {
        let basename = crate::parse::multicall_basename(program);
        if basename == crate::parse::multicall_basename(&self.name)
            || (!self.bin.is_empty() && basename == crate::parse::multicall_basename(&self.bin))
        {
            return None;
        }
        self.views.values().find_map(|view| {
            (basename == crate::parse::multicall_basename(&view.bin)
                || basename == crate::parse::multicall_basename(&view.id))
            .then_some(view.id.as_str())
        })
    }

    pub(crate) fn parse(ctx: &ParsingContext, input: &str) -> Result<Spec, UsageErr> {
        let kdl: KdlDocument = input
            .parse()
            .map_err(|err: kdl::KdlError| UsageErr::KdlError(err))?;
        let mut schema = Self {
            ..Default::default()
        };
        // The file being read, before anything in it can fail: a build script that watches this list
        // should watch a spec that does not parse too, or the next build is a stale success.
        if !ctx.file.as_os_str().is_empty() {
            schema.sources.push(ctx.file.clone());
        }
        for node in kdl.nodes().iter().map(|n| NodeHelper::new(ctx, n)) {
            match node.name() {
                "name" => schema.name = node.arg(0)?.ensure_string()?,
                "bin" => {
                    schema.bin = node.arg(0)?.ensure_string()?;
                    if schema.name.is_empty() {
                        schema.name.clone_from(&schema.bin);
                    }
                }
                "version" => schema.version = Some(node.arg(0)?.ensure_string()?),
                "long_version" => schema.long_version = Some(node.arg(0)?.ensure_string()?),
                "author" => schema.author = Some(node.arg(0)?.ensure_string()?),
                "source_code_link_template" => {
                    schema.source_code_link_template = Some(node.arg(0)?.ensure_string()?)
                }
                "repository" => schema.repository = Some(node.arg(0)?.ensure_string()?),
                "about" => schema.about = Some(node.arg(0)?.ensure_string()?),
                "long_about" => schema.about_long = Some(node.arg(0)?.ensure_string()?),
                "about_long" => schema.about_long = Some(node.arg(0)?.ensure_string()?),
                "about_md" => schema.about_md = Some(node.arg(0)?.ensure_string()?),
                "license" => schema.license = Some(node.arg(0)?.ensure_string()?),
                "before_help" => schema.before_help = Some(node.arg(0)?.ensure_string()?),
                "after_help" => schema.after_help = Some(node.arg(0)?.ensure_string()?),
                "before_long_help" | "before_help_long" => {
                    schema.before_help_long = Some(node.arg(0)?.ensure_string()?)
                }
                "after_long_help" | "after_help_long" => {
                    schema.after_help_long = Some(node.arg(0)?.ensure_string()?)
                }
                "usage" => schema.usage = node.arg(0)?.ensure_string()?,
                "arg" => {
                    let arg = SpecArg::parse(ctx, &node)?;
                    // The same rule the `cmd` block applies: a delimiter with nowhere to
                    // put what it splits drops everything after the first separator.
                    if arg.delimiter.is_some() && !arg.var {
                        bail_parse!(
                            ctx,
                            node.node.name().span(),
                            "argument <{}> has a delimiter and holds one value; add \
                             `var=#true` for the values it splits into",
                            arg.name
                        );
                    }
                    schema.cmd.args.push(arg);
                }
                "flag" => schema.cmd.flags.push(SpecFlag::parse(ctx, &node)?),
                // The root command's groups, as its flags and arguments are: a spec
                // whose top level declares flags can group them there too.
                "group" => schema.cmd.groups.push(crate::SpecGroup::parse(ctx, &node)?),
                // The root is a command like any other, so it can discover its own
                // subcommands by running something. A CLI whose top-level commands
                // come from plugins has no other way to say so.
                "mount" => schema.cmd.mounts.push(crate::SpecMount::parse(ctx, &node)?),
                "cmd" => {
                    let node: SpecCommand = SpecCommand::parse(ctx, &node)?;
                    schema.cmd.subcommands.insert(node.name.to_string(), node);
                }
                "flagset" => {
                    let set = SpecFlagSet::parse(ctx, &node)?;
                    if schema.flagsets.insert(set.name.clone(), set).is_some() {
                        bail_parse!(ctx, node.span(), "a flagset may be declared only once");
                    }
                }
                // The root is a command like any other: if its own flags repeat a set, it
                // says so the same way a subcommand does.
                "use" => {
                    let at = schema.cmd.flags.len();
                    schema.cmd.uses.push(SpecUse::parse(ctx, &node, at)?);
                }
                "config" => schema.config = SpecConfig::parse(ctx, &node)?,
                "complete" => {
                    let complete = SpecComplete::parse(ctx, &node)?;
                    schema.complete.insert(complete.name.clone(), complete);
                }
                "view" => {
                    let view = SpecView::parse(ctx, &node)?;
                    if schema.views.insert(view.id.clone(), view).is_some() {
                        bail_parse!(
                            ctx,
                            node.span(),
                            "a view identifier may be declared only once"
                        );
                    }
                }
                "disable_help" => schema.disable_help = Some(node.arg(0)?.ensure_bool()?),
                "min_usage_version" => {
                    let v = node.arg(0)?.ensure_string()?;
                    check_usage_version(&v);
                    schema.min_usage_version = Some(v);
                }
                "unknown_flags" => {
                    let raw = node.arg(0)?.ensure_string()?;
                    match raw.parse() {
                        Ok(mode) => schema.unknown_flags = Some(mode),
                        Err(_) => bail_parse!(
                            ctx,
                            node.span(),
                            "unsupported unknown_flags {raw}, expected one of: {}",
                            crate::spec::unknown_flags::UNKNOWN_FLAGS_VALUES
                        ),
                    }
                }
                "default_subcommand" => {
                    schema.default_subcommand = Some(node.arg(0)?.ensure_string()?)
                }
                "multicall" => {
                    schema.multicall = node.arg(0)?.ensure_bool()?;
                    schema.multicall_set = true;
                }
                "external_subcommand" => {
                    schema.cmd.external_subcommand = node.arg(0)?.ensure_bool()?;
                }
                "arg_required_else_help" => {
                    schema.cmd.arg_required_else_help = node.arg(0)?.ensure_bool()?;
                }
                "disable_help_flag" => {
                    schema.cmd.disable_help_flag = node.arg(0)?.ensure_bool()?;
                }
                "disable_help_subcommand" => {
                    schema.cmd.disable_help_subcommand = node.arg(0)?.ensure_bool()?;
                }
                "disable_version_flag" => {
                    schema.cmd.disable_version_flag = node.arg(0)?.ensure_bool()?;
                }
                "dont_delimit_trailing_values" => {
                    schema.cmd.dont_delimit_trailing_values = node.arg(0)?.ensure_bool()?;
                }
                "args_override_self" => {
                    schema.cmd.args_override_self = node.arg(0)?.ensure_bool()?;
                }
                "subcommand_negates_reqs" => {
                    schema.cmd.subcommand_negates_reqs = node.arg(0)?.ensure_bool()?;
                }
                "args_conflicts_with_subcommands" => {
                    schema.cmd.args_conflicts_with_subcommands = node.arg(0)?.ensure_bool()?;
                }
                "subcommand_precedence_over_arg" => {
                    schema.cmd.subcommand_precedence_over_arg = node.arg(0)?.ensure_bool()?;
                }
                "allow_missing_positional" => {
                    schema.cmd.allow_missing_positional = node.arg(0)?.ensure_bool()?;
                }
                "deprecated" => schema.cmd.deprecated = Some(node.arg(0)?.ensure_string()?),
                "deprecated_warn_at" => {
                    schema.cmd.deprecated_warn_at = Some(node.arg(0)?.ensure_string()?);
                }
                "deprecated_remove_at" => {
                    schema.cmd.deprecated_remove_at = Some(node.arg(0)?.ensure_string()?);
                }
                "subcommand_required" => {
                    schema.cmd.subcommand_required = node.arg(0)?.ensure_bool()?;
                }
                "subcommand_help_heading" => {
                    schema.cmd.subcommand_help_heading = Some(node.arg(0)?.ensure_string()?);
                }
                "subcommand_value_name" => {
                    schema.cmd.subcommand_value_name = Some(node.arg(0)?.ensure_string()?);
                }
                "next_line_help" => {
                    schema.cmd.next_line_help = node.arg(0)?.ensure_bool()?;
                }
                "flatten_help" => {
                    schema.cmd.flatten_help = node.arg(0)?.ensure_bool()?;
                }
                "term_width" => {
                    schema.cmd.term_width = Some(node.arg(0)?.ensure_usize()?);
                }
                "max_term_width" => {
                    schema.cmd.max_term_width = Some(node.arg(0)?.ensure_usize()?);
                }
                "example" => {
                    let code = node.ensure_arg_len(1..=1)?.arg(0)?.ensure_string()?;
                    let mut example = SpecExample::new(code.trim().to_string());
                    for (k, v) in node.props() {
                        match k {
                            "header" => example.header = Some(v.ensure_string()?),
                            "help" => example.help = Some(v.ensure_string()?),
                            "lang" => example.lang = v.ensure_string()?,
                            k => bail_parse!(ctx, v.entry.span(), "unsupported example key {k}"),
                        }
                    }
                    schema.examples.push(example);
                }
                "include" => {
                    let file = node
                        .props()
                        .get("file")
                        .map(|v| v.ensure_string())
                        .transpose()?
                        .ok_or_else(|| ctx.build_err("missing file".into(), node.span()))?;
                    let file = Path::new(&file);
                    let file = match file.is_relative() {
                        true => ctx
                            .file
                            .parent()
                            .ok_or_else(|| {
                                let msg = if ctx.file.as_os_str().is_empty() {
                                    "relative includes require a source file".to_string()
                                } else {
                                    format!("cannot get parent of {}", ctx.file.display())
                                };
                                ctx.build_err(msg, node.span())
                            })?
                            .join(file),
                        false => file.to_path_buf(),
                    };
                    info!("include: {}", file.display());
                    let other = Self::parse_file_with_metadata_inference(&file, false)?;
                    // Two *declarations* of one name are refused, the same as two in a single
                    // file. Letting the incoming set win would make which declaration a
                    // `use` gets depend on whether the `include` stands above or below it —
                    // and only in that direction, since a `flagset` written after an
                    // `include` already fails here.
                    //
                    // Which declaration, not which name: a file of shared sets is included
                    // by every file whose `use` nodes name them, since each file resolves
                    // its own. A spec that includes two of those files sees the shared set
                    // arrive twice, and that is one declaration by two routes.
                    let clash = other.flagsets.values().find(|incoming| {
                        schema
                            .flagsets
                            .get(&incoming.name)
                            .is_some_and(|own| own.declared_in != incoming.declared_in)
                    });
                    if let Some(incoming) = clash {
                        let name = &incoming.name;
                        let owner = schema.flagsets[name].declared_in.clone();
                        let owner = match owner.as_os_str().is_empty() {
                            true => "this spec".to_string(),
                            false => owner.display().to_string(),
                        };
                        bail_parse!(
                            ctx,
                            node.span(),
                            "a flagset may be declared only once: \"{name}\" is declared in \
                             {} and in {owner}",
                            incoming.declared_in.display()
                        );
                    }
                    schema.merge(other);
                }
                k => bail_parse!(ctx, node.node.name().span(), "unsupported spec key {k}"),
            }
        }
        schema.cmd.name = if schema.bin.is_empty() {
            schema.name.clone()
        } else {
            schema.bin.clone()
        };
        // Before ancestors, because a command's usage string is built from its flags.
        flagset::expand(ctx, &mut schema.cmd, &mut schema.flagsets)?;
        set_subcommand_ancestors(&mut schema.cmd, &[]);
        Ok(schema)
    }

    pub fn merge(&mut self, other: Spec) {
        macro_rules! merge_str {
            ($field:ident) => {
                if !other.$field.is_empty() {
                    self.$field = other.$field;
                }
            };
        }
        macro_rules! merge_opt {
            ($field:ident) => {
                if other.$field.is_some() {
                    self.$field = other.$field;
                }
            };
        }
        macro_rules! merge_extend {
            ($field:ident) => {
                if !other.$field.is_empty() {
                    self.$field.extend(other.$field);
                }
            };
        }

        merge_str!(name);
        merge_str!(bin);
        merge_str!(usage);
        merge_opt!(about);
        merge_opt!(source_code_link_template);
        merge_opt!(repository);
        merge_opt!(version);
        merge_opt!(long_version);
        merge_opt!(author);
        merge_opt!(about_long);
        merge_opt!(about_md);
        merge_opt!(license);
        merge_opt!(before_help);
        merge_opt!(after_help);
        merge_opt!(before_help_long);
        merge_opt!(after_help_long);
        merge_opt!(disable_help);
        merge_opt!(min_usage_version);
        merge_opt!(default_subcommand);
        if other.multicall_set {
            self.multicall = other.multicall;
            self.multicall_set = true;
        }
        merge_opt!(unknown_flags);
        merge_extend!(complete);
        merge_extend!(views);
        // An included file's sets are visible to the file that includes it, which is how a
        // spec keeps its shared declarations in a file of their own. Its own `use` nodes are
        // already resolved by the time it gets here, so nothing is expanded twice. Two files
        // declaring one name never reach this extend: the `include` refuses them, rather
        // than one silently taking the other's name. What does reach it is the same shared
        // file arriving by two routes, which overwrites an entry with itself.
        merge_extend!(flagsets);
        merge_extend!(examples);
        // An included spec brings the files *it* read, which is how a nested include is watched.
        merge_extend!(sources);

        if !other.config.is_empty() {
            self.config.merge(&other.config);
        }
        self.cmd.merge(other.cmd);
    }
}

pub(crate) fn spec_flag_forms_overlap(a: &SpecFlag, b: &SpecFlag) -> bool {
    fn long_forms(flag: &SpecFlag) -> impl Iterator<Item = &str> {
        flag.long
            .iter()
            .chain(&flag.hidden_aliases)
            .map(String::as_str)
            .chain(
                flag.negate
                    .as_deref()
                    .map(|name| name.strip_prefix("--").unwrap_or(name)),
            )
    }
    fn short_forms(flag: &SpecFlag) -> impl Iterator<Item = &char> {
        flag.short.iter().chain(&flag.hidden_short_aliases)
    }

    long_forms(a).any(|name| long_forms(b).any(|other| other == name))
        || short_forms(a).any(|name| short_forms(b).any(|other| other == name))
}

fn flag_matches_selector(flag: &SpecFlag, selector: &str) -> bool {
    selector.strip_prefix("--").is_some_and(|name| {
        flag.long
            .iter()
            .chain(&flag.hidden_aliases)
            .any(|long| long == name)
            || flag
                .negate
                .as_deref()
                .is_some_and(|negate| negate.strip_prefix("--").unwrap_or(negate) == name)
    }) || selector
        .strip_prefix('-')
        .filter(|short| short.len() == 1)
        .and_then(|short| short.chars().next())
        .is_some_and(|short| {
            flag.short
                .iter()
                .chain(&flag.hidden_short_aliases)
                .any(|candidate| *candidate == short)
        })
}

fn check_usage_version(version: &str) {
    let cur = versions::Versioning::new(env!("CARGO_PKG_VERSION")).unwrap();
    match versions::Versioning::new(version) {
        Some(v) => {
            if cur < v {
                warn!(
                    "This usage spec requires at least version {version}, but you are using version {cur} of usage"
                );
            }
        }
        _ => warn!("Invalid version: {version}"),
    }
}

/// Read a file, keeping its path in the error.
///
/// `std::fs::read_to_string` reports "No such file or directory" and nothing about which
/// file, and these paths come from a command line.
fn read_to_string(file: &Path) -> Result<String, UsageErr> {
    std::fs::read_to_string(file).map_err(|err| UsageErr::FileError(err, file.to_path_buf()))
}

/// A comment line that opens or continues an embedded spec: `#USAGE`, `//USAGE`, `::USAGE`,
/// or their `[USAGE]` spellings.
static USAGE_COMMENT: LazyLock<Regex> =
    LazyLock::new(|| Regex::new(r"^(?:#|//|::)(?:USAGE| ?\[USAGE\])(.*)$").unwrap());
/// The same, without capturing the rest of the line: used only to answer whether a script
/// carries an embedded spec at all.
static HAS_USAGE_COMMENT: LazyLock<Regex> =
    LazyLock::new(|| Regex::new(r"^(?:#|//|::)(?:USAGE| ?\[USAGE\])").unwrap());
/// A comment line with nothing on it, which continues a spec rather than ending it.
static BLANK_COMMENT: LazyLock<Regex> = LazyLock::new(|| Regex::new(r"^(?:#|//|::)\s*$").unwrap());

fn split_script(file: &Path) -> Result<String, UsageErr> {
    let full = read_to_string(file)?;
    // If file has a shebang and USAGE comments, extract the spec from comments
    if full.starts_with("#!") && full.lines().any(|l| HAS_USAGE_COMMENT.is_match(l)) {
        return Ok(extract_usage_from_comments(&full));
    }
    // Otherwise treat the whole file as a KDL spec (e.g., .usage.kdl files)
    Ok(full)
}

fn extract_usage_from_comments(full: &str) -> String {
    let mut usage = vec![];
    let mut found = false;
    for line in full.lines() {
        if let Some(captures) = USAGE_COMMENT.captures(line) {
            found = true;
            let content = captures.get(1).map_or("", |m| m.as_str());
            usage.push(content.trim());
        } else if found {
            // Allow blank comment lines to continue parsing
            if BLANK_COMMENT.is_match(line) {
                continue;
            }
            // if there is a non-blank non-USAGE line, stop reading
            break;
        }
    }
    usage.join("\n")
}

fn set_subcommand_ancestors(cmd: &mut SpecCommand, ancestors: &[String]) {
    for subcmd in cmd.subcommands.values_mut() {
        subcmd.full_cmd = ancestors
            .iter()
            .cloned()
            .chain(once(subcmd.name.clone()))
            .collect();
        let child_ancestors = subcmd.full_cmd.clone();
        set_subcommand_ancestors(subcmd, &child_ancestors);
    }
    if cmd.usage.is_empty() {
        cmd.usage = cmd.usage();
    }
}

impl Display for Spec {
    fn fmt(&self, f: &mut Formatter<'_>) -> std::fmt::Result {
        let mut doc = KdlDocument::new();
        let nodes = &mut doc.nodes_mut();
        if !self.name.is_empty() {
            let mut node = KdlNode::new("name");
            node.push(string_entry(None, &self.name));
            nodes.push(node);
        }
        if !self.bin.is_empty() {
            let mut node = KdlNode::new("bin");
            node.push(string_entry(None, &self.bin));
            nodes.push(node);
        }
        if let Some(version) = &self.version {
            let mut node = KdlNode::new("version");
            node.push(string_entry(None, version));
            nodes.push(node);
        }
        if let Some(version) = &self.long_version {
            let mut node = KdlNode::new("long_version");
            node.push(string_entry(None, version));
            nodes.push(node);
        }
        if let Some(author) = &self.author {
            let mut node = KdlNode::new("author");
            node.push(string_entry(None, author));
            nodes.push(node);
        }
        if let Some(about) = &self.about {
            let mut node = KdlNode::new("about");
            node.push(string_entry(None, about));
            nodes.push(node);
        }
        if let Some(source_code_link_template) = &self.source_code_link_template {
            let mut node = KdlNode::new("source_code_link_template");
            node.push(string_entry(None, source_code_link_template));
            nodes.push(node);
        }
        if let Some(repository) = &self.repository {
            let mut node = KdlNode::new("repository");
            node.push(string_entry(None, repository));
            nodes.push(node);
        }
        if let Some(about_md) = &self.about_md {
            let mut node = KdlNode::new("about_md");
            node.push(string_entry(None, about_md));
            nodes.push(node);
        }
        if let Some(long_about) = &self.about_long {
            let mut node = KdlNode::new("long_about");
            node.push(string_entry(None, long_about));
            nodes.push(node);
        }
        if let Some(license) = &self.license {
            let mut node = KdlNode::new("license");
            node.push(string_entry(None, license));
            nodes.push(node);
        }
        if let Some(before_help) = &self.before_help {
            let mut node = KdlNode::new("before_help");
            node.push(string_entry(None, before_help));
            nodes.push(node);
        }
        if let Some(after_help) = &self.after_help {
            let mut node = KdlNode::new("after_help");
            node.push(string_entry(None, after_help));
            nodes.push(node);
        }
        if let Some(before_help_long) = &self.before_help_long {
            let mut node = KdlNode::new("before_long_help");
            node.push(string_entry(None, before_help_long));
            nodes.push(node);
        }
        if let Some(after_help_long) = &self.after_help_long {
            let mut node = KdlNode::new("after_long_help");
            node.push(string_entry(None, after_help_long));
            nodes.push(node);
        }
        if let Some(disable_help) = self.disable_help {
            let mut node = KdlNode::new("disable_help");
            node.push(KdlEntry::new(disable_help));
            nodes.push(node);
        }
        if let Some(min_usage_version) = &self.min_usage_version {
            let mut node = KdlNode::new("min_usage_version");
            node.push(string_entry(None, min_usage_version));
            nodes.push(node);
        }
        if let Some(unknown_flags) = &self.unknown_flags {
            let mut node = KdlNode::new("unknown_flags");
            node.push(string_entry(None, unknown_flags.as_str()));
            nodes.push(node);
        }
        if let Some(default_subcommand) = &self.default_subcommand {
            let mut node = KdlNode::new("default_subcommand");
            node.push(string_entry(None, default_subcommand));
            nodes.push(node);
        }
        if self.multicall_set {
            let mut node = KdlNode::new("multicall");
            node.push(KdlEntry::new(self.multicall));
            nodes.push(node);
        }
        if self.cmd.external_subcommand {
            let mut node = KdlNode::new("external_subcommand");
            node.push(KdlEntry::new(true));
            nodes.push(node);
        }
        if self.cmd.arg_required_else_help {
            let mut node = KdlNode::new("arg_required_else_help");
            node.push(KdlEntry::new(true));
            nodes.push(node);
        }
        if self.cmd.disable_help_flag {
            let mut node = KdlNode::new("disable_help_flag");
            node.push(KdlEntry::new(true));
            nodes.push(node);
        }
        if self.cmd.disable_help_subcommand {
            let mut node = KdlNode::new("disable_help_subcommand");
            node.push(KdlEntry::new(true));
            nodes.push(node);
        }
        if self.cmd.disable_version_flag {
            let mut node = KdlNode::new("disable_version_flag");
            node.push(KdlEntry::new(true));
            nodes.push(node);
        }
        if self.cmd.dont_delimit_trailing_values {
            let mut node = KdlNode::new("dont_delimit_trailing_values");
            node.push(true);
            nodes.push(node);
        }
        if !self.cmd.args_override_self {
            let mut node = KdlNode::new("args_override_self");
            node.push(false);
            nodes.push(node);
        }
        if self.cmd.subcommand_negates_reqs {
            let mut node = KdlNode::new("subcommand_negates_reqs");
            node.push(true);
            nodes.push(node);
        }
        if self.cmd.args_conflicts_with_subcommands {
            let mut node = KdlNode::new("args_conflicts_with_subcommands");
            node.push(true);
            nodes.push(node);
        }
        if self.cmd.subcommand_precedence_over_arg {
            let mut node = KdlNode::new("subcommand_precedence_over_arg");
            node.push(true);
            nodes.push(node);
        }
        if self.cmd.allow_missing_positional {
            let mut node = KdlNode::new("allow_missing_positional");
            node.push(true);
            nodes.push(node);
        }
        if let Some(message) = &self.cmd.deprecated {
            let mut node = KdlNode::new("deprecated");
            node.push(string_entry(None, message));
            nodes.push(node);
        }
        if let Some(at) = &self.cmd.deprecated_warn_at {
            let mut node = KdlNode::new("deprecated_warn_at");
            node.push(string_entry(None, at));
            nodes.push(node);
        }
        if let Some(at) = &self.cmd.deprecated_remove_at {
            let mut node = KdlNode::new("deprecated_remove_at");
            node.push(string_entry(None, at));
            nodes.push(node);
        }
        if self.cmd.subcommand_required && !self.cmd.subcommands.is_empty() {
            let mut node = KdlNode::new("subcommand_required");
            node.push(true);
            nodes.push(node);
        }
        if let Some(heading) = &self.cmd.subcommand_help_heading {
            let mut node = KdlNode::new("subcommand_help_heading");
            node.push(string_entry(None, heading));
            nodes.push(node);
        }
        if let Some(name) = &self.cmd.subcommand_value_name {
            let mut node = KdlNode::new("subcommand_value_name");
            node.push(string_entry(None, name));
            nodes.push(node);
        }
        if self.cmd.next_line_help {
            let mut node = KdlNode::new("next_line_help");
            node.push(true);
            nodes.push(node);
        }
        if self.cmd.flatten_help {
            let mut node = KdlNode::new("flatten_help");
            node.push(true);
            nodes.push(node);
        }
        if let Some(width) = self.cmd.term_width {
            let mut node = KdlNode::new("term_width");
            node.push(width as i128);
            nodes.push(node);
        }
        if let Some(width) = self.cmd.max_term_width {
            let mut node = KdlNode::new("max_term_width");
            node.push(width as i128);
            nodes.push(node);
        }
        if !self.usage.is_empty() {
            let mut node = KdlNode::new("usage");
            node.push(string_entry(None, &self.usage));
            nodes.push(node);
        }
        for flag in self.cmd.flags.iter() {
            nodes.push(flag.into());
        }
        for arg in self.cmd.args.iter() {
            nodes.push(arg.into());
        }
        // Written here rather than by SpecCommand, because the root's own nodes
        // live at the top level of the document instead of inside a `cmd` block.
        for mount in self.cmd.mounts.iter() {
            nodes.push(mount.into());
        }
        for group in self.cmd.groups.iter() {
            nodes.push(group.into());
        }
        for example in self.examples.iter() {
            nodes.push(example.into());
        }
        for complete in self.complete.values() {
            nodes.push(complete.into());
        }
        for complete in self.cmd.complete.values() {
            nodes.push(complete.into());
        }
        for view in self.views.values() {
            let rendered: KdlDocument = view
                .to_string()
                .parse()
                .expect("a view always renders valid KDL");
            nodes.extend(rendered.nodes().iter().cloned());
        }
        for cmd in self.cmd.subcommands.values() {
            nodes.push(cmd.into())
        }
        if !self.config.is_empty() {
            nodes.push((&self.config).into());
        }
        doc.autoformat_config(&kdl::FormatConfigBuilder::new().build());
        write!(f, "{doc}")
    }
}

impl FromStr for Spec {
    type Err = UsageErr;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        Self::parse(&Default::default(), s)
    }
}

#[cfg(feature = "clap")]
impl From<&clap::Command> for Spec {
    fn from(cmd: &clap::Command) -> Self {
        let mut spec = Spec {
            name: cmd.get_name().to_string(),
            bin: cmd.get_bin_name().unwrap_or(cmd.get_name()).to_string(),
            cmd: cmd.into(),
            version: cmd.get_version().map(|v| v.to_string()),
            long_version: cmd.get_long_version().map(|v| v.to_string()),
            about: cmd.get_about().map(|a| a.to_string()),
            about_long: cmd.get_long_about().map(|a| a.to_string()),
            usage: cmd.clone().render_usage().to_string(),
            // The root is a command too, and its own answer has nowhere else to go: a spec says
            // this at the top level, which is the field a reader puts it back into.
            unknown_flags: crate::spec::cmd::SpecCommand::from(cmd).unknown_flags,
            multicall: cmd.is_multicall_set(),
            multicall_set: cmd.is_multicall_set(),
            ..Default::default()
        };
        // The same pass the KDL parser makes, and for the same reason: a command has to know
        // where it sits. Without it every subcommand of a clap-derived spec had `full_cmd`
        // empty — and `SpecCommand::usage()` joins `full_cmd`, so their usage lines came out
        // blank. Now they say what a user would type.
        set_subcommand_ancestors(&mut spec.cmd, &[]);
        spec
    }
}

#[inline]
pub fn is_true(b: &bool) -> bool {
    *b
}

#[inline]
pub fn is_false(b: &bool) -> bool {
    !is_true(b)
}

#[cfg(test)]
mod tests {
    use super::*;
    use insta::assert_snapshot;

    #[test]
    fn test_display() {
        let spec = Spec::parse(
            &Default::default(),
            r#"
name "Usage CLI"
bin "usage"
arg "arg1"
flag "-f --force" global=#true
cmd "config" {
  cmd "set" {
    arg "key" help="Key to set"
    arg "value"
  }
}
complete "file" run="ls" descriptions=#true
        "#,
        )
        .unwrap();
        assert_snapshot!(spec, @r#"
        name "Usage CLI"
        bin usage
        flag "-f --force" global=#true
        arg <arg1>
        complete file run=ls descriptions=#true
        cmd config {
            cmd set {
                arg <key> help="Key to set"
                arg <value>
            }
        }
        "#);
    }

    #[test]
    fn test_repository_round_trips() {
        let spec = Spec::parse(
            &Default::default(),
            r#"
bin "mise"
repository "https://github.com/jdx/mise"
source_code_link_template "https://github.com/jdx/mise/blob/main/src/cli/{{path}}.rs"
        "#,
        )
        .unwrap();
        assert_eq!(
            spec.repository.as_deref(),
            Some("https://github.com/jdx/mise")
        );
        // A spec that is parsed and re-emitted must not lose it, which is the
        // failure mode for every field added to this struct.
        assert_snapshot!(spec, @r#"
        name mise
        bin mise
        source_code_link_template "https://github.com/jdx/mise/blob/main/src/cli/{{path}}.rs"
        repository "https://github.com/jdx/mise"
        "#);
    }

    #[test]
    fn test_repository_merges_like_the_other_optionals() {
        // Extra specs are merged over a generated one, which is how a clap CLI
        // declares anything clap has no concept of.
        let mut generated = Spec::parse(&Default::default(), r#"bin "mise""#).unwrap();
        let extra = Spec::parse(
            &Default::default(),
            r#"repository "https://github.com/jdx/mise""#,
        )
        .unwrap();
        generated.merge(extra);
        assert_eq!(
            generated.repository.as_deref(),
            Some("https://github.com/jdx/mise")
        );
    }

    #[test]
    #[cfg(feature = "clap")]
    fn test_clap() {
        let cmd = clap::Command::new("test");
        assert_snapshot!(Spec::from(&cmd), @r#"
        name test
        bin test
        unknown_flags error
        args_override_self #false
        usage "Usage: test"
        "#);
    }

    #[test]
    #[cfg(feature = "clap")]
    fn a_clap_subcommand_knows_where_it_sits() {
        // The KDL parser makes this pass; the clap conversion did not, so every subcommand of
        // a clap-derived spec had an empty `full_cmd`. Two things read it: `usage()`, which
        // joins it and so produced a usage line with no command in it, and help rendering,
        // which uses it to tell a subcommand's page from the program's.
        let cmd = clap::Command::new("ex").subcommand(
            clap::Command::new("go")
                .about("Go somewhere")
                .subcommand(clap::Command::new("fast").about("Quickly")),
        );
        let spec = Spec::from(&cmd);

        let go = spec.cmd.subcommands.get("go").expect("go");
        assert_eq!(go.full_cmd, ["go"]);
        // `usage()` names the command and then what it takes — `go` has a subcommand, so it
        // says so. The point is that the command's own name is in there at all.
        assert_eq!(go.usage, "go <SUBCOMMAND>");

        // And all the way down, which is what makes it a walk rather than one level.
        let fast = go.subcommands.get("fast").expect("fast");
        assert_eq!(fast.full_cmd, ["go", "fast"]);
        assert_eq!(fast.usage, "go fast");
    }

    #[test]
    #[cfg(feature = "clap")]
    fn a_delimited_default_becomes_the_values_clap_would_split_it_into() {
        // clap splits by the delimiter before anyone sees a value, defaults included, so the
        // joined string is not something the CLI ever holds. The spec has no delimiter — it has
        // a list, which says the same thing.
        //
        // mise's `--fs-events` is why: `default_value = "create,remove,rename,modify,metadata"`
        // beside `value_parser` listing those as its choices, so the recorded default was a
        // single value its own spec forbade.
        let cmd = clap::Command::new("test").arg(
            clap::Arg::new("events")
                .long("events")
                .value_delimiter(',')
                .action(clap::ArgAction::Append)
                .value_parser(["a", "b", "c"])
                .default_value("a,b"),
        );
        let spec = Spec::from(&cmd);
        let flag = spec.cmd.flags.iter().find(|f| f.name == "events").unwrap();
        assert_eq!(flag.default, ["a", "b"]);

        // And without a delimiter the value is whatever was written, commas and all: a path list
        // is not every CLI's idea of a separator, so splitting on speculation would be worse.
        let cmd = clap::Command::new("test")
            .arg(clap::Arg::new("events").long("events").default_value("a,b"));
        let spec = Spec::from(&cmd);
        let flag = spec.cmd.flags.iter().find(|f| f.name == "events").unwrap();
        assert_eq!(flag.default, ["a,b"]);
    }

    #[test]
    fn multicall_round_trips() {
        let spec = Spec::parse(
            &Default::default(),
            r#"
name "busybox"
bin "busybox"
multicall #true
cmd "ls"
cmd "cat"
        "#,
        )
        .unwrap();
        assert!(spec.multicall);
        let emitted = spec.to_string();
        assert!(
            emitted.contains("multicall #true"),
            "lost on the way out: {emitted}"
        );
        let again: Spec = emitted.parse().unwrap();
        assert!(again.multicall);
    }

    #[test]
    fn a_declared_view_promotes_a_command_and_selected_globals() {
        let spec: Spec = r#"
name "aube"
bin "aube"
about_md "Host **markdown**"
before_help "host before"
before_long_help "host long before"
after_help "host after"
after_long_help "host long after"
example "aube host" header="host example"
flag "-v --verbose" global=#true
flag "--config <FILE>" global=#true
view "aubr" root="run" {
  global "--verbose"
}
cmd "run" help="Run a package script" {
  before_help "run before"
  before_long_help "run long before"
  after_help "run after"
  after_long_help "run long after"
  example "aubr task" header="run example"
  flag "--if-present"
  arg "[SCRIPT]"
  cmd "nested"
}
"#
        .parse()
        .unwrap();

        let rendered = spec.to_string();
        assert!(rendered.contains("view aubr root=run"), "{rendered}");
        let reparsed: Spec = rendered.parse().unwrap();
        let applet = reparsed.for_view("aubr").unwrap();
        assert_eq!(applet.name, "aubr");
        assert_eq!(applet.bin, "aubr");
        assert_eq!(applet.about.as_deref(), Some("Run a package script"));
        assert_eq!(applet.about_md, None);
        assert_eq!(applet.before_help.as_deref(), Some("run before"));
        assert_eq!(applet.before_help_long.as_deref(), Some("run long before"));
        assert_eq!(applet.after_help.as_deref(), Some("run after"));
        assert_eq!(applet.after_help_long.as_deref(), Some("run long after"));
        assert_eq!(applet.examples.len(), 1);
        assert_eq!(applet.examples[0].header.as_deref(), Some("run example"));
        assert!(applet.cmd.flags.iter().any(|flag| flag.name == "verbose"));
        assert!(applet
            .cmd
            .flags
            .iter()
            .any(|flag| flag.name == "if-present"));
        assert!(!applet.cmd.flags.iter().any(|flag| flag.name == "config"));
        assert!(applet.cmd.subcommands.contains_key("nested"));
        assert!(applet.views.is_empty());
        assert!(applet.to_string().contains("bin aubr"));
    }

    #[test]
    fn a_view_preserves_the_host_version_entry_policy() {
        let spec: Spec = r#"
bin "host"
version "1.2.3"
disable_version_flag #true
view "runner" root=run
cmd "run"
"#
        .parse()
        .unwrap();

        let view = spec.for_view("runner").unwrap();
        assert!(view.cmd.disable_version_flag);
        let emitted = view.to_string();
        assert!(emitted.contains("disable_version_flag #true"), "{emitted}");
        let reparsed: Spec = emitted.parse().unwrap();
        assert!(reparsed.cmd.disable_version_flag);
    }

    #[test]
    fn a_promoted_flag_shadows_every_carried_global_spelling() {
        let spec: Spec = r#"
bin "host"
flag "--color" global=#true negate="--no-color"
view "runner" root=run {
  global "--color"
}
cmd "run" {
  flag "--no-color"
}
"#
        .parse()
        .unwrap();

        let view = spec.for_view("runner").unwrap();
        assert_eq!(view.cmd.flags.len(), 1);
        assert_eq!(view.cmd.flags[0].long, ["no-color"]);
    }

    #[test]
    fn a_view_keeps_only_its_own_and_carried_global_completers() {
        let spec: Spec = r#"
bin "host"
flag "--host <HOST>" global=#true
flag "--carried <CARRIED>" global=#true
complete "host" run="host candidates"
complete "carried" run="carried candidates"
view "runner" root=run {
  global "--carried"
}
cmd "run" {
  arg "<HOST>"
  complete "host" run="view candidates"
}
"#
        .parse()
        .unwrap();

        let view = spec.for_view("runner").unwrap();
        assert_eq!(
            view.complete.get("host").unwrap().run.as_deref(),
            Some("view candidates")
        );
        assert_eq!(
            view.complete.get("carried").unwrap().run.as_deref(),
            Some("carried candidates")
        );
        assert!(view.cmd.complete.is_empty());
        assert_eq!(view.complete.len(), 2);
        assert_eq!(view.to_string().matches("complete ").count(), 2);
    }

    #[test]
    fn a_view_projects_groups_of_carried_globals() {
        let spec: Spec = r#"
bin "host"
flag "--json" global=#true
flag "--yaml" global=#true
flag "--toml" global=#true
group "format" "--json" "--yaml" "--toml" required=#true
view "all" root=run globals=#true
view "json" root=run {
  global "--json"
}
cmd "run"
"#
        .parse()
        .unwrap();

        let all = spec.for_view("all").unwrap();
        assert_eq!(all.cmd.groups.len(), 1);
        assert_eq!(all.cmd.groups[0].members, ["--json", "--yaml", "--toml"]);
        assert!(all.to_string().parse::<Spec>().is_ok());

        let json = spec.for_view("json").unwrap();
        assert!(json.cmd.groups.is_empty());
        assert!(
            json.cmd
                .flags
                .iter()
                .find(|flag| flag.name == "json")
                .unwrap()
                .required
        );
        assert!(json.to_string().parse::<Spec>().is_ok());
    }

    #[test]
    fn a_view_refuses_unknown_commands_and_non_global_carryovers() {
        let missing: Spec = "bin \"ex\"\nview \"x\" root=missing\n".parse().unwrap();
        assert!(missing
            .for_view("x")
            .unwrap_err()
            .to_string()
            .contains("missing"));

        let local: Spec =
            "bin \"ex\"\nflag \"--local\"\nview \"x\" root=go { global \"--local\" }\ncmd go\n"
                .parse()
                .unwrap();
        assert!(local
            .for_view("x")
            .unwrap_err()
            .to_string()
            .contains("not a root global"));
    }

    #[test]
    fn an_included_spec_can_enable_or_disable_multicall() {
        let dir = tempfile::tempdir().unwrap();
        let included = dir.path().join("included.usage.kdl");
        let root = dir.path().join("root.usage.kdl");

        std::fs::write(&included, "multicall #false\n").unwrap();
        std::fs::write(
            &root,
            "multicall #true\ninclude file=\"./included.usage.kdl\"\n",
        )
        .unwrap();
        let spec = Spec::parse_file(&root).unwrap();
        assert!(!spec.multicall);
        assert!(spec.to_string().contains("multicall #false"));

        std::fs::write(&included, "multicall #true\n").unwrap();
        std::fs::write(
            &root,
            "multicall #false\ninclude file=\"./included.usage.kdl\"\n",
        )
        .unwrap();
        let spec = Spec::parse_file(&root).unwrap();
        assert!(spec.multicall);
        assert!(spec.to_string().contains("multicall #true"));
    }

    #[test]
    #[cfg(feature = "clap")]
    fn multicall_comes_across_from_clap() {
        let cmd = clap::Command::new("busybox")
            .multicall(true)
            .subcommand(clap::Command::new("ls"))
            .subcommand(clap::Command::new("cat"));
        let spec = Spec::from(&cmd);
        assert!(spec.multicall);
        assert!(
            spec.to_string().contains("multicall #true"),
            "{}",
            spec.to_string()
        );

        let plain = clap::Command::new("ex").subcommand(clap::Command::new("ls"));
        assert!(!Spec::from(&plain).multicall);
    }

    macro_rules! extract_usage_tests {
        ($($name:ident: $input:expr, $expected:expr,)*) => {
        $(
            #[test]
            fn $name() {
                let result = extract_usage_from_comments($input);
                let expected = $expected.trim_start_matches('\n').trim_end();
                assert_eq!(result, expected);
            }
        )*
        }
    }

    extract_usage_tests! {
        test_extract_usage_from_comments_original_hash:
            r#"
#!/bin/bash
#USAGE bin "test"
#USAGE flag "--foo" help="test"
echo "hello"
            "#,
            r#"
bin "test"
flag "--foo" help="test"
            "#,

        test_extract_usage_from_comments_original_double_slash:
            r#"
#!/usr/bin/env node
//USAGE bin "test"
//USAGE flag "--foo" help="test"
console.log("hello");
            "#,
            r#"
bin "test"
flag "--foo" help="test"
            "#,

        test_extract_usage_from_comments_bracket_with_space:
            r#"
#!/bin/bash
# [USAGE] bin "test"
# [USAGE] flag "--foo" help="test"
echo "hello"
            "#,
            r#"
bin "test"
flag "--foo" help="test"
            "#,

        test_extract_usage_from_comments_bracket_no_space:
            r#"
#!/bin/bash
#[USAGE] bin "test"
#[USAGE] flag "--foo" help="test"
echo "hello"
            "#,
            r#"
bin "test"
flag "--foo" help="test"
            "#,

        test_extract_usage_from_comments_double_slash_bracket_with_space:
            r#"
#!/usr/bin/env node
// [USAGE] bin "test"
// [USAGE] flag "--foo" help="test"
console.log("hello");
            "#,
            r#"
bin "test"
flag "--foo" help="test"
            "#,

        test_extract_usage_from_comments_double_slash_bracket_no_space:
            r#"
#!/usr/bin/env node
//[USAGE] bin "test"
//[USAGE] flag "--foo" help="test"
console.log("hello");
            "#,
            r#"
bin "test"
flag "--foo" help="test"
            "#,

        test_extract_usage_from_comments_stops_at_gap:
            r#"
#!/bin/bash
#USAGE bin "test"
#USAGE flag "--foo" help="test"

#USAGE flag "--bar" help="should not be included"
echo "hello"
            "#,
            r#"
bin "test"
flag "--foo" help="test"
            "#,

        test_extract_usage_from_comments_with_content_after_marker:
            r#"
#!/bin/bash
# [USAGE] bin "test"
# [USAGE] flag "--verbose" help="verbose mode"
# [USAGE] arg "input" help="input file"
echo "hello"
            "#,
            r#"
bin "test"
flag "--verbose" help="verbose mode"
arg "input" help="input file"
            "#,

        test_extract_usage_from_comments_double_colon_original:
            r#"
::USAGE bin "test"
::USAGE flag "--foo" help="test"
echo "hello"
            "#,
            r#"
bin "test"
flag "--foo" help="test"
            "#,

        test_extract_usage_from_comments_double_colon_bracket_with_space:
            r#"
:: [USAGE] bin "test"
:: [USAGE] flag "--foo" help="test"
echo "hello"
            "#,
            r#"
bin "test"
flag "--foo" help="test"
            "#,

        test_extract_usage_from_comments_double_colon_bracket_no_space:
            r#"
::[USAGE] bin "test"
::[USAGE] flag "--foo" help="test"
echo "hello"
            "#,
            r#"
bin "test"
flag "--foo" help="test"
            "#,

        test_extract_usage_from_comments_double_colon_stops_at_gap:
            r#"
::USAGE bin "test"
::USAGE flag "--foo" help="test"

::USAGE flag "--bar" help="should not be included"
echo "hello"
            "#,
            r#"
bin "test"
flag "--foo" help="test"
            "#,

        test_extract_usage_from_comments_double_colon_with_content_after_marker:
            r#"
::USAGE bin "test"
::USAGE flag "--verbose" help="verbose mode"
::USAGE arg "input" help="input file"
echo "hello"
            "#,
            r#"
bin "test"
flag "--verbose" help="verbose mode"
arg "input" help="input file"
            "#,

        test_extract_usage_from_comments_double_colon_bracket_with_space_multiple_lines:
            r#"
:: [USAGE] bin "myapp"
:: [USAGE] flag "--config <file>" help="config file"
:: [USAGE] flag "--verbose" help="verbose output"
:: [USAGE] arg "input" help="input file"
:: [USAGE] arg "[output]" help="output file" required=#false
echo "done"
            "#,
            r#"
bin "myapp"
flag "--config <file>" help="config file"
flag "--verbose" help="verbose output"
arg "input" help="input file"
arg "[output]" help="output file" required=#false
            "#,

        test_extract_usage_from_comments_empty:
            r#"
#!/bin/bash
echo "hello"
            "#,
            "",

        test_extract_usage_from_comments_lowercase_usage:
            r#"
#!/bin/bash
#usage bin "test"
#usage flag "--foo" help="test"
echo "hello"
            "#,
            "",

        test_extract_usage_from_comments_mixed_case_usage:
            r#"
#!/bin/bash
#Usage bin "test"
#Usage flag "--foo" help="test"
echo "hello"
            "#,
            "",

        test_extract_usage_from_comments_space_before_usage:
            r#"
#!/bin/bash
# USAGE bin "test"
# USAGE flag "--foo" help="test"
echo "hello"
            "#,
            "",

        test_extract_usage_from_comments_double_slash_lowercase:
            r#"
#!/usr/bin/env node
//usage bin "test"
//usage flag "--foo" help="test"
console.log("hello");
            "#,
            "",

        test_extract_usage_from_comments_double_slash_mixed_case:
            r#"
#!/usr/bin/env node
//Usage bin "test"
//Usage flag "--foo" help="test"
console.log("hello");
            "#,
            "",

        test_extract_usage_from_comments_double_slash_space_before_usage:
            r#"
#!/usr/bin/env node
// USAGE bin "test"
// USAGE flag "--foo" help="test"
console.log("hello");
            "#,
            "",

        test_extract_usage_from_comments_bracket_lowercase:
            r#"
#!/bin/bash
#[usage] bin "test"
#[usage] flag "--foo" help="test"
echo "hello"
            "#,
            "",

        test_extract_usage_from_comments_bracket_mixed_case:
            r#"
#!/bin/bash
#[Usage] bin "test"
#[Usage] flag "--foo" help="test"
echo "hello"
            "#,
            "",

        test_extract_usage_from_comments_bracket_space_lowercase:
            r#"
#!/bin/bash
# [usage] bin "test"
# [usage] flag "--foo" help="test"
echo "hello"
            "#,
            "",

        test_extract_usage_from_comments_double_colon_lowercase:
            r#"
::usage bin "test"
::usage flag "--foo" help="test"
echo "hello"
            "#,
            "",

        test_extract_usage_from_comments_double_colon_mixed_case:
            r#"
::Usage bin "test"
::Usage flag "--foo" help="test"
echo "hello"
            "#,
            "",

        test_extract_usage_from_comments_double_colon_space_before_usage:
            r#"
:: USAGE bin "test"
:: USAGE flag "--foo" help="test"
echo "hello"
            "#,
            "",

        test_extract_usage_from_comments_double_colon_bracket_lowercase:
            r#"
::[usage] bin "test"
::[usage] flag "--foo" help="test"
echo "hello"
            "#,
            "",

        test_extract_usage_from_comments_double_colon_bracket_mixed_case:
            r#"
::[Usage] bin "test"
::[Usage] flag "--foo" help="test"
echo "hello"
            "#,
            "",

        test_extract_usage_from_comments_double_colon_bracket_space_lowercase:
            r#"
:: [usage] bin "test"
:: [usage] flag "--foo" help="test"
echo "hello"
            "#,
            "",
    }

    #[test]
    fn test_spec_with_examples() {
        let spec = Spec::parse(
            &Default::default(),
            r#"
name "demo"
bin "demo"
example "demo --help" header="Getting help" help="Display help information"
example "demo --version" header="Check version"
        "#,
        )
        .unwrap();

        assert_eq!(spec.examples.len(), 2);

        assert_eq!(spec.examples[0].code, "demo --help");
        assert_eq!(spec.examples[0].header, Some("Getting help".to_string()));
        assert_eq!(
            spec.examples[0].help,
            Some("Display help information".to_string())
        );

        assert_eq!(spec.examples[1].code, "demo --version");
        assert_eq!(spec.examples[1].header, Some("Check version".to_string()));
        assert_eq!(spec.examples[1].help, None);
    }

    #[test]
    fn test_spec_examples_display() {
        let spec = Spec::parse(
            &Default::default(),
            r#"
name "demo"
bin "demo"
example "demo --help" header="Getting help" help="Show help"
example "demo --version"
        "#,
        )
        .unwrap();

        let output = format!("{}", spec);
        assert!(
            output.contains("example \"demo --help\" header=\"Getting help\" help=\"Show help\"")
        );
        assert!(output.contains("example \"demo --version\""));
    }

    #[test]
    fn test_parse_script_str() {
        let spec = Spec::parse_script_str(
            r#"
#!/bin/bash
#USAGE bin "test"
#USAGE flag "--foo" help="test"
echo "hello"
            "#,
        )
        .unwrap();

        assert_eq!(spec.bin, "test");
        assert_eq!(spec.name, "test");
        assert_eq!(spec.cmd.flags.len(), 1);
        assert_eq!(spec.cmd.flags[0].long, ["foo"]);
    }

    #[test]
    fn test_parse_script_str_rejects_relative_includes() {
        let err = Spec::parse_script_str(r#"#USAGE include file="relative.usage.kdl""#)
            .expect_err("relative includes need a source path");

        match err {
            UsageErr::InvalidInput(msg, _, _) => {
                assert_eq!(msg, "relative includes require a source file");
            }
            err => panic!("unexpected error: {err:?}"),
        }
    }

    #[test]
    fn test_include_does_not_infer_metadata_from_included_filename() {
        let dir = tempfile::tempdir().unwrap();
        let included = dir.path().join("overrides.usage.kdl");
        let root = dir.path().join("my-script.usage.kdl");
        std::fs::write(&included, "").unwrap();
        std::fs::write(&root, "include file=\"./overrides.usage.kdl\"\n").unwrap();

        let spec = Spec::parse_file(&root).unwrap();

        assert_eq!(spec.name, "my-script.usage.kdl");
        assert_eq!(spec.bin, "my-script.usage.kdl");
        assert!(spec.cmd.name.is_empty());
    }

    #[test]
    fn injected_nested_mounts_ignore_the_mounted_specs_root_default() {
        let mut spec: Spec = "mount run=outer".parse().unwrap();
        let outputs = HashMap::from([
            (
                "outer".to_string(),
                "default_subcommand run\nmount run=nested\ncmd run".to_string(),
            ),
            ("nested".to_string(), "cmd leaf".to_string()),
        ]);

        spec.resolve_mount_outputs(&outputs).unwrap();

        assert!(spec.cmd.subcommands.contains_key("leaf"));
    }
}
