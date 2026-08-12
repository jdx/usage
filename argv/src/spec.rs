//! Cold-path metadata, and writing it out as a spec.
//!
//! The tables in the crate root are what a parse reads: names, shorts, whether a
//! flag takes a value. Everything *else* a CLI knows about itself — help text,
//! choices, defaults, what a command does to the world — lives here instead, in a
//! parallel tree that points at those tables.
//!
//! Two reasons for the split. A successful parse never touches any of this, so
//! keeping it out of the hot tables keeps them dense; and a CLI that wants only a
//! parser does not compile it at all, since this module is behind the `spec`
//! feature.
//!
//! What the metadata does *not* do is repeat the tables. [`FlagMeta`] borrows the
//! [`Flag`] it describes, so long and short forms have exactly one definition and
//! cannot drift from what the parser matches.
//!
//! # Emitting
//!
//! Everything here lowers into the spec without loss, which is deliberate: a
//! field the spec cannot express would make the emitted KDL a summary rather than
//! a definition. When something is missing the spec gains it first —
//! `help_heading` went in that way.
//!
use core::fmt::Write as _;

use crate::UnknownFlags;
use crate::{Arg, Command, DoubleDash, Flag};

/// The first key that appears twice anywhere in a command tree, if any.
///
/// Keys are what a parse dispatches on, and a derive assigns them without being able
/// to see other expansions — it hashes the type name to keep them apart. That makes
/// a collision astronomically unlikely rather than impossible, so it is checked
/// where a CLI is written out, which every adopter does in a test.
fn duplicate_key(cmd: &Command<'_>) -> Option<u64> {
    let mut keys = std::vec::Vec::new();
    collect_keys(cmd, &mut keys);
    keys.sort_unstable();
    keys.windows(2)
        .find(|pair| pair[0] == pair[1])
        .map(|pair| pair[0])
}

fn collect_keys(cmd: &Command<'_>, keys: &mut std::vec::Vec<u64>) {
    keys.push(cmd.key);
    keys.extend(cmd.flags.iter().map(|f| f.key));
    keys.extend(cmd.args.iter().map(|a| a.key));
    for sub in cmd.subcommands {
        collect_keys(sub, keys);
    }
}

/// A whole CLI: the root command plus what describes the program itself.
#[derive(Debug, Clone, Copy)]
pub struct Spec<'a> {
    /// The program's name.
    pub name: &'a str,
    /// The binary as invoked, when it differs from `name`.
    pub bin: Option<&'a str>,
    pub version: Option<&'a str>,
    pub about: Option<&'a str>,
    pub long_about: Option<&'a str>,
    /// Which command the root falls back to when a word matches no subcommand.
    /// mise uses this so `mise foo` completes as `mise run foo`.
    pub default_subcommand: Option<&'a str>,
    pub root: &'a CommandMeta<'a>,
}

/// What a command knows about itself beyond how it parses.
#[derive(Debug, Clone, Copy)]
pub struct CommandMeta<'a> {
    /// The parse table this describes. Names, aliases, and structure come from
    /// here rather than being repeated.
    pub cmd: &'a Command<'a>,
    pub about: Option<&'a str>,
    pub long_about: Option<&'a str>,
    /// Aliases that work but are not shown in help or completions. Everything in
    /// `cmd.aliases` and not here is visible.
    pub hidden_aliases: &'a [&'a str],
    /// Whether the command is hidden from help and completions.
    pub hide: bool,
    /// What running this does to the world, for a caller deciding whether to
    /// confirm first. clap cannot express this, which is why mise keeps a
    /// 330-entry table to bolt it on afterwards.
    pub effect: Option<Effect>,
    /// A command to run at parse time to discover further subcommands.
    ///
    /// Only meaningful on a subcommand. The spec accepts `mount` inside a `cmd`
    /// block and nowhere else, so setting this on the root is a mistake that
    /// [`Spec::to_kdl`] catches in debug builds.
    pub mount: Option<&'a str>,
    /// A token that starts a fresh invocation of this command, such as mise's
    /// `:::`.
    pub restart_token: Option<&'a str>,
    pub examples: &'a [Example<'a>],
    /// Metadata for `cmd.flags`, in the same order.
    pub flags: &'a [FlagMeta<'a>],
    /// Metadata for `cmd.args`, in the same order.
    pub args: &'a [ArgMeta<'a>],
    /// Metadata for `cmd.subcommands`, in the same order.
    pub subcommands: &'a [&'a CommandMeta<'a>],
}

