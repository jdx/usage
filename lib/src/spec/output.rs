//! What a command writes, and how a consumer should read it.
//!
//! A spec has always described a command's inputs exhaustively and its outputs not at
//! all. Across the jdx.dev fleet that gap is 123 hand-written flag declarations in three
//! incompatible spellings — mise and pitchfork say `-J --json`, hk says
//! `--format=human|json|jsonl`, aube says both `--json` and
//! `--reporter=default|append-only|ndjson|silent` — and not one of them says what the
//! JSON contains.
//!
//! ```kdl
//! cmd "check" {
//!     output "human" default=#true help="Human-readable report"
//!     output "json"  framing="json"  help="One report object"  { schema #"""{…}"""# }
//!     output "jsonl" framing="jsonl" help="One event per line" { schema #"""{…}"""# }
//!     select "--format"
//! }
//! ```
//!
//! # Name and framing are not the same thing
//!
//! The positional is the token a *user* types. [`SpecOutput::framing`] is the contract a
//! *consumer* reads. They are separate because aube spells its line-delimited output
//! `ndjson` and hk spells the identical wire format `jsonl`: a generated SDK that keyed
//! off the name would offer `exec_ndjson()` for one and `exec_jsonl()` for the other, and
//! a caller would be back to knowing which CLI they were talking to — the thing this
//! exists to delete.
//!
//! # Two ways to ask, one model
//!
//! A command-level `select "--format"` names a flag whose *value* picks an output. An
//! `output "json" select="--json"` names a boolean flag whose *presence* picks that one.
//! Both are common in the wild and both lower here; [`SpecOutput::select_argv`] answers
//! "which words pick this" so no consumer has to know which spelling was used.
//!
//! # Selection is resolved, not just recorded
//!
//! [`resolve_selectors`] runs once after the whole document is read and fills the
//! selecting flag's `choices` from the output names. That is what lets completion, the
//! docs renderers, the fig exporter and the SDK choice types all work without any of them
//! learning about outputs. Two consequences worth knowing before reading a re-emitted
//! spec, both documented on `docs/spec/reference/output.md`:
//!
//! - the choices appear in the output even though the source did not write them, the same
//!   way `include` and `flagset` expansion do not survive a round trip;
//! - a `select` naming an inherited global gets a narrowed copy of that flag inside the
//!   command, because two commands under one global rarely produce the same outputs.
//!
//! Resolution is idempotent: a second pass sees choices that already match and validates
//! them instead of rewriting, so a spec that has been through it round-trips unchanged.

use std::collections::BTreeSet;
use std::path::Path;

use kdl::{KdlDocument, KdlEntry, KdlNode};
use serde::Serialize;
use strum::{Display as StrumDisplay, EnumString};

use crate::error::{Result, UsageErr};
use crate::spec::choices::{SpecChoice, SpecChoices};
use crate::spec::cmd::SpecCommand;
use crate::spec::context::ParsingContext;
use crate::spec::helpers::{string_entry, NodeHelper};
use crate::spec::{is_false, Spec};
use crate::SpecFlag;

/// Selection is resolved after the whole document is read, so there is no node span left
/// to point at. Same shape as the view checks, for the same reason.
fn invalid(msg: String) -> UsageErr {
    UsageErr::InvalidOutput(msg)
}

/// The wire format of what a command writes to stdout.
///
/// This is the machine contract, distinct from the name a user types for it. It is what
/// decides a consumer's *shape*: [`Framing::Json`] is read to EOF and parsed once, so a
/// generated client returns a value; [`Framing::Jsonl`] arrives a line at a time and may
/// never end, so a generated client has to return an iterator and must not buffer.
#[derive(
    Debug, Default, Copy, Clone, PartialEq, Eq, PartialOrd, Ord, EnumString, StrumDisplay, Serialize,
)]
#[strum(serialize_all = "snake_case")]
#[serde(rename_all = "snake_case")]
pub enum Framing {
    /// Human-readable text. Nothing may be assumed about its structure, and it is the
    /// default because an output that says nothing about its framing is prose.
    #[default]
    Text,
    /// One JSON document, read to end of stream.
    Json,
    /// One JSON document per line. Line-delimited, unbounded, and read incrementally —
    /// `ndjson` is the same thing under another name.
    Jsonl,
}

