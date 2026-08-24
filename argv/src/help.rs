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
use std::borrow::Cow;

use crate::spec::{
    AdmonitionKind, AdmonitionMeta, ArgMeta, CommandMeta, Example, FlagMeta, Spec, ViewMeta,
};
use crate::Command;
use crate::DoubleDash;

/// The indent a page uses where it cannot align to its column.
const BLOCK_INDENT: usize = 4;

/// How many flags or arguments are listed individually before collapsing to a placeholder.
///
/// usage-lib's number. Beyond it the line would be longer than it is useful, so it becomes
/// `[FLAGS]` or `[ARGS]…` and the sections below carry the detail.
const INLINE_LIMIT: usize = 2;

/// The sections a [`help_template`](crate::spec::Spec::help_template) may name.
///
/// A closed vocabulary on purpose. The alternative — handing a template the metadata tree and
/// letting it lay a page out — makes this renderer's internals public API and asks every
/// implementation of the spec to agree on a template language's semantics rather than on where
/// a section starts and ends.
///
/// What each one holds:
///
/// | section        | content                                                              |
/// | -------------- | -------------------------------------------------------------------- |
/// | `about`        | `before_help`, the version banner, and the description               |
/// | `usage`        | the `Usage:` synopsis, however many lines it takes                   |
/// | `commands`     | the subcommand list, or the flattened bodies under `flatten_help`     |
/// | `args`         | every argument group, each under its heading                          |
/// | `flags`        | this command's flag groups, then the globals it inherits             |
/// | `grouped_args` | arguments with a declared help heading                               |
/// | `ungrouped_args` | arguments under the default `Arguments` heading                    |
/// | `grouped_flags` | flags with a declared help heading                                  |
/// | `ungrouped_flags` | flags under `Flags`, plus inherited global flags                   |
/// | `after_help`   | examples, `after_help`, and the author/license footer on a long page  |
pub const SECTIONS: [&str; 10] = [
    "about",
    "usage",
    "commands",
    "args",
    "flags",
    "grouped_args",
    "ungrouped_args",
    "grouped_flags",
    "ungrouped_flags",
    "after_help",
];

/// The first placeholder in a template that names no section, if there is one.
///
/// The check a spec is held to wherever one is written down: KDL refuses a template at parse
/// and the derive refuses one at compile time, so a page is never rendered from a template
/// whose sections cannot all be filled. `Err` reports an opening `{{` with no `}}` after it,
/// which is a typo rather than a section name.
///
/// ```
/// use usage_argv::help::unsupported_section;
///
/// assert_eq!(unsupported_section("{{about}}{{usage}}"), Ok(None));
/// assert_eq!(unsupported_section("{{ options }}"), Ok(Some("options")));
/// assert!(unsupported_section("{{usage").is_err());
/// ```
pub fn unsupported_section(template: &str) -> Result<Option<&str>, &'static str> {
    let mut rest = template;
    while let Some(at) = rest.find("{{") {
        let after = &rest[at + 2..];
        let Some(end) = after.find("}}") else {
            return Err("a `{{` with no `}}` after it");
        };
        let name = after[..end].trim();
        if !SECTIONS.contains(&name) {
            return Ok(Some(name));
        }
        rest = &after[end + 2..];
    }
    Ok(None)
}

/// The pieces of a page, before anything decides what order they go in.
///
/// Built in one pass and assembled twice over: concatenated in the default order, which is the
/// page every CLI without a template gets and is what the fleet gate compares byte for byte, or
/// substituted into a template. `flattened` is not a section an author can name — it is the
/// other half of `commands`, and only one of the two is ever non-empty.
#[derive(Default)]
struct Sections {
    about: String,
    usage: String,
    commands: String,
    args: String,
    flags: String,
    grouped_args: String,
    ungrouped_args: String,
    grouped_flags: String,
    ungrouped_flags: String,
    flattened: String,
    after_help: String,
}

impl Sections {
    /// The default page: every section in the order the renderer wrote them.
    ///
    /// A plain concatenation, so this is the same string the renderer produced before sections
    /// were separable — the separating blank lines belong to the sections themselves.
    fn concatenated(&self) -> String {
        let mut out = String::new();
        for part in [
            &self.about,
            &self.usage,
            &self.commands,
            &self.args,
            &self.flags,
            &self.flattened,
            &self.after_help,
        ] {
            out.push_str(part);
        }
        out
    }

    fn named(&self, name: &str) -> Option<String> {
        Some(match name {
            "about" => self.about.trim().to_string(),
            "usage" => self.usage.trim().to_string(),
            // Whichever form this command's command list took. `flatten_help` replaces the
            // list with the subcommands' own bodies, so a template that places `{{commands}}`
            // places whichever one the command has.
            "commands" => {
                let mut out = self.commands.trim().to_string();
                let flattened = self.flattened.trim();
                if !flattened.is_empty() {
                    if !out.is_empty() {
                        out.push_str("\n\n");
                    }
                    out.push_str(flattened);
                }
                out
            }
            "args" => self.args.trim().to_string(),
            "flags" => self.flags.trim().to_string(),
            "grouped_args" => self.grouped_args.trim().to_string(),
            "ungrouped_args" => self.ungrouped_args.trim().to_string(),
            "grouped_flags" => self.grouped_flags.trim().to_string(),
            "ungrouped_flags" => self.ungrouped_flags.trim().to_string(),
            "after_help" => self.after_help.trim().to_string(),
            _ => return None,
        })
    }

    /// A page laid out by an author's template.
    ///
    /// Each section arrives trimmed, so the template owns the whitespace between them: a
    /// template is a layout, and a section carrying the blank line above it could not be moved
    /// without carrying that decision along. A placeholder naming no section is left as it was
    /// written — the vocabulary is checked where a spec is authored, so one reaching here is
    /// text an author meant literally.
    ///
    /// A section that came out empty leaves no gap behind, which is what lets one template
    /// serve a whole CLI: see `usage::help_template::collapse_blank_runs`, whose rule this is.
    fn substituted(&self, template: &str) -> String {
        let mut out = String::with_capacity(template.len());
        let mut rest = template;
        while let Some(at) = rest.find("{{") {
            out.push_str(&rest[..at]);
            let after = &rest[at + 2..];
            let Some(end) = after.find("}}") else {
                out.push_str(&rest[at..]);
                return collapse_blank_runs(&out);
            };
            match self.named(after[..end].trim()) {
                Some(text) => out.push_str(&text),
                None => out.push_str(&rest[at..at + 2 + end + 2]),
            }
            rest = &after[end + 2..];
        }
        out.push_str(rest);
        collapse_blank_runs(&out)
    }
}

/// A page's runs of blank lines, each reduced to a single blank line.
///
/// The twin of `usage::help_template::collapse_blank_runs`, and the reason a template can name a
/// section a given command does not have. A whitespace-only line counts as blank, since that is
/// what an empty placeholder on an indented line leaves; a section's own indentation does not,
/// since that is the page.
fn collapse_blank_runs(page: &str) -> String {
    let mut out = String::with_capacity(page.len());
    let mut blank = false;
    for line in page.split('\n') {
        if line.trim().is_empty() {
            blank = !out.is_empty();
            continue;
        }
        if !out.is_empty() {
            out.push('\n');
            if blank {
                out.push('\n');
            }
        }
        blank = false;
        out.push_str(line);
    }
    out
}

/// The finished page: laid out by the spec's template where it has one, and trimmed.
///
/// usage-lib trims the whole document and puts back one newline, which is what keeps the blank
/// lines between sections from becoming trailing ones. That applies to a template's output too:
/// a page ends in exactly one newline however it was assembled.
fn assemble(spec: &Spec<'_>, sections: &Sections) -> String {
    let page = match spec
        .help_template
        .filter(|template| !template.trim().is_empty())
    {
        Some(template) => sections.substituted(template),
        None => sections.concatenated(),
    };
    let trimmed = page.trim();
    let mut done = String::with_capacity(trimmed.len() + 1);
    done.push_str(trimmed);
    done.push('\n');
    done
}

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
        Self::auto_for(std::io::stdout().is_terminal())
    }

    /// Colour when stderr is a terminal and the environment permits it.
    pub fn auto_stderr() -> Style {
        use std::io::IsTerminal as _;
        Self::auto_for(std::io::stderr().is_terminal())
    }

    fn auto_for(is_terminal: bool) -> Style {
        let forced = std::env::var_os("CLICOLOR_FORCE").is_some_and(|v| v != "0");
        let refused = std::env::var_os("NO_COLOR").is_some_and(|v| !v.is_empty());
        if refused {
            Style::PLAIN
        } else if forced || is_terminal {
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

    /// Render the small Markdown vocabulary accepted in help prose.
    ///
    /// Plain output deliberately keeps the source spelling: generated artifacts and pipes
    /// remain byte-for-byte compatible, while a terminal can turn the same familiar syntax
    /// into presentation. This is inline prose, not a Markdown document — lists, headings and
    /// links belong to the help page's existing structure.
    fn inline(self, text: &str) -> String {
        if !self.coloured {
            return text.to_string();
        }
        styled_inline(text, None)
    }
}

/// Markdown-like emphasis in author-written help.
///
/// Kept deliberately small and dependency-free: help is cold-path code, but the renderer is a
/// foundational crate and pulling a document parser into every adopter for four inline spans
/// would be disproportionate. Delimiters must close on the same line; an unmatched delimiter
/// is ordinary text.
fn styled_inline(text: &str, parent: Option<&str>) -> String {
    let mut out = String::with_capacity(text.len());
    let mut at = 0;
    let mut allow_run_remainder = false;
    while at < text.len() {
        let rest = &text[at..];

        // Markdown escapes are useful in prose that genuinely means `*`, `_`, `~`, or `` ` ``.
        if let Some(escaped) = rest
            .strip_prefix('\\')
            .and_then(|after| after.chars().next())
        {
            if matches!(escaped, '*' | '_' | '~' | '`' | '\\') {
                out.push(escaped);
                at += 1 + escaped.len_utf8();
                allow_run_remainder = false;
                continue;
            }
        }

        let span = [
            ("***", "1;3", "22;23", false, true),
            ("___", "1;3", "22;23", true, true),
            ("**", "1", "22", false, true),
            ("__", "1", "22", true, true),
            ("~~", "9", "29", false, true),
            ("*", "3", "23", false, true),
            ("_", "3", "23", true, true),
            ("`", "36", "39", false, false),
        ]
        .into_iter()
        .find_map(|(delimiter, open, close, word_boundary, recurse)| {
            rest.strip_prefix(delimiter)?;
            let marker = delimiter.chars().next().expect("a delimiter has a marker");
            let previous = text[..at].chars().next_back();
            // Do not reinterpret part of a delimiter run when the longer form was rejected.
            // In particular, neither underscore in `word__word__` may become an italic opener.
            if (previous == Some(marker) && !allow_run_remainder)
                || (delimiter.len() == 1 && rest[delimiter.len()..].starts_with(marker))
            {
                return None;
            }
            if word_boundary && previous.is_some_and(char::is_alphanumeric) {
                return None;
            }
            let content_start = at + delimiter.len();
            let (end, after) = closing_delimiter(text, content_start, delimiter, word_boundary, 0)?;
            Some((delimiter, open, close, recurse, content_start, end, after))
        });

        if let Some((delimiter, open, close, recurse, content_start, end, after)) = span {
            out.push_str("\u{1b}[");
            out.push_str(open);
            out.push('m');
            if recurse {
                out.push_str(&styled_inline(&text[content_start..end], Some(open)));
            } else {
                out.push_str(&text[content_start..end]);
            }
            out.push_str("\u{1b}[");
            out.push_str(close);
            out.push('m');
            if let Some(parent) = parent {
                out.push_str("\u{1b}[");
                out.push_str(parent);
                out.push('m');
            }
            let marker = delimiter.chars().next().expect("a delimiter has a marker");
            allow_run_remainder =
                text[after..].starts_with(marker) && text[..after].ends_with(marker);
            at = after;
            continue;
        }

        let ch = rest.chars().next().expect("at is on a character boundary");
        out.push(ch);
        at += ch.len_utf8();
        allow_run_remainder = false;
    }
    out
}

/// Find a span's close while stepping over valid spans of another width.
///
/// Italic, bold, and combined spans may nest within one another. Combined closing runs such as
/// the final `***` in `*italic and **bold***` are shared: two markers close bold and the last
/// closes italic. `reserve` tells a nested search how many markers its parent still needs from
/// such a run.
fn closing_delimiter(
    text: &str,
    content_start: usize,
    delimiter: &str,
    word_boundary: bool,
    reserve: usize,
) -> Option<(usize, usize)> {
    let marker = delimiter.chars().next()?;
    let width = delimiter.len();
    let mut search_at = content_start;

    while let Some(found) = text[search_at..].find(marker) {
        let run_start = search_at + found;
        let run_len = text[run_start..]
            .chars()
            .take_while(|ch| *ch == marker)
            .count();
        let run_end = run_start + run_len;
        let escaped = text[..run_start]
            .chars()
            .rev()
            .take_while(|ch| *ch == '\\')
            .count()
            % 2
            == 1;
        if escaped {
            // A backslash escapes one marker, not its whole run. The rest may still close the
            // span: `*italic \**` is an escaped literal star followed by the italic close.
            search_at = run_start + marker.len_utf8();
            continue;
        }

        let nested_width = match (run_len, marker) {
            (1..=3, '*' | '_') if run_len != width => run_len,
            _ => 0,
        };
        if nested_width != 0 {
            let nested = &text[run_start..run_start + nested_width];
            if let Some((_, after)) =
                closing_delimiter(text, run_start + nested_width, nested, marker == '_', width)
            {
                search_at = after;
                continue;
            }
        }

        if run_len >= width {
            let after = run_start + width;
            let left_in_run = run_end - after;
            let leaves_parent_close = left_in_run == 0 || left_in_run >= reserve;
            let boundary_ok = !word_boundary
                || !text[after..]
                    .chars()
                    .next()
                    .is_some_and(char::is_alphanumeric);
            if run_start > content_start
                && !text[content_start..run_start].trim().is_empty()
                && leaves_parent_close
                && boundary_ok
            {
                return Some((run_start, after));
            }
        }
        search_at = run_end;
    }
    None
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
            .find_map(|(i, c)| {
                (c.is_whitespace() || matches!(c, ',' | '=' | '[' | ']' | '>')).then_some(i)
            })
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
    inherit_version_actions: bool,
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
        headings.extend(
            meta.subcommands
                .iter()
                .filter(|sub| !sub.hide)
                .filter_map(|sub| sub.help_heading)
                .map(str::to_string),
        );
    }

    let (own, inherited) = own_and_global(chain, inherit_version_actions);
    let visible_arg = |arg: &&ArgMeta<'_>| {
        !arg.hide
            && if long {
                !arg.hide_long_help
            } else {
                !arg.hide_short_help
            }
    };
    let mut args: Vec<_> = meta.args.iter().filter(visible_arg).collect();
    order_args(&mut args, meta.args);
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
    let mut own: Vec<_> = own.into_iter().filter(visible_flag).collect();
    order_flags(&mut own, meta.flags);
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
    if own
        .iter()
        .any(|flag| flag_help_heading(meta, flag).is_none())
    {
        headings.push("Flags".to_string());
    }
    headings.extend(
        own.iter()
            .filter_map(|flag| flag_help_heading(meta, flag))
            .map(str::to_string),
    );
    if !inherited.is_empty() {
        headings.push("Global flags".to_string());
    }

    let mut flag_usages: Vec<String> = own.iter().map(|flag| column_usage(flag)).collect();
    flag_usages.extend(inherited.into_iter().map(|(_, usage)| usage));
    flag_usages.sort_unstable_by_key(|usage| core::cmp::Reverse(usage.len()));

    let mut synopsis = String::new();
    usage_section(&mut synopsis, spec, path, meta);
    let synopsis = synopsis.lines().map(str::to_string).collect();
    (headings, flag_usages, synopsis)
}

