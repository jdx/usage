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

use crate::spec::{ArgMeta, CommandMeta, Example, FlagMeta, Spec};
use crate::Command;
use crate::DoubleDash;

/// How many flags or arguments are listed individually before collapsing to a placeholder.
///
/// usage-lib's number. Beyond it the line would be longer than it is useful, so it becomes
/// `[FLAGS]` or `[ARGS]…` and the sections below carry the detail.
const INLINE_LIMIT: usize = 2;

/// Whether help output is coloured.
///
/// Plain rendering remains available for generated documents and snapshots;
/// process-facing help uses [`Style::auto`].
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Style {
    coloured: bool,
}

impl Style {
    /// Plain text, suitable for a pipe or a generated artifact.
    pub const PLAIN: Style = Style { coloured: false };
    /// ANSI-coloured text, regardless of the output destination.
    pub const COLOURED: Style = Style { coloured: true };

    /// Colour when stdout is a terminal and the environment permits it.
    pub fn auto() -> Style {
        use std::io::IsTerminal as _;
        let forced = std::env::var_os("CLICOLOR_FORCE").is_some_and(|v| v != "0");
        let refused = std::env::var_os("NO_COLOR").is_some_and(|v| !v.is_empty());
        if refused {
            Style::PLAIN
        } else if forced || std::io::stdout().is_terminal() {
            Style::COLOURED
        } else {
            Style::PLAIN
        }
    }

    fn wrap(self, code: &str, text: &str) -> String {
        if self.coloured {
            format!("\u{1b}[{code}m{text}\u{1b}[0m")
        } else {
            text.to_string()
        }
    }

    fn heading(self, text: &str) -> String {
        self.wrap("1;4;32", text)
    }

    fn literal(self, text: &str) -> String {
        self.wrap("36", text)
    }
}

fn styled_flag_usage(usage: &str, style: Style) -> String {
    let mut out = String::with_capacity(usage.len());
    let mut rest = usage;
    while let Some(start) = rest.find('-') {
        let previous_allows = start == 0
            || rest[..start]
                .chars()
                .next_back()
                .is_some_and(|c| c.is_whitespace() || matches!(c, ',' | ':' | '[' | '<'));
        if !previous_allows {
            out.push_str(&rest[..=start]);
            rest = &rest[start + 1..];
            continue;
        }
        let end = rest[start..]
            .char_indices()
            .skip(1)
            .find_map(|(i, c)| (c.is_whitespace() || matches!(c, ',' | ']' | '>')).then_some(i))
            .unwrap_or(rest.len() - start)
            + start;
        out.push_str(&rest[..start]);
        out.push_str(&style.literal(&rest[start..end]));
        rest = &rest[end..];
    }
    out.push_str(rest);
    out
}

fn help_structure(
    spec: &Spec<'_>,
    path: &[&str],
    chain: &[&CommandMeta<'_>],
    long: bool,
) -> (Vec<String>, Vec<String>, Vec<String>) {
    let meta = *chain.last().expect("a page is always about some command");
    let mut headings = Vec::new();
    if !page_examples(spec, meta).is_empty() {
        headings.push("Examples".to_string());
    }
    if meta.flatten_help {
        flat_help_headings(&path[1.min(path.len())..], meta, &mut headings);
    } else if meta.subcommands.iter().any(|sub| !sub.hide) {
        headings.push(
            meta.subcommand_help_heading
                .unwrap_or("Commands")
                .to_string(),
        );
    }

    let (own, inherited) = own_and_global(chain);
    let visible_arg = |arg: &&ArgMeta<'_>| {
        !arg.hide
            && if long {
                !arg.hide_long_help
            } else {
                !arg.hide_short_help
            }
    };
    let args: Vec<_> = meta.args.iter().filter(visible_arg).collect();
    if args.iter().any(|arg| arg.help_heading.is_none()) {
        headings.push("Arguments".to_string());
    }
    headings.extend(
        args.iter()
            .filter_map(|arg| arg.help_heading)
            .map(str::to_string),
    );

    let visible_flag = |flag: &&FlagMeta<'_>| {
        !flag.hide
            && if long {
                !flag.hide_long_help
            } else {
                !flag.hide_short_help
            }
    };
    let own: Vec<_> = own.into_iter().filter(visible_flag).collect();
    let inherited: Vec<_> = inherited
        .into_iter()
        .filter(|(flag, _)| {
            if long {
                !flag.hide_long_help
            } else {
                !flag.hide_short_help
            }
        })
        .collect();
    if own.iter().any(|flag| flag.help_heading.is_none()) {
        headings.push("Flags".to_string());
    }
    headings.extend(
        own.iter()
            .filter_map(|flag| flag.help_heading)
            .map(str::to_string),
    );
    if !inherited.is_empty() {
        headings.push("Global flags".to_string());
    }

    let mut flag_usages: Vec<String> = own.iter().map(|flag| column_usage(flag)).collect();
    flag_usages.extend(inherited.into_iter().map(|(_, usage)| usage));
    flag_usages.sort_by_key(|usage| core::cmp::Reverse(usage.len()));

    let mut synopsis = String::new();
    usage_section(&mut synopsis, spec, path, meta);
    let synopsis = synopsis.lines().map(str::to_string).collect();
    (headings, flag_usages, synopsis)
}

fn flat_help_headings(path: &[&str], meta: &CommandMeta<'_>, headings: &mut Vec<String>) {
    let mut visible: Vec<_> = meta.subcommands.iter().filter(|sub| !sub.hide).collect();
    visible.sort_by_key(|sub| sub.cmd.name);
    for sub in visible {
        let mut sub_path = path.to_vec();
        sub_path.push(sub.cmd.name);
        headings.push(sub_path.join(" "));
        if sub.flatten_help {
            flat_help_headings(&sub_path, sub, headings);
        }
    }
}

fn styled_help(
    page: &str,
    style: Style,
    headings: &[String],
    flag_usages: &[String],
    synopsis: &[String],
) -> String {
    if !style.coloured {
        return page.to_string();
    }
    let mut out = String::with_capacity(page.len());
    for line in page.split_inclusive('\n') {
        let (body, newline) = line
            .strip_suffix('\n')
            .map_or((line, ""), |body| (body, "\n"));
        if synopsis.iter().any(|known| known == body) && body.starts_with("Usage:") {
            let usage = body.strip_prefix("Usage:").unwrap_or_default();
            out.push_str(&style.heading("Usage:"));
            out.push_str(&style.literal(usage));
        } else if synopsis.iter().any(|known| known == body) {
            out.push_str(&style.literal(body));
        } else if body
            .strip_suffix(':')
            .is_some_and(|heading| headings.iter().any(|known| known == heading))
        {
            out.push_str(&style.heading(body));
        } else {
            let styled = body.strip_prefix("  ").and_then(|entry| {
                flag_usages.iter().find_map(|usage| {
                    entry
                        .strip_prefix(usage)
                        .filter(|rest| rest.is_empty() || rest.starts_with(char::is_whitespace))
                        .map(|rest| format!("  {}{rest}", styled_flag_usage(usage, style)))
                })
            });
            out.push_str(styled.as_deref().unwrap_or(body));
        }
        out.push_str(newline);
    }
    out
}

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
    usage_line_with_subcommands(path, meta, true)
}

fn usage_line_with_subcommands(
    path: &[&str],
    meta: &CommandMeta<'_>,
    include_subcommands: bool,
) -> String {
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

    if include_subcommands && !meta.cmd.subcommands.is_empty() {
        let name = meta.subcommand_value_name.unwrap_or("SUBCOMMAND");
        let _ = write!(out, " <{name}>");
    }
    out
}