impl Framing {
    pub fn as_str(&self) -> &'static str {
        match self {
            Framing::Text => "text",
            Framing::Json => "json",
            Framing::Jsonl => "jsonl",
        }
    }

    /// Whether a consumer has to read this incrementally rather than to the end.
    pub fn is_streaming(&self) -> bool {
        matches!(self, Framing::Jsonl)
    }
}

/// The values `framing=` accepts, for error messages.
pub(crate) const FRAMING_VALUES: &str = "text, json, jsonl";

/// How a command's output is asked for.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[serde(tag = "kind", rename_all = "snake_case")]
pub enum Selector {
    /// A flag whose value names the output: `--format jsonl`.
    Value { flag: String, value: String },
    /// A boolean flag whose presence picks it: `--json`.
    Present { flag: String },
}

impl Selector {
    /// The words that pick this output, ready to append to an argv.
    pub fn argv(&self) -> Vec<String> {
        match self {
            Selector::Value { flag, value } => vec![flag.clone(), value.clone()],
            Selector::Present { flag } => vec![flag.clone()],
        }
    }

    /// The flag doing the selecting, with its leading dashes.
    pub fn flag(&self) -> &str {
        match self {
            Selector::Value { flag, .. } | Selector::Present { flag } => flag,
        }
    }
}

/// One thing a command can write.
#[derive(Debug, Default, Clone, Serialize)]
#[non_exhaustive]
pub struct SpecOutput {
    /// The token a user types for it, e.g. `human`, `json`, `ndjson`.
    pub name: String,
    /// The wire format, which is what a consumer keys off.
    #[serde(skip_serializing_if = "is_text")]
    pub framing: Framing,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub help: Option<String>,
    /// A JSON Schema for what is written, carried verbatim.
    ///
    /// Opaque on purpose: usage-lib has no runtime JSON dependency and is not going to
    /// grow one to hold a string it never inspects. Consumers that want it parsed —
    /// today that is only the MCP server — parse it themselves and fall back to the raw
    /// text, so a malformed schema degrades rather than failing a spec.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub schema: Option<String>,
    /// What the command writes when nothing selects otherwise.
    #[serde(skip_serializing_if = "is_false")]
    pub default: bool,
    /// Declared in order to be taken away: a command that inherits a CLI-wide output it
    /// cannot produce redeclares the name with `hide=#true`. The same spelling as a
    /// hidden `choice` or `alias`.
    #[serde(skip_serializing_if = "is_false")]
    pub hide: bool,
    /// A boolean flag whose presence picks this output, for the `--json` spelling.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub select: Option<String>,
}

fn is_text(framing: &Framing) -> bool {
    *framing == Framing::Text
}

impl SpecOutput {
    pub fn new(name: impl Into<String>) -> Self {
        Self {
            name: name.into(),
            ..Default::default()
        }
    }

    pub fn framing(mut self, framing: Framing) -> Self {
        self.framing = framing;
        self
    }

    pub fn help(mut self, help: impl Into<String>) -> Self {
        self.help = Some(help.into());
        self
    }

    pub fn schema(mut self, schema: impl Into<String>) -> Self {
        self.schema = Some(schema.into());
        self
    }

    pub fn default_output(mut self) -> Self {
        self.default = true;
        self
    }

    /// How this output is asked for, given the command it belongs to.
    ///
    /// [`None`] when no flag selects it. This is valid for an always-produced output;
    /// generated per-output SDK methods require a selector and therefore omit it.
    pub fn select_argv(&self, cmd: &SpecCommand) -> Option<Selector> {
        self.select_argv_with(cmd.select.as_deref())
    }

    pub(crate) fn select_argv_with(&self, command_select: Option<&str>) -> Option<Selector> {
        if let Some(flag) = &self.select {
            return Some(Selector::Present { flag: flag.clone() });
        }
        command_select.map(|flag| Selector::Value {
            flag: flag.to_string(),
            value: self.name.clone(),
        })
    }

