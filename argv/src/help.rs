//! The one-line summary of how a command is invoked.
//!
//! `Usage: mise use [OPTIONS] <TOOL@VERSION>…` — the line at the top of `--help`, and the
//! first thing a CLI framework has to be able to produce.
//!
//! Built from the same `&'static` metadata a parse ignores, so a binary that never asks for
//! help pays nothing for being able to. Nothing here is on the hot path.
//!
//! # Matching usage-lib
//!
//! usage-lib renders this from a spec, through a tera template over a runtime model. This
//! crate cannot: there is no `Spec` at run time, only the tables. So the rules are
//! reimplemented, and the test that matters compares the two outputs over every command in
//! mise's real spec — 211 of them — because an adopter's help text changing is a visible
//! regression even when it is a small one.
//!
//! Where the two disagree the difference is recorded, in the same spirit as the parser's
//! corpus: usage-lib is the reference, and a divergence is a decision rather than an accident.

use core::fmt::Write as _;

use crate::spec::{ArgMeta, CommandMeta, FlagMeta, Spec};
use crate::DoubleDash;

/// How many flags or arguments are listed individually before collapsing to a placeholder.
///
/// usage-lib's number. Beyond it the line would be longer than it is useful, so it becomes
/// `[FLAGS]` or `[ARGS]…` and the sections below carry the detail.
const INLINE_LIMIT: usize = 2;

/// The `Usage:` line's body, without the `Usage: ` prefix.
///
/// `path` is the command as invoked, starting with the binary: `["mise", "config", "ls"]`.
/// The metadata holds a tree and each node knows its own name, so the path a *particular*
/// invocation took has to come from the caller — which is the parser, or a `help` command
/// naming a command explicitly.
///
/// ```
/// use usage_argv::help::usage_line;
/// use usage_argv::spec::{ArgMeta, CommandMeta, FlagMeta};
/// use usage_argv::{Arg, Command, Flag};
///
/// static FORCE: Flag = Flag { name: "force", longs: &["force"], ..Flag::BOOL };
/// static TOOL: Arg = Arg { name: "TOOL", ..Arg::REQUIRED };
/// static CMD: Command = Command {
///     name: "use",
///     flags: &[&FORCE],
///     args: &[&TOOL],
///     ..Command::EMPTY
/// };
/// static META: CommandMeta = CommandMeta {
///     cmd: &CMD,
///     flags: &[FlagMeta { flag: &FORCE, ..FlagMeta::EMPTY }],
///     args: &[ArgMeta { arg: &TOOL, required: true, ..ArgMeta::EMPTY }],
///     ..CommandMeta::EMPTY
/// };
///
/// assert_eq!(usage_line(&["mise", "use"], &META), "mise use [--force] <TOOL>");
/// ```
pub fn usage_line(path: &[&str], meta: &CommandMeta<'_>) -> String {
    let mut out = String::new();
    for (i, part) in path.iter().enumerate() {
        if i > 0 {
            out.push(' ');
        }
        out.push_str(part);
    }

    // Hidden entries are absent from the line as they are from the sections: help describes
    // what a user is invited to type.
    let flags: usize = meta.flags.iter().filter(|f| !f.hide).count();
    if flags > 0 {
        let required = meta.flags.iter().any(|f| !f.hide && flag_demanded(f));
        if flags <= INLINE_LIMIT {
            for flag in meta.flags.iter().filter(|f| !f.hide) {
                // A required flag is angled, like a required argument: the brackets are what
                // say whether leaving it out is allowed.
                let (open, close) = if flag_demanded(flag) {
                    ('<', '>')
                } else {
                    ('[', ']')
                };
                let _ = write!(out, " {open}{}{close}", flag_usage(flag));
            }
        } else if required {
            out.push_str(" <FLAGS>");
        } else {
            out.push_str(" [FLAGS]");
        }
    }

    let args: usize = meta.args.iter().filter(|a| !a.hide).count();
    if args > 0 {
        let required = meta.args.iter().any(|a| !a.hide && demanded(a));
        if args <= INLINE_LIMIT {
            for arg in meta.args.iter().filter(|a| !a.hide) {
                let _ = write!(out, " {}", arg_usage(arg));
            }
        } else if required {
            out.push_str(" <ARGS>…");
        } else {
            out.push_str(" [ARGS]…");
        }
    }

    if !meta.cmd.subcommands.is_empty() {
        out.push_str(" <SUBCOMMAND>");
    }
    out
}