impl CommandMeta<'_> {
    /// Metadata for a command with nothing declared, for struct update syntax.
    pub const EMPTY: CommandMeta<'static> = CommandMeta {
        cmd: &Command::EMPTY,
        about: None,
        long_about: None,
        hidden_aliases: &[],
        hide: false,
        effect: None,
        mount: None,
        restart_token: None,
        examples: &[],
        flags: &[],
        args: &[],
        subcommands: &[],
    };
}

/// What a flag knows about itself beyond how it parses.
#[derive(Debug, Clone, Copy)]
pub struct FlagMeta<'a> {
    pub flag: &'a Flag<'a>,
    /// Short help, shown by `-h`.
    pub help: Option<&'a str>,
    /// Long help, shown by `--help`.
    pub long_help: Option<&'a str>,
    /// The placeholder for the flag's value, such as `n` in `--jobs <n>`.
    pub value_name: Option<&'a str>,
    pub env: Option<&'a str>,
    pub default: &'a [&'a str],
    pub choices: &'a [&'a str],
    pub required: bool,
    pub hide: bool,
    /// Whether repetition is counted rather than collected, as in `-vvv`.
    pub count: bool,
    /// Whether the flag may be given more than once. Distinct from
    /// [`Flag::variadic`], which is one occurrence taking several values.
    pub repeatable: bool,
    pub var_min: Option<usize>,
    pub var_max: Option<usize>,
    /// Flags this one displaces when both are given.
    pub overrides: &'a [&'a str],
    /// Flags that cannot be given alongside this one.
    ///
    /// Where [`overrides`](Self::overrides) resolves a collision by letting the last
    /// flag win, this reports it: the combination has no meaning, so honouring one
    /// side silently would hide a mistake.
    pub conflicts: &'a [&'a str],
    /// Flags that make this one necessary.
    pub required_if: &'a [&'a str],
    /// Flags that make this one unnecessary.
    pub required_unless: &'a [&'a str],
    /// Heading to list this flag under in help output. Presentational: it groups
    /// a long flag list into sections and changes nothing about parsing.
    pub help_heading: Option<&'a str>,
    pub effect: Option<Effect>,
}

impl FlagMeta<'_> {
    /// Metadata for a flag with nothing declared, for struct update syntax.
    pub const EMPTY: FlagMeta<'static> = FlagMeta {
        flag: &Flag::BOOL,
        help: None,
        long_help: None,
        value_name: None,
        env: None,
        default: &[],
        choices: &[],
        required: false,
        hide: false,
        count: false,
        repeatable: false,
        var_min: None,
        var_max: None,
        overrides: &[],
        conflicts: &[],
        required_if: &[],
        required_unless: &[],
        help_heading: None,
        effect: None,
    };
}

/// What a positional argument knows about itself beyond how it parses.
#[derive(Debug, Clone, Copy)]
pub struct ArgMeta<'a> {
    pub arg: &'a Arg<'a>,
    pub help: Option<&'a str>,
    pub long_help: Option<&'a str>,
    pub env: Option<&'a str>,
    pub default: &'a [&'a str],
    pub choices: &'a [&'a str],
    /// Whether the argument must be filled. The parser does not enforce this —
    /// it is checked once the last token has been read — but the spec has to say
    /// it, and help output has to show it.
    pub required: bool,
    pub hide: bool,
    pub var_min: Option<usize>,
    pub var_max: Option<usize>,
    /// Heading to list this argument under in help output.
    pub help_heading: Option<&'a str>,
}

impl ArgMeta<'_> {
    /// Metadata for an argument with nothing declared, for struct update syntax.
    pub const EMPTY: ArgMeta<'static> = ArgMeta {
        arg: &Arg::REQUIRED,
        help: None,
        long_help: None,
        env: None,
        default: &[],
        choices: &[],
        required: true,
        hide: false,
        var_min: None,
        var_max: None,
        help_heading: None,
    };
}