    pub(crate) fn parse(ctx: &ParsingContext, node: &NodeHelper) -> Result<Self> {
        node.ensure_arg_len(1..=1)?;
        let mut output = SpecOutput::new(node.arg(0)?.ensure_string()?);
        for (k, v) in node.props() {
            match k {
                "framing" => output.framing = parse_framing(ctx, v.ensure_string()?, v.entry)?,
                "help" => output.help = Some(v.ensure_string()?),
                "schema" => output.schema = Some(v.ensure_string()?),
                "default" => output.default = v.ensure_bool()?,
                "hide" => output.hide = v.ensure_bool()?,
                "select" => output.select = Some(v.ensure_string()?),
                k => bail_parse!(ctx, v.entry.span(), "unsupported output key {k}"),
            }
        }
        for child in node.children() {
            match child.name() {
                // A schema is the one field long enough to want its own line, and long
                // text already spells itself this way: `long_help`, `help_md` and the
                // before/after help blocks are all child nodes for the same reason.
                "schema" => parse_schema(ctx, &child, &mut output)?,
                "help" => output.help = Some(child.arg(0)?.ensure_string()?),
                k => bail_parse!(
                    ctx,
                    child.node.name().span(),
                    "unsupported output value key {k}"
                ),
            }
        }
        if output.name.is_empty() {
            bail_parse!(ctx, node.span(), "an output needs a name");
        }
        Ok(output)
    }
}

fn parse_schema(ctx: &ParsingContext, node: &NodeHelper, output: &mut SpecOutput) -> Result<()> {
    if let Some(file) = node.get("file") {
        node.ensure_arg_len(0..=0)?;
        for (key, value) in node.props() {
            if key != "file" {
                bail_parse!(ctx, value.entry.span(), "unsupported schema key {key}");
            }
        }
        let declared = file.ensure_string()?;
        let path = Path::new(&declared);
        let path = if path.is_relative() {
            ctx.file
                .parent()
                .filter(|_| !ctx.file.as_os_str().is_empty())
                .ok_or_else(|| {
                    ctx.build_err(
                        "relative schema files require a source file".into(),
                        node.span(),
                    )
                })?
                .join(path)
        } else {
            path.to_path_buf()
        };
        output.schema = Some(
            std::fs::read_to_string(&path).map_err(|err| UsageErr::FileError(err, path.clone()))?,
        );
        ctx.record_source(path);
    } else {
        node.ensure_arg_len(1..=1)?;
        if let Some((key, value)) = node.props().into_iter().next() {
            bail_parse!(ctx, value.entry.span(), "unsupported schema key {key}");
        }
        output.schema = Some(node.arg(0)?.ensure_string()?);
    }
    Ok(())
}

fn parse_framing(ctx: &ParsingContext, raw: String, entry: &KdlEntry) -> Result<Framing> {
    raw.parse().map_err(|_| {
        ctx.build_err(
            format!("unsupported framing {raw}, expected one of: {FRAMING_VALUES}"),
            entry.span(),
        )
    })
}

impl From<&SpecOutput> for KdlNode {
    fn from(output: &SpecOutput) -> KdlNode {
        let mut node = KdlNode::new("output");
        node.push(string_entry(None, &output.name));
        if output.framing != Framing::Text {
            node.push(string_entry(Some("framing"), output.framing.as_str()));
        }
        if let Some(help) = &output.help {
            node.push(string_entry(Some("help"), help));
        }
        if output.default {
            node.push(KdlEntry::new_prop("default", true));
        }
        if output.hide {
            node.push(KdlEntry::new_prop("hide", true));
        }
        if let Some(select) = &output.select {
            node.push(string_entry(Some("select"), select));
        }
        if let Some(schema) = &output.schema {
            let mut schema_node = KdlNode::new("schema");
            schema_node.push(string_entry(None, schema));
            let mut children = KdlDocument::new();
            children.nodes_mut().push(schema_node);
            node.set_children(children);
        }
        node
    }
}

/// The outputs in effect for a command, CLI-wide declarations folded in.
///
/// Nearest wins, by name: a command redeclaring an inherited output refines it rather
/// than adding a second entry with the same token. `hide=#true` is how one is taken away,
/// so hidden entries are dropped after the fold rather than before it — a command can
/// only hide what it has already inherited.
///
/// Folded on read rather than at parse time, following `unknown_flags`. Folding early
/// would write the root's outputs into every command block on re-emission, which is a
/// spec nobody wrote.
pub fn effective_outputs(spec: &Spec, path: &[SpecCommand]) -> Vec<SpecOutput> {
    effective_outputs_ref(spec, path.iter())
}