fn flat_help_headings(path: &[&str], meta: &CommandMeta<'_>, headings: &mut Vec<String>) {
    let mut visible: Vec<_> = meta.subcommands.iter().filter(|sub| !sub.hide).collect();
    order_commands(&mut visible);
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
            // Examples are shell source, where paired backticks are command substitution rather
            // than prose markup. Every other non-structural line may contain author emphasis;
            // option and argument spellings are harmless because intraword underscores do not
            // open spans and unmatched shell globs remain literal.
            let body = styled.as_deref().unwrap_or(body);
            if body.trim_start().starts_with("$ ") {
                out.push_str(body);
            } else {
                out.push_str(&style.inline(body));
            }
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
    let flags: usize = meta.flags.iter().filter(|f| !f.hide && !f.builtin).count();
    if flags > 0 {
        let required = meta
            .flags
            .iter()
            .any(|f| !f.hide && !f.builtin && flag_demanded(f));
        if flags <= INLINE_LIMIT {
            for flag in meta.flags.iter().filter(|f| !f.hide && !f.builtin) {
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
    visible.sort_unstable_by_key(|sub| sub.cmd.name);
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
        // A flag whose only spelling is its negation — clap's `SetFalse`, tak's
        // `--no-credit` — is named after that spelling, so the prefix would repeat it:
        // `no-credit: --no-credit`.
        _ => show.negate && flag.negate == Some(flag.name),
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
        let exact = exact_arity(meta.value_var_min, meta.value_var_max);
        if meta.value_names.len() <= 1 && exact.is_some_and(|n| n > 1) {
            let name = meta
                .value_names
                .first()
                .copied()
                .or(meta.value_name)
                .unwrap_or(flag.name);
            for index in 0..exact.unwrap() {
                append_flag_value(
                    &mut out,
                    name,
                    meta.value_optional,
                    flag.require_equals,
                    index == 0,
                );
            }
        } else if meta.value_names.len() <= 1 {
            let name = meta
                .value_names
                .first()
                .copied()
                .or(meta.value_name)
                .unwrap_or(flag.name);
            append_flag_value(
                &mut out,
                name,
                meta.value_optional,
                flag.require_equals,
                true,
            );
        } else {
            for (index, name) in meta.value_names.iter().enumerate() {
                append_flag_value(
                    &mut out,
                    name,
                    meta.value_optional,
                    flag.require_equals,
                    index == 0,
                );
            }
        }
        if flag.variadic && meta.value_names.len() <= 1 && exact.is_none() {
            out.push('…');
        }
    }
    out
}

fn append_flag_value(
    out: &mut String,
    name: &str,
    optional: bool,
    require_equals: bool,
    first: bool,
) {
    if first && optional && require_equals {
        let _ = write!(out, "[={name}]");
    } else {
        let separator = if first && require_equals { "=" } else { " " };
        let (open, close) = if optional { ('[', ']') } else { ('<', '>') };
        let _ = write!(out, "{separator}{open}{name}{close}");
    }
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
    short_help_with(spec, path, chain, false)
}

fn short_help_with(
    spec: &Spec<'_>,
    path: &[&str],
    chain: &[&CommandMeta<'_>],
    inherit_version_actions: bool,
) -> String {
    assemble(
        spec,
        &short_sections(spec, path, chain, inherit_version_actions),
    )
}

fn short_sections(
    spec: &Spec<'_>,
    path: &[&str],
    chain: &[&CommandMeta<'_>],
    inherit_version_actions: bool,
) -> Sections {
    let meta = *chain.last().expect("a page is always about some command");
    let (own, inherited) = own_and_global(chain, inherit_version_actions);
    let own: Vec<_> = own
        .into_iter()
        .filter(|flag| !flag.hide_short_help)
        .collect();
    let inherited: Vec<_> = inherited
        .into_iter()
        .filter(|(flag, _)| !flag.hide_short_help)
        .collect();
    let mut sections = Sections::default();
    // The narrow page wraps too. Its descriptions used to run off the end of the terminal,
    // which the wide page has never done — and `-h` is the form most people type.
    let width = terminal_width(meta);
    let out = &mut sections.about;

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
    command_deprecation(out, meta, 0);
    usage_section(&mut sections.usage, spec, path, meta);

    // The path without the binary: it is the sort key the reference orders the list by, even
    // now that the row itself shows the child's own name.
    if !meta.flatten_help {
        commands_section(
            &mut sections.commands,
            &path[1.min(path.len())..],
            meta,
            width,
            false,
        );
    }

    // The short page lines its columns up too. It did not: every description began directly
    // after the name it belonged to, so nothing in `-h` lined up with anything — and `-h` is
    // the form most people type. One column per section over its visible entries, which is
    // the rule the long page already follows.
    let mut args: Vec<&ArgMeta<'_>> = meta
        .args
        .iter()
        .filter(|a| !a.hide && !a.hide_short_help)
        .collect();
    order_args(&mut args, meta.args);
    let arg_col = args
        .iter()
        .map(|a| arg_usage(a).chars().count())
        .max()
        .map(|longest| usage_column_width(longest, width))
        .unwrap_or(0);
    split_groups_section(
        SectionSink {
            page: &mut sections.args,
            ungrouped: &mut sections.ungrouped_args,
            grouped: &mut sections.grouped_args,
        },
        "Arguments",
        args.iter().copied(),
        |a| a.help_heading,
        // Section prose belongs to the long page, like an entry's admonitions.
        |_| None,
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
                    if a.hide_env { &[] } else { a.env_fallback },
                    if a.hide_env { &[] } else { a.deprecated_env },
                    if a.hide_default_value { &[] } else { a.default },
                    BLOCK_INDENT,
                );
                return;
            }
            let environment =
                inline_environment_notes(a.hide_env, a.env_fallback, a.deprecated_env);
            let notes = inline_annotations(
                if a.hide_possible_values {
                    &[]
                } else {
                    a.choices
                },
                if a.hide_env { None } else { a.env },
                environment.as_deref(),
                if a.hide_default_value { &[] } else { a.default },
                None,
            );
            entry(
                out,
                &usage,
                with_annotations(a.help, notes).as_deref(),
                arg_col,
                width,
                false,
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
        .map(|longest| usage_column_width(longest, width))
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
                if f.hide_env { &[] } else { f.env_fallback },
                if f.hide_env { &[] } else { f.deprecated_env },
                if f.hide_default_value { &[] } else { f.default },
                BLOCK_INDENT,
            );
            flag_notes(out, f, BLOCK_INDENT);
            return;
        }
        let deprecation =
            deprecation_label(f.deprecated, f.deprecated_warn_at, f.deprecated_remove_at);
        let environment = inline_environment_notes(f.hide_env, f.env_fallback, f.deprecated_env);
        let notes = inline_annotations(
            if f.hide_possible_values {
                &[]
            } else {
                f.choices
            },
            if f.hide_env { None } else { f.env },
            environment.as_deref(),
            if f.hide_default_value { &[] } else { f.default },
            deprecation.as_deref(),
        );
        entry(
            out,
            &usage,
            with_annotations(f.help, notes).as_deref(),
            flag_col,
            width,
            false,
        );
    };
    split_groups_section(
        SectionSink {
            page: &mut sections.flags,
            ungrouped: &mut sections.ungrouped_flags,
            grouped: &mut sections.grouped_flags,
        },
        "Flags",
        own.iter().copied(),
        |f| flag_help_heading(meta, f),
        |_| None,
        |out, f| short_entry(out, f, column_usage(f)),
    );
    // After the command's own, and under a heading that says where they came from: `--config`
    // belongs to the program, not to this command, and a reader should be able to see that.
    // The text is precomputed, since a spelling a descendant claimed is left out of it.
    split_groups_section(
        SectionSink {
            page: &mut sections.flags,
            ungrouped: &mut sections.ungrouped_flags,
            grouped: &mut sections.grouped_flags,
        },
        "Global flags",
        inherited.iter(),
        |_| None,
        |_| None,
        |out, (f, usage)| short_entry(out, f, usage.clone()),
    );
    if meta.flatten_help {
        flat_commands_short(
            &mut sections.flattened,
            &path[1.min(path.len())..],
            meta,
            width,
        );
    }
    examples_section(&mut sections.after_help, spec, meta);
    if let Some(after) = meta.after_help.or(spec.root.after_help) {
        let _ = writeln!(sections.after_help, "\n{after}");
    }

    sections
}

/// The list of subcommands, and the `help` command every CLI with subcommands has.
/// The entry every command list ends with, unless the CLI turned it off.
const HELP_SUBCOMMAND: &str = "help";
const HELP_SUBCOMMAND_SUMMARY: &str = "Print this message or the help of the given subcommand(s)";

/// The command list, identical on both pages.
///
/// The name alone occupies the column, so the summaries line up down the page and the syntax a
/// command takes belongs to that command's own page. Both pages read the same summary: a
/// parent's list says what each child is *for*, and what a child does at length belongs on the
/// child's own page rather than repeated in every ancestor's.
fn commands_section(
    out: &mut String,
    path: &[&str],
    meta: &CommandMeta<'_>,
    width: usize,
    long: bool,
) {
    let mut visible: Vec<&&CommandMeta<'_>> = meta.subcommands.iter().filter(|c| !c.hide).collect();
    order_commands(&mut visible);
    // Nothing visible, no section — `mise direnv` and `mise dotfiles` have subcommands and
    // every one of them is hidden. The usage *line* still says `<SUBCOMMAND>`, because
    // usage-lib computes it before filtering and stores it; matching the reference means
    // matching that too, odd as the pair looks together.
    if visible.is_empty() {
        return;
    }
    // Sorted by the rendered usage rather than by name, as usage-lib sorts them — for a
    // command with no flags or arguments the two agree, and where they differ this is the
    // order a reader sees in the reference. The usage is the sort key and nothing else now:
    // the row shows the name.
    let mut lines: Vec<(String, &&CommandMeta<'_>)> = visible
        .iter()
        .map(|sub| {
            let mut sub_path: Vec<&str> = path.to_vec();
            sub_path.push(sub.cmd.name);
            (usage_line(&sub_path, sub), *sub)
        })
        .collect();
    lines.sort_unstable_by(|a, b| {
        a.1.display_order
            .unwrap_or(999)
            .cmp(&b.1.display_order.unwrap_or(999))
            .then_with(|| a.0.cmp(&b.0))
    });

    // One column for the page, not for the section: a CLI that groups its commands reads as one
    // table with rules through it, which is what the flag list already does.
    let show_help = !meta.cmd.disable_help_subcommand;
    let col = lines
        .iter()
        .map(|(_, sub)| sub.cmd.name.chars().count())
        .chain(show_help.then(|| HELP_SUBCOMMAND.chars().count()))
        .max()
        .map(|longest| usage_column_width(longest, width))
        .unwrap_or(0);

    let default_title = meta.subcommand_help_heading.unwrap_or("Commands");
    let mut headings = vec![None];
    for (_, sub) in &lines {
        let heading = command_help_section(sub, default_title);
        if !headings.contains(&heading) {
            headings.push(heading);
        }
    }
    for heading in headings {
        let title = heading.unwrap_or(default_title);
        let _ = writeln!(out, "\n{title}:");
        // A `help_heading` on a subcommand builds a section like a flag's does, so it takes
        // prose on the same terms: the long page only, and only once declared.
        if long {
            if let Some(prose) = heading.and_then(|title| heading_help(meta, title)) {
                write_indented(out, prose, 2);
                out.push('\n');
            }
        }
        for (_, sub) in lines
            .iter()
            .filter(|(_, sub)| command_help_section(sub, default_title) == heading)
        {
            entry(
                out,
                sub.cmd.name,
                command_row(sub).as_deref(),
                col,
                width,
                meta.next_line_help,
            );
        }
        if heading.is_none() && show_help {
            entry(
                out,
                HELP_SUBCOMMAND,
                Some(HELP_SUBCOMMAND_SUMMARY),
                col,
                width,
                meta.next_line_help,
            );
        }
    }
}

/// Everything that follows a command's name in its parent's list, as one string.
///
/// Aliases and deprecation trail the summary rather than sitting beside the name, so they wrap
/// with the text instead of pushing every description out of the column.
fn command_row<'a>(sub: &'a CommandMeta<'a>) -> Option<Cow<'a, str>> {
    // A command that wrote only a long description still has a summary: its first line. Both
    // pages read the same one, so `-h` never says less about a command than `--help` does.
    let summary = summarize(sub.about)
        .or_else(|| summarize(sub.long_about.and_then(|about| about.lines().next())));
    // Visible aliases only: a hidden alias works and is not advertised, which is the whole of
    // the distinction.
    let mut visible_aliases = sub
        .cmd
        .aliases
        .iter()
        .copied()
        .filter(|a| !sub.hidden_aliases.contains(a))
        .peekable();
    let label = deprecation_label(
        sub.deprecated,
        sub.deprecated_warn_at,
        sub.deprecated_remove_at,
    );
    // A summary and nothing else is what almost every row is, and borrowing it there keeps the
    // whole list off the allocator — which `usage --help` notices, since it renders one.
    if visible_aliases.peek().is_none() && label.is_none() {
        return summary.map(Cow::Borrowed);
    }
    let mut row = String::new();
    if let Some(summary) = summary {
        row.push_str(summary);
    }
    if visible_aliases.peek().is_some() {
        if !row.is_empty() {
            row.push(' ');
        }
        row.push_str("[aliases: ");
        for (index, alias) in visible_aliases.enumerate() {
            if index > 0 {
                row.push_str(", ");
            }
            row.push_str(alias);
        }
        row.push(']');
    }
    if let Some(label) = label {
        if !row.is_empty() {
            row.push(' ');
        }
        row.push_str(&label);
    }
    Some(Cow::Owned(row))
}

fn flat_commands_short(out: &mut String, path: &[&str], meta: &CommandMeta<'_>, width: usize) {
    let mut visible: Vec<_> = meta.subcommands.iter().filter(|sub| !sub.hide).collect();
    order_commands(&mut visible);
    for sub in visible {
        let mut sub_path = path.to_vec();
        sub_path.push(sub.cmd.name);
        let _ = writeln!(out, "\n{}:", sub_path.join(" "));
        if let Some(about) = sub.about.filter(|about| !about.trim().is_empty()) {
            let _ = writeln!(out, "{}", about.trim_end());
        }
        command_deprecation(out, sub, 0);

        let mut args: Vec<_> = sub
            .args
            .iter()
            .filter(|arg| !arg.hide && !arg.hide_short_help)
            .collect();
        order_args(&mut args, sub.args);
        let mut flags: Vec<&FlagMeta<'_>> = sub
            .flags
            .iter()
            .filter(|flag| !flag.flag.global && !flag.hide && !flag.hide_short_help)
            .collect();
        order_flags(&mut flags, sub.flags);
        let arg_col = args
            .iter()
            .map(|arg| arg_usage(arg).chars().count())
            .max()
            .map(|longest| usage_column_width(longest, width))
            .unwrap_or(0);
        let flag_col = flags
            .iter()
            .map(|flag| column_usage(flag).chars().count())
            .max()
            .map(|longest| usage_column_width(longest, width))
            .unwrap_or(0);
        for arg in args {
            let usage = arg_usage(arg);
            if meta.next_line_help {
                let _ = writeln!(out, "  {usage}");
                if let Some(help) = arg.help.filter(|help| !help.trim().is_empty()) {
                    write_indented(out, help, BLOCK_INDENT);
                }
                long_annotations(
                    out,
                    if arg.hide_possible_values {
                        &[]
                    } else {
                        arg.choices
                    },
                    if arg.hide_env { None } else { arg.env },
                    if arg.hide_env { &[] } else { arg.env_fallback },
                    if arg.hide_env {
                        &[]
                    } else {
                        arg.deprecated_env
                    },
                    if arg.hide_default_value {
                        &[]
                    } else {
                        arg.default
                    },
                    BLOCK_INDENT,
                );
                continue;
            }
            let environment =
                inline_environment_notes(arg.hide_env, arg.env_fallback, arg.deprecated_env);
            let notes = inline_annotations(
                if arg.hide_possible_values {
                    &[]
                } else {
                    arg.choices
                },
                if arg.hide_env { None } else { arg.env },
                environment.as_deref(),
                if arg.hide_default_value {
                    &[]
                } else {
                    arg.default
                },
                None,
            );
            entry(
                out,
                &usage,
                with_annotations(arg.help, notes).as_deref(),
                arg_col,
                width,
                false,
            );
        }
        for flag in flags {
            let usage = column_usage(flag);
            if meta.next_line_help {
                let _ = writeln!(out, "  {usage}");
                if let Some(help) = flag.help.filter(|help| !help.trim().is_empty()) {
                    write_indented(out, help, BLOCK_INDENT);
                }
                long_annotations(
                    out,
                    if flag.hide_possible_values {
                        &[]
                    } else {
                        flag.choices
                    },
                    if flag.hide_env { None } else { flag.env },
                    if flag.hide_env {
                        &[]
                    } else {
                        flag.env_fallback
                    },
                    if flag.hide_env {
                        &[]
                    } else {
                        flag.deprecated_env
                    },
                    if flag.hide_default_value {
                        &[]
                    } else {
                        flag.default
                    },
                    BLOCK_INDENT,
                );
                flag_notes(out, flag, BLOCK_INDENT);
                continue;
            }
            let deprecation = deprecation_label(
                flag.deprecated,
                flag.deprecated_warn_at,
                flag.deprecated_remove_at,
            );
            let environment =
                inline_environment_notes(flag.hide_env, flag.env_fallback, flag.deprecated_env);
            let notes = inline_annotations(
                if flag.hide_possible_values {
                    &[]
                } else {
                    flag.choices
                },
                if flag.hide_env { None } else { flag.env },
                environment.as_deref(),
                if flag.hide_default_value {
                    &[]
                } else {
                    flag.default
                },
                deprecation.as_deref(),
            );
            entry(
                out,
                &usage,
                with_annotations(flag.help, notes).as_deref(),
                flag_col,
                width,
                false,
            );
        }
        if sub.flatten_help {
            flat_commands_short(out, &sub_path, sub, width);
        }
        out.push('\n');
    }
}

/// The section a flag appears under: its own heading, else the flatten site's
/// `next_help_heading` when this flag arrived through that group.
fn flag_help_heading<'a>(meta: &'a CommandMeta<'a>, flag: &'a FlagMeta<'a>) -> Option<&'a str> {
    flag.help_heading
        .or_else(|| flatten_site_heading(meta.flatten_groups, flag))
}

fn flatten_site_heading<'a>(
    groups: &'a [crate::spec::FlattenGroup<'a>],
    flag: &FlagMeta<'_>,
) -> Option<&'a str> {
    for group in groups {
        if group
            .meta
            .flags
            .iter()
            .any(|candidate| core::ptr::eq(candidate.flag, flag.flag))
        {
            return group
                .help_heading
                .or_else(|| flatten_site_heading(group.meta.flatten_groups, flag));
        }
        if let Some(heading) = flatten_site_heading(group.meta.flatten_groups, flag) {
            return Some(heading);
        }
    }
    None
}