/// A worked example, for documentation.
#[derive(Debug, Clone, Copy)]
pub struct Example<'a> {
    pub code: &'a str,
    pub header: Option<&'a str>,
    pub help: Option<&'a str>,
}

/// What running a command does to the world.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Effect {
    /// Only inspects state.
    Read,
    /// Creates or modifies state, but removes nothing unrecoverable.
    Write,
    /// May delete or irreversibly overwrite something.
    Destructive,
}

impl Effect {
    /// The spelling used in a spec.
    pub fn as_str(self) -> &'static str {
        match self {
            Effect::Read => "read",
            Effect::Write => "write",
            Effect::Destructive => "destructive",
        }
    }
}

impl Spec<'_> {
    /// Write this CLI as a usage spec, in KDL.
    pub fn to_kdl(&self) -> String {
        debug_assert!(
            duplicate_key(self.root.cmd).is_none(),
            "two things in this CLI share a key ({:?}), so a parse would bind the \
             wrong one. A derive builds keys from a hash of the type they came from, \
             so this means two type names collided.",
            duplicate_key(self.root.cmd)
        );
        let mut out = String::new();
        // Unwrap-free: writing into a String cannot fail, and `write!` returning
        // Result is an artifact of the trait rather than a real outcome.
        let _ = self.write_kdl(&mut out);
        out
    }

    fn write_kdl(&self, out: &mut String) -> core::fmt::Result {
        prop(out, "name", self.name)?;
        prop(out, "bin", self.bin.unwrap_or(self.name))?;
        if let Some(version) = self.version {
            prop(out, "version", version)?;
        }
        // A description may be given on the spec or on its root command — a derive
        // naturally has one doc comment and no reason to care which field it lands
        // in — so either is written, the spec's first.
        if let Some(about) = self.about.or(self.root.about) {
            prop(out, "about", about)?;
        }
        if let Some(long_about) = self.long_about.or(self.root.long_about) {
            prop(out, "long_about", long_about)?;
        }
        // Written only when it is not the default, so an ordinary spec stays quiet
        // about it.
        if self.root.cmd.unknown_flags == UnknownFlags::Error {
            prop(out, "unknown_flags", "error")?;
        }
        if let Some(default_subcommand) = self.default_subcommand {
            prop(out, "default_subcommand", default_subcommand)?;
        }
        // The root's own nodes sit at the top level rather than inside a `cmd`
        // block, so they are written here instead of by write_command.
        //
        // A mount is the exception: the spec only accepts one inside a `cmd`
        // block, so a root mount is not expressible. Emitting it anyway would
        // produce a document that does not parse, and dropping it quietly is the
        // lossiness this module claims not to have — so it fails loudly in debug
        // builds instead, and PLAN.md carries it as a possible spec extension.
        debug_assert!(
            self.root.mount.is_none(),
            "a mount on the root command cannot be written: the spec accepts \
             `mount` only inside a `cmd` block"
        );
        // The same is true of everything else that lives on a `cmd` node. Setting
        // one on the root is a mistake, and silently dropping it is the lossiness
        // this module claims not to have.
        debug_assert!(
            self.root.effect.is_none()
                && !self.root.hide
                && self.root.restart_token.is_none()
                && self.root.cmd.aliases.is_empty()
                && self.root.hidden_aliases.is_empty(),
            "the root command cannot carry an effect, hide, a restart token, or \
             aliases: the spec accepts those only inside a `cmd` block"
        );
        for example in self.root.examples {
            write_example(out, example, 0)?;
        }
        write_body(out, self.root, 0)
    }
}