/// Reference-based form used by tree walkers that already hold the command chain.
pub(crate) fn effective_outputs_ref<'a>(
    spec: &Spec,
    path: impl IntoIterator<Item = &'a SpecCommand>,
) -> Vec<SpecOutput> {
    let mut out: Vec<SpecOutput> = spec.outputs.clone();
    for cmd in path {
        for output in &cmd.outputs {
            match out.iter_mut().find(|o| o.name == output.name) {
                Some(existing) => *existing = output.clone(),
                None => out.push(output.clone()),
            }
        }
    }
    out.retain(|o| !o.hide);
    out
}

/// The value-taking selector in effect for a command, if it reaches that command.
pub fn effective_select(spec: &Spec, path: &[SpecCommand]) -> Option<String> {
    let refs = path.iter().collect::<Vec<_>>();
    effective_select_ref(spec, &refs)
}

/// Reference-based form used by tree walkers that already hold the command chain.
pub(crate) fn effective_select_ref(spec: &Spec, path: &[&SpecCommand]) -> Option<String> {
    let mut select = spec.select.clone();
    for cmd in path {
        if let Some(own) = &cmd.select {
            select = Some(own.clone());
        }
    }
    let selected = path.last().copied().unwrap_or(&spec.cmd);
    select.filter(|name| selected.flags.iter().any(|flag| flag_named(flag, name)))
}

/// The flag a command's `select` names, and whether it was found locally.
///
/// The whole flag list is searched, not just the local one, because the ergonomic
/// declaration is one global `--format` at the root and per-command `output` nodes under
/// it.
fn find_selector<'a>(
    cmd: &'a SpecCommand,
    inherited: &'a [SpecFlag],
    name: &str,
) -> Option<(&'a SpecFlag, bool)> {
    if let Some(flag) = cmd.flags.iter().find(|f| flag_named(f, name)) {
        return Some((flag, true));
    }
    inherited
        .iter()
        .find(|f| flag_named(f, name))
        .map(|f| (f, false))
}

fn flag_named(flag: &SpecFlag, name: &str) -> bool {
    let bare = name.trim_start_matches('-');
    flag.long.iter().any(|l| l == bare)
        || flag.short.iter().any(|s| s.to_string() == bare)
        || flag.name == bare
}

/// Fill each selecting flag's `choices` from the outputs it picks among.
///
/// Runs once over the whole tree after the document is read, because a `select` may name
/// a flag declared on an ancestor and a command being parsed cannot see its ancestors
/// yet.
pub(crate) fn resolve_selectors(spec: &mut Spec) -> Result<()> {
    let root_outputs = spec.outputs.clone();
    let root_select = spec.select.clone();
    resolve_cmd(&mut spec.cmd, &[], &root_outputs, root_select.as_deref())
}

fn resolve_cmd(
    cmd: &mut SpecCommand,
    inherited_flags: &[SpecFlag],
    inherited_outputs: &[SpecOutput],
    inherited_select: Option<&str>,
) -> Result<()> {
    // What this command actually offers, and how it is asked for, both inherited unless
    // it says otherwise.
    let mut outputs: Vec<SpecOutput> = inherited_outputs.to_vec();
    for output in &cmd.outputs {
        match outputs.iter_mut().find(|o| o.name == output.name) {
            Some(existing) => *existing = output.clone(),
            None => outputs.push(output.clone()),
        }
    }
    outputs.retain(|o| !o.hide);

    // The globals a child inherits are taken *before* this command narrows anything, so a
    // child narrows the flag as written rather than as this command left it. Without
    // that, a root that declares outputs of its own hands every subcommand a `--format`
    // whose choices are the root's, and the subcommand's own outputs read as a
    // disagreement with a hand-written list.
    let mut available: Vec<SpecFlag> = inherited_flags.to_vec();
    available.extend(cmd.flags.iter().filter(|f| f.global).cloned());

    // A `select` inherits only as far as the flag it names does. A non-global `--format`
    // on `install` says nothing about `install from`, and the author wrote the `select` on
    // the parent — so it is dropped here rather than reported. A `select` written *on*
    // this command is a different matter: that one has to name something.
    let select = match cmd.select.as_deref() {
        Some(own) => Some(own.to_owned()),
        None => inherited_select
            .filter(|name| find_selector(cmd, inherited_flags, name).is_some())
            .map(str::to_owned),
    };

    check_declarations(cmd, &outputs)?;
    if let Some(name) = &select {
        if !outputs.is_empty() {
            materialize(cmd, inherited_flags, name, &outputs)?;
        }
    }
    check_boolean_selectors(cmd, inherited_flags, &outputs)?;

    for sub in cmd.subcommands.values_mut() {
        resolve_cmd(sub, &available, &outputs, select.as_deref())?;
    }
    Ok(())
}