/// Write the synopsis for a page, preferring the root's explicit alternatives.
///
/// An explicit synopsis belongs to the program rather than every command below it. Subcommand
/// pages still derive their own invocation from the route and command metadata.
fn usage_section(out: &mut String, spec: &Spec<'_>, path: &[&str], meta: &CommandMeta<'_>) {
    if path.len() <= 1 {
        if let Some(usage) = spec.usage.filter(|usage| !usage.trim().is_empty()) {
            let _ = writeln!(out, "{}", usage.trim());
            return;
        }
    }
    let mut visible: Vec<_> = meta.subcommands.iter().filter(|sub| !sub.hide).collect();
    visible.sort_by_key(|sub| sub.cmd.name);
    if meta.flatten_help && !visible.is_empty() {
        let mut lines = Vec::new();
        if !meta.subcommand_required || meta.cmd.args_conflicts_with_subcommands {
            lines.push(usage_line_with_subcommands(path, meta, false));
        }
        for sub in visible {
            let mut sub_path = path.to_vec();
            sub_path.push(sub.cmd.name);
            lines.push(usage_line(&sub_path, sub));
        }
        if let Some((first, rest)) = lines.split_first() {
            let _ = writeln!(out, "Usage: {first}");
            for line in rest {
                let _ = writeln!(out, "       {line}");
            }
        }
    } else {
        let _ = writeln!(out, "Usage: {}", usage_line(path, meta));
    }
}

/// How one flag appears in the usage line: `-f --force`, plus its value if it takes one.
fn flag_usage(meta: &FlagMeta<'_>) -> String {
    flag_usage_masked(meta, &Shown::all(meta))
}

/// The spellings of one flag that a page should offer.
///
/// Not "hide the long" and "hide the short": a flag may answer to several of each, and a
/// descendant claiming `--jobs` leaves an inherited `--workers` working. What is shown is the
/// first of each kind that nothing nearer has taken.
struct Shown<'a> {
    long: Option<&'a str>,
    short: Option<u8>,
    /// Whether the negation is still this flag's to offer. `--no-color` is a spelling like any
    /// other and something nearer can claim it.
    negate: bool,
}

impl<'a> Shown<'a> {
    /// Everything the flag has, for a command's own flags — nothing above them to claim any.
    fn all(meta: &'a FlagMeta<'a>) -> Self {
        Shown {
            long: meta
                .flag
                .longs
                .iter()
                .copied()
                .find(|long| !meta.hidden_longs.contains(long)),
            short: meta
                .flag
                .shorts
                .iter()
                .copied()
                .find(|short| !meta.hidden_shorts.contains(short)),
            negate: meta.flag.negate.is_some(),
        }
    }

    /// What is left of a flag once everything nearer has had its pick.
    ///
    /// `taken` is the longs and shorts already claimed; `taken_negations` the negations;
    /// `every_form` every long and short in scope at any distance, because the parser resolves
    /// a word against all of those before it looks at a negation at all.
    fn surviving(
        meta: &'a FlagMeta<'a>,
        taken: &[String],
        taken_negations: &[String],
        every_form: &[String],
    ) -> Self {
        let mine: Vec<String> = meta
            .flag
            .longs
            .iter()
            .map(|l| format!("--{l}"))
            .chain(meta.flag.shorts.iter().map(|s| format!("-{}", *s as char)))
            .collect();
        Shown {
            long: meta
                .flag
                .longs
                .iter()
                .copied()
                .find(|l| !meta.hidden_longs.contains(l) && !taken.contains(&format!("--{l}"))),
            short: meta.flag.shorts.iter().copied().find(|s| {
                !meta.hidden_shorts.contains(s) && !taken.contains(&format!("-{}", *s as char))
            }),
            negate: meta.flag.negate.is_some_and(|n| {
                let spelling = format!("--{n}");
                // A long anywhere in scope wins over this, this flag's own excepted.
                !taken_negations.contains(&spelling)
                    && (!every_form.contains(&spelling) || mine.contains(&spelling))
            }),
        }
    }

    fn nothing(&self) -> bool {
        self.long.is_none() && self.short.is_none() && !self.negate
    }
}