/// Write a command's contents: its flags, arguments, and subcommands.
///
/// Separate from [`write_command`] because the root's contents sit at the top
/// level of the document rather than inside a `cmd` node.
fn write_body(out: &mut String, meta: &CommandMeta<'_>, depth: usize) -> core::fmt::Result {
    let enclosing_unknown_flags = meta.cmd.unknown_flags;
    // Indexing by metadata position below cannot see a table entry with no
    // metadata, which would be silently unwritten. Check the lengths first.
    debug_assert_eq!(
        meta.cmd.flags.len(),
        meta.flags.len(),
        "every flag in the parse table needs metadata, or it will not be written"
    );
    debug_assert_eq!(
        meta.cmd.args.len(),
        meta.args.len(),
        "every argument in the parse table needs metadata"
    );
    debug_assert_eq!(
        meta.cmd.subcommands.len(),
        meta.subcommands.len(),
        "every subcommand in the parse table needs metadata"
    );
    for (i, flag) in meta.flags.iter().enumerate() {
        // The two tables are written in the same order by construction, so a
        // mismatch means a table was edited without its metadata.
        debug_assert!(
            meta.cmd
                .flags
                .get(i)
                .is_some_and(|f| core::ptr::eq(*f, flag.flag)),
            "flag metadata is out of step with the parse table"
        );
        write_flag(out, flag, depth)?;
    }
    for (i, arg) in meta.args.iter().enumerate() {
        debug_assert!(
            meta.cmd
                .args
                .get(i)
                .is_some_and(|a| core::ptr::eq(*a, arg.arg)),
            "argument metadata is out of step with the parse table"
        );
        write_arg(out, arg, depth)?;
    }
    for sub in meta.subcommands {
        write_command(out, sub, depth, enclosing_unknown_flags)?;
    }
    Ok(())
}

fn write_command(
    out: &mut String,
    meta: &CommandMeta<'_>,
    depth: usize,
    inherited_unknown_flags: UnknownFlags,
) -> core::fmt::Result {
    indent(out, depth)?;
    write!(out, "cmd {}", quoted(meta.cmd.name))?;
    if let Some(help) = meta.about {
        write!(out, " help={}", quoted(help))?;
    }
    if meta.hide {
        out.push_str(" hide=#true");
    }
    if let Some(effect) = meta.effect {
        write!(out, " effect={}", quoted(effect.as_str()))?;
    }
    // Written only where it changes, since the spec inherits it. The tables hold the
    // effective value per command, so repeating the enclosing command's answer would
    // say nothing — but a command that differs has to say so, or the setting is lost
    // on the way out.
    if meta.cmd.unknown_flags != inherited_unknown_flags {
        write!(
            out,
            " unknown_flags={}",
            quoted(match meta.cmd.unknown_flags {
                UnknownFlags::Value => "value",
                UnknownFlags::Error => "error",
            })
        )?;
    }
    if let Some(token) = meta.restart_token {
        write!(out, " restart_token={}", quoted(token))?;
    }
    out.push_str(" {\n");

    let inner = depth + 1;
    for alias in meta.cmd.aliases {
        indent(out, inner)?;
        write!(out, "alias {}", quoted(alias))?;
        if meta.hidden_aliases.contains(alias) {
            out.push_str(" hide=#true");
        }
        out.push('\n');
    }
    if let Some(long_about) = meta.long_about {
        indent(out, inner)?;
        writeln!(out, "long_help {}", quoted(long_about))?;
    }
    if let Some(mount) = meta.mount {
        indent(out, inner)?;
        writeln!(out, "mount run={}", quoted(mount))?;
    }
    for example in meta.examples {
        write_example(out, example, inner)?;
    }
    write_body(out, meta, inner)?;

    indent(out, depth)?;
    out.push_str("}\n");
    Ok(())
}

fn write_example(out: &mut String, example: &Example<'_>, depth: usize) -> core::fmt::Result {
    indent(out, depth)?;
    write!(out, "example {}", quoted(example.code))?;
    if let Some(header) = example.header {
        write!(out, " header={}", quoted(header))?;
    }
    if let Some(help) = example.help {
        write!(out, " help={}", quoted(help))?;
    }
    out.push('\n');
    Ok(())
}