/// The checks that hold whether or not anything selects the outputs.
fn check_declarations(cmd: &SpecCommand, outputs: &[SpecOutput]) -> Result<()> {
    let defaults: Vec<&str> = outputs
        .iter()
        .filter(|o| o.default)
        .map(|o| o.name.as_str())
        .collect();
    if defaults.len() > 1 {
        return Err(invalid(format!(
            "`{}` has more than one default output ({}); only one can be what runs when \
             nothing selects otherwise",
            cmd.name,
            defaults.join(", ")
        )));
    }
    Ok(())
}

/// Fill the selecting flag's choices, or check the ones already written.
fn materialize(
    cmd: &mut SpecCommand,
    inherited: &[SpecFlag],
    name: &str,
    outputs: &[SpecOutput],
) -> Result<()> {
    let Some((found, local)) = find_selector(cmd, inherited, name) else {
        let declared: Vec<String> = cmd
            .flags
            .iter()
            .chain(inherited)
            .map(|f| f.usage())
            .collect();
        return Err(invalid(format!(
            "select `{name}` on `{}` names no flag here or above it (declared: {})",
            cmd.name,
            if declared.is_empty() {
                "none".to_string()
            } else {
                declared.join(", ")
            }
        )));
    };

    if found.arg.is_none() {
        return Err(invalid(format!(
            "select `{name}` on `{}` names a flag that takes no value, so it cannot carry an \
             output name; a boolean picks one output with `output … select=\"{name}\"`",
            cmd.name
        )));
    }

    // An inherited flag is copied down before it is narrowed. Two commands under one
    // global `--format` rarely produce the same outputs, and writing the union onto the
    // shared flag would offer each of them values only the other accepts.
    let mut flag = found.clone();
    let arg = flag.arg.as_mut().expect("checked just above");
    if let Some(existing) = &arg.choices {
        // Written by hand, and richer than a list of names: per-choice help, aliases,
        // hidden values. Overwriting would delete all of that, so this only checks that
        // the two agree about which values exist.
        let declared: BTreeSet<String> = existing.values().into_iter().collect();
        let expected: BTreeSet<String> = outputs.iter().map(|o| o.name.clone()).collect();
        if declared != expected {
            let mut detail = Vec::new();
            let only_flag = difference(&declared, &expected);
            let only_output = difference(&expected, &declared);
            if !only_flag.is_empty() {
                detail.push(format!("the flag accepts {only_flag} with no output"));
            }
            if !only_output.is_empty() {
                detail.push(format!("no choice offers {only_output}"));
            }
            return Err(invalid(format!(
                "`{}` selects outputs with `{name}`, but its choices disagree: {}",
                cmd.name,
                detail.join(", ")
            )));
        }
        return Ok(());
    }

    arg.choices = Some(SpecChoices {
        choices: outputs.iter().map(|o| o.name.clone()).collect(),
        // Only where there is something to say. `choices` is the authoritative list and
        // `details` is looked up against it, so an entry per output would emit a `choice`
        // block per value to carry nothing — the shorthand `choices human json` form is
        // what a spec with no per-value help should keep.
        details: outputs
            .iter()
            .filter(|o| o.help.is_some())
            .map(|o| SpecChoice {
                value: o.name.clone(),
                help: o.help.clone(),
                ..Default::default()
            })
            .collect(),
        ..Default::default()
    });

    if local {
        if let Some(slot) = cmd.flags.iter_mut().find(|f| flag_named(f, name)) {
            *slot = flag;
        }
    } else {
        cmd.flags.push(flag);
    }
    Ok(())
}