/// The same, with a spelling left out because something nearer claimed it.
///
/// A descendant may take one of an ancestor's two spellings — its own `-v` beside the root's
/// `-v, --verbose` — and the parser still accepts the other, so the page has to offer the other
/// and not the one that now means something else.
fn flag_usage_masked(meta: &FlagMeta<'_>, show: &Shown) -> String {
    let flag = meta.flag;
    let mut out = String::new();

    // The declared name, when it is not the one the forms would imply. A flag called
    // `verbose` reachable only as `-v` has to say so, or help would name something the
    // spec does not.
    //
    // Judged on the forms this page is *showing*. mise's root has a global `-E --env`; a
    // descendant that claims `--env` leaves `-E` inherited, and `-E… <ENV>` alone gives a
    // reader nothing to connect it to the `--env` they saw elsewhere. `env: -E… <ENV>` does.
    let long = show.long;
    let short = show.short.as_ref();
    let implied = long.or_else(|| short.map(|_| ""));
    let implied_matches = match (implied, short) {
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
    if let Some(short) = short {
        if !out.is_empty() {
            out.push(' ');
        }
        let _ = write!(out, "-{}", *short as char);
    }
    if let Some(long) = long {
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
        // Angled where the value must be given, squared where it need not — the same brackets
        // an argument uses, and for the same reason. pitchfork's `--bump` is the fleet's case.
        let (open, close) = if meta.value_optional {
            ('[', ']')
        } else {
            ('<', '>')
        };
        let exact = exact_arity(meta.value_var_min, meta.value_var_max);
        if meta.value_names.len() <= 1 && exact.is_some_and(|n| n > 1) {
            let name = meta
                .value_names
                .first()
                .copied()
                .or(meta.value_name)
                .unwrap_or(flag.name);
            for _ in 0..exact.unwrap() {
                let _ = write!(out, " {open}{name}{close}");
            }
        } else if meta.value_names.len() <= 1 {
            let name = meta
                .value_names
                .first()
                .copied()
                .or(meta.value_name)
                .unwrap_or(flag.name);
            let _ = write!(out, " {open}{name}{close}");
        } else {
            for name in meta.value_names {
                let _ = write!(out, " {open}{name}{close}");
            }
        }
        if flag.variadic && meta.value_names.len() <= 1 && exact.is_none() {
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

/// How a usage line writes an argument: `<TOOL>`, `[TOOL]`, `[TOOL]…`, `[-- COMMAND]…`.
///
/// Shared with the diagnostics, which name the same argument in an error and must not spell it
/// differently from the page above it.
pub(crate) fn arg_usage(meta: &ArgMeta<'_>) -> String {
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
    let exact = exact_arity(meta.var_min, meta.var_max);
    if meta.value_names.len() <= 1 && exact.is_some_and(|n| n > 1) {
        for index in 0..exact.unwrap() {
            if index > 0 {
                out.push(' ');
            }
            let _ = write!(out, "{open}{}{close}", arg.name);
        }
    } else if meta.value_names.len() <= 1 {
        if arg.double_dash == DoubleDash::Required {
            let _ = write!(out, "{open}-- {}{close}", arg.name);
        } else {
            let _ = write!(out, "{open}{}{close}", arg.name);
        }
    } else {
        if arg.double_dash == DoubleDash::Required {
            out.push_str("-- ");
        }
        for (index, name) in meta.value_names.iter().enumerate() {
            if index > 0 {
                out.push(' ');
            }
            let _ = write!(out, "{open}{name}{close}");
        }
    }
    if arg.var && meta.value_names.len() <= 1 && exact.is_none() {
        out.push('…');
    }
    out
}

fn exact_arity(min: Option<usize>, max: Option<usize>) -> Option<usize> {
    match (min, max) {
        (Some(min), Some(max)) if min == max => Some(min),
        _ => None,
    }
}

/// Everything `-h` prints.
///
/// The short form: one line per entry, its help beside it. `--help` renders the same content
/// through a wider layout, which is the next thing to build — the two differ in presentation
/// and in which help text they prefer, not in what they cover.
///
/// `path` is the command as invoked, as for [`usage_line`].
pub fn short_help(spec: &Spec<'_>, path: &[&str], chain: &[&CommandMeta<'_>]) -> String {
    let meta = *chain.last().expect("a page is always about some command");
    let (own, inherited) = own_and_global(chain);
    let own: Vec<_> = own
        .into_iter()
        .filter(|flag| !flag.hide_short_help)
        .collect();
    let inherited: Vec<_> = inherited
        .into_iter()
        .filter(|(flag, _)| !flag.hide_short_help)
        .collect();
    let mut out = String::new();

    // Text the command puts above everything else, and below it. The short form has only the
    // one pair; the long form prefers the long variants.
    if let Some(before) = meta.before_help.or(spec.root.before_help) {
        let _ = writeln!(out, "{before}\n");
    }

    // The program, then what it is for — on the program's own page. A subcommand's page says
    // what the subcommand does; see the long form for why. usage-lib prints the name when the
    // spec gives one and the binary otherwise, and only when there is a version beside it.
    let root = path.len() <= 1;
    if root {
        if let Some(version) = spec.version {
            let name = if spec.name.is_empty() {
                spec.bin.unwrap_or_default()
            } else {
                spec.name
            };
            let _ = writeln!(out, "{name} {version}");
        }
    }
    let about = if root { spec.about } else { meta.about };
    if let Some(about) = about {
        // Trimmed for the same reason the entries below are: the blank line after the
        // description is written here, so one already in the text doubles it.
        let _ = writeln!(out, "{}\n", about.trim_end());
    }
    usage_section(&mut out, spec, path, meta);

    // The path without the binary, which is what a listed subcommand shows: usage-lib prints
    // `tool-alias get <TOOL>` under `mise tool-alias`, the whole path from the root rather
    // than the child's own name.
    if !meta.flatten_help {
        commands_section(&mut out, &path[1.min(path.len())..], meta);
    }

    // The short page lines its columns up too. It did not: every description began directly
    // after the name it belonged to, so nothing in `-h` lined up with anything — and `-h` is
    // the form most people type. One column per section over its visible entries, which is
    // the rule the long page already follows.
    let args: Vec<&ArgMeta<'_>> = meta
        .args
        .iter()
        .filter(|a| !a.hide && !a.hide_short_help)
        .collect();
    let arg_col = args
        .iter()
        .map(|a| arg_usage(a).chars().count())
        .max()
        .unwrap_or(0);
    groups_section(
        &mut out,
        "Arguments",
        args.iter().copied(),
        |a| a.help_heading,
        |out, a| {
            let usage = arg_usage(a);
            if meta.next_line_help {
                let _ = writeln!(out, "  {usage}");
                if let Some(help) = a.help.filter(|h| !h.trim().is_empty()) {
                    write_indented(out, help, 4);
                }
                long_annotations(
                    out,
                    if a.hide_possible_values {
                        &[]
                    } else {
                        a.choices
                    },
                    if a.hide_env { None } else { a.env },
                    if a.hide_default_value { &[] } else { a.default },
                );
                return;
            }
            match a.help.filter(|h| !h.trim().is_empty()) {
                Some(help) => {
                    let _ = write!(out, "  {usage:<arg_col$}  {help}");
                }
                None => {
                    let _ = write!(out, "  {usage}");
                }
            }
            annotations(
                out,
                if a.hide_possible_values {
                    &[]
                } else {
                    a.choices
                },
                if a.hide_env { None } else { a.env },
                if a.hide_default_value { &[] } else { a.default },
            );
        },
    );
    // One column over *both* lists, so the two sections read as one table with a rule through
    // it rather than two tables that happen to be adjacent.
    let flag_col = own
        .iter()
        .map(|f| column_usage(f).chars().count())
        .chain(inherited.iter().map(|(_, u)| u.chars().count()))
        .max()
        .unwrap_or(0);
    let short_entry = |out: &mut String, f: &FlagMeta<'_>, usage: String| {
        if meta.next_line_help {
            let _ = writeln!(out, "  {usage}");
            if let Some(help) = f.help.filter(|h| !h.trim().is_empty()) {
                write_indented(out, help, 4);
            }
            long_annotations(
                out,
                if f.hide_possible_values {
                    &[]
                } else {
                    f.choices
                },
                if f.hide_env { None } else { f.env },
                if f.hide_default_value { &[] } else { f.default },
            );
            return;
        }
        match f.help.filter(|h| !h.trim().is_empty()) {
            Some(help) => {
                let _ = write!(out, "  {usage:<flag_col$}  {help}");
            }
            None => {
                let _ = write!(out, "  {usage}");
            }
        }
        annotations(
            out,
            if f.hide_possible_values {
                &[]
            } else {
                f.choices
            },
            if f.hide_env { None } else { f.env },
            if f.hide_default_value { &[] } else { f.default },
        );
    };
    groups_section(
        &mut out,
        "Flags",
        own.iter().copied(),
        |f| f.help_heading,
        |out, f| short_entry(out, f, column_usage(f)),
    );
    // After the command's own, and under a heading that says where they came from: `--config`
    // belongs to the program, not to this command, and a reader should be able to see that.
    // The text is precomputed, since a spelling a descendant claimed is left out of it.
    groups_section(
        &mut out,
        "Global flags",
        inherited.iter(),
        |_| None,
        |out, (f, usage)| short_entry(out, f, usage.clone()),
    );
    if meta.flatten_help {
        flat_commands_short(&mut out, &path[1.min(path.len())..], meta);
    }
    examples_section(&mut out, spec, meta);
    if let Some(after) = meta.after_help.or(spec.root.after_help) {
        let _ = writeln!(out, "\n{after}");
    }

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
    let heading = meta.subcommand_help_heading.unwrap_or("Commands");
    let _ = writeln!(out, "\n{heading}:");

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
            if meta.next_line_help {
                out.push('\n');
                write_indented(out, about.trim_end(), 4);
                continue;
            }
            // The row writes its own newline below. Trim trailing whitespace in both
            // layouts, as usage-lib does before choosing a layout.
            let _ = write!(out, "  {}", about.trim_end());
        }
        out.push('\n');
    }
    if meta.next_line_help {
        let _ = writeln!(
            out,
            "  help\n    Print this message or the help of the given subcommand(s)"
        );
    } else {
        let _ = writeln!(
            out,
            "  help  Print this message or the help of the given subcommand(s)"
        );
    }
}