/// How one flag appears in the usage line: `-f --force`, plus its value if it takes one.
fn flag_usage(meta: &FlagMeta<'_>) -> String {
    let flag = meta.flag;
    let mut out = String::new();

    // The declared name, when it is not the one the forms would imply. A flag called
    // `verbose` reachable only as `-v` has to say so, or help would name something the
    // spec does not.
    let implied = flag
        .longs
        .first()
        .copied()
        .or_else(|| flag.shorts.first().map(|_| ""));
    let implied_matches = match (implied, flag.shorts.first()) {
        (Some(long), _) if !long.is_empty() => long == flag.name,
        (Some(_), Some(short)) => {
            let mut buf = [0u8; 4];
            (*short as char).encode_utf8(&mut buf) == flag.name
        }
        _ => false,
    };
    if !implied_matches {
        let _ = write!(out, "{}:", flag.name);
    }
    if let Some(short) = flag.shorts.first() {
        if !out.is_empty() {
            out.push(' ');
        }
        let _ = write!(out, "-{}", *short as char);
    }
    if let Some(long) = flag.longs.first() {
        if !out.is_empty() {
            out.push(' ');
        }
        let _ = write!(out, "--{long}");
    }

    // A repeatable flag, which is the spec's `var=#true` — not one occurrence taking several
    // values, which is the value's own business below.
    if meta.repeatable {
        out.push('…');
    }
    if flag.takes_value {
        let name = meta.value_name.unwrap_or(flag.name);
        let _ = write!(out, " <{name}>");
        if flag.variadic {
            out.push('…');
        }
    }
    out
}

/// How one positional argument appears: `<TOOL>`, `[FILES]…`, `-- <ARGS>`.
/// Whether a flag must be given, which is not quite what `required` says.
///
/// Same rule as [`demanded`], for the same reason: usage-lib clears `required` on a flag that
/// declares a default before rendering, so reading the flag alone printed `<--out>` for a flag
/// the parser fills when it is left out.
fn flag_demanded(meta: &FlagMeta<'_>) -> bool {
    meta.required && meta.default.is_empty()
}

/// Whether an argument must be given, which is not quite what `required` says.
///
/// usage-lib clears `required` while *parsing* a spec that declares a default — a defaulted
/// argument is one the user may leave out — and then renders the usage line from `required`
/// alone. The derive keeps the two separate, so reading `required` on its own printed `<file>`
/// where usage-lib prints `[file]`, for an argument the parser is perfectly happy to omit.
///
/// Applied here rather than by clearing the flag in the metadata, because the metadata is what
/// the emitted spec is built from and `required` there means what the author wrote.
fn demanded(meta: &ArgMeta<'_>) -> bool {
    meta.required && meta.default.is_empty()
}

fn arg_usage(meta: &ArgMeta<'_>) -> String {
    let arg = meta.arg;
    let mut out = String::new();
    let (open, close) = if demanded(meta) {
        ('<', '>')
    } else {
        ('[', ']')
    };
    // An argument that only takes what follows a `--` shows the separator, because typing the
    // value without it does not reach this argument at all — and the brackets go *outside*
    // it, as usage-lib writes it: `[-- COMMAND]…`, one optional thing rather than a literal
    // `--` followed by an optional word.
    if arg.double_dash == DoubleDash::Required {
        let _ = write!(out, "{open}-- {}{close}", arg.name);
    } else {
        let _ = write!(out, "{open}{}{close}", arg.name);
    }
    if arg.var {
        out.push('…');
    }
    out
}

/// Everything `-h` prints.
///
/// The short form: one line per entry, its help beside it. `--help` renders the same content
/// through a wider layout, which is the next thing to build — the two differ in presentation
/// and in which help text they prefer, not in what they cover.
///
/// `path` is the command as invoked, as for [`usage_line`].
pub fn short_help(spec: &Spec<'_>, path: &[&str], meta: &CommandMeta<'_>) -> String {
    let mut out = String::new();

    // The program, then what it is for. usage-lib prints the name when the spec gives one and
    // the binary otherwise, and only when there is a version to put beside it.
    if let Some(version) = spec.version {
        let name = if spec.name.is_empty() {
            spec.bin.unwrap_or_default()
        } else {
            spec.name
        };
        let _ = writeln!(out, "{name} {version}");
    }
    if let Some(about) = spec.about {
        let _ = writeln!(out, "{about}\n");
    }
    let _ = writeln!(out, "Usage: {}", usage_line(path, meta));

    // The path without the binary, which is what a listed subcommand shows: usage-lib prints
    // `tool-alias get <TOOL>` under `mise tool-alias`, the whole path from the root rather
    // than the child's own name.
    commands_section(&mut out, &path[1.min(path.len())..], meta);
    groups_section(
        &mut out,
        "Arguments",
        meta.args.iter().filter(|a| !a.hide),
        |a| a.help_heading,
        |out, a| {
            let _ = write!(out, "  {}", arg_usage(a));
            if let Some(help) = a.help {
                let _ = write!(out, "  {help}");
            }
            annotations(out, a.choices, a.env, a.default);
        },
    );
    groups_section(
        &mut out,
        "Flags",
        meta.flags.iter().filter(|f| !f.hide),
        |f| f.help_heading,
        |out, f| {
            let _ = write!(out, "  {}", display_usage(f));
            if let Some(help) = f.help {
                let _ = write!(out, "  {help}");
            }
            annotations(out, f.choices, f.env, &[]);
        },
    );
    examples_section(&mut out, meta);

    // usage-lib trims the whole document and puts back one newline, which is what keeps the
    // blank lines between sections from becoming trailing ones.
    let trimmed = out.trim();
    let mut done = String::with_capacity(trimmed.len() + 1);
    done.push_str(trimmed);
    done.push('\n');
    done
}