/// The prose introducing a section, from this command's own declarations or from a
/// flattened type that declared the text where it declared the group.
fn heading_help<'a>(meta: &'a CommandMeta<'a>, title: &str) -> Option<&'a str> {
    if let Some(found) = meta
        .headings
        .iter()
        .find(|heading| heading.title == title)
        .map(|heading| heading.help)
    {
        return Some(found);
    }
    // The host's own declaration wins; only then does a flattened type get to speak for
    // the section it contributed.
    meta.flatten_groups
        .iter()
        .find_map(|group| heading_help(group.meta, title))
}

/// Where a rendered section goes: the page, plus the two partitions a template can name.
///
/// One value rather than three parameters, because the three always travel together — a
/// section written to the page but not to its partition is a bug, not a configuration.
struct SectionSink<'s> {
    page: &'s mut String,
    ungrouped: &'s mut String,
    grouped: &'s mut String,
}

/// One section per heading, unheaded first, while also keeping the named and default groups
/// available to a template.
fn split_groups_section<'m, T: 'm>(
    sink: SectionSink<'_>,
    default_title: &str,
    items: impl Iterator<Item = &'m T> + Clone,
    heading_of: impl Fn(&T) -> Option<&str>,
    prose_of: impl Fn(&str) -> Option<&'m str>,
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
    if let Some(index) = headings.iter().position(Option::is_none) {
        let unheaded = headings.remove(index);
        headings.insert(0, unheaded);
    }

    for heading in headings {
        let mut section = String::new();
        let title = heading.unwrap_or(default_title);
        let _ = writeln!(section, "\n{title}:");
        // Between the heading and its entries, where a reader looking at the section is
        // already looking. Written verbatim like an admonition rather than rewrapped, so
        // an author's own line breaks survive and the reference renderer can match it.
        //
        // Only a declared heading takes prose. The default title names the group that
        // exists because entries did not ask for a section, and it is not one title: the
        // same unheaded flags render under `Flags` here and under `Global Flags` in a
        // Markdown page, so keying prose to it would mean different things per renderer.
        if let Some(prose) = heading.and_then(&prose_of) {
            write_indented(&mut section, prose, 2);
            section.push('\n');
        }
        for item in items.clone().filter(|i| heading_of(i) == heading) {
            write_item(&mut section, item);
        }
        sink.page.push_str(&section);
        match heading {
            Some(_) => sink.grouped.push_str(&section),
            None => sink.ungrouped.push_str(&section),
        }
    }
}