fn flat_commands_short(out: &mut String, path: &[&str], meta: &CommandMeta<'_>) {
    let mut visible: Vec<_> = meta.subcommands.iter().filter(|sub| !sub.hide).collect();
    visible.sort_by_key(|sub| sub.cmd.name);
    for sub in visible {
        let mut sub_path = path.to_vec();
        sub_path.push(sub.cmd.name);
        let _ = writeln!(out, "\n{}:", sub_path.join(" "));
        if let Some(about) = sub.about.filter(|about| !about.trim().is_empty()) {
            let _ = writeln!(out, "{}", about.trim_end());
        }

        let args: Vec<_> = sub
            .args
            .iter()
            .filter(|arg| !arg.hide && !arg.hide_short_help)
            .collect();
        let flags: Vec<&FlagMeta<'_>> = sub
            .flags
            .iter()
            .filter(|flag| !flag.flag.global && !flag.hide && !flag.hide_short_help)
            .collect();
        let col = args
            .iter()
            .map(|arg| arg_usage(arg).chars().count())
            .chain(flags.iter().map(|flag| column_usage(flag).chars().count()))
            .max()
            .unwrap_or(0);
        for arg in args {
            let usage = arg_usage(arg);
            if let Some(help) = arg.help.filter(|help| !help.trim().is_empty()) {
                if meta.next_line_help {
                    let _ = writeln!(out, "  {usage}");
                    write_indented(out, help, 4);
                } else {
                    let _ = write!(out, "  {usage:<col$}  {help}");
                }
            } else {
                let _ = write!(out, "  {usage}");
            }
            annotations(
                out,
                if arg.hide_possible_values {
                    &[]
                } else {
                    arg.choices
                },
                if arg.hide_env { None } else { arg.env },
                if arg.hide_default_value {
                    &[]
                } else {
                    arg.default
                },
            );
        }
        for flag in flags {
            let usage = column_usage(flag);
            if let Some(help) = flag.help.filter(|help| !help.trim().is_empty()) {
                if meta.next_line_help {
                    let _ = writeln!(out, "  {usage}");
                    write_indented(out, help, 4);
                } else {
                    let _ = write!(out, "  {usage:<col$}  {help}");
                }
            } else {
                let _ = write!(out, "  {usage}");
            }
            annotations(
                out,
                if flag.hide_possible_values {
                    &[]
                } else {
                    flag.choices
                },
                if flag.hide_env { None } else { flag.env },
                if flag.hide_default_value {
                    &[]
                } else {
                    flag.default
                },
            );
        }
        if sub.flatten_help {
            flat_commands_short(out, &sub_path, sub);
        }
        out.push('\n');
    }
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

/// How a usage line writes a flag: its first long form, or its short if that is all it has.
///
/// Shared with the diagnostics for the same reason as [`arg_usage`], and gated with them: under
/// `spec` alone nothing calls it, and a `dead_code` warning is an error in this workspace.
#[cfg(feature = "diagnostics")]
pub(crate) fn flag_spelling(meta: &FlagMeta<'_>) -> String {
    meta.flag
        .longs
        .iter()
        .find(|long| !meta.hidden_longs.contains(long))
        .map(|long| format!("--{long}"))
        .or_else(|| {
            meta.flag
                .shorts
                .iter()
                .find(|short| !meta.hidden_shorts.contains(short))
                .map(|short| format!("-{}", *short as char))
        })
        .unwrap_or_else(|| meta.flag.name.to_string())
}

fn display_usage_masked(meta: &FlagMeta<'_>, show: &Shown) -> String {
    let usage = flag_usage_masked(meta, show);
    match meta.flag.negate.filter(|_| show.negate) {
        Some(negate) => format!("{usage} / --{negate}"),
        None => usage,
    }
}

/// The width of the short column: `-x, `, or the blank that stands in for it.
///
/// Fixed, because a short form is one character. clap's, measured.
const SHORT_COL: usize = 4;

/// A flag as the *flags section* lists it, with its long form in a column of its own.
///
/// Separate from [`flag_usage`], which feeds the usage line — `Usage: ex [-f --force]` must
/// not be padded, and this must be. clap's shape, measured from clap 4:
///
/// ```text
///       --github-release
///   -n, --dry-run
///   -o, --output <OUTPUT>
///   -j <JOBS>
/// ```
///
/// Two rules in there worth stating. The short column is only spent where there is a long form
/// to line up *with*: a flag with no long one writes `-j <JOBS>` and does not pad, which is
/// what clap does. And a flag with neither — usage can name one the forms do not imply,
/// `verbose: -v`, which clap has no equivalent for — takes the same path as short-only.
fn column_usage(meta: &FlagMeta<'_>) -> String {
    column_usage_masked(meta, &Shown::all(meta))
}

fn column_usage_masked(meta: &FlagMeta<'_>, show: &Shown) -> String {
    let rest = display_usage_masked(meta, show);
    let Some(long) = show.long else {
        return rest;
    };
    // Only when the text actually begins with the long form. The `name:` prefix case does not,
    // and splitting it would put `verbose:` in a column meant for `-v, `.
    let Some(at) = rest.find(&format!("--{long}")) else {
        return rest;
    };
    let (before, after) = rest.split_at(at);
    let short = before.trim();
    // Only a bare short form belongs in the short column. A flag may carry a declared name the
    // forms do not imply — `jobs: -j --parallel` — and that prefix is not something to line up
    // with a comma after: it rendered `jobs: -j,--parallel`, losing the space entirely, because
    // the glued string is already wider than the column.
    let bare_short = short.is_empty()
        || (short.starts_with('-') && !short.starts_with("--") && short.chars().count() == 2);
    if !bare_short {
        return rest;
    }
    let short = match short {
        "" => String::new(),
        s => format!("{s},"),
    };
    format!("{short:<SHORT_COL$}{after}")
}

fn examples_section(out: &mut String, spec: &Spec<'_>, meta: &CommandMeta<'_>) {
    let examples = page_examples(spec, meta);
    if examples.is_empty() {
        return;
    }
    let _ = writeln!(out, "\nExamples:");
    for example in examples {
        if let Some(header) = example.header {
            let _ = writeln!(out, "  {header}:");
        }
        let _ = writeln!(out, "    $ {}", example.code);
    }
}

/// The examples a page shows: the command's own, or the spec's where it has none.
///
/// Top-level `example` nodes are the root's, and the reference shows them on every page whose
/// command declares none of its own — the same rule the text around a page follows, and for
/// the same reason: the top level is where a spec says something about the whole CLI.
fn page_examples<'a>(spec: &Spec<'a>, meta: &CommandMeta<'a>) -> &'a [Example<'a>] {
    if meta.examples.is_empty() {
        spec.root.examples
    } else {
        meta.examples
    }
}

/// The width help is wrapped to.
///
/// A fixed width wins over terminal detection and the maximum, as in clap. Zero means
/// unbounded for either setting. Without a declaration both implementations read `COLUMNS`
/// and fall back to 80.
fn terminal_width(meta: &CommandMeta<'_>) -> usize {
    if let Some(width) = meta.term_width {
        return if width == 0 { usize::MAX } else { width };
    }
    let detected = std::env::var("COLUMNS")
        .ok()
        .and_then(|s| s.parse().ok())
        .unwrap_or(80);
    match meta.max_term_width {
        Some(0) | None => detected,
        Some(max) => detected.min(max),
    }
}

/// Everything `--help` prints.
///
/// The same content as [`short_help`] through a wider layout: help is aligned into a column and
/// wrapped, the long form of each description is preferred over the short one, and the
/// annotations — choices, environment, default — each get their own line.
///
/// An entry whose help contains a line break is laid out as a block instead, its text indented
/// under the usage rather than beside it, because there is no column that keeps a line the
/// author already broke readable.
pub fn long_help(spec: &Spec<'_>, path: &[&str], chain: &[&CommandMeta<'_>]) -> String {
    let meta = *chain.last().expect("a page is always about some command");
    let (own, inherited) = own_and_global(chain);
    let own: Vec<_> = own
        .into_iter()
        .filter(|flag| !flag.hide_long_help)
        .collect();
    let inherited: Vec<_> = inherited
        .into_iter()
        .filter(|(flag, _)| !flag.hide_long_help)
        .collect();
    let width = terminal_width(meta);
    let mut out = String::new();

    if let Some(before) = meta
        .before_long_help
        .or(meta.before_help)
        .or(spec.root.before_long_help)
        .or(spec.root.before_help)
    {
        let _ = writeln!(out, "{before}\n");
    }

    // The banner and the program's own description belong to the program's page. A
    // subcommand's page describes the subcommand: `communique generate --help` said
    // "Editorialized release notes powered by AI" and never once said what `generate` does,
    // which is the question that was asked. clap prints the command's own description here.
    let root = path.len() <= 1;
    if root {
        if let Some(version) = spec.version {
            let name = if spec.name.is_empty() {
                spec.bin.unwrap_or_default()
            } else {
                spec.name
            };
            let _ = writeln!(out, "{name} {version}");
        }
    }
    let about = if root {
        spec.long_about.or(spec.about)
    } else {
        meta.long_about.or(meta.about)
    };
    if let Some(about) = about {
        // Trimmed for the same reason the entries below are: the blank line after the
        // description is written here, so one already in the text doubles it.
        let _ = writeln!(out, "{}\n", about.trim_end());
    }
    usage_section(&mut out, spec, path, meta);

    if !meta.flatten_help {
        long_commands_section(&mut out, &path[1.min(path.len())..], meta);
    }

    // One column width per section, over its visible entries — the same two the reference
    // computes, and separately, so a long flag does not push the arguments out.
    let args: Vec<&ArgMeta<'_>> = meta
        .args
        .iter()
        .filter(|a| !a.hide && !a.hide_long_help)
        .collect();
    let arg_col = args
        .iter()
        .map(|a| arg_usage(a).chars().count())
        .max()
        .unwrap_or(0);
    groups_section(
        &mut out,
        "Arguments",
        args.iter().copied(),
        |a| a.help_heading,
        |out, a| {
            let text = a.long_help.or(a.help);
            entry(
                out,
                &arg_usage(a),
                text,
                arg_col,
                width,
                meta.next_line_help,
            );
            long_annotations(
                out,
                if a.hide_possible_values {
                    &[]
                } else {
                    a.choices
                },
                if a.hide_env { None } else { a.env },
                if a.hide_default_value { &[] } else { a.default },
            );
        },
    );

    // One column over *both* lists, so the two sections read as one table with a rule through
    // it rather than two tables that happen to be adjacent.
    let flag_col = own
        .iter()
        .map(|f| column_usage(f).chars().count())
        .chain(inherited.iter().map(|(_, u)| u.chars().count()))
        .max()
        .unwrap_or(0);
    groups_section(
        &mut out,
        "Flags",
        own.iter().copied(),
        |f| f.help_heading,
        |out, f| {
            let text = f.long_help.or(f.help);
            entry(
                out,
                &column_usage(f),
                text,
                flag_col,
                width,
                meta.next_line_help,
            );
            long_annotations(
                out,
                if f.hide_possible_values {
                    &[]
                } else {
                    f.choices
                },
                if f.hide_env { None } else { f.env },
                if f.hide_default_value { &[] } else { f.default },
            );
        },
    );
    // After the command's own, and under a heading that says where they came from: `--config`
    // belongs to the program, not to this command, and a reader should be able to see that.
    // Not grouped by `help_heading` — an ancestor's headings describe that command's page, and
    // borrowing them here would put a section title on flags that are only visiting.
    groups_section(
        &mut out,
        "Global flags",
        inherited.iter(),
        |_| None,
        |out, (f, usage)| {
            let text = f.long_help.or(f.help);
            entry(out, usage, text, flag_col, width, meta.next_line_help);
            long_annotations(
                out,
                if f.hide_possible_values {
                    &[]
                } else {
                    f.choices
                },
                if f.hide_env { None } else { f.env },
                if f.hide_default_value { &[] } else { f.default },
            );
        },
    );
    if meta.flatten_help {
        flat_commands_long(&mut out, &path[1.min(path.len())..], meta, width);
    }

    let examples = page_examples(spec, meta);
    if !examples.is_empty() {
        let _ = writeln!(out, "\nExamples:");
        for example in examples {
            if let Some(header) = example.header {
                let _ = writeln!(out, "  {header}:");
            }
            // The description comes *before* the command, which is the order the reference
            // prints them in: it introduces the line rather than commenting on it.
            if let Some(help) = example.help {
                let _ = writeln!(out, "    {help}");
            }
            let _ = writeln!(out, "    $ {}", example.code);
        }
    }

    // mise puts an Examples section here on 115 commands, which is why a page without it is
    // missing the part a reader came for.
    if let Some(after) = meta
        .after_long_help
        .or(meta.after_help)
        .or(spec.root.after_long_help)
        .or(spec.root.after_help)
    {
        let _ = writeln!(out, "\n{after}");
    }

    let trimmed = out.trim();
    let mut done = String::with_capacity(trimmed.len() + 1);
    done.push_str(trimmed);
    done.push('\n');
    done
}