/// The list of subcommands, and the `help` command every CLI with subcommands has.
fn commands_section(out: &mut String, path: &[&str], meta: &CommandMeta<'_>) {
    let visible: Vec<&&CommandMeta<'_>> = meta.subcommands.iter().filter(|c| !c.hide).collect();
    // Nothing visible, no section — `mise direnv` and `mise dotfiles` have subcommands and
    // every one of them is hidden. The usage *line* still says `<SUBCOMMAND>`, because
    // usage-lib computes it before filtering and stores it; matching the reference means
    // matching that too, odd as the pair looks together.
    if visible.is_empty() {
        return;
    }
    let _ = writeln!(out, "\nCommands:");

    // Sorted by the rendered usage rather than by name, as usage-lib sorts them — for a
    // command with no flags or arguments the two agree, and where they differ this is the
    // order a reader sees in the reference.
    let mut lines: Vec<(String, &&CommandMeta<'_>)> = visible
        .iter()
        .map(|sub| {
            let mut sub_path: Vec<&str> = path.to_vec();
            sub_path.push(sub.cmd.name);
            (usage_line(&sub_path, sub), *sub)
        })
        .collect();
    lines.sort_by(|a, b| a.0.cmp(&b.0));

    for (usage, sub) in &lines {
        let _ = write!(out, "  {usage}");
        // Visible aliases only: a hidden alias works and is not advertised, which is the
        // whole of the distinction.
        let visible_aliases: Vec<&str> = sub
            .cmd
            .aliases
            .iter()
            .copied()
            .filter(|a| !sub.hidden_aliases.contains(a))
            .collect();
        if !visible_aliases.is_empty() {
            let _ = write!(out, " [aliases: {}]", visible_aliases.join(", "));
        }
        if let Some(about) = sub.about {
            let _ = write!(out, "  {about}");
        }
        out.push('\n');
    }
    let _ = writeln!(
        out,
        "  help  Print this message or the help of the given subcommand(s)"
    );
}

/// One section per heading, unheaded first, in the order the headings first appear.
fn groups_section<'m, T: 'm>(
    out: &mut String,
    default_title: &str,
    items: impl Iterator<Item = &'m T> + Clone,
    heading_of: impl Fn(&T) -> Option<&'m str>,
    mut write_item: impl FnMut(&mut String, &T),
) {
    // Headings in first-seen order, with the unheaded group before them. Collected rather
    // than sorted so that "first seen" means what it says.
    let mut headings: Vec<Option<&str>> = Vec::new();
    for item in items.clone() {
        let heading = heading_of(item);
        if !headings.contains(&heading) {
            headings.push(heading);
        }
    }
    headings.sort_by_key(|h| h.is_some());

    for heading in headings {
        let _ = writeln!(out, "\n{}:", heading.unwrap_or(default_title));
        for item in items.clone().filter(|i| heading_of(i) == heading) {
            write_item(out, item);
        }
    }
}

/// The bracketed notes after an entry's help: choices, environment, default.
fn annotations(out: &mut String, choices: &[&str], env: Option<&str>, default: &[&str]) {
    if !choices.is_empty() {
        let _ = write!(out, " [{}]", choices.join(", "));
    }
    if let Some(env) = env {
        let _ = write!(out, " [env: {env}]");
    }
    if !default.is_empty() {
        let _ = write!(out, " (default: {})", default.join(", "));
    }
    out.push('\n');
}

/// A flag as the flags section lists it, which includes its negation.
fn display_usage(meta: &FlagMeta<'_>) -> String {
    let usage = flag_usage(meta);
    match meta.flag.negate {
        Some(negate) => format!("{usage} / --{negate}"),
        None => usage,
    }
}

fn examples_section(out: &mut String, meta: &CommandMeta<'_>) {
    if meta.examples.is_empty() {
        return;
    }
    let _ = writeln!(out, "\nExamples:");
    for example in meta.examples {
        if let Some(header) = example.header {
            let _ = writeln!(out, "  {header}:");
        }
        let _ = writeln!(out, "    $ {}", example.code);
    }
}