fn difference(a: &BTreeSet<String>, b: &BTreeSet<String>) -> String {
    a.difference(b)
        .map(|v| format!("`{v}`"))
        .collect::<Vec<_>>()
        .join(", ")
}

/// An `output … select="--json"` has to name a flag that exists and takes no value.
fn check_boolean_selectors(
    cmd: &SpecCommand,
    inherited: &[SpecFlag],
    outputs: &[SpecOutput],
) -> Result<()> {
    for output in outputs.iter().filter(|o| o.select.is_some()) {
        let name = output.select.as_deref().expect("filtered");
        let Some((flag, _)) = find_selector(cmd, inherited, name) else {
            return Err(invalid(format!(
                "output `{}` on `{}` is selected by `{name}`, which names no flag here or \
                 above it",
                output.name, cmd.name
            )));
        };
        if flag.arg.is_some() {
            return Err(invalid(format!(
                "output `{}` on `{}` is selected by `{name}`, which takes a value; a flag that \
                 carries an output name belongs on the command as `select \"{name}\"`",
                output.name, cmd.name
            )));
        }
    }
    Ok(())
}

/// Whether anything at all is declared, so a consumer can skip the whole concept.
pub fn has_outputs(spec: &Spec, path: &[SpecCommand]) -> bool {
    !spec.outputs.is_empty() || path.iter().any(|c| !c.outputs.is_empty())
}

#[cfg(test)]
mod tests {
    use super::*;
    use insta::assert_snapshot;

    fn parse(src: &str) -> Spec {
        src.parse().expect("the fixture should parse")
    }

    /// The message, not the `Display`: a spanned parse error renders its text as a miette
    /// label, so `to_string()` on one is just "Invalid usage config".
    fn error(src: &str) -> String {
        match src.parse::<Spec>().expect_err("should be rejected") {
            UsageErr::InvalidInput(msg, ..) => msg,
            other => other.to_string(),
        }
    }

    #[test]
    fn framing_defaults_to_text_and_names_stay_the_users() {
        let spec = parse(
            r#"
name "ex"
cmd "ls" {
    output "human" default=#true
    output "ndjson" framing="jsonl"
}
"#,
        );
        let ls = &spec.cmd.subcommands["ls"];
        assert_eq!(ls.outputs[0].framing, Framing::Text);
        assert_eq!(ls.outputs[1].framing, Framing::Jsonl);
        // The point of the split: the token a user types is not the contract a consumer
        // reads. aube calls this `ndjson` and hk calls it `jsonl`.
        assert_eq!(ls.outputs[1].name, "ndjson");
        assert!(ls.outputs[1].framing.is_streaming());
    }

    #[test]
    fn an_unknown_framing_names_the_ones_that_exist() {
        assert!(error(
            r#"
name "ex"
cmd "ls" { output "x" framing="protobuf" }
"#
        )
        .contains("expected one of: text, json, jsonl"));
    }

    #[test]
    fn a_select_fills_the_flags_choices() {
        let spec = parse(
            r#"
name "ex"
cmd "ls" {
    flag "--format <FMT>"
    output "human" help="A table"
    output "json" framing="json"
    select "--format"
}
"#,
        );
        let choices = spec.cmd.subcommands["ls"].flags[0]
            .arg
            .as_ref()
            .unwrap()
            .choices
            .as_ref()
            .unwrap();
        assert_eq!(choices.values(), vec!["human", "json"]);
        // Output help becomes choice help, which is what a shell shows beside a candidate.
        assert_eq!(choices.details[0].help.as_deref(), Some("A table"));
    }

    #[test]
    fn a_global_selector_is_narrowed_per_command() {
        // One `--format` at the root, two commands that write different things. Writing
        // the union onto the shared flag would offer `ls` a value only `render` accepts,
        // so each gets a copy holding just its own.
        let spec = parse(
            r#"
name "ex"
flag "--format <FMT>" global=#true
cmd "ls"     { output "human"; output "json" framing="json"; select "--format" }
cmd "render" { output "svg";   output "png"; select "--format" }
"#,
        );
        let choices_of = |cmd: &str| {
            spec.cmd.subcommands[cmd]
                .flags
                .iter()
                .find(|f| f.long.iter().any(|l| l == "format"))
                .and_then(|f| f.arg.as_ref())
                .and_then(|a| a.choices.as_ref())
                .map(|c| c.values())
        };
        assert_eq!(choices_of("ls"), Some(vec!["human".into(), "json".into()]));
        assert_eq!(choices_of("render"), Some(vec!["svg".into(), "png".into()]));
        // The root's own copy is left alone: it is the flag both narrowings came from.
        assert!(spec.cmd.flags[0].arg.as_ref().unwrap().choices.is_none());
    }