/// Write text with every line indented, leaving blank lines blank.
///
/// An indented empty line would be trailing whitespace, which the reference does not emit and
/// a diff would show as a line that is not empty.
fn write_indented(out: &mut String, text: &str, indent: usize) {
    let pad = " ".repeat(indent);
    for (i, line) in text.lines().enumerate() {
        // The first line is always indented, even when it is empty, and later blank lines are
        // left blank. That is not a choice: the reference writes the indent literally before the
        // text and indents the *rest* with a filter that skips blanks, so an opening empty line
        // comes out as whitespace and a later one does not.
        // `is_empty`, not `trim().is_empty()`: the reference's filter skips a line with nothing
        // on it and still indents one that holds only spaces, so emptying the latter would lose
        // whitespace the author wrote.
        if i == 0 || !line.is_empty() {
            let _ = writeln!(out, "{pad}{line}");
        } else {
            out.push('\n');
        }
    }
    // A text that ends with a break has a blank line at the end, and `lines()` does not report
    // it. The reference writes the text verbatim, so the blank is part of what it prints.
    if text.ends_with('\n') {
        out.push('\n');
    }
}

/// One entry: its usage, and its help either beside it or beneath it.
fn entry(
    out: &mut String,
    usage: &str,
    help: Option<&str>,
    col: usize,
    width: usize,
    next_line: bool,
) {
    let Some(help) = help.filter(|h| !h.trim().is_empty()) else {
        let _ = writeln!(out, "  {usage}");
        return;
    };

    // The column layout only works for text that has not been broken already, and only when
    // there is room left for it to say anything.
    let indent = 2 + col + 2;
    let room = width.saturating_sub(indent);
    if next_line || help.contains('\n') || room < 10 {
        let _ = writeln!(out, "  {usage}");
        write_indented(out, help, 4);
        return;
    }

    let lines = wrap(help, room);
    let _ = writeln!(out, "  {usage:<col$}  {}", lines[0]);
    for line in &lines[1..] {
        let _ = writeln!(out, "{}{line}", " ".repeat(indent));
    }
    // No blank line after a wrapped entry. The reference's template asks for one, and its
    // whitespace trimming eats it before it reaches the output — so a wrapped entry is followed
    // directly by the next, and matching means matching that.
}