fn write_flag(out: &mut String, meta: &FlagMeta<'_>, depth: usize) -> core::fmt::Result {
    indent(out, depth)?;
    write!(out, "flag {}", quoted(&flag_forms(meta.flag)))?;

    if let Some(help) = meta.help {
        write!(out, " help={}", quoted(help))?;
    }
    if meta.required {
        out.push_str(" required=#true");
    }
    if meta.flag.global {
        out.push_str(" global=#true");
    }
    if meta.hide {
        out.push_str(" hide=#true");
    }
    if meta.count {
        out.push_str(" count=#true");
    }
    if meta.repeatable {
        out.push_str(" var=#true");
    }
    if let Some(min) = meta.var_min {
        write!(out, " var_min={min}")?;
    }
    if let Some(max) = meta.var_max {
        write!(out, " var_max={max}")?;
    }
    if let Some(negate) = meta.flag.negate {
        // The spec writes the negation with its dashes; the table stores the bare
        // name, since that is what a token is matched against.
        write!(out, " negate={}", quoted(&format!("--{negate}")))?;
    }
    if let Some(heading) = meta.help_heading {
        write!(out, " help_heading={}", quoted(heading))?;
    }
    if let Some(effect) = meta.effect {
        write!(out, " effect={}", quoted(effect.as_str()))?;
    }
    if let Some(env) = meta.env {
        write!(out, " env={}", quoted(env))?;
    }
    write_single_default(out, meta.default)?;
    write_single_list(out, "overrides", meta.overrides)?;
    write_single_list(out, "conflicts", meta.conflicts)?;
    write_single_list(out, "required_if", meta.required_if)?;
    write_single_list(out, "required_unless", meta.required_unless)?;

    let has_children = meta.long_help.is_some()
        || meta.flag.takes_value
        || !meta.choices.is_empty()
        || meta.default.len() > 1
        || meta.overrides.len() > 1
        || meta.conflicts.len() > 1
        || meta.required_if.len() > 1
        || meta.required_unless.len() > 1;
    if !has_children {
        out.push('\n');
        return Ok(());
    }

    out.push_str(" {\n");
    let inner = depth + 1;
    if let Some(long_help) = meta.long_help {
        indent(out, inner)?;
        writeln!(out, "long_help {}", quoted(long_help))?;
    }
    write_many_defaults(out, meta.default, inner)?;
    write_many_list(out, "overrides", meta.overrides, inner)?;
    write_many_list(out, "conflicts", meta.conflicts, inner)?;
    write_many_list(out, "required_if", meta.required_if, inner)?;
    write_many_list(out, "required_unless", meta.required_unless, inner)?;
    if meta.flag.takes_value {
        indent(out, inner)?;
        let name = meta.value_name.unwrap_or(meta.flag.name);
        write!(
            out,
            "arg {}",
            quoted(&placeholder(name, meta.flag.variadic))
        )?;
        if meta.choices.is_empty() {
            out.push('\n');
        } else {
            out.push_str(" {\n");
            write_choices(out, meta.choices, inner + 1)?;
            indent(out, inner)?;
            out.push_str("}\n");
        }
    } else if !meta.choices.is_empty() {
        write_choices(out, meta.choices, inner)?;
    }
    indent(out, depth)?;
    out.push_str("}\n");
    Ok(())
}

fn write_arg(out: &mut String, meta: &ArgMeta<'_>, depth: usize) -> core::fmt::Result {
    indent(out, depth)?;
    let name = if meta.arg.name.is_empty() {
        "arg"
    } else {
        meta.arg.name
    };
    write!(out, "arg {}", quoted(&arg_placeholder(name, meta)))?;

    if let Some(help) = meta.help {
        write!(out, " help={}", quoted(help))?;
    }
    if meta.hide {
        out.push_str(" hide=#true");
    }
    if let Some(min) = meta.var_min {
        write!(out, " var_min={min}")?;
    }
    if let Some(max) = meta.var_max {
        write!(out, " var_max={max}")?;
    }
    if meta.arg.double_dash != DoubleDash::Optional {
        let mode = match meta.arg.double_dash {
            DoubleDash::Required => "required",
            DoubleDash::Preserve => "preserve",
            DoubleDash::Automatic => "automatic",
            DoubleDash::Optional => unreachable!("excluded by the branch above"),
        };
        write!(out, " double_dash={}", quoted(mode))?;
    }
    if let Some(heading) = meta.help_heading {
        write!(out, " help_heading={}", quoted(heading))?;
    }
    if let Some(env) = meta.env {
        write!(out, " env={}", quoted(env))?;
    }
    write_single_default(out, meta.default)?;

    let has_children =
        meta.long_help.is_some() || !meta.choices.is_empty() || meta.default.len() > 1;
    if !has_children {
        out.push('\n');
        return Ok(());
    }

    out.push_str(" {\n");
    let inner = depth + 1;
    if let Some(long_help) = meta.long_help {
        indent(out, inner)?;
        writeln!(out, "long_help {}", quoted(long_help))?;
    }
    write_many_defaults(out, meta.default, inner)?;
    write_choices(out, meta.choices, inner)?;
    indent(out, depth)?;
    out.push_str("}\n");
    Ok(())
}