    #[test]
    fn choices_written_by_hand_are_checked_rather_than_replaced() {
        // A hand-written block carries per-choice help and aliases that a list of output
        // names does not, so agreement is checked and nothing is overwritten.
        let spec = parse(
            r#"
name "ex"
cmd "ls" {
    flag "--format <FMT>" {
        arg "<FMT>" { choices { choice "human" help="Kept"; choice "json" } }
    }
    output "human"
    output "json" framing="json"
    select "--format"
}
"#,
        );
        let choices = spec.cmd.subcommands["ls"].flags[0]
            .arg
            .as_ref()
            .unwrap()
            .choices
            .as_ref()
            .unwrap();
        assert_eq!(choices.details[0].help.as_deref(), Some("Kept"));
    }

    #[test]
    fn choices_that_disagree_with_the_outputs_are_an_error() {
        let err = error(
            r#"
name "ex"
cmd "ls" {
    flag "--format <FMT>" { arg "<FMT>" { choices "human" "yaml" } }
    output "human"
    output "json" framing="json"
    select "--format"
}
"#,
        );
        assert!(err.contains("`yaml` with no output"), "{err}");
        assert!(err.contains("no choice offers `json`"), "{err}");
    }

    #[test]
    fn a_select_naming_no_flag_says_what_is_declared() {
        let err = error(
            r#"
name "ex"
cmd "ls" {
    flag "--verbose"
    output "json" framing="json"
    select "--format"
}
"#,
        );
        assert!(err.contains("names no flag here or above it"), "{err}");
        assert!(err.contains("--verbose"), "{err}");
    }

    #[test]
    fn a_select_naming_a_boolean_points_at_the_other_spelling() {
        let err = error(
            r#"
name "ex"
cmd "ls" {
    flag "--format"
    output "json" framing="json"
    select "--format"
}
"#,
        );
        assert!(err.contains("takes no value"), "{err}");
        assert!(err.contains("output … select=\"--format\""), "{err}");
    }

    #[test]
    fn a_boolean_selector_picks_one_output() {
        let spec = parse(
            r#"
name "ex"
cmd "ls" {
    flag "--json"
    output "text" default=#true
    output "json" framing="json" select="--json"
}
"#,
        );
        let ls = &spec.cmd.subcommands["ls"];
        let json = ls.outputs.iter().find(|o| o.name == "json").unwrap();
        assert_eq!(
            json.select_argv(ls).map(|s| s.argv()),
            Some(vec!["--json".to_string()])
        );
        // Nothing selects `text` on its own; it is what runs when `--json` is absent.
        let text = ls.outputs.iter().find(|o| o.name == "text").unwrap();
        assert_eq!(text.select_argv(ls), None);
    }

    #[test]
    fn a_boolean_selector_naming_a_value_flag_points_back() {
        let err = error(
            r#"
name "ex"
cmd "ls" {
    flag "--format <FMT>"
    output "json" framing="json" select="--format"
}
"#,
        );
        assert!(err.contains("which takes a value"), "{err}");
        assert!(err.contains("`select \"--format\"`"), "{err}");
    }