/// Break text at word boundaries to fit a width, keeping any breaks it already has.
fn wrap(text: &str, width: usize) -> Vec<String> {
    let mut lines = Vec::new();
    for paragraph in text.split('\n') {
        if paragraph.is_empty() {
            lines.push(String::new());
            continue;
        }
        let mut line = String::new();
        for word in paragraph.split_whitespace() {
            let word_width = word.chars().count();
            if !line.is_empty() && line.chars().count() + 1 + word_width > width {
                lines.push(std::mem::take(&mut line));
            }
            if !line.is_empty() {
                line.push(' ');
            }
            line.push_str(word);
        }
        if !line.is_empty() {
            lines.push(line);
        }
    }
    if lines.is_empty() {
        lines.push(String::new());
    }
    lines
}

/// The annotations, each on its own line as the wider layout puts them.
fn long_annotations(out: &mut String, choices: &[&str], env: Option<&str>, default: &[&str]) {
    if !choices.is_empty() {
        let _ = writeln!(out, "    [possible values: {}]", choices.join(", "));
    }
    if let Some(env) = env {
        let _ = writeln!(out, "    [env: {env}]");
    }
    if !default.is_empty() {
        let _ = writeln!(out, "    (default: {})", default.join(", "));
    }
}

/// The commands list, with each command's help beneath its usage.
fn long_commands_section(out: &mut String, path: &[&str], meta: &CommandMeta<'_>) {
    let visible: Vec<&&CommandMeta<'_>> = meta.subcommands.iter().filter(|c| !c.hide).collect();
    if visible.is_empty() {
        return;
    }
    let heading = meta.subcommand_help_heading.unwrap_or("Commands");
    let _ = writeln!(out, "\n{heading}:");

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
        out.push('\n');
        if let Some(about) = sub.long_about.or(sub.about) {
            // Trailing whitespace trimmed: the blank line after each entry is written below, and
            // a description that happens to end in a newline — which clap's `long_about` often
            // does, reaching the spec verbatim — added a second one and left a stray blank in
            // the middle of the list.
            write_indented(out, about.trim_end(), 4);
        }
        // A blank line between entries, which the wider layout can afford and which keeps a
        // multi-line description from running into the next command's name.
        out.push('\n');
    }
    let _ = writeln!(
        out,
        "  help\n    Print this message or the help of the given subcommand(s)"
    );
}

fn flat_commands_long(out: &mut String, path: &[&str], meta: &CommandMeta<'_>, width: usize) {
    let mut visible: Vec<_> = meta.subcommands.iter().filter(|sub| !sub.hide).collect();
    visible.sort_by_key(|sub| sub.cmd.name);
    for sub in visible {
        let mut sub_path = path.to_vec();
        sub_path.push(sub.cmd.name);
        let _ = writeln!(out, "\n{}:", sub_path.join(" "));
        if let Some(about) = sub
            .long_about
            .or(sub.about)
            .filter(|about| !about.trim().is_empty())
        {
            let _ = writeln!(out, "{}", about.trim_end());
        }

        let args: Vec<_> = sub
            .args
            .iter()
            .filter(|arg| !arg.hide && !arg.hide_long_help)
            .collect();
        let flags: Vec<&FlagMeta<'_>> = sub
            .flags
            .iter()
            .filter(|flag| !flag.flag.global && !flag.hide && !flag.hide_long_help)
            .collect();
        let col = args
            .iter()
            .map(|arg| arg_usage(arg).chars().count())
            .chain(flags.iter().map(|flag| column_usage(flag).chars().count()))
            .max()
            .unwrap_or(0);
        for arg in args {
            entry(
                out,
                &arg_usage(arg),
                arg.long_help.or(arg.help),
                col,
                width,
                meta.next_line_help,
            );
            long_annotations(
                out,
                if arg.hide_possible_values {
                    &[]
                } else {
                    arg.choices
                },
                if arg.hide_env { None } else { arg.env },
                if arg.hide_default_value {
                    &[]
                } else {
                    arg.default
                },
            );
        }
        for flag in flags {
            entry(
                out,
                &column_usage(flag),
                flag.long_help.or(flag.help),
                col,
                width,
                meta.next_line_help,
            );
            long_annotations(
                out,
                if flag.hide_possible_values {
                    &[]
                } else {
                    flag.choices
                },
                if flag.hide_env { None } else { flag.env },
                if flag.hide_default_value {
                    &[]
                } else {
                    flag.default
                },
            );
        }
        if sub.flatten_help {
            flat_commands_long(out, &sub_path, sub, width);
        }
        out.push('\n');
    }
}

/// The path and metadata for a command, found by identity within a spec.
///
/// [`Error::Help`](crate::Error::Help) carries the `Command` the request was about, because the
/// parse tables are what a parse walks and the metadata is behind a feature. Rendering needs the
/// metadata and the path a user typed to reach it, and both are in the tree — so this walks it,
/// comparing addresses rather than names, which two commands can share.
///
/// `None` when the command is not in this spec, which means the two came from different CLIs.
pub fn find<'a>(
    spec: &'a Spec<'a>,
    cmd: &Command<'_>,
) -> Option<(Vec<&'a str>, Vec<&'a CommandMeta<'a>>)> {
    fn walk<'a>(
        path: &mut Vec<&'a str>,
        chain: &mut Vec<&'a CommandMeta<'a>>,
        meta: &'a CommandMeta<'a>,
        cmd: &Command<'_>,
    ) -> bool {
        chain.push(meta);
        if core::ptr::eq(meta.cmd, cmd) {
            return true;
        }
        for sub in meta.subcommands {
            path.push(sub.cmd.name);
            if walk(path, chain, sub, cmd) {
                return true;
            }
            path.pop();
        }
        chain.pop();
        false
    }

    let mut path = vec![spec.bin.unwrap_or(spec.name)];
    let mut chain = Vec::new();
    walk(&mut path, &mut chain, spec.root, cmd).then_some((path, chain))
}

/// The entries for `--help` and `--version`, which the parser supplies and no spec declares.
///
/// Listed because help is written for people: a reader looking for how to get help should find
/// it on the page. This reverses the rule these two used to follow — that a page lists exactly
/// what its spec declares — and the reason is that the spec has its own readers, and they are
/// not the ones reading this.
///
/// Four spellings each, because a CLI may have claimed either form for itself. The parser
/// yields to a declaration (`in_scope` looks a command's own flags up first), so a page that
/// claimed otherwise would be describing a flag that never binds.
mod supplied {
    use crate::spec::FlagMeta;
    use crate::Flag;

    macro_rules! entry {
        ($name:ident, $flag:ident, $key:expr, $label:expr, $longs:expr, $shorts:expr, $help:expr) => {
            static $flag: Flag<'static> = Flag {
                key: $key,
                name: $label,
                longs: $longs,
                shorts: $shorts,
                ..Flag::BOOL
            };
            pub static $name: FlagMeta<'static> = FlagMeta {
                flag: &$flag,
                help: Some($help),
                ..FlagMeta::EMPTY
            };
        };
    }

    entry!(
        HELP_BOTH,
        HB,
        crate::HELP_LONG_KEY,
        "help",
        &["help"],
        b"h",
        "Print help"
    );
    entry!(
        HELP_LONG_ONLY,
        HL,
        crate::HELP_LONG_KEY,
        "help",
        &["help"],
        b"",
        "Print help"
    );
    // Named `h`, not `help`: the declared name is judged against the forms the entry shows,
    // and a short-only entry called `help` reads as a renamed flag — it printed `help: -h`.
    entry!(
        HELP_SHORT_ONLY,
        HS,
        crate::HELP_SHORT_KEY,
        "h",
        &[],
        b"h",
        "Print help"
    );
    entry!(
        VERSION_BOTH,
        VB,
        crate::VERSION_LONG_KEY,
        "version",
        &["version"],
        b"V",
        "Print version"
    );
    entry!(
        VERSION_LONG_ONLY,
        VL,
        crate::VERSION_LONG_KEY,
        "version",
        &["version"],
        b"",
        "Print version"
    );
    entry!(
        VERSION_SHORT_ONLY,
        VS,
        crate::VERSION_SHORT_KEY,
        "V",
        &[],
        b"V",
        "Print version"
    );
}