fn order_args<'a>(items: &mut Vec<&'a ArgMeta<'a>>, declared: &'a [ArgMeta<'a>]) {
    items.sort_unstable_by_key(|item| {
        let position = declared
            .iter()
            .position(|candidate| core::ptr::eq(candidate, *item))
            .unwrap_or(usize::MAX);
        (item.display_order.unwrap_or(position), position)
    });
}

fn order_flags<'a>(items: &mut Vec<&'a FlagMeta<'a>>, declared: &'a [FlagMeta<'a>]) {
    items.sort_unstable_by_key(|item| {
        let position = declared
            .iter()
            .position(|candidate| core::ptr::eq(candidate, *item))
            .unwrap_or(usize::MAX);
        (item.display_order.unwrap_or(position), position)
    });
}

fn order_commands(items: &mut Vec<&&CommandMeta<'_>>) {
    items.sort_unstable_by(|a, b| {
        a.display_order
            .unwrap_or(999)
            .cmp(&b.display_order.unwrap_or(999))
            .then_with(|| a.cmd.name.cmp(b.cmd.name))
    });
}

fn command_help_section<'a>(sub: &'a CommandMeta<'a>, default_title: &str) -> Option<&'a str> {
    sub.help_heading.filter(|heading| *heading != default_title)
}

/// The bracketed notes after an entry's help, composed before the narrow layout wraps them.
fn inline_annotations(
    choices: &[&str],
    env: Option<&str>,
    environment: Option<&str>,
    default: &[&str],
    suffix: Option<&str>,
) -> Option<String> {
    let mut out = String::new();
    let mut push = |part: &str| {
        if !out.is_empty() {
            out.push(' ');
        }
        out.push_str(part);
    };
    if !choices.is_empty() {
        push(&format!("[{}]", choices.join(", ")));
    }
    if let Some(env) = env {
        push(&format!("[env: {env}]"));
    }
    if let Some(environment) = environment {
        push(environment);
    }
    if !default.is_empty() {
        push(&format!("(default: {})", default.join(", ")));
    }
    if let Some(suffix) = suffix {
        push(suffix);
    }
    (!out.is_empty()).then_some(out)
}

/// A narrow entry's description with its annotations joined on.
///
/// The wide layout gives each annotation a line of its own; the narrow one has no room for
/// that, so they ride along with the description — and they have to be joined *before* it is
/// wrapped, or an entry with a long description keeps its `[env: …]` out past the column the
/// wrapping was supposed to bring the text back into.
///
/// A description with nothing to add to it is borrowed rather than copied, which is most of
/// them.
fn with_annotations<'a>(
    help: Option<&'a str>,
    annotations: Option<String>,
) -> Option<Cow<'a, str>> {
    match (summarize(help), annotations) {
        (Some(help), None) => Some(Cow::Borrowed(help)),
        (None, Some(annotations)) => Some(Cow::Owned(annotations)),
        (Some(help), Some(annotations)) => Some(Cow::Owned(format!("{help} {annotations}"))),
        (None, None) => None,
    }
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
        .or_else(|| meta.flag.negate.map(|negate| format!("--{negate}")))
        .unwrap_or_else(|| meta.flag.name.to_string())
}