/// A lone value goes on the node as a property; see [`write_many_list`] for why
/// several cannot.
fn write_single_list(out: &mut String, key: &str, values: &[&str]) -> core::fmt::Result {
    if let [only] = values {
        write!(out, " {key}={}", quoted(only))?;
    }
    Ok(())
}

/// Several values, as `overrides "a" "b"`.
///
/// The same trap as defaults: `overrides="a" overrides="b"` is one node with a
/// property set twice, and only the last one survives.
fn write_many_list(
    out: &mut String,
    key: &str,
    values: &[&str],
    depth: usize,
) -> core::fmt::Result {
    if values.len() < 2 {
        return Ok(());
    }
    indent(out, depth)?;
    write!(out, "{key}")?;
    for value in values {
        write!(out, " {}", quoted(value))?;
    }
    out.push('\n');
    Ok(())
}

/// A lone default goes on the node as a property.
///
/// Several cannot: KDL properties are unique per node, so `default="a"
/// default="b"` keeps only the last one. Those go in a child block instead, which
/// is what [`write_many_defaults`] emits.
fn write_single_default(out: &mut String, defaults: &[&str]) -> core::fmt::Result {
    if let [only] = defaults {
        write!(out, " default={}", quoted(only))?;
    }
    Ok(())
}

/// Several defaults, as `default { "a"; "b" }`.
fn write_many_defaults(out: &mut String, defaults: &[&str], depth: usize) -> core::fmt::Result {
    if defaults.len() < 2 {
        return Ok(());
    }
    indent(out, depth)?;
    out.push_str("default {\n");
    for value in defaults {
        indent(out, depth + 1)?;
        writeln!(out, "{}", quoted(value))?;
    }
    indent(out, depth)?;
    out.push_str("}\n");
    Ok(())
}

fn write_choices(out: &mut String, choices: &[&str], depth: usize) -> core::fmt::Result {
    if choices.is_empty() {
        return Ok(());
    }
    indent(out, depth)?;
    out.push_str("choices");
    for choice in choices {
        write!(out, " {}", quoted(choice))?;
    }
    out.push('\n');
    Ok(())
}

/// The `-s --long` form a spec uses to declare a flag.
fn flag_forms(flag: &Flag<'_>) -> String {
    let mut forms = String::new();
    for short in flag.shorts {
        if !forms.is_empty() {
            forms.push(' ');
        }
        // A short is one byte, and a non-ASCII one could not have been matched
        // against a token in the first place.
        forms.push('-');
        forms.push(*short as char);
    }
    for long in flag.longs {
        if !forms.is_empty() {
            forms.push(' ');
        }
        forms.push_str("--");
        forms.push_str(long);
    }
    forms
}

/// `<name>` or `<name>...`, the spec's way of writing a value placeholder.
fn placeholder(name: &str, variadic: bool) -> String {
    let ellipsis = if variadic { "..." } else { "" };
    format!("<{name}>{ellipsis}")
}

/// A positional's placeholder: angle brackets when required, square when not.
fn arg_placeholder(name: &str, meta: &ArgMeta<'_>) -> String {
    let ellipsis = if meta.arg.var { "..." } else { "" };
    if meta.required {
        format!("<{name}>{ellipsis}")
    } else {
        format!("[{name}]{ellipsis}")
    }
}

fn indent(out: &mut String, depth: usize) -> core::fmt::Result {
    for _ in 0..depth {
        out.push_str("    ");
    }
    Ok(())
}

fn prop(out: &mut String, key: &str, value: &str) -> core::fmt::Result {
    writeln!(out, "{key} {}", quoted(value))
}