/// The supplied entries a page should list, given what the command already claims.
///
/// `--version` only where the parser actually accepts it: on a command whose table says so,
/// which the derive sets on the root when a version is declared. A page offering one that the
/// parser would refuse is worse than a page that stays quiet.
fn supplied_entries(cmd: &Command<'_>, taken: &[String]) -> Vec<&'static FlagMeta<'static>> {
    // Against the same set every other decision on this page uses, so a spelling claimed by a
    // hidden declaration or by a negation is claimed here too. Offering a `--help` that
    // something else binds is exactly the lie the model exists to prevent.
    let pick = |long: &str, short: char, both, l, s| match (
        taken.contains(&format!("--{long}")),
        taken.contains(&format!("-{short}")),
    ) {
        (true, true) => None,
        (true, false) => Some(s),
        (false, true) => Some(l),
        (false, false) => Some(both),
    };

    let mut out = Vec::new();
    out.extend(pick(
        "help",
        'h',
        &supplied::HELP_BOTH,
        &supplied::HELP_LONG_ONLY,
        &supplied::HELP_SHORT_ONLY,
    ));
    // Only where the parser accepts one, which is the root of a CLI that declared a version.
    if cmd.version {
        out.extend(pick(
            "version",
            'V',
            &supplied::VERSION_BOTH,
            &supplied::VERSION_LONG_ONLY,
            &supplied::VERSION_SHORT_ONLY,
        ));
    }
    out
}

/// Every flag a page should list, split into the command's own and the ones it inherits.
///
/// The rule the parser follows on the way down, and the same one the diagnostics suggest
/// from: a command's own flags, and from each ancestor only what it declared `global`.
///
/// Inherited flags were listed nowhere. `communique generate` accepts `--config`, `--verbose`
/// and `--quiet` from its root, and its page mentioned none of them — a flag a user can type
/// and cannot discover, which is the worst way for help to be wrong.
fn own_and_global<'a>(
    chain: &[&'a CommandMeta<'a>],
) -> (Vec<&'a FlagMeta<'a>>, Vec<(&'a FlagMeta<'a>, String)>) {
    let Some((here, ancestors)) = chain.split_last() else {
        return (Vec::new(), Vec::new());
    };
    let own: Vec<&FlagMeta<'_>> = here.flags.iter().filter(|f| !f.hide).collect();

    // Which spellings are already spoken for at this command, and by whom.
    //
    // The parser's rule, exactly: `in_scope` chains a command's own flags before its
    // ancestors' — nearest first — and takes the first match. So a page offers a spelling only
    // where the flag it is describing is the one that would bind it.
    //
    // Three things this counts that an earlier version did not. **Hidden flags**, which `hide`
    // keeps off the page while the parser still binds them — on the command *and* on an
    // ancestor, or a farther global gets advertised while a nearer hidden one answers.
    // **Negations**, which are spellings like any other and can be claimed. And **every** long
    // and short a flag answers to rather than only its first: a descendant taking `--jobs`
    // leaves an inherited `--workers` working, and it should still be findable.
    // Two sets, because the parser has two passes. `long_flag` asks `find_long` over the whole
    // scope before it asks `find_negation`, so *any* long beats *any* negation — a nearer
    // command's `--cache` negation does not take the spelling from a farther command's `--cache`
    // long, and reading them as one set said it did.
    fn forms<'f>(f: &'f FlagMeta<'_>) -> impl Iterator<Item = String> + 'f {
        f.flag
            .longs
            .iter()
            .map(|l| format!("--{l}"))
            .chain(f.flag.shorts.iter().map(|s| format!("-{}", *s as char)))
    }
    fn negation(f: &FlagMeta<'_>) -> Option<String> {
        f.flag.negate.map(|n| format!("--{n}"))
    }

    // Every long and short anything in scope answers to, near or far: one of these always
    // beats a negation, so a negation survives only where none of them is the same word.
    let every_form: Vec<String> = here
        .flags
        .iter()
        .chain(
            ancestors
                .iter()
                .flat_map(|m| m.flags.iter())
                .filter(|f| f.flag.global),
        )
        .flat_map(forms)
        .collect();

    let mut taken: Vec<String> = here.flags.iter().flat_map(forms).collect();
    let mut taken_negations: Vec<String> = here.flags.iter().filter_map(negation).collect();
    let mut keep: Vec<(*const FlagMeta<'_>, Shown<'_>)> = Vec::new();
    for meta in ancestors.iter().rev() {
        for f in meta.flags.iter().filter(|f| f.flag.global) {
            let show = Shown::surviving(f, &taken, &taken_negations, &every_form);
            // Reserved whether or not it is shown: a hidden one still binds, and so does one
            // whose every spelling something nearer already took.
            taken.extend(forms(f));
            taken_negations.extend(negation(f));
            if f.hide || show.nothing() {
                continue;
            }
            keep.push((f as *const _, show));
        }
    }
    let inherited: Vec<(&FlagMeta<'_>, String)> = ancestors
        .iter()
        .flat_map(|meta| meta.flags.iter())
        .filter_map(|f| {
            keep.iter()
                .find(|(p, _)| core::ptr::eq(*p, f as *const _))
                .map(|(_, show)| (f, column_usage_masked(f, show)))
        })
        .collect();

    // Last in the command's own section, which is where clap has them: they carry no
    // `help_heading`, so a CLI that groups its flags gets them at the end of the ungrouped
    // list rather than inside somebody's section.
    //
    // Given `taken` rather than the two lists: that set already counts hidden declarations and
    // negations, and a `--help` the page offers while something else binds it is exactly the
    // lie this whole model exists to prevent.
    let mut own = own;
    // Forms *and* negations: `long_flag` asks `find_negation` before it offers `--version`,
    // so a declared negation beats a supplied flag even though it loses to a long.
    let claimed: Vec<String> = taken
        .iter()
        .cloned()
        .chain(taken_negations.iter().cloned())
        .collect();
    own.extend(supplied_entries(here.cmd, &claimed));
    (own, inherited)
}

/// The page a help request asks for, ready to print.
///
/// The two forms differ as clap has them: `-h` is the short one and `--help` the long one.
pub fn render(spec: &Spec<'_>, cmd: &Command<'_>, long: bool) -> Option<String> {
    let (path, chain) = find(spec, cmd)?;
    Some(if long {
        long_help(spec, &path, &chain)
    } else {
        short_help(spec, &path, &chain)
    })
}

/// Render a help page with an explicit colour policy.
pub fn render_styled(
    spec: &Spec<'_>,
    cmd: &Command<'_>,
    long: bool,
    style: Style,
) -> Option<String> {
    let (path, chain) = find(spec, cmd)?;
    let page = if long {
        long_help(spec, &path, &chain)
    } else {
        short_help(spec, &path, &chain)
    };
    let (headings, flag_usages, synopsis) = help_structure(spec, &path, &chain, long);
    Some(styled_help(
        &page,
        style,
        &headings,
        &flag_usages,
        &synopsis,
    ))
}

/// The route the words took to a command, for rendering its page unambiguously.
///
/// Rebuilt by re-parsing, because [`Error::Help`](crate::Error::Help) carries the command and
/// not the way there — putting a route in it would put an allocation in every parser error.
/// The parse is deterministic, so walking the same argv reaches the same place.
///
/// `ex help config set` asks about a command *deeper* than the parse reached, so the route is
/// extended over [`Parser::help_span`](crate::Parser::help_span) — the words the parser itself
/// resolved as a command path, which is the only reading that cannot mistake a flag's value for
/// a command name.
///
/// `None` where the command is not below this spec at all, which a caller should treat as a
/// reason to fall back rather than a failure.
pub fn route_to<'t>(
    root: &'t Command<'t>,
    argv: &[&std::ffi::OsStr],
    cmd: &Command<'_>,
) -> Option<Vec<&'t Command<'t>>> {
    let mut parser = crate::Parser::new(root, argv);
    while let Some(event) = parser.next_event() {
        if event.is_err() {
            break;
        }
    }
    let (help_from, help_to) = parser.help_span();
    let mut route: Vec<&Command<'_>> = parser.command_path().into_iter().map(|(c, _)| c).collect();
    if route.is_empty() {
        route.push(root);
    }

    // Already there for `--help`, whose span is empty. For the `help` word the parse stopped at
    // the command that *saw* it, and the words naming the one being asked about are exactly the
    // span — which the parser resolved itself, one subcommand at a time.
    //
    // Taken from the parser rather than re-scanned out of `argv`, because only the parser knows
    // which tokens were in command position. Scanning every token from where the parse stopped
    // read `ex --config alpha help beta shared` as a descent into `alpha`, since a flag's
    // detached value is just a word — and the wrong mount's page passed the arrival check
    // below, both mounts being one address.
    //
    // By name and not by address for the same reason: looking for a child that *contains* the
    // target picks whichever mount comes first, which is the bug this function exists for.
    for token in argv.get(help_from..help_to).unwrap_or_default() {
        let here = *route.last()?;
        let word = token.as_encoded_bytes();
        // Through `find_named`, so this walk ranks names above aliases exactly as the parse
        // that reached here did. Matching on name and alias together instead answered with
        // whichever subcommand came first, which for a colliding word is a different command
        // than the one the parser selected.
        let next = crate::find_named(here, word)?;
        route.push(next);
    }
    // Only if the walk actually arrived: a caller should fall back rather than be handed a
    // page about some other command.
    core::ptr::eq(*route.last()?, cmd).then_some(route)
}