fn display_usage_masked(meta: &FlagMeta<'_>, show: &Shown) -> String {
    let usage = flag_usage_masked(meta, show);
    match meta.flag.negate.filter(|_| show.negate) {
        // A flag whose only spelling is its negation has nothing before it: the name prefix
        // would repeat the spelling, so `flag_usage_masked` writes nothing and the negation
        // is the whole entry. Joining with a space put one at the front of the column.
        Some(negate) if usage.is_empty() => format!("--{negate}"),
        Some(negate) if show.long.is_none() && show.short.is_none() => {
            format!("{usage} --{negate}")
        }
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

/// Keep a single long spelling from narrowing every description on the page.
///
/// After the two-space indent and gap, usage gets at most two fifths of what remains. Entries
/// beyond the cap use block layout on their own; an explicitly unbounded page keeps its natural
/// column.
fn usage_column_width(longest: usize, terminal_width: usize) -> usize {
    if terminal_width == usize::MAX {
        return longest;
    }
    let available = terminal_width.saturating_sub(4);
    let cap = available / 5 * 2 + available % 5 * 2 / 5;
    longest.min(cap)
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
    long_help_with(spec, path, chain, false)
}

fn long_help_with(
    spec: &Spec<'_>,
    path: &[&str],
    chain: &[&CommandMeta<'_>],
    inherit_version_actions: bool,
) -> String {
    assemble(
        spec,
        &long_sections(spec, path, chain, inherit_version_actions),
    )
}

fn long_sections(
    spec: &Spec<'_>,
    path: &[&str],
    chain: &[&CommandMeta<'_>],
    inherit_version_actions: bool,
) -> Sections {
    let meta = *chain.last().expect("a page is always about some command");
    let (own, inherited) = own_and_global(chain, inherit_version_actions);
    let own: Vec<_> = own
        .into_iter()
        .filter(|flag| !flag.hide_long_help)
        .collect();
    let inherited: Vec<_> = inherited
        .into_iter()
        .filter(|(flag, _)| !flag.hide_long_help)
        .collect();
    let width = terminal_width(meta);
    let mut sections = Sections::default();
    let out = &mut sections.about;

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
    command_deprecation(out, meta, 0);
    usage_section(&mut sections.usage, spec, path, meta);

    if !meta.flatten_help {
        commands_section(
            &mut sections.commands,
            &path[1.min(path.len())..],
            meta,
            width,
            true,
        );
    }

    // One column width per section, over its visible entries — the same two the reference
    // computes, and separately, so a long flag does not push the arguments out.
    let mut args: Vec<&ArgMeta<'_>> = meta
        .args
        .iter()
        .filter(|a| !a.hide && !a.hide_long_help)
        .collect();
    order_args(&mut args, meta.args);
    let arg_col = args
        .iter()
        .map(|a| arg_usage(a).chars().count())
        .max()
        .map(|longest| usage_column_width(longest, width))
        .unwrap_or(0);
    split_groups_section(
        SectionSink {
            page: &mut sections.args,
            ungrouped: &mut sections.ungrouped_args,
            grouped: &mut sections.grouped_args,
        },
        "Arguments",
        args.iter().copied(),
        |a| a.help_heading,
        |title| heading_help(meta, title),
        |out, a| {
            let text = a.long_help.or(a.help);
            let indent = entry(
                out,
                &arg_usage(a),
                text,
                arg_col,
                width,
                meta.next_line_help,
            );
            admonitions(out, a.admonitions);
            long_annotations(
                out,
                if a.hide_possible_values {
                    &[]
                } else {
                    a.choices
                },
                if a.hide_env { None } else { a.env },
                if a.hide_env { &[] } else { a.env_fallback },
                if a.hide_env { &[] } else { a.deprecated_env },
                if a.hide_default_value { &[] } else { a.default },
                indent,
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
        .map(|longest| usage_column_width(longest, width))
        .unwrap_or(0);
    split_groups_section(
        SectionSink {
            page: &mut sections.flags,
            ungrouped: &mut sections.ungrouped_flags,
            grouped: &mut sections.grouped_flags,
        },
        "Flags",
        own.iter().copied(),
        |f| flag_help_heading(meta, f),
        |title| heading_help(meta, title),
        |out, f| {
            let text = f.long_help.or(f.help);
            let indent = entry(
                out,
                &column_usage(f),
                text,
                flag_col,
                width,
                meta.next_line_help,
            );
            admonitions(out, f.admonitions);
            long_annotations(
                out,
                if f.hide_possible_values {
                    &[]
                } else {
                    f.choices
                },
                if f.hide_env { None } else { f.env },
                if f.hide_env { &[] } else { f.env_fallback },
                if f.hide_env { &[] } else { f.deprecated_env },
                if f.hide_default_value { &[] } else { f.default },
                indent,
            );
            flag_notes(out, f, indent);
        },
    );
    // After the command's own, and under a heading that says where they came from: `--config`
    // belongs to the program, not to this command, and a reader should be able to see that.
    // Not grouped by `help_heading` — an ancestor's headings describe that command's page, and
    // borrowing them here would put a section title on flags that are only visiting.
    split_groups_section(
        SectionSink {
            page: &mut sections.flags,
            ungrouped: &mut sections.ungrouped_flags,
            grouped: &mut sections.grouped_flags,
        },
        "Global flags",
        inherited.iter(),
        |_| None,
        |_| None,
        |out, (f, usage)| {
            let text = f.long_help.or(f.help);
            let indent = entry(out, usage, text, flag_col, width, meta.next_line_help);
            admonitions(out, f.admonitions);
            long_annotations(
                out,
                if f.hide_possible_values {
                    &[]
                } else {
                    f.choices
                },
                if f.hide_env { None } else { f.env },
                if f.hide_env { &[] } else { f.env_fallback },
                if f.hide_env { &[] } else { f.deprecated_env },
                if f.hide_default_value { &[] } else { f.default },
                indent,
            );
            flag_notes(out, f, indent);
        },
    );
    if meta.flatten_help {
        flat_commands_long(
            &mut sections.flattened,
            &path[1.min(path.len())..],
            meta,
            width,
        );
    }

    let out = &mut sections.after_help;
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
    let after = meta
        .after_long_help
        .or(meta.after_help)
        .or(spec.root.after_long_help)
        .or(spec.root.after_help);
    if let Some(after) = after {
        let _ = writeln!(out, "\n{after}");
    }
    if spec.author.is_some() || spec.license.is_some() {
        // The reference template starts the footer in a new paragraph without trimming the
        // configured trailing help. A newline deliberately present in `after_help` therefore
        // remains an additional blank line before package metadata.
        out.push('\n');
        if let Some(author) = spec.author {
            let _ = writeln!(out, "Author: {author}");
        }
        if let Some(license) = spec.license {
            let _ = writeln!(out, "License: {license}");
        }
    }

    sections
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
) -> usize {
    // The column layout only works for text that has not been broken already, and only when
    // there is room left for it to say anything.
    let indent = 2 + col + 2;
    let room = width.saturating_sub(indent);
    // A long outlier leaves the shared column to the ordinary entries and uses a block alone.
    let overflow = usage.chars().count() > col;
    let block = next_line || overflow || room < 10;
    let Some(help) = help.filter(|h| !h.trim().is_empty()) else {
        let _ = writeln!(out, "  {usage}");
        // An entry with nothing in the column still has annotations to place, and the column is
        // where they go: it is this entry's row that is empty, not the table's.
        return if block { BLOCK_INDENT } else { indent };
    };

    if overflow && !next_line && !help.contains('\n') {
        let _ = writeln!(out, "  {usage}");
        write_wrapped_block(out, help, width);
        return BLOCK_INDENT;
    }

    if block || help.contains('\n') {
        let _ = writeln!(out, "  {usage}");
        write_indented(out, help, BLOCK_INDENT);
        return BLOCK_INDENT;
    }

    // Text that already fits is text `wrap` would hand straight back, so skip it and the two
    // allocations it makes. Worth the check because it is the common case — most descriptions
    // are shorter than the column leaves room for.
    if fits(help, room) {
        // Assembled rather than formatted. This is the row every entry on every page takes, and
        // `{usage:<col$}` drags in the whole formatting machinery to pad a string.
        out.push_str("  ");
        out.push_str(usage);
        // Padded by characters, as the format directive it replaces was: a usage can hold a `…`.
        for _ in 0..col.saturating_sub(usage.chars().count()) {
            out.push(' ');
        }
        out.push_str("  ");
        out.push_str(help);
        out.push('\n');
        return indent;
    }

    let lines = wrap(help, room);
    let _ = writeln!(out, "  {usage:<col$}  {}", lines[0]);
    for line in &lines[1..] {
        let _ = writeln!(out, "{}{line}", " ".repeat(indent));
    }
    // No blank line after a wrapped entry. The reference's template asks for one, and its
    // whitespace trimming eats it before it reaches the output — so a wrapped entry is followed
    // directly by the next, and matching means matching that.
    indent
}

/// Wrap an overflowing entry's description across the width below its usage.
fn write_wrapped_block(out: &mut String, help: &str, width: usize) {
    let room = width.saturating_sub(BLOCK_INDENT);
    for line in wrap(help, room) {
        let _ = writeln!(out, "{}{}", " ".repeat(BLOCK_INDENT), line);
    }
}

/// Whether [`wrap`] would return this text unchanged as a single line.
///
/// True only for text already spelled the way `wrap` spells it — one line, single spaces
/// between words, none at either end — and no wider than the room available.
///
/// Answering conservatively is free: a false answer costs only the wrap it was going to avoid,
/// and `wrap` returns the same single line anyway. So this reads bytes rather than characters —
/// a byte count is never below a character count, which settles the width without counting, and
/// anything non-ASCII is handed to `wrap` rather than reasoned about here, since the whitespace
/// it would fold is not all ASCII.
fn fits(text: &str, room: usize) -> bool {
    if text.len() > room {
        return false;
    }
    // The start counts as a space so that a leading one, which `wrap` would drop, fails here.
    let mut after_space = true;
    for &byte in text.as_bytes() {
        match byte {
            // A run of spaces collapses, which is `wrap` rewriting the text.
            b' ' if after_space => return false,
            b' ' => after_space = true,
            // Any other whitespace becomes a space, and any non-ASCII byte may be some.
            b'\t' | b'\n' | b'\r' | 0x0b | 0x0c => return false,
            0x80.. => return false,
            _ => after_space = false,
        }
    }
    // A trailing space would be dropped too.
    !after_space
}

/// Break text at word boundaries to fit a width, keeping any breaks it already has.
///
/// The width of the line under construction is carried along rather than recounted for each
/// word. Recounting made this quadratic in the length of a line, which went unnoticed while
/// only the long page wrapped and was 3% of `usage --help` once the command list did too.
fn wrap(text: &str, width: usize) -> Vec<String> {
    let mut lines = Vec::new();
    for paragraph in text.split('\n') {
        if paragraph.is_empty() {
            lines.push(String::new());
            continue;
        }
        let mut line = String::new();
        let mut line_width = 0;
        for word in paragraph.split_whitespace() {
            let word_width = word.chars().count();
            if !line.is_empty() && line_width + 1 + word_width > width {
                lines.push(std::mem::take(&mut line));
                line_width = 0;
            }
            if !line.is_empty() {
                line.push(' ');
                line_width += 1;
            }
            line.push_str(word);
            line_width += word_width;
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
///
/// `indent` is where the entry above them ended up: the description column when the
/// description reached it, and [`BLOCK_INDENT`] when it did not. An annotation is a note about
/// the same entry, so it belongs under the text it qualifies rather than in the gutter beside a
/// column it is ignoring.
fn long_annotations(
    out: &mut String,
    choices: &[&str],
    env: Option<&str>,
    env_fallback: &[&str],
    deprecated_env: &[&str],
    default: &[&str],
    indent: usize,
) {
    // Most entries annotate nothing, and this is called for every one of them — so the indent
    // is not built until there is a line to put it on.
    if choices.is_empty()
        && env.is_none()
        && env_fallback.is_empty()
        && deprecated_env.is_empty()
        && default.is_empty()
    {
        return;
    }
    let pad = " ".repeat(indent);
    if !choices.is_empty() {
        let _ = writeln!(out, "{pad}[possible values: {}]", choices.join(", "));
    }
    if let Some(env) = env {
        let _ = writeln!(out, "{pad}[env: {env}]");
    }
    environment_notes(out, env_fallback, deprecated_env, indent);
    if !default.is_empty() {
        let _ = writeln!(out, "{pad}(default: {})", default.join(", "));
    }
}

fn admonitions(out: &mut String, blocks: &[AdmonitionMeta<'_>]) {
    for block in blocks {
        let label = match block.kind {
            AdmonitionKind::Note => "Note",
            AdmonitionKind::Warning => "Warning",
        };
        write_indented(out, &format!("{label}: {}", block.text), 4);
    }
}

/// A description reduced to what a list can show, or nothing if it says nothing.
fn summarize(text: Option<&str>) -> Option<&str> {
    text.map(str::trim_end).filter(|text| !text.is_empty())
}

fn deprecation_label(
    message: Option<&str>,
    warn_at: Option<&str>,
    remove_at: Option<&str>,
) -> Option<String> {
    if message.is_none() && warn_at.is_none() && remove_at.is_none() {
        return None;
    }
    let mut parts = Vec::new();
    if let Some(message) = message {
        parts.push(message.to_string());
    }
    if let Some(at) = warn_at {
        parts.push(format!("warns at {at}"));
    }
    if let Some(at) = remove_at {
        parts.push(format!("removed at {at}"));
    }
    Some(format!("[deprecated: {}]", parts.join("; ")))
}

fn command_deprecation(out: &mut String, meta: &CommandMeta<'_>, indent: usize) {
    if let Some(label) = deprecation_label(
        meta.deprecated,
        meta.deprecated_warn_at,
        meta.deprecated_remove_at,
    ) {
        let _ = writeln!(out, "{}{label}", " ".repeat(indent));
    }
}

fn inline_environment_notes(hide: bool, fallbacks: &[&str], deprecated: &[&str]) -> Option<String> {
    let mut notes = Vec::new();
    if !hide {
        notes.extend(fallbacks.iter().map(|env| format!("[env fallback: {env}]")));
        notes.extend(
            deprecated
                .iter()
                .map(|env| format!("[deprecated env: {env}]")),
        );
    }
    (!notes.is_empty()).then(|| notes.join(" "))
}

fn environment_notes(out: &mut String, fallbacks: &[&str], deprecated: &[&str], indent: usize) {
    for env in fallbacks {
        let _ = writeln!(out, "{}[env fallback: {env}]", " ".repeat(indent));
    }
    for env in deprecated {
        let _ = writeln!(out, "{}[deprecated env: {env}]", " ".repeat(indent));
    }
}

fn flag_notes(out: &mut String, meta: &FlagMeta<'_>, indent: usize) {
    if let Some(label) = deprecation_label(
        meta.deprecated,
        meta.deprecated_warn_at,
        meta.deprecated_remove_at,
    ) {
        let _ = writeln!(out, "{}{label}", " ".repeat(indent));
    }
}

fn flat_commands_long(out: &mut String, path: &[&str], meta: &CommandMeta<'_>, width: usize) {
    let mut visible: Vec<_> = meta.subcommands.iter().filter(|sub| !sub.hide).collect();
    order_commands(&mut visible);
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
        command_deprecation(out, sub, 0);

        let mut args: Vec<_> = sub
            .args
            .iter()
            .filter(|arg| !arg.hide && !arg.hide_long_help)
            .collect();
        order_args(&mut args, sub.args);
        let mut flags: Vec<&FlagMeta<'_>> = sub
            .flags
            .iter()
            .filter(|flag| !flag.flag.global && !flag.hide && !flag.hide_long_help)
            .collect();
        order_flags(&mut flags, sub.flags);
        let arg_col = args
            .iter()
            .map(|arg| arg_usage(arg).chars().count())
            .max()
            .map(|longest| usage_column_width(longest, width))
            .unwrap_or(0);
        let flag_col = flags
            .iter()
            .map(|flag| column_usage(flag).chars().count())
            .max()
            .map(|longest| usage_column_width(longest, width))
            .unwrap_or(0);
        for arg in args {
            entry(
                out,
                &arg_usage(arg),
                arg.long_help.or(arg.help),
                arg_col,
                width,
                meta.next_line_help,
            );
            admonitions(out, arg.admonitions);
            long_annotations(
                out,
                if arg.hide_possible_values {
                    &[]
                } else {
                    arg.choices
                },
                if arg.hide_env { None } else { arg.env },
                if arg.hide_env { &[] } else { arg.env_fallback },
                if arg.hide_env {
                    &[]
                } else {
                    arg.deprecated_env
                },
                if arg.hide_default_value {
                    &[]
                } else {
                    arg.default
                },
                BLOCK_INDENT,
            );
        }
        for flag in flags {
            entry(
                out,
                &column_usage(flag),
                flag.long_help.or(flag.help),
                flag_col,
                width,
                meta.next_line_help,
            );
            admonitions(out, flag.admonitions);
            long_annotations(
                out,
                if flag.hide_possible_values {
                    &[]
                } else {
                    flag.choices
                },
                if flag.hide_env { None } else { flag.env },
                if flag.hide_env {
                    &[]
                } else {
                    flag.env_fallback
                },
                if flag.hide_env {
                    &[]
                } else {
                    flag.deprecated_env
                },
                if flag.hide_default_value {
                    &[]
                } else {
                    flag.default
                },
                BLOCK_INDENT,
            );
            flag_notes(out, flag, BLOCK_INDENT);
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
    spec: &Spec<'a>,
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
    use crate::{ArgAction, Flag};

    macro_rules! entry {
        ($name:ident, $flag:ident, $key:expr, $label:expr, $longs:expr, $shorts:expr, $help:expr, $action:expr) => {
            static $flag: Flag<'static> = Flag {
                key: $key,
                name: $label,
                longs: $longs,
                shorts: $shorts,
                action: $action,
                ..Flag::BOOL
            };
            pub static $name: FlagMeta<'static> = FlagMeta {
                flag: &$flag,
                help: Some($help),
                builtin: true,
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
        "Print help",
        ArgAction::Help
    );
    entry!(
        HELP_LONG_ONLY,
        HL,
        crate::HELP_LONG_KEY,
        "help",
        &["help"],
        b"",
        "Print help",
        ArgAction::Help
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
        "Print help",
        ArgAction::Help
    );
    entry!(
        VERSION_BOTH,
        VB,
        crate::VERSION_LONG_KEY,
        "version",
        &["version"],
        b"V",
        "Print version",
        ArgAction::Version
    );
    entry!(
        VERSION_LONG_ONLY,
        VL,
        crate::VERSION_LONG_KEY,
        "version",
        &["version"],
        b"",
        "Print version",
        ArgAction::Version
    );
    entry!(
        VERSION_SHORT_ONLY,
        VS,
        crate::VERSION_SHORT_KEY,
        "V",
        &[],
        b"V",
        "Print version",
        ArgAction::Version
    );
}

/// The supplied entries a page should list, given what the command already claims.
///
/// `--version` only where the parser actually accepts it: on a command whose table says so,
/// which the derive sets on the root when a version is declared. A page offering one that the
/// parser would refuse is worse than a page that stays quiet.
pub(crate) fn supplied_entries(
    cmd: &Command<'_>,
    taken: &[String],
) -> Vec<&'static FlagMeta<'static>> {
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
    if !cmd.disable_help_flag {
        out.extend(pick(
            "help",
            'h',
            &supplied::HELP_BOTH,
            &supplied::HELP_LONG_ONLY,
            &supplied::HELP_SHORT_ONLY,
        ));
    }
    // Only where the parser accepts one, which is the root of a CLI that declared a version.
    if cmd.version && !cmd.disable_version_flag {
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
    inherit_version_actions: bool,
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
        .chain(ancestors.iter().flat_map(|m| m.flags.iter()).filter(|f| {
            f.flag.global || (inherit_version_actions && crate::is_version_flag(f.flag))
        }))
        .flat_map(forms)
        .collect();

    let mut taken: Vec<String> = here.flags.iter().flat_map(forms).collect();
    let mut taken_negations: Vec<String> = here.flags.iter().filter_map(negation).collect();
    let mut keep: Vec<(*const FlagMeta<'_>, Shown<'_>)> = Vec::new();
    for meta in ancestors.iter().rev() {
        for f in meta.flags.iter().filter(|f| {
            f.flag.global || (inherit_version_actions && crate::is_version_flag(f.flag))
        }) {
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
    let mut inherited: Vec<(&FlagMeta<'_>, String)> = ancestors
        .iter()
        .flat_map(|meta| meta.flags.iter())
        .filter_map(|f| {
            keep.iter()
                .find(|(p, _)| core::ptr::eq(*p, f as *const _))
                .map(|(_, show)| (f, column_usage_masked(f, show)))
        })
        .collect();
    let inherited_positions: Vec<*const FlagMeta<'_>> = inherited
        .iter()
        .map(|(flag, _)| *flag as *const _)
        .collect();
    inherited.sort_unstable_by_key(|(flag, _)| {
        let position = inherited_positions
            .iter()
            .position(|candidate| core::ptr::eq(*candidate, *flag as *const _))
            .unwrap_or(usize::MAX);
        (flag.display_order.unwrap_or(position), position)
    });

    // Last in the command's own section, which is where clap has them: they carry no
    // `help_heading`, so a CLI that groups its flags gets them at the end of the ungrouped
    // list rather than inside somebody's section.
    //
    // Given `taken` rather than the two lists: that set already counts hidden declarations and
    // negations, and a `--help` the page offers while something else binds it is exactly the
    // lie this whole model exists to prevent.
    let mut own = own;
    order_flags(&mut own, here.flags);
    // Forms *and* negations: `long_flag` asks `find_negation` before it offers `--version`,
    // so a declared negation beats a supplied flag even though it loses to a long.
    let claimed: Vec<String> = taken
        .iter()
        .cloned()
        .chain(taken_negations.iter().cloned())
        .collect();
    if inherit_version_actions {
        if let Some(root) = ancestors.first() {
            inherited.extend(
                supplied_entries(root.cmd, &claimed)
                    .into_iter()
                    .filter(|flag| {
                        matches!(
                            flag.flag.key,
                            crate::VERSION_LONG_KEY | crate::VERSION_SHORT_KEY
                        )
                    })
                    .map(|flag| (flag, column_usage(flag))),
            );
        }
    }
    own.extend(supplied_entries(here.cmd, &claimed));
    (own, inherited)
}

/// A named section of one command's help page.
///
/// `id` is a deterministic, command-local spelling suitable for `tool help <id>` and completion;
/// `title` is the heading users see. Topics include the ordinary Commands, Arguments, Flags,
/// and Global flags sections plus every declared `help_heading` group that has visible entries.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Topic {
    pub id: String,
    pub title: String,
}

fn topic_id(title: &str) -> String {
    let mut out = String::new();
    let mut separator = false;
    for ch in title.chars() {
        if ch.is_alphanumeric() {
            if separator && !out.is_empty() {
                out.push('-');
            }
            out.extend(ch.to_lowercase());
            separator = false;
        } else {
            separator = true;
        }
    }
    if out.is_empty() {
        "topic".to_string()
    } else {
        out
    }
}

fn topic_blocks<'m>(
    sections: &Sections,
    prose_of: impl Fn(&str) -> Option<&'m str>,
) -> Vec<(String, String)> {
    let mut topics: Vec<(String, String)> = Vec::new();
    for section in [&sections.commands, &sections.args, &sections.flags] {
        let mut title: Option<&str> = None;
        let mut block = String::new();
        let finish =
            |title: Option<&str>, block: &mut String, topics: &mut Vec<(String, String)>| {
                let Some(title) = title else {
                    block.clear();
                    return;
                };
                let text = block.trim().to_string();
                let Some((_, body)) = text.split_once('\n') else {
                    block.clear();
                    return;
                };
                if body.trim().is_empty() {
                    block.clear();
                    return;
                }
                if let Some((_, existing)) = topics.iter_mut().find(|(known, _)| known == title) {
                    // One heading can build a block in more than one section — args and flags
                    // both naming it — and each block introduces itself with the prose. Merged
                    // into the single topic that heading addresses, it must say it once.
                    let mut body = body;
                    if let Some(prose) = prose_of(title) {
                        let mut introduction = String::new();
                        write_indented(&mut introduction, prose, 2);
                        if let Some(rest) = body.strip_prefix(introduction.trim_end_matches('\n')) {
                            body = rest.trim_start_matches('\n');
                        }
                    }
                    if !existing.is_empty() {
                        existing.push_str("\n\n");
                    }
                    existing.push_str(body);
                } else {
                    topics.push((title.to_string(), text));
                }
                block.clear();
            };
        for line in section.lines() {
            let heading = (!line.starts_with(char::is_whitespace))
                .then(|| line.strip_suffix(':'))
                .flatten();
            if let Some(heading) = heading {
                finish(title, &mut block, &mut topics);
                title = Some(heading);
            }
            if title.is_some() {
                if !block.is_empty() {
                    block.push('\n');
                }
                block.push_str(line);
            }
        }
        finish(title, &mut block, &mut topics);
    }
    topics
}

fn topics_with_blocks(
    spec: &Spec<'_>,
    cmd: &Command<'_>,
    long: bool,
) -> Option<Vec<(Topic, String)>> {
    let (path, chain) = find(spec, cmd)?;
    let sections = if long {
        long_sections(spec, &path, &chain, false)
    } else {
        short_sections(spec, &path, &chain, false)
    };
    let mut used = Vec::<String>::new();
    Some(
        topic_blocks(&sections, |title| heading_help(chain.last()?, title))
            .into_iter()
            .map(|(title, block)| {
                let base = topic_id(&title);
                let mut id = base.clone();
                let mut suffix = 2;
                while used.contains(&id) {
                    id = format!("{base}-{suffix}");
                    suffix += 1;
                }
                used.push(id.clone());
                (Topic { id, title }, block)
            })
            .collect(),
    )
}

/// List the addressable topics on one command's short or long help page.
pub fn topics(spec: &Spec<'_>, cmd: &Command<'_>, long: bool) -> Option<Vec<Topic>> {
    Some(
        topics_with_blocks(spec, cmd, long)?
            .into_iter()
            .map(|(topic, _)| topic)
            .collect(),
    )
}

/// Render one addressable help topic by its [`Topic::id`] or visible title.
///
/// The result contains the heading and its visible entries, independent of the page's
/// `help_template`. This makes a topic suitable for `tool help configuration`, an editor panel,
/// or an interactive picker without making the group a fake subcommand.
pub fn render_topic(spec: &Spec<'_>, cmd: &Command<'_>, topic: &str, long: bool) -> Option<String> {
    topics_with_blocks(spec, cmd, long)?
        .into_iter()
        .find(|(known, _)| known.id == topic || known.title.eq_ignore_ascii_case(topic))
        .map(|(_, mut block)| {
            block.push('\n');
            block
        })
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
    let (headings, flag_usages, synopsis) = help_structure(spec, &path, &chain, long, false);
    Some(styled_help(
        &page,
        style,
        &headings,
        &flag_usages,
        &synopsis,
    ))
}

/// Long help for a command and every visible descendant, in depth-first order.
pub fn render_all(spec: &Spec<'_>, cmd: &Command<'_>) -> Option<String> {
    render_all_styled(spec, cmd, Style::PLAIN)
}

/// Recursive long help with an explicit colour policy.
pub fn render_all_styled(spec: &Spec<'_>, cmd: &Command<'_>, style: Style) -> Option<String> {
    let (path, chain) = find(spec, cmd)?;
    Some(recursive_help(spec, path, chain, style, false))
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

/// Recover a command route from the full argv of a declared executable view.
///
/// `argv` includes the view executable as argv0. The promoted root is inserted internally,
/// matching the derive-generated `parse_from_argv` without requiring callers to reconstruct its
/// private rewrite.
pub fn route_to_view<'t>(
    root: &'t Command<'t>,
    argv: &[&std::ffi::OsStr],
    cmd: &Command<'_>,
    view: &ViewMeta<'_>,
) -> Option<Vec<&'t Command<'t>>> {
    let words = argv.get(1..).unwrap_or_default();
    let mut rewritten =
        Vec::with_capacity(words.len() + view.root.split_ascii_whitespace().count());
    rewritten.extend(view.root.split_ascii_whitespace().map(std::ffi::OsStr::new));
    rewritten.extend_from_slice(words);
    route_to(root, &rewritten, cmd)
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
    let (headings, flag_usages, synopsis) = help_structure(spec, &path, &chain, long, false);
    Some(styled_help(
        &page,
        style,
        &headings,
        &flag_usages,
        &synopsis,
    ))
}

/// Render help through a spec-declared executable view.
///
/// The parser still walks the canonical static tables. This changes only the cold presentation:
/// the promoted command becomes the displayed root and only the root globals declared by the
/// view remain inherited.
pub fn render_view_at_styled(
    spec: &Spec<'_>,
    route: &[&Command<'_>],
    view: &ViewMeta<'_>,
    long: bool,
    style: Style,
) -> Option<String> {
    let (canonical_path, canonical_chain) = route_context(spec, route)?;
    let depth = view.root.split_ascii_whitespace().count();
    let promoted = *canonical_chain.get(depth)?;

    let (root_flags, root_groups) = view_root_fields(spec, promoted, view);
    let root_command = Command {
        // Version actions belong to the executable, not to the promoted command. The parser
        // retains that host policy before projection, so help must synthesize the same flag.
        version: spec.root.cmd.version,
        disable_version_flag: spec.root.cmd.disable_version_flag,
        ..*promoted.cmd
    };
    let root = CommandMeta {
        cmd: &root_command,
        flags: &root_flags,
        groups: &root_groups,
        ..*promoted
    };
    let mut chain = Vec::with_capacity(canonical_chain.len());
    chain.push(&root);
    chain.extend_from_slice(canonical_chain.get(depth + 1..).unwrap_or_default());

    let mut path = Vec::with_capacity(canonical_path.len().saturating_sub(depth));
    path.push(view.bin);
    path.extend_from_slice(canonical_path.get(depth + 1..).unwrap_or_default());
    let viewed = Spec {
        name: view.name,
        bin: Some(view.bin),
        about: promoted.about,
        long_about: promoted.long_about,
        usage: None,
        default_subcommand: None,
        multicall: false,
        root: &root,
        ..*spec
    };
    let page = if long {
        long_help_with(&viewed, &path, &chain, true)
    } else {
        short_help_with(&viewed, &path, &chain, true)
    };
    let (headings, flag_usages, synopsis) = help_structure(&viewed, &path, &chain, long, true);
    Some(styled_help(
        &page,
        style,
        &headings,
        &flag_usages,
        &synopsis,
    ))
}

/// Recursive long help for a command reached by a known route.
pub fn render_all_at(spec: &Spec<'_>, route: &[&Command<'_>]) -> Option<String> {
    render_all_at_styled(spec, route, Style::PLAIN)
}

/// Route-specific recursive long help with an explicit colour policy.
pub fn render_all_at_styled(
    spec: &Spec<'_>,
    route: &[&Command<'_>],
    style: Style,
) -> Option<String> {
    let (path, chain) = route_context(spec, route)?;
    Some(recursive_help(spec, path, chain, style, false))
}

/// Recursive long help through a spec-declared executable view.
pub fn render_all_view_at_styled(
    spec: &Spec<'_>,
    route: &[&Command<'_>],
    view: &ViewMeta<'_>,
    style: Style,
) -> Option<String> {
    let (canonical_path, canonical_chain) = route_context(spec, route)?;
    let depth = view.root.split_ascii_whitespace().count();
    let promoted = *canonical_chain.get(depth)?;
    let (root_flags, root_groups) = view_root_fields(spec, promoted, view);
    let root_command = Command {
        version: spec.root.cmd.version,
        disable_version_flag: spec.root.cmd.disable_version_flag,
        ..*promoted.cmd
    };
    let root = CommandMeta {
        cmd: &root_command,
        flags: &root_flags,
        groups: &root_groups,
        ..*promoted
    };
    let mut chain = Vec::with_capacity(canonical_chain.len().saturating_sub(depth));
    chain.push(&root);
    chain.extend_from_slice(canonical_chain.get(depth + 1..).unwrap_or_default());
    let mut path = Vec::with_capacity(canonical_path.len().saturating_sub(depth));
    path.push(view.bin);
    path.extend_from_slice(canonical_path.get(depth + 1..).unwrap_or_default());
    let viewed = Spec {
        name: view.name,
        bin: Some(view.bin),
        about: promoted.about,
        long_about: promoted.long_about,
        usage: None,
        default_subcommand: None,
        multicall: false,
        root: &root,
        ..*spec
    };
    Some(recursive_help(&viewed, path, chain, style, true))
}

/// Which page a help request asks for.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Page {
    /// `-h`: the short page for one command.
    Short,
    /// `--help`: the long page for one command.
    Long,
    /// [`ArgAction::HelpAll`](crate::ArgAction::HelpAll): the long page for the command and
    /// every visible descendant.
    All,
}

/// The page a help request becomes, by the route the words took.
///
/// The parser reports the command a request arrived at, but a page is about the route that
/// reached it: one `Subcommands` type mounted under two parents is one address, and a page
/// found by searching for that address carries the first mount's path and globals. Falls back
/// to rendering by address where the route cannot be rebuilt, which only a command from
/// another CLI's tables can reach.
///
/// One function rather than a shape each caller reassembles. `parse()` renders a request this
/// way, and so does anything that wants to know what a command line would have printed
/// without running the program — a test harness, most of all, since a page it renders
/// differently from the process is a page that proves nothing.
pub fn page(
    spec: &Spec<'_>,
    root: &Command<'_>,
    argv: &[&std::ffi::OsStr],
    cmd: &Command<'_>,
    page: Page,
    style: Style,
) -> Option<String> {
    match route_to(root, argv, cmd) {
        Some(route) => match page {
            Page::Short => render_at_styled(spec, &route, false, style),
            Page::Long => render_at_styled(spec, &route, true, style),
            Page::All => render_all_at_styled(spec, &route, style),
        },
        None => match page {
            Page::Short => render_styled(spec, cmd, false, style),
            Page::Long => render_styled(spec, cmd, true, style),
            Page::All => render_all_styled(spec, cmd, style),
        },
    }
}

/// The page a help request becomes when a declared executable view is what the user invoked.
///
/// `argv` includes argv0 here, as [`route_to_view`] requires: the view's own name is what
/// selected it. The fallback is the canonical page, which is better than nothing where the
/// route cannot be rebuilt.
pub fn page_view(
    spec: &Spec<'_>,
    root: &Command<'_>,
    argv: &[&std::ffi::OsStr],
    cmd: &Command<'_>,
    view: &ViewMeta<'_>,
    page: Page,
    style: Style,
) -> Option<String> {
    match route_to_view(root, argv, cmd, view) {
        Some(route) => match page {
            Page::Short => render_view_at_styled(spec, &route, view, false, style),
            Page::Long => render_view_at_styled(spec, &route, view, true, style),
            Page::All => render_all_view_at_styled(spec, &route, view, style),
        },
        None => match page {
            Page::Short => render_styled(spec, cmd, false, style),
            Page::Long => render_styled(spec, cmd, true, style),
            Page::All => render_all_styled(spec, cmd, style),
        },
    }
}

pub(crate) fn view_root_flags<'a>(
    spec: &'a Spec<'a>,
    promoted: &CommandMeta<'a>,
    view: &ViewMeta<'a>,
) -> Vec<FlagMeta<'a>> {
    let selected = |flag: &&FlagMeta<'a>| {
        let carried = crate::is_version_flag(flag.flag)
            || (flag.flag.global
                && (view.all_globals
                    || view.globals.iter().any(|selector| {
                        selector
                            .strip_prefix("--")
                            .is_some_and(|long| flag.flag.longs.contains(&long))
                            || selector
                                .strip_prefix('-')
                                .filter(|short| short.len() == 1)
                                .and_then(|short| short.as_bytes().first().copied())
                                .is_some_and(|short| flag.flag.shorts.contains(&short))
                    })));
        carried
            && !promoted
                .flags
                .iter()
                .any(|local| crate::spec::flag_forms_overlap(flag.flag, local.flag))
    };
    let mut flags: Vec<FlagMeta<'a>> = spec.root.flags.iter().filter(selected).copied().collect();
    flags.extend_from_slice(promoted.flags);
    flags
}

pub(crate) fn view_root_fields<'a>(
    spec: &'a Spec<'a>,
    promoted: &CommandMeta<'a>,
    view: &ViewMeta<'a>,
) -> (Vec<FlagMeta<'a>>, Vec<crate::spec::GroupMeta<'a>>) {
    let mut flags = view_root_flags(spec, promoted, view);
    let carried = flags.len().saturating_sub(promoted.flags.len());
    let matches = |flag: &FlagMeta<'_>, selector: &str| {
        selector
            .strip_prefix("--")
            .is_some_and(|long| flag.flag.longs.contains(&long))
            || selector
                .strip_prefix('-')
                .filter(|short| short.len() == 1)
                .and_then(|short| short.as_bytes().first().copied())
                .is_some_and(|short| flag.flag.shorts.contains(&short))
    };
    let mut groups = Vec::new();
    for group in spec.root.groups {
        let members: Vec<usize> = group
            .members
            .iter()
            .filter_map(|selector| {
                flags[..carried]
                    .iter()
                    .position(|flag| matches(flag, selector))
            })
            .collect();
        match members.as_slice() {
            [only] if group.required => flags[*only].required = true,
            [_, _, ..] => {
                // Help and diagnostics only need the relationship and its requiredness; the
                // parser continues to enforce the canonical group. Retaining the original
                // selector slice avoids allocating self-referential metadata on this cold path.
                groups.push(*group);
            }
            _ => {}
        }
    }
    groups.extend_from_slice(promoted.groups);
    (flags, groups)
}

fn recursive_help<'a>(
    spec: &'a Spec<'a>,
    path: Vec<&'a str>,
    chain: Vec<&'a CommandMeta<'a>>,
    style: Style,
    inherit_version_actions: bool,
) -> String {
    fn append<'a>(
        out: &mut String,
        spec: &'a Spec<'a>,
        path: &mut Vec<&'a str>,
        chain: &mut Vec<&'a CommandMeta<'a>>,
        style: Style,
        inherit_version_actions: bool,
    ) {
        if !out.is_empty() {
            out.push('\n');
        }
        let page = long_help_with(spec, path, chain, inherit_version_actions);
        let (headings, flag_usages, synopsis) =
            help_structure(spec, path, chain, true, inherit_version_actions);
        out.push_str(&styled_help(
            &page,
            style,
            &headings,
            &flag_usages,
            &synopsis,
        ));

        let current = *chain.last().expect("a recursive page has a command");
        let mut children: Vec<_> = current.subcommands.iter().filter(|cmd| !cmd.hide).collect();
        children.sort_unstable_by_key(|cmd| (cmd.display_order.unwrap_or(999), cmd.cmd.name));
        for child in children {
            path.push(child.cmd.name);
            chain.push(child);
            append(out, spec, path, chain, style, inherit_version_actions);
            chain.pop();
            path.pop();
        }
    }

    let mut out = String::new();
    let mut path = path;
    let mut chain = chain;
    append(
        &mut out,
        spec,
        &mut path,
        &mut chain,
        style,
        inherit_version_actions,
    );
    out
}

#[cfg(test)]
mod style_tests {
    use super::{
        commands_section, display_usage_masked, flag_notes, flag_usage, flat_commands_short,
        inline_environment_notes, long_help, render_view_at_styled, styled_flag_usage, styled_help,
        styled_inline, Shown, Style,
    };
    use crate::spec::{CommandMeta, FlagMeta, Spec, ViewMeta};
    use crate::{ArgAction, Command, Flag};

    #[test]
    fn optional_equals_values_put_the_equals_inside_the_brackets() {
        let flag = Flag {
            name: "color",
            longs: &["color"],
            require_equals: true,
            ..Flag::VALUE
        };
        let meta = FlagMeta {
            flag: &flag,
            value_name: Some("WHEN"),
            value_optional: true,
            ..FlagMeta::EMPTY
        };

        assert_eq!(flag_usage(&meta), "--color[=WHEN]");
    }

    #[test]
    fn a_negation_left_after_positive_spellings_are_masked_keeps_its_flag_name() {
        let flag = Flag {
            name: "color",
            shorts: b"c",
            longs: &["color"],
            negate: Some("no-color"),
            ..Flag::BOOL
        };
        let meta = FlagMeta {
            flag: &flag,
            ..FlagMeta::EMPTY
        };
        let shown = Shown {
            long: None,
            short: None,
            negate: true,
        };

        assert_eq!(display_usage_masked(&meta, &shown), "color: --no-color");
    }

    #[test]
    fn a_flag_spelled_only_as_its_negation_writes_that_spelling_and_nothing_before_it() {
        // clap's `SetFalse`, tak's `--no-credit`: the flag is *named* after its negation, so
        // the `name:` prefix would repeat the spelling and there is no positive form to join
        // it to. Both halves wrote nothing, and the join put a space at the front of the
        // column: `" --no-credit"`.
        let flag = Flag {
            name: "no-credit",
            negate: Some("no-credit"),
            ..Flag::BOOL
        };
        let meta = FlagMeta {
            flag: &flag,
            ..FlagMeta::EMPTY
        };
        let shown = Shown {
            long: None,
            short: None,
            negate: true,
        };

        assert_eq!(display_usage_masked(&meta, &shown), "--no-credit");
    }

    #[test]
    fn flattened_next_line_deprecation_follows_help_without_a_blank_row() {
        let flag = Flag {
            name: "old",
            longs: &["old"],
            ..Flag::BOOL
        };
        let flag_meta = FlagMeta {
            flag: &flag,
            help: Some("Use the old mode"),
            deprecated: Some("use --new"),
            ..FlagMeta::EMPTY
        };
        let sub_cmd = Command {
            name: "run",
            ..Command::EMPTY
        };
        let sub_meta = CommandMeta {
            cmd: &sub_cmd,
            flags: &[flag_meta],
            ..CommandMeta::EMPTY
        };
        let subcommands = [&sub_meta];
        let root_meta = CommandMeta {
            next_line_help: true,
            subcommands: &subcommands,
            ..CommandMeta::EMPTY
        };
        let mut page = String::new();

        flat_commands_short(&mut page, &["tool"], &root_meta, 80);

        assert!(
            page.contains("    Use the old mode\n    [deprecated: use --new]"),
            "{page}"
        );
        assert!(!page.contains("Use the old mode\n\n    [deprecated"));
    }

    #[test]
    fn flattened_next_line_flags_without_help_still_end_their_usage_rows() {
        let old = Flag {
            name: "old",
            longs: &["old"],
            ..Flag::BOOL
        };
        let new = Flag {
            name: "new",
            longs: &["new"],
            ..Flag::BOOL
        };
        let flags = [
            FlagMeta {
                flag: &old,
                deprecated: Some("use --new"),
                ..FlagMeta::EMPTY
            },
            FlagMeta {
                flag: &new,
                ..FlagMeta::EMPTY
            },
        ];
        let sub_cmd = Command {
            name: "run",
            ..Command::EMPTY
        };
        let sub_meta = CommandMeta {
            cmd: &sub_cmd,
            flags: &flags,
            ..CommandMeta::EMPTY
        };
        let subcommands = [&sub_meta];
        let root_meta = CommandMeta {
            next_line_help: true,
            subcommands: &subcommands,
            ..CommandMeta::EMPTY
        };
        let mut page = String::new();

        flat_commands_short(&mut page, &["tool"], &root_meta, 80);

        assert!(page.contains("--old\n"), "{page}");
        assert!(page.contains("[deprecated: use --new]\n"), "{page}");
        assert!(page.contains("--new\n"), "{page}");
        assert!(!page.contains("--old    [deprecated"), "{page}");
    }

    #[test]
    fn hidden_environment_names_include_fallbacks_and_deprecated_aliases() {
        let flag = Flag {
            name: "token",
            ..Flag::BOOL
        };
        let meta = FlagMeta {
            flag: &flag,
            hide_env: true,
            env_fallback: &["OLD_TOKEN"],
            deprecated_env: &["LEGACY_TOKEN"],
            ..FlagMeta::EMPTY
        };
        let mut page = String::new();

        flag_notes(&mut page, &meta, 4);

        assert!(page.is_empty());

        let visible = inline_environment_notes(false, &["OLD_TOKEN"], &["LEGACY_TOKEN"])
            .expect("visible environment notes");
        assert!(visible.contains("[env fallback: OLD_TOKEN]"));
        assert!(visible.contains("[deprecated env: LEGACY_TOKEN]"));
        assert!(inline_environment_notes(true, &["OLD_TOKEN"], &["LEGACY_TOKEN"]).is_none());
    }

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

        commands_section(&mut page, &[], &root_meta, 80, false);

        assert!(page.contains("  run   run it\n  help"));
        assert!(!page.contains("  run   run it\n\n  help"));
    }

    #[test]
    fn long_help_preserves_configured_spacing_before_package_metadata() {
        let command = Command {
            name: "ex",
            ..Command::EMPTY
        };
        let root = CommandMeta {
            cmd: &command,
            after_help: Some("More help.\n"),
            ..CommandMeta::EMPTY
        };
        let spec = Spec {
            name: "ex",
            author: Some("Example Author"),
            root: &root,
            ..Spec::EMPTY
        };

        let page = long_help(&spec, &["ex"], &[&root]);

        assert!(
            page.contains("More help.\n\n\nAuthor: Example Author\n"),
            "{page}"
        );
    }

    #[test]
    fn view_help_keeps_declared_and_synthesized_host_version_actions() {
        let build_info = Flag {
            name: "build-info",
            longs: &["build-info"],
            action: ArgAction::Version,
            ..Flag::BOOL
        };
        let nested_command = Command {
            name: "status",
            ..Command::EMPTY
        };
        let child_command = Command {
            name: "serve",
            subcommands: &[&nested_command],
            ..Command::EMPTY
        };
        let root_command = Command {
            name: "host",
            flags: &[&build_info],
            subcommands: &[&child_command],
            version: true,
            ..Command::EMPTY
        };
        let build_info_meta = FlagMeta {
            flag: &build_info,
            help: Some("Print build information"),
            ..FlagMeta::EMPTY
        };
        let nested_meta = CommandMeta {
            cmd: &nested_command,
            ..CommandMeta::EMPTY
        };
        let child_meta = CommandMeta {
            cmd: &child_command,
            subcommands: &[&nested_meta],
            ..CommandMeta::EMPTY
        };
        let root_meta = CommandMeta {
            cmd: &root_command,
            flags: &[build_info_meta],
            subcommands: &[&child_meta],
            ..CommandMeta::EMPTY
        };
        let spec = Spec {
            name: "host",
            bin: Some("host"),
            root: &root_meta,
            ..Spec::EMPTY
        };
        let view = ViewMeta {
            id: "server",
            name: "server",
            bin: "server",
            root: "serve",
            all_globals: false,
            globals: &[],
        };

        let page = render_view_at_styled(
            &spec,
            &[&root_command, &child_command, &nested_command],
            &view,
            false,
            Style::PLAIN,
        )
        .expect("view route");

        assert!(page.contains("--build-info"), "{page}");
        assert!(page.contains("-V, --version"), "{page}");
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

    #[test]
    fn equals_separates_a_coloured_flag_from_its_value() {
        assert_eq!(
            styled_flag_usage("--output=<FILE>", Style::COLOURED),
            "\u{1b}[36m--output\u{1b}[0m=<FILE>"
        );
        assert_eq!(
            styled_flag_usage("--color[=WHEN]", Style::COLOURED),
            "\u{1b}[36m--color\u{1b}[0m[=WHEN]"
        );
    }

    #[test]
    fn coloured_help_renders_inline_markdown_emphasis() {
        let page = "Use **force** for *all* files, _including_hidden_, `--literally`, and ~~never~~ this.\n  --dry_run  Keep snake_case and an unmatched * glob\n\nExamples:\n    $ echo `date`\n";
        let coloured = styled_help(page, Style::COLOURED, &[], &[], &[]);

        assert!(
            coloured.contains("\u{1b}[1mforce\u{1b}[22m"),
            "{coloured:?}"
        );
        assert!(coloured.contains("\u{1b}[3mall\u{1b}[23m"), "{coloured:?}");
        assert!(
            coloured.contains("\u{1b}[3mincluding_hidden\u{1b}[23m"),
            "{coloured:?}"
        );
        assert!(
            coloured.contains("\u{1b}[36m--literally\u{1b}[39m"),
            "{coloured:?}"
        );
        assert!(
            coloured.contains("\u{1b}[9mnever\u{1b}[29m"),
            "{coloured:?}"
        );
        assert!(coloured.contains("--dry_run  Keep snake_case and an unmatched * glob"));
        assert!(coloured.contains("    $ echo `date`"));
        assert!(!coloured.contains("**force**"));
    }

    #[test]
    fn inline_emphasis_nests_and_can_be_escaped() {
        assert_eq!(
            styled_inline("**bold and *italic*** plus \\*literal\\*", None),
            "\u{1b}[1mbold and \u{1b}[3mitalic\u{1b}[23m\u{1b}[1m\u{1b}[22m plus *literal*"
        );
        assert_eq!(
            styled_inline("*italic and **bold***", None),
            "\u{1b}[3mitalic and \u{1b}[1mbold\u{1b}[22m\u{1b}[3m\u{1b}[23m"
        );
        assert_eq!(
            styled_inline("_italic and __bold___", None),
            "\u{1b}[3mitalic and \u{1b}[1mbold\u{1b}[22m\u{1b}[3m\u{1b}[23m"
        );
    }

    #[test]
    fn intraword_underscore_runs_remain_literal() {
        assert_eq!(
            styled_inline("foo__bar__ foo___bar___ baz_qux", None),
            "foo__bar__ foo___bar___ baz_qux"
        );
    }

    #[test]
    fn an_escape_skips_one_closing_marker() {
        assert_eq!(
            styled_inline("*italic \\**", None),
            "\u{1b}[3mitalic *\u{1b}[23m"
        );
        assert_eq!(
            styled_inline("**bold \\***", None),
            "\u{1b}[1mbold *\u{1b}[22m"
        );
    }

    #[test]
    fn a_shared_delimiter_run_is_bold_and_italic() {
        assert_eq!(
            styled_inline("***combined***", None),
            "\u{1b}[1;3mcombined\u{1b}[22;23m"
        );
        assert_eq!(
            styled_inline("___combined___", None),
            "\u{1b}[1;3mcombined\u{1b}[22;23m"
        );
    }

    #[test]
    fn combined_emphasis_can_nest_in_single_emphasis() {
        assert_eq!(
            styled_inline("*italic ***combined*** tail*", None),
            "\u{1b}[3mitalic \u{1b}[1;3mcombined\u{1b}[22;23m\u{1b}[3m tail\u{1b}[23m"
        );
        assert_eq!(
            styled_inline("**bold ***combined*** tail**", None),
            "\u{1b}[1mbold \u{1b}[1;3mcombined\u{1b}[22;23m\u{1b}[1m tail\u{1b}[22m"
        );
    }

    #[test]
    fn a_closing_run_remainder_can_open_an_adjacent_span() {
        assert_eq!(
            styled_inline("*italic***bold**", None),
            "\u{1b}[3mitalic\u{1b}[23m\u{1b}[1mbold\u{1b}[22m"
        );
        assert_eq!(
            styled_inline("**bold***italic*", None),
            "\u{1b}[1mbold\u{1b}[22m\u{1b}[3mitalic\u{1b}[23m"
        );
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
#[cfg(test)]
#[test]
fn an_overflowing_entry_wraps_even_when_the_page_is_very_narrow() {
    let mut page = String::new();
    let width = 10;
    let col = usage_column_width("--long".chars().count(), width);

    entry(&mut page, "--long", Some("alpha beta"), col, width, false);

    assert_eq!(page, "  --long\n    alpha\n    beta\n");
}