/// Quote a value as a KDL string.
///
/// Always quoted, even where KDL would accept a bare identifier: deciding when a
/// string is bare-safe means encoding KDL's identifier rules, and getting that
/// subtly wrong produces a spec that parses as something else. Quoting always is
/// less pretty and cannot be wrong.
fn quoted(value: &str) -> String {
    let mut out = String::with_capacity(value.len() + 2);
    out.push('"');
    for ch in value.chars() {
        match ch {
            '"' => out.push_str("\\\""),
            '\\' => out.push_str("\\\\"),
            '\n' => out.push_str("\\n"),
            '\r' => out.push_str("\\r"),
            '\t' => out.push_str("\\t"),
            // KDL forbids other literal control characters in a quoted string, and
            // help text really does contain them: a CLI that colors its help with
            // ANSI codes has an escape character in the middle of it.
            c if c.is_control() => {
                let _ = write!(out, "\\u{{{:x}}}", c as u32);
            }
            c => out.push(c),
        }
    }
    out.push('"');
    out
}

/// A struct that describes one command's flags and arguments.
///
/// Implemented by the derive on a subcommand's argument struct. The root's
/// generated parse needs to name the type that accumulates a subcommand's values
/// while parsing, and cannot know which module the derive put it in — an
/// associated type is how it names it regardless.
pub trait CommandArgs: Sized {
    /// Values collected so far. Partly-filled by construction, since a parse can
    /// stop early.
    ///
    /// No `Default` bound: [`CommandArgs::start`] is what produces a fresh one,
    /// because a command with subcommands of its own has nested state that a derived
    /// `Default` cannot set up.
    type Partial;

    /// The parse tables for this command.
    ///
    /// A const rather than a method, because a parent splices it into its own
    /// `static` tables and a method call is not allowed there. That is the whole
    /// reason this trait exists: the tables stay static all the way down, so
    /// nothing is built at run time to start a parse.
    const COMMAND: &'static Command<'static>;

    /// The metadata for this command.
    const META: &'static CommandMeta<'static>;

    /// A partial with any declared defaults already in place.
    ///
    /// Not `Default::default()`, because a default has to be there before parsing
    /// starts: nothing afterwards distinguishes it from what the user typed.
    fn start() -> Self::Partial;

    /// Take one event, and say whether it belonged to this command.
    ///
    /// Keys are unique across a CLI, so an event that is not this command's is left
    /// for whoever owns it rather than mistaken for a local field.
    fn apply(partial: &mut Self::Partial, event: &crate::Event<'_, '_>) -> bool;

    /// Everything this command decides after the last token: required-ness,
    /// choices, and how many values a variadic got.
    ///
    /// Separate from [`CommandArgs::build`] because only the command that was
    /// actually reached is judged — a flag that `install` requires says nothing
    /// about an invocation that ran `run`.
    /// Defaulted, so a hand-written implementation with nothing to check is not
    /// forced to say so — and adding a check to the derive does not break one.
    fn check<'t, 'v>(partial: &mut Self::Partial) -> Result<(), crate::Error<'t, 'v>> {
        let _ = partial;
        Ok(())
    }

    /// Build the struct from what was collected.
    ///
    /// Fallible because a command can require a subcommand of its own, and "none was
    /// given" is only knowable here — at the point where the value has to exist.
    fn build<'t, 'v>(partial: Self::Partial) -> Result<Self, crate::Error<'t, 'v>>;
}

/// An enum whose variants are a command's subcommands.
///
/// Implemented by the derive on the enum a `subcommand` field holds.
pub trait Subcommands: Sized {
    /// Values collected for whichever variant is being filled.
    type Partial: Default;

    /// The parse tables for the variants, to splice into the parent command's
    /// `static` tables — hence a const. See [`CommandArgs::COMMAND`].
    const COMMANDS: &'static [&'static Command<'static>];

    /// The metadata for the variants, in the same order.
    const METAS: &'static [&'static CommandMeta<'static>];

    /// Take one event, and say whether it belonged to one of these commands.
    fn apply(partial: &mut Self::Partial, event: &crate::Event<'_, '_>) -> bool;

    /// Check the selected command's requirements, and nothing else's.
    ///
    /// A flag that `install` requires says nothing about an invocation that ran
    /// `run`, so only the command that was actually reached is judged.
    ///
    /// Identified by its position in [`Subcommands::COMMANDS`] rather than by its
    /// key: the position is found from the table's own address, so two commands whose
    /// keys happen to collide still cannot be confused for one another.
    fn check<'t, 'v>(
        partial: &mut Self::Partial,
        selected: usize,
    ) -> Result<(), crate::Error<'t, 'v>>;