/// The same page, for a command reached by a known route.
///
/// [`render`] has only a `&Command` to go on and finds it by address. That is enough until one
/// `Subcommands` type is mounted under two parents: both splice the same `&'static [Command]`,
/// so the two mounts *are* one address and the search returns whichever comes first. A page for
/// the second one then carried the first one's path and the first one's globals.
///
/// The route tells them apart, and the parser has it — `Parser::command_path` is the sequence of
/// commands the words actually went through. Callers holding only a command keep [`render`] and
/// its answer; callers that parsed something should prefer this.
fn route_context<'a>(
    spec: &'a Spec<'a>,
    route: &[&Command<'_>],
) -> Option<(Vec<&'a str>, Vec<&'a CommandMeta<'a>>)> {
    let mut names = vec![spec.bin.unwrap_or(spec.name)];
    let mut chain = vec![spec.root];
    for cmd in route.iter().skip(1) {
        // Matched among *this* command's children, which is unambiguous even when the child is
        // shared: a parent's own list is its own.
        let here = chain.last()?;
        let next = here
            .subcommands
            .iter()
            .find(|sub| core::ptr::eq(sub.cmd, *cmd))?;
        names.push(next.cmd.name);
        chain.push(next);
    }
    Some((names, chain))
}

pub fn render_at(spec: &Spec<'_>, route: &[&Command<'_>], long: bool) -> Option<String> {
    let (names, chain) = route_context(spec, route)?;
    Some(if long {
        long_help(spec, &names, &chain)
    } else {
        short_help(spec, &names, &chain)
    })
}

/// Render a route-specific help page with an explicit colour policy.
pub fn render_at_styled(
    spec: &Spec<'_>,
    route: &[&Command<'_>],
    long: bool,
    style: Style,
) -> Option<String> {
    let (path, chain) = route_context(spec, route)?;
    let page = if long {
        long_help(spec, &path, &chain)
    } else {
        short_help(spec, &path, &chain)
    };
    let (headings, flag_usages, synopsis) = help_structure(spec, &path, &chain, long);
    Some(styled_help(
        &page,
        style,
        &headings,
        &flag_usages,
        &synopsis,
    ))
}

#[cfg(test)]
mod style_tests {
    use super::{commands_section, styled_help, Style};
    use crate::spec::CommandMeta;
    use crate::Command;

    #[test]
    fn short_command_rows_trim_trailing_help_whitespace() {
        let sub_cmd = Command {
            name: "run",
            ..Command::EMPTY
        };
        let sub_meta = CommandMeta {
            cmd: &sub_cmd,
            about: Some("run it\n"),
            ..CommandMeta::EMPTY
        };
        let subcommands = [&sub_meta];
        let root_meta = CommandMeta {
            subcommands: &subcommands,
            ..CommandMeta::EMPTY
        };
        let mut page = String::new();

        commands_section(&mut page, &[], &root_meta);

        assert!(page.contains("  run  run it\n  help"));
        assert!(!page.contains("  run  run it\n\n  help"));
    }

    #[test]
    fn coloured_help_styles_structure_without_changing_plain_text() {
        let page = "A summary ending in:\nUsage: prose is not a synopsis\nExamples:\n\nUsage: ex [OPTIONS]\n       ex --all\n\nOptions:\n  -f, --force  Force it\n    [possible values: --auto]\n    (default: -1)\n";
        let headings = vec!["Options".to_string()];
        let usages = vec!["-f, --force".to_string()];
        let synopsis = vec![
            "Usage: ex [OPTIONS]".to_string(),
            "       ex --all".to_string(),
        ];
        assert_eq!(
            styled_help(page, Style::PLAIN, &headings, &usages, &synopsis),
            page
        );

        let coloured = styled_help(page, Style::COLOURED, &headings, &usages, &synopsis);
        assert!(coloured.contains("\u{1b}[1;4;32mUsage:\u{1b}[0m"));
        assert!(coloured.contains("\u{1b}[1;4;32mOptions:\u{1b}[0m"));
        assert!(coloured.contains("\u{1b}[36m-f\u{1b}[0m"));
        assert!(coloured.contains("\u{1b}[36m--force\u{1b}[0m"));
        assert!(coloured.contains("A summary ending in:\nUsage: prose is not a synopsis"));
        assert!(coloured.contains("Usage: prose is not a synopsis\nExamples:"));
        assert!(coloured.contains("\u{1b}[36m       ex --all\u{1b}[0m"));
        assert!(coloured.contains("[possible values: --auto]"));
        assert!(coloured.contains("(default: -1)"));
        assert_eq!(strip_ansi(&coloured), page);
    }

    fn strip_ansi(text: &str) -> String {
        let mut out = String::new();
        let mut chars = text.chars().peekable();
        while let Some(ch) = chars.next() {
            if ch == '\u{1b}' && chars.peek() == Some(&'[') {
                chars.next();
                for code in chars.by_ref() {
                    if code == 'm' {
                        break;
                    }
                }
            } else {
                out.push(ch);
            }
        }
        out
    }
}