    #[test]
    fn two_defaults_are_an_error() {
        let err = error(
            r#"
name "ex"
cmd "ls" { output "a" default=#true; output "b" default=#true }
"#,
        );
        assert!(err.contains("more than one default output (a, b)"), "{err}");
    }

    #[test]
    fn cli_wide_outputs_are_inherited_and_can_be_taken_away() {
        let spec = parse(
            r#"
name "ex"
output "human" default=#true
output "json" framing="json"
select "--format"
flag "--format <FMT>" global=#true
cmd "stream" {
    output "human" hide=#true
    output "jsonl" framing="jsonl"
}
"#,
        );
        let stream = spec.cmd.subcommands["stream"].clone();
        let names: Vec<String> = effective_outputs(&spec, std::slice::from_ref(&stream))
            .into_iter()
            .map(|o| o.name)
            .collect();
        // `human` was inherited and then hidden; `json` still comes down from the root.
        assert_eq!(names, vec!["json", "jsonl"]);
        let choices = stream
            .flags
            .iter()
            .find(|f| f.long.iter().any(|l| l == "format"))
            .and_then(|f| f.arg.as_ref())
            .and_then(|a| a.choices.as_ref())
            .map(|c| c.values());
        assert_eq!(choices, Some(vec!["json".into(), "jsonl".into()]));
    }

    #[test]
    fn resolution_is_idempotent_so_a_spec_round_trips() {
        let src = r#"
name "ex"
flag "--format <FMT>" global=#true
cmd "ls" {
    output "human" default=#true help="A table"
    output "json" framing="json"
    select "--format"
}
"#;
        let once = parse(src).to_string();
        let twice = parse(&once).to_string();
        // The second pass sees choices that already match and validates them instead of
        // rewriting, which is what keeps a re-emitted spec stable.
        assert_eq!(once, twice);
        assert_snapshot!(once, @r#"
        name ex
        flag --format global=#true {
            arg <FMT>
        }
        cmd ls {
            flag --format global=#true {
                arg <FMT> {
                    choices {
                        choice human help="A table"
                        choice json
                    }
                }
            }
            output human help="A table" default=#true
            output json framing=json
            select "--format"
        }
        "#);
    }

    #[test]
    fn a_schema_survives_a_round_trip_with_its_newlines() {
        let src = "name \"ex\"\ncmd \"ls\" {\n    output \"json\" framing=\"json\" {\n        schema \"{\\n  \\\"type\\\": \\\"object\\\"\\n}\"\n    }\n}\n";
        let spec = parse(src);
        let schema = spec.cmd.subcommands["ls"].outputs[0]
            .schema
            .clone()
            .unwrap();
        assert_eq!(schema, "{\n  \"type\": \"object\"\n}");
        let again = parse(&spec.to_string());
        assert_eq!(
            again.cmd.subcommands["ls"].outputs[0].schema.as_deref(),
            Some(schema.as_str())
        );
    }

    #[test]
    fn an_external_schema_is_relative_to_the_kdl_that_declares_it() {
        let dir = tempfile::tempdir().unwrap();
        let shared = dir.path().join("shared");
        std::fs::create_dir(&shared).unwrap();
        let root = dir.path().join("root.usage.kdl");
        let included = shared.join("outputs.usage.kdl");
        let schema_file = shared.join("report.schema.json");
        let schema = "{\n  \"type\": \"object\"\n}\n";
        std::fs::write(&schema_file, schema).unwrap();
        std::fs::write(
            &included,
            "output \"json\" framing=\"json\" { schema file=\"report.schema.json\" }\n",
        )
        .unwrap();
        std::fs::write(
            &root,
            "name \"ex\"\ninclude file=\"shared/outputs.usage.kdl\"\n",
        )
        .unwrap();

        let spec = Spec::parse_file(&root).unwrap();
        assert_eq!(spec.outputs[0].schema.as_deref(), Some(schema));
        assert!(spec.sources.contains(&schema_file));

        // Re-emission embeds what was loaded, just as it expands an include, so the result
        // does not depend on the original adjacent file.
        let emitted = spec.to_string();
        assert!(!emitted.contains("report.schema.json"));
        assert_eq!(parse(&emitted).outputs[0].schema.as_deref(), Some(schema));
    }

    #[test]
    fn a_relative_schema_needs_a_source_file() {
        let err = error("name \"ex\"\noutput \"json\" { schema file=\"report.schema.json\" }\n");
        assert_eq!(err, "relative schema files require a source file");
    }

    #[test]
    fn an_unreadable_schema_names_its_resolved_path() {
        let dir = tempfile::tempdir().unwrap();
        let root = dir.path().join("root.usage.kdl");
        let missing = dir.path().join("missing.schema.json");
        std::fs::write(
            &root,
            "name \"ex\"\noutput \"json\" { schema file=\"missing.schema.json\" }\n",
        )
        .unwrap();

        match Spec::parse_file(&root).unwrap_err() {
            UsageErr::FileError(_, file) => assert_eq!(file, missing),
            err => panic!("unexpected error: {err:?}"),
        }
    }
}