    /// Build the variant at `selected`, a position in [`Subcommands::COMMANDS`].
    ///
    /// `None` when no variant was selected, which a caller reads as "no subcommand
    /// was given". An `Err` comes from building the variant that *was* selected.
    fn select<'t, 'v>(
        partial: Self::Partial,
        selected: usize,
    ) -> Result<Option<Self>, crate::Error<'t, 'v>>;
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn quoting_escapes_what_would_break_a_document() {
        assert_eq!(quoted("plain"), r#""plain""#);
        assert_eq!(quoted(r#"say "hi""#), r#""say \"hi\"""#);
        assert_eq!(quoted("a\\b"), r#""a\\b""#);
        assert_eq!(quoted("one\ntwo"), r#""one\ntwo""#);
    }

    #[test]
    fn a_subcommand_writes_unknown_flags_only_where_it_differs() {
        // The tables hold the effective value per command, so repeating the enclosing
        // command's answer says nothing — but a command that differs has to say so, or
        // the setting never reaches the spec.
        static STRICT_SUB: Command = Command {
            name: "build",
            unknown_flags: UnknownFlags::Error,
            ..Command::EMPTY
        };
        static LENIENT_SUB: Command = Command {
            name: "exec",
            unknown_flags: UnknownFlags::Value,
            ..Command::EMPTY
        };
        static ROOT: Command = Command {
            name: "ex",
            subcommands: &[&STRICT_SUB, &LENIENT_SUB],
            unknown_flags: UnknownFlags::Error,
            ..Command::EMPTY
        };
        static STRICT_META: CommandMeta = CommandMeta {
            cmd: &STRICT_SUB,
            ..CommandMeta::EMPTY
        };
        static LENIENT_META: CommandMeta = CommandMeta {
            cmd: &LENIENT_SUB,
            ..CommandMeta::EMPTY
        };
        static ROOT_META: CommandMeta = CommandMeta {
            cmd: &ROOT,
            subcommands: &[&STRICT_META, &LENIENT_META],
            ..CommandMeta::EMPTY
        };

        let mut out = String::new();
        write_body(&mut out, &ROOT_META, 0).unwrap();

        // Counted rather than checked with `contains`, which is how a duplicated
        // write survived review: `unknown_flags="value" unknown_flags="value"` contains
        // the string it was checked for. A KDL node carrying the same property twice
        // keeps only the last, so once is the whole point.
        let line = |name: &str| -> String {
            out.lines()
                .map(str::trim)
                .find(|l| l.starts_with(&format!(r#"cmd "{name}""#)))
                .unwrap_or_else(|| panic!("no `{name}` command was written:\n{out}"))
                .to_string()
        };

        let build = line("build");
        assert!(
            !build.contains("unknown_flags"),
            "a subcommand matching the enclosing command should not repeat it: {build}"
        );

        let exec = line("exec");
        assert_eq!(
            exec.matches(r#"unknown_flags="value""#).count(),
            1,
            "a differing subcommand declares it exactly once: {exec}"
        );
    }

    #[test]
    fn flag_forms_lists_shorts_then_longs() {
        static F: Flag = Flag {
            longs: &["jobs", "workers"],
            shorts: b"jw",
            ..Flag::VALUE
        };
        assert_eq!(flag_forms(&F), "-j -w --jobs --workers");
    }

    #[test]
    fn placeholders_show_arity_and_optionality() {
        static REQ: Arg = Arg {
            name: "file",
            ..Arg::REQUIRED
        };
        static VAR: Arg = Arg {
            name: "rest",
            ..Arg::VAR
        };
        let required = ArgMeta {
            arg: &REQ,
            ..ArgMeta::EMPTY
        };
        let optional_var = ArgMeta {
            arg: &VAR,
            required: false,
            ..ArgMeta::EMPTY
        };
        assert_eq!(arg_placeholder("file", &required), "<file>");
        assert_eq!(arg_placeholder("rest", &optional_var), "[rest]...");
        assert_eq!(placeholder("n", false), "<n>");
        assert_eq!(placeholder("pattern", true), "<pattern>...");
    }
}
