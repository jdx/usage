//! What a user reads when a command line does not parse.
//!
//! Held to clap's shape on purpose. mise's users read clap's errors today, and the help output on
//! either side of this is already byte-identical to usage-lib's — so the error text is the last
//! thing an adopter's users would notice changing, and the aim is that they do not.
//!
//! The skeleton, measured from clap 4 rather than remembered:
//!
//! ```text
//! error: unexpected argument '--fore' found
//!
//!   tip: a similar argument exists: '--force'
//!
//! Usage: mise use [OPTIONS] [TOOL@VERSION]…
//!
//! For more information, try '--help'.
//! ```
//!
//! Two deliberate departures. The usage line is *ours* — the same one `--help` prints, rendered
//! from the spec — because an error that disagrees with the help about how a command is spelled is
//! worse than one that disagrees with clap. And which errors carry a usage block follows clap:
//! the ones about the shape of the command line do, the ones about a single value do not.

use core::fmt::Write as _;

use crate::spec::{CommandMeta, Spec};
use crate::{Command, Error};

/// Whether to colour, and what with.
///
/// The codes are clap's, so that a terminal shows the same thing: bold red for `error:`, yellow
/// for the offending text, green for a suggestion and for what was expected, bold underline for
/// `Usage:`.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Style {
    coloured: bool,
}

impl Style {
    /// Plain text, for a pipe or a test.
    pub const PLAIN: Style = Style { coloured: false };
    /// Coloured, whatever the terminal is.
    pub const COLOURED: Style = Style { coloured: true };

    /// Colour when stderr is a terminal and the environment has not asked otherwise.
    ///
    /// `NO_COLOR` wins over everything, per the convention: a user who sets it has said once, for
    /// every program, that they do not want this. `CLICOLOR_FORCE` is the other direction, for a
    /// pipe that ends up somewhere that does render colour.
    pub fn auto() -> Style {
        use std::io::IsTerminal as _;
        let forced = std::env::var_os("CLICOLOR_FORCE").is_some_and(|v| v != "0");
        let refused = std::env::var_os("NO_COLOR").is_some_and(|v| !v.is_empty());
        if refused {
            return Style::PLAIN;
        }
        if forced || std::io::stderr().is_terminal() {
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

    /// `error:`, and anything else that is the failure itself.
    fn error(self, text: &str) -> String {
        self.wrap("1m\u{1b}[31", text)
    }

    /// What the user typed that did not work.
    fn invalid(self, text: &str) -> String {
        self.wrap("33", text)
    }

    /// What would have worked: a suggestion, a possible value, a missing argument.
    fn valid(self, text: &str) -> String {
        self.wrap("32", text)
    }

    /// A heading, such as `Usage:`.
    fn heading(self, text: &str) -> String {
        self.wrap("1m\u{1b}[4", text)
    }

    /// Something to be typed as it is written.
    fn literal(self, text: &str) -> String {
        self.wrap("1", text)
    }
}

/// A flag as the user named it, without a value they attached to it.
///
/// `--jobs=4` names `--jobs`; the parser splits on the `=` before looking the name up, so an
/// error about the whole token is about something nobody typed. Both halves of the message
/// depend on it: clap prints `'--fore'` for `--fore=1`, and it scores `fore` — with the value
/// left on, `fore=1` falls under the 0.7 bar and the tip disappears exactly where a mistyped
/// value-taking flag is most likely to be written.
///
/// Long flags only. A short cluster is refused whole, so `-xy` is not `-x` with something
/// attached, and `-j=4` is a value clap keeps.
fn flag_named(token: &str) -> &str {
    match token.strip_prefix("--") {
        Some(body) => match body.find('=') {
            Some(i) => &token[..i + 2],
            None => token,
        },
        None => token,
    }
}

/// Every long spelling a flag answers to, its negation included.
///
/// The parser takes `--no-color` through `find_negation`, and the completions offer it, so a
/// suggestion that leaves it out is the odd one — a near miss of a name that works gets silence.
/// clap has no separate notion of a negation, so the two forms are two arguments there and it
/// suggests either; matching that is the point.
fn long_spellings<'a>(meta: &'a crate::spec::FlagMeta<'a>) -> impl Iterator<Item = &'a str> {
    meta.flag.longs.iter().copied().chain(meta.flag.negate)
}

/// Every flag a word at this command could have named: its own, then any ancestor's globals.
///
/// The same set the parser would have accepted, which is what makes a suggestion one that works.
fn flags_in_scope<'a, 'c>(
    chain: &'c [&'a CommandMeta<'a>],
) -> impl Iterator<Item = &'a crate::spec::FlagMeta<'a>> + 'c {
    // The command's own flags, and from each ancestor only what it declared global — the rule
    // the parser follows on the way down. The chain and not the tree: an earlier version
    // collected globals from every branch it walked through, so a global declared on one command
    // was suggested under an unrelated one — a tip naming a flag the parser would refuse, which
    // is worse than no tip.
    let depth = chain.len();
    chain.iter().enumerate().flat_map(move |(i, meta)| {
        let own = i + 1 == depth;
        meta.flags
            .iter()
            .filter(move |f| !f.hide && (own || f.flag.global))
    })
}

/// How alike two words are, from 0 (nothing in common) to 1 (the same word).
///
/// Jaro, and deliberately not Jaro-Winkler. clap decides whether to suggest something with
/// `strsim::jaro`, and says why in its own source:
///
/// ```text
/// // GH #4660: using `jaro` because `jaro_winkler` implementation in `strsim-rs` is wrong
/// ```
///
/// Winkler's variant adds a bonus for a shared prefix, which sounds right for a mistyped flag and
/// would move the bar: some words clear 0.7 with it and not without, and the ranking changes too.
/// Since the point of this module is that an adopter's users see the same tips they saw under
/// clap, the algorithm has to be the same one. The bonus is three lines away if it is ever wanted
/// on both sides.
///
/// Written out rather than depended on, because this crate takes no dependencies.
fn jaro(a: &str, b: &str) -> f64 {
    let (a, b): (Vec<char>, Vec<char>) = (a.chars().collect(), b.chars().collect());
    if a.is_empty() && b.is_empty() {
        return 1.0;
    }
    if a.is_empty() || b.is_empty() {
        return 0.0;
    }

    // Two characters count as matching if they are the same and no further apart than this.
    let window = (a.len().max(b.len()) / 2).saturating_sub(1);
    let mut a_matched = vec![false; a.len()];
    let mut b_matched = vec![false; b.len()];
    let mut matches = 0usize;

    for (i, ch) in a.iter().enumerate() {
        let start = i.saturating_sub(window);
        let end = (i + window + 1).min(b.len());
        for j in start..end {
            if !b_matched[j] && b[j] == *ch {
                a_matched[i] = true;
                b_matched[j] = true;
                matches += 1;
                break;
            }
        }
    }
    if matches == 0 {
        return 0.0;
    }

    // Matching characters that arrive in a different order are half a transposition each.
    let mut transpositions = 0usize;
    let mut k = 0usize;
    for (i, matched) in a_matched.iter().enumerate() {
        if !matched {
            continue;
        }
        while !b_matched[k] {
            k += 1;
        }
        if a[i] != b[k] {
            transpositions += 1;
        }
        k += 1;
    }

    let matches = matches as f64;
    (matches / a.len() as f64
        + matches / b.len() as f64
        + (matches - transpositions as f64 / 2.0) / matches)
        / 3.0
}

/// Everything close enough to `typed` to be worth saying, in the order clap says them.
///
/// The threshold is clap's — a score above 0.7 — so the two suggest in the same cases. Below it a
/// suggestion is noise: offering `--quiet` for `--zzz` is worse than offering nothing, because a
/// user reads it as the CLI having understood them.
///
/// All of them, not the best one: clap lists every candidate over the bar, and `mise config lss`
/// really is close to both `ls` and `list`. Sorted *ascending* by score, which is what clap does —
/// so the closest match comes last. That reads oddly, and it is preserved here because the point
/// of this module is that an adopter's users see no change; it is a difference worth undoing on
/// both sides rather than on one.
fn nearest<'a>(typed: &str, candidates: impl Iterator<Item = &'a str>) -> Vec<&'a str> {
    let mut scored: Vec<(f64, &str)> = candidates
        .map(|candidate| (jaro(typed, candidate), candidate))
        .filter(|(score, _)| *score > 0.7)
        .collect();
    scored.sort_by(|a, b| a.0.total_cmp(&b.0).then_with(|| a.1.cmp(b.1)));
    scored.dedup_by(|a, b| a.1 == b.1);
    scored.into_iter().map(|(_, candidate)| candidate).collect()
}

/// A tip naming what was probably meant, or nothing when nothing was close.
///
/// `noun` is the singular — clap writes "a similar argument exists" for one and "some similar
/// arguments exist" for several, and the plural is the singular with an `s`.
fn tip(style: Style, noun: &str, near: &[&str]) -> String {
    match near {
        [] => String::new(),
        [one] => format!(
            "\n  {} a similar {noun} exists: '{}'\n",
            style.valid("tip:"),
            style.valid(one)
        ),
        many => {
            let listed: Vec<String> = many
                .iter()
                .map(|candidate| format!("'{}'", style.valid(candidate)))
                .collect();
            format!(
                "\n  {} some similar {noun}s exist: {}\n",
                style.valid("tip:"),
                listed.join(", ")
            )
        }
    }
}

/// A name as a usage line writes it: `<TOOL>`, `[TOOL]…`, `--jobs`.
///
/// The error carries the spec's name for a thing; a user reads the form the help shows. Both come
/// from `help` rather than being decided again here — an error and the page above it describing
/// one argument differently is the confusing kind of inconsistency, and rewriting the rule is how
/// that happens. A flag is spelled with its dashes, which is the whole of what `--jobs` versus
/// `jobs` is about.
fn shown<'a>(meta: Option<&'a CommandMeta<'a>>, name: &str) -> String {
    let Some(meta) = meta else {
        return name.to_string();
    };
    if let Some(arg) = meta.args.iter().find(|a| a.arg.name == name) {
        return crate::help::arg_usage(arg);
    }
    // A flag can be named by an error too — a missing required one, or a value that is not among
    // its choices — and the spec's name for it has no dashes.
    if let Some(flag) = meta
        .flags
        .iter()
        .find(|f| f.flag.name == name || f.value_name == Some(name))
    {
        return crate::help::flag_spelling(flag);
    }
    name.to_string()
}

/// A group member is stored as a selector (`--file` or `-f`), not a field name.
fn group_member_shown(meta: Option<&CommandMeta<'_>>, selector: &str) -> String {
    let Some(meta) = meta else {
        return selector.to_string();
    };
    let found = meta.flags.iter().find(|flag| {
        flag.flag
            .longs
            .iter()
            .any(|long| selector == format!("--{long}"))
            || flag
                .flag
                .shorts
                .iter()
                .any(|short| selector == format!("-{}", *short as char))
            || flag
                .flag
                .negate
                .is_some_and(|negate| selector == format!("--{negate}"))
    });
    found
        .map(|flag| {
            let mut shown = crate::help::flag_spelling(flag);
            if flag.flag.takes_value {
                let name = flag.value_name.unwrap_or(flag.flag.name);
                let _ = write!(shown, " <{name}>");
                if flag.flag.variadic {
                    shown.push('…');
                }
            }
            shown
        })
        .unwrap_or_else(|| selector.to_string())
}

/// The word that was bound to a named argument, recovered from argv.
///
/// The parse itself does not carry it: an error that owned the offending text would allocate on
/// the one path this crate promises not to, so [`Error::InvalidChoice`] names the argument and
/// stops. Recovering it here is what that promise assumes — the diagnostics are a layer that may
/// do the work, and by the time one is being written the parse has already failed.
fn value_bound_to(
    root: &Command<'_>,
    argv: &[&std::ffi::OsStr],
    name: &str,
    refused: &[&str],
) -> Option<String> {
    let mut parser = crate::Parser::new(root, argv);
    let mut last = None;
    while let Some(event) = parser.next_event() {
        let value = match event {
            Ok(crate::Event::Arg { arg, value, .. }) if arg.name == name => value,
            Ok(crate::Event::Flag {
                flag,
                value: Some(value),
                ..
            }) if flag.name == name => value,
            Ok(_) => continue,
            Err(_) => break,
        };
        let value = String::from_utf8_lossy(value).into_owned();
        // The *offending* one, not the last. A repeatable flag or a variadic argument may be
        // given several values, and the check refuses the first that is not allowed — reporting
        // whichever came last would name a value that is perfectly good and leave the wrong one
        // unmentioned.
        if !refused.is_empty() && !refused.contains(&value.as_str()) {
            return Some(value);
        }
        last = Some(value);
    }
    last
}

/// The commands the words went through, root first, ending at the one an error is about.
///
/// Walked rather than carried on the error: only some variants know their command, and a caller
/// that has just been handed an error has the argv it came from. The walk stops where the parse
/// stopped, which is the command whose usage line belongs in the message.
///
/// The whole path and not just its end, because the end does not identify itself. One
/// `Subcommands` type mounted under two parents is one `Command` in both — the same address —
/// so a search of the metadata tree for that address finds whichever mount comes first. That is
/// how `ex beta shared --betaglobl` came back describing `ex alpha shared`, suggesting alpha's
/// globals and not beta's. The route is the only thing that tells the two apart, so the route is
/// what gets carried.
fn path_taken<'t>(root: &'t Command<'t>, argv: &[&std::ffi::OsStr]) -> Vec<&'t Command<'t>> {
    let mut path = vec![root];
    let mut parser = crate::Parser::new(root, argv);
    while let Some(event) = parser.next_event() {
        match event {
            Ok(crate::Event::Command(cmd)) => path.push(cmd),
            Ok(_) => {}
            Err(_) => break,
        }
    }
    path
}

/// The metadata for each command along a path, and the path as a user would type it.
///
/// Each step is matched among *that command's* children, which is what makes it unambiguous:
/// two mounts of one `Subcommands` type share an address, but a parent's own child list is its
/// own. Returns nothing if the path leaves this spec, which cannot happen for a path this module
/// produced and is not worth a panic if it ever does.
fn resolve<'a>(
    spec: &'a Spec<'a>,
    path: &[&Command<'_>],
) -> Option<(Vec<&'a str>, Vec<&'a CommandMeta<'a>>)> {
    let mut names = vec![spec.bin.unwrap_or(spec.name)];
    let mut chain = vec![spec.root];
    for cmd in path.iter().skip(1) {
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

/// Render `error` the way a user should read it.
///
/// `argv` is what was being parsed, which is how the message finds the command to show a usage
/// line for. [`Error::Help`] and [`Error::Version`] render as nothing: neither is a failure, and a
/// caller that has not
/// handled it before reaching here has a bug this cannot paper over.
pub fn render(
    spec: &Spec<'_>,
    argv: &[&std::ffi::OsStr],
    error: &Error<'_, '_>,
    style: Style,
) -> String {
    let taken = path_taken(spec.root.cmd, argv);
    let cmd = *taken.last().expect("the root is always on the path");
    let resolved = resolve(spec, &taken);
    let chain: &[&CommandMeta<'_>] = resolved.as_ref().map(|(_, c)| &c[..]).unwrap_or(&[]);
    let here = chain.last().copied();
    let path = resolved
        .as_ref()
        .map(|(names, _)| names.join(" "))
        .unwrap_or_else(|| spec.bin.unwrap_or(spec.name).to_string());
    let usage = match (&resolved, here) {
        (Some((names, _)), Some(meta)) => crate::help::usage_line(names, meta),
        _ => path.clone(),
    };

    let mut out = String::new();
    let mut with_usage = false;

    match error {
        // The shape of the command line: clap shows a usage block for these.
        Error::UnknownFlag { token } => {
            with_usage = true;
            let whole = String::from_utf8_lossy(token);
            let typed = flag_named(&whole);
            let _ = writeln!(
                out,
                "{} unexpected argument '{}' found",
                style.error("error:"),
                style.invalid(typed)
            );
            // Scored without the dashes, and only then written back with them. Every flag
            // starts `--`, and the prefix bonus in Jaro-Winkler counts that agreement — so
            // `--fore` came out similar to `--quiet`, which it is not. clap compares the bare
            // names for the same reason.
            let bare = typed.trim_start_matches('-');
            let names: Vec<&str> = flags_in_scope(chain).flat_map(long_spellings).collect();
            let near: Vec<String> = nearest(bare, names.into_iter())
                .into_iter()
                .map(|name| format!("--{name}"))
                .collect();
            out.push_str(&tip(
                style,
                "argument",
                &near.iter().map(String::as_str).collect::<Vec<_>>(),
            ));
        }
        Error::UnexpectedArg { token } => {
            with_usage = true;
            let word = String::from_utf8_lossy(token);
            // What the word *looks like* decides, before anything about the command does. A
            // dash-prefixed token is a flag the user got wrong — telling them `--forc` is an
            // unrecognized subcommand is answering a question they did not ask, and it happens
            // on exactly the commands where the mistake is easiest to make: the ones with
            // subcommands, where a bare word would have been one.
            if word.starts_with('-') && word != "-" {
                // Same rule as a refused flag: a value attached with `=` is not part of the
                // name, and the word reaches here by the same spelling mistake.
                let named = flag_named(&word);
                let _ = writeln!(
                    out,
                    "{} unexpected argument '{}' found",
                    style.error("error:"),
                    style.invalid(named)
                );
                let bare = named.trim_start_matches('-');
                let names: Vec<&str> = flags_in_scope(chain).flat_map(long_spellings).collect();
                let near: Vec<String> = nearest(bare, names.into_iter())
                    .into_iter()
                    .map(|name| format!("--{name}"))
                    .collect();
                out.push_str(&tip(
                    style,
                    "argument",
                    &near.iter().map(String::as_str).collect::<Vec<_>>(),
                ));
            } else if cmd.subcommands.is_empty() {
                let _ = writeln!(
                    out,
                    "{} unexpected argument '{}' found",
                    style.error("error:"),
                    style.invalid(&word)
                );
            } else {
                let _ = writeln!(
                    out,
                    "{} unrecognized subcommand '{}'",
                    style.error("error:"),
                    style.invalid(&word)
                );
                // Every name a subcommand answers to, hidden ones included: a user who typed a
                // near miss of an old alias should be told the name it still works under.
                let names: Vec<&str> = cmd
                    .subcommands
                    .iter()
                    .flat_map(|sub| core::iter::once(sub.name).chain(sub.aliases.iter().copied()))
                    .collect();
                out.push_str(&tip(
                    style,
                    "subcommand",
                    &nearest(&word, names.into_iter()),
                ));
            }
        }
        Error::SubcommandConflict { subcommand } => {
            with_usage = true;
            let _ = writeln!(
                out,
                "{} the subcommand '{}' cannot be used with arguments on its parent command",
                style.error("error:"),
                style.invalid(subcommand.name)
            );
        }
        Error::MissingRequired { name } => {
            with_usage = true;
            let _ = writeln!(
                out,
                "{} the following required arguments were not provided:",
                style.error("error:")
            );
            let _ = writeln!(out, "  {}", style.valid(&shown(here, name)));
        }
        Error::DuplicateFlag { name } => {
            with_usage = true;
            let _ = writeln!(
                out,
                "{} the argument '{}' cannot be used multiple times",
                style.error("error:"),
                style.invalid(&shown(here, name))
            );
        }
        Error::MissingSubcommand => {
            // A bare command that can do nothing on its own is a request for orientation.
            // clap prints the command's help page here, including the available subcommands,
            // while keeping exit 2; an error plus only `<SUBCOMMAND>` tells the reader what is
            // missing and withholds the list they need to fix it.
            let help_style = if style == Style::COLOURED {
                crate::help::Style::COLOURED
            } else {
                crate::help::Style::PLAIN
            };
            if let Some(help) = crate::help::render_at_styled(spec, &taken, false, help_style) {
                return help;
            }
            with_usage = true;
            let _ = writeln!(
                out,
                "{} '{}' requires a subcommand but one was not provided",
                style.error("error:"),
                style.invalid(&path)
            );
        }

        // About one value: clap shows no usage block, on the grounds that the shape was right.
        Error::MissingFlagValue { flag } => {
            let name = flag
                .longs
                .first()
                .map(|l| format!("--{l}"))
                .or_else(|| flag.shorts.first().map(|s| format!("-{}", *s as char)))
                .unwrap_or_else(|| flag.name.to_string());
            let value = here
                .and_then(|meta| {
                    meta.flags
                        .iter()
                        .find(|m| core::ptr::eq(m.flag, *flag))
                        .and_then(|m| m.value_name)
                })
                .map(|v| format!(" <{v}>"))
                .unwrap_or_default();
            let _ = writeln!(
                out,
                "{} a value is required for '{}' but none was supplied",
                style.error("error:"),
                style.invalid(&format!("{name}{value}"))
            );
        }
        Error::InvalidChoice { name, choices } => {
            let shown_name = shown(here, name);
            match value_bound_to(spec.root.cmd, argv, name, choices) {
                Some(value) => {
                    let _ = writeln!(
                        out,
                        "{} invalid value '{}' for '{}'",
                        style.error("error:"),
                        style.invalid(&value),
                        style.literal(&shown_name)
                    );
                }
                // Nothing in argv bound to it, which means the value came from somewhere else —
                // an environment variable, or a default the spec declared.
                None => {
                    let _ = writeln!(
                        out,
                        "{} invalid value for '{}'",
                        style.error("error:"),
                        style.literal(&shown_name)
                    );
                }
            }
            let listed: Vec<String> = choices.iter().map(|c| style.valid(c)).collect();
            let _ = writeln!(out, "  [possible values: {}]", listed.join(", "));
            if let Some(typed) = value_bound_to(spec.root.cmd, argv, name, choices) {
                out.push_str(&tip(
                    style,
                    "value",
                    &nearest(&typed, choices.iter().copied()),
                ));
            }
        }
        Error::InvalidValue(invalid) => {
            let _ = writeln!(
                out,
                "{} invalid value '{}' for '{}': {}",
                style.error("error:"),
                style.invalid(&invalid.value),
                style.literal(&shown(here, invalid.name)),
                invalid.reason
            );
        }
        Error::MissingGroup { group, members } => {
            with_usage = true;
            // clap's own shape for a required group, which is the required-arguments
            // message with the members listed under it. The group's name goes on the
            // first line rather than into the list, since it is not something to type.
            let _ = writeln!(
                out,
                "{} one of the following required arguments was not provided ({group}):",
                style.error("error:")
            );
            for member in *members {
                let _ = writeln!(out, "  {}", style.valid(&group_member_shown(here, member)));
            }
        }
        Error::ConflictingFlags { name, other } => {
            // Spelled by `help`, like every other name in this module — and like clap, which
            // writes `the argument '--force' cannot be used with '--jobs <JOBS>'`.
            let _ = writeln!(
                out,
                "{} the argument '{}' cannot be used with '{}'",
                style.error("error:"),
                style.invalid(&shown(here, name)),
                style.invalid(&shown(here, other))
            );
            with_usage = true;
        }
        Error::VarTooFew { name, min, got } => {
            let _ = writeln!(
                out,
                "{} {min} values required for '{}' but {got} were provided",
                style.error("error:"),
                style.literal(&shown(here, name))
            );
        }
        Error::VarTooMany { name, max, got } => {
            let _ = writeln!(
                out,
                "{} {max} values allowed for '{}' but {got} were provided",
                style.error("error:"),
                style.literal(&shown(here, name))
            );
        }
        Error::ArgRequiresDoubleDash { arg } => {
            with_usage = true;
            let _ = writeln!(
                out,
                "{} '{}' can only be given after '{}'",
                style.error("error:"),
                style.literal(&shown(here, arg.name)),
                style.literal("--")
            );
        }
        Error::TooDeep => {
            let _ = writeln!(
                out,
                "{} this command line nests deeper than the parser goes",
                style.error("error:")
            );
        }
        // Not a failure. A caller reaching here with one has skipped handling it, and inventing a
        // message would hide that rather than help.
        // Neither is a failure, and a caller that has not handled them before reaching here
        // has a bug this cannot paper over.
        Error::Help { .. } | Error::Version { .. } => return String::new(),
    }

    if with_usage {
        let _ = writeln!(
            out,
            "\n{} {}",
            style.heading("Usage:"),
            style.literal(&usage)
        );
    }
    let _ = writeln!(
        out,
        "\nFor more information, try '{}'.",
        style.literal("--help")
    );
    out
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::spec::{ArgMeta, FlagMeta};
    use crate::{Arg, Flag};

    static FORCE: Flag = Flag {
        key: 1,
        name: "force",
        longs: &["force"],
        shorts: b"f",
        // A negation, because a flag's spellings are not only its `longs` and the parser takes
        // this one — so a suggestion that cannot offer it is offering less than the CLI accepts.
        negate: Some("no-force"),
        ..Flag::BOOL
    };
    static JOBS: Flag = Flag {
        key: 2,
        name: "jobs",
        longs: &["jobs"],
        ..Flag::VALUE
    };
    static TOOL: Arg = Arg {
        key: 3,
        name: "TOOL",
        ..Arg::REQUIRED
    };
    /// Variadic and choice-bearing, so "which value was refused" has a wrong answer available.
    static SHELLS: Arg = Arg {
        key: 7,
        name: "SHELLS",
        ..Arg::VAR
    };
    static USE: Command = Command {
        name: "use",
        flags: &[&FORCE, &JOBS],
        args: &[&TOOL, &SHELLS],
        ..Command::EMPTY
    };
    static QUIET: Flag = Flag {
        key: 4,
        name: "quiet",
        longs: &["quiet"],
        global: true,
        ..Flag::BOOL
    };
    /// A second command close to the same typo, so the plural wording is reachable.
    static LOCAL: Flag = Flag {
        key: 5,
        name: "local",
        longs: &["local"],
        global: true,
        ..Flag::BOOL
    };
    static USER: Command = Command {
        name: "user",
        flags: &[&LOCAL],
        ..Command::EMPTY
    };
    /// Declared on the root and *not* global, so it belongs to the root alone.
    static SETUP: Flag = Flag {
        key: 6,
        name: "setup",
        longs: &["setup"],
        ..Flag::BOOL
    };
    static ROOT: Command = Command {
        name: "ex",
        flags: &[&QUIET, &SETUP],
        // `user` first, so the walk to `use` passes through it: a sibling that is visited on the
        // way is exactly what leaked into scope before.
        subcommands: &[&USER, &USE],
        ..Command::EMPTY
    };
    static USE_META: CommandMeta = CommandMeta {
        cmd: &USE,
        about: Some("Use a tool"),
        flags: &[
            FlagMeta {
                flag: &FORCE,
                help: Some("Force it"),
                ..FlagMeta::EMPTY
            },
            FlagMeta {
                flag: &JOBS,
                help: Some("How many"),
                value_name: Some("JOBS"),
                ..FlagMeta::EMPTY
            },
        ],
        args: &[
            ArgMeta {
                arg: &TOOL,
                help: Some("Which tool"),
                required: true,
                ..ArgMeta::EMPTY
            },
            ArgMeta {
                arg: &SHELLS,
                help: Some("Which shells"),
                choices: &["bash", "zsh"],
                required: false,
                ..ArgMeta::EMPTY
            },
        ],
        ..CommandMeta::EMPTY
    };
    static USER_META: CommandMeta = CommandMeta {
        cmd: &USER,
        about: Some("Manage users"),
        flags: &[FlagMeta {
            flag: &LOCAL,
            help: Some("Only this checkout"),
            ..FlagMeta::EMPTY
        }],
        ..CommandMeta::EMPTY
    };
    static ROOT_META: CommandMeta = CommandMeta {
        cmd: &ROOT,
        flags: &[
            FlagMeta {
                flag: &QUIET,
                help: Some("Say less"),
                ..FlagMeta::EMPTY
            },
            FlagMeta {
                flag: &SETUP,
                help: Some("Set things up"),
                ..FlagMeta::EMPTY
            },
        ],
        subcommands: &[&USER_META, &USE_META],
        ..CommandMeta::EMPTY
    };
    static SPEC: Spec = Spec {
        name: "ex",
        bin: Some("ex"),
        root: &ROOT_META,
        ..Spec::EMPTY
    };

    fn rendered(words: &[&str], error: Error<'static, 'static>) -> String {
        let owned: Vec<std::ffi::OsString> = words.iter().map(std::ffi::OsString::from).collect();
        let argv: Vec<&std::ffi::OsStr> = owned.iter().map(|o| o.as_os_str()).collect();
        render(&SPEC, &argv, &error, Style::PLAIN)
    }

    #[test]
    fn the_usage_line_is_the_one_the_help_prints() {
        // Not clap's, which spells a usage line its own way. An error that disagrees with the
        // help about how a command is written is worse than one that disagrees with clap.
        let message = rendered(&["use"], Error::UnknownFlag { token: b"--fore" });
        let line = message
            .lines()
            .find_map(|l| l.strip_prefix("Usage: "))
            .expect("a usage line");
        assert_eq!(line, crate::help::usage_line(&["ex", "use"], &USE_META));
    }

    #[test]
    fn a_missing_group_lists_value_taking_members_completely() {
        static FILE: Flag = Flag {
            name: "file",
            longs: &["file"],
            takes_value: true,
            ..Flag::BOOL
        };
        static ROOT: Command = Command {
            name: "grouped",
            flags: &[&FILE],
            ..Command::EMPTY
        };
        static META: CommandMeta = CommandMeta {
            cmd: &ROOT,
            flags: &[FlagMeta {
                flag: &FILE,
                value_name: Some("PATH"),
                ..FlagMeta::EMPTY
            }],
            ..CommandMeta::EMPTY
        };
        static SPEC: Spec = Spec {
            name: "grouped",
            bin: Some("grouped"),
            root: &META,
            ..Spec::EMPTY
        };
        let message = render(
            &SPEC,
            &[],
            &Error::MissingGroup {
                group: "input",
                members: &["--file"],
            },
            Style::PLAIN,
        );
        assert!(message.contains("  --file <PATH>"), "{message}");
    }

    #[test]
    fn a_missing_subcommand_prints_the_choices() {
        let message = rendered(&[], Error::MissingSubcommand);
        assert!(message.contains("Commands:"), "{message}");
        assert!(message.contains("use"), "{message}");
        assert!(message.contains("user"), "{message}");
        assert!(!message.contains("requires a subcommand"), "{message}");

        let coloured = render(&SPEC, &[], &Error::MissingSubcommand, Style::COLOURED);
        assert!(
            coloured.contains("\u{1b}[1;4;32mCommands:\u{1b}[0m"),
            "{coloured}"
        );
    }

    #[test]
    fn a_missing_value_names_what_it_wanted() {
        // No usage block: the shape of the command line was right, one value was missing — which
        // is the distinction clap draws too.
        assert_eq!(
            rendered(&["use"], Error::MissingFlagValue { flag: &JOBS }),
            "error: a value is required for '--jobs <JOBS>' but none was supplied\n\
             \n\
             For more information, try '--help'.\n"
        );
    }

    #[test]
    fn a_word_where_a_subcommand_was_expected_says_so() {
        // The root has subcommands, so an unexpected word there is an unrecognized subcommand;
        // inside `use`, which has none, the same error is an unexpected argument.
        let at_root = rendered(&[], Error::UnexpectedArg { token: b"nonesuch" });
        assert!(
            at_root.starts_with("error: unrecognized subcommand 'nonesuch'"),
            "{at_root}"
        );
        let in_use = rendered(&["use"], Error::UnexpectedArg { token: b"extra" });
        assert!(
            in_use.starts_with("error: unexpected argument 'extra' found"),
            "{in_use}"
        );
    }

    #[test]
    fn a_required_argument_is_listed_the_way_clap_lists_it() {
        assert_eq!(
            rendered(&["use"], Error::MissingRequired { name: "<TOOL>" }),
            "error: the following required arguments were not provided:\n  \
             <TOOL>\n\
             \n\
             Usage: ex use [-f --force] [--jobs <JOBS>] <TOOL> [SHELLS]…\n\
             \n\
             For more information, try '--help'.\n"
        );
    }

    #[test]
    fn colour_is_the_same_codes_clap_uses() {
        // Measured from clap 4 rather than remembered: bold red for `error:`, yellow for what was
        // typed, bold underline for `Usage:`, bold for what to type.
        let owned = [std::ffi::OsString::from("use")];
        let argv: Vec<&std::ffi::OsStr> = owned.iter().map(|o| o.as_os_str()).collect();
        let message = render(
            &SPEC,
            &argv,
            &Error::UnknownFlag { token: b"--fore" },
            Style::COLOURED,
        );
        assert!(
            message.starts_with("\u{1b}[1m\u{1b}[31merror:\u{1b}[0m"),
            "{message:?}"
        );
        assert!(message.contains("\u{1b}[33m--fore\u{1b}[0m"), "{message:?}");
        assert!(
            message.contains("\u{1b}[1m\u{1b}[4mUsage:\u{1b}[0m"),
            "{message:?}"
        );
        // And nothing at all when plain, which is what a pipe or a test gets.
        assert!(!rendered(&["use"], Error::UnknownFlag { token: b"--fore" }).contains('\u{1b}'));
    }

    #[test]
    fn a_help_request_renders_nothing() {
        // It is not a failure, and a caller that reaches here with one has skipped handling it —
        // which a message would hide rather than help.
        assert_eq!(
            rendered(
                &["use"],
                Error::Help {
                    cmd: &USE,
                    long: true
                }
            ),
            ""
        );
    }
    #[test]
    fn a_name_is_spelled_the_way_the_help_spells_it() {
        // One rule, taken from `help` rather than decided again here: an error and the page above
        // it describing the same argument differently is the confusing kind of inconsistency.
        let message = rendered(&["use"], Error::MissingRequired { name: "TOOL" });
        assert!(message.contains("  <TOOL>"), "{message}");

        // A variadic keeps its ellipsis, exactly as the usage line writes it.
        let message = rendered(
            &["use"],
            Error::VarTooFew {
                name: "SHELLS",
                min: 2,
                got: 1,
            },
        );
        assert!(message.contains("'[SHELLS]…'"), "{message}");

        // And a *flag* is spelled with its dashes: the spec calls it `jobs`, a user reads
        // `--jobs`.
        let message = rendered(&["use"], Error::MissingRequired { name: "jobs" });
        assert!(message.contains("  --jobs"), "{message}");

        let message = rendered(&["use"], Error::DuplicateFlag { name: "jobs" });
        assert!(
            message.contains("the argument '--jobs' cannot be used multiple times"),
            "{message}"
        );

        // Every variant, not most of them. These two printed the spec's name while the ones
        // directly above and below them did not, so one argument could appear two ways in two
        // messages from the same command — and clap writes the dashes here too:
        //
        //     error: the argument '--force' cannot be used with '--jobs <JOBS>'
        let message = rendered(
            &["use"],
            Error::ConflictingFlags {
                name: "force",
                other: "jobs",
            },
        );
        assert!(
            message.contains("the argument '--force' cannot be used with '--jobs'"),
            "{message}"
        );

        let message = rendered(&["use"], Error::ArgRequiresDoubleDash { arg: &SHELLS });
        assert!(
            message.contains("'[SHELLS]…' can only be given"),
            "{message}"
        );
    }

    #[test]
    fn the_value_named_is_the_one_that_was_refused() {
        // A variadic given several values: the check refuses the first that is not allowed, so
        // naming whichever came last would name a value that is perfectly good and leave the
        // wrong one unmentioned.
        let owned = [
            std::ffi::OsString::from("use"),
            std::ffi::OsString::from("node"),
            std::ffi::OsString::from("fsh"),
            std::ffi::OsString::from("zsh"),
        ];
        let argv: Vec<&std::ffi::OsStr> = owned.iter().map(|o| o.as_os_str()).collect();
        let message = render(
            &SPEC,
            &argv,
            &Error::InvalidChoice {
                name: "SHELLS",
                choices: &["bash", "zsh"],
            },
            Style::PLAIN,
        );
        // The first line names the value, so assert on that line alone: the tip below it lists
        // every choice, `zsh` among them, and a whole-message search cannot tell the two apart.
        assert!(
            message.starts_with("error: invalid value 'fsh' for '[SHELLS]…'"),
            "named a value that was fine: {message}"
        );
    }

    #[test]
    fn a_near_miss_is_suggested_the_way_clap_suggests_one() {
        let message = rendered(&["use"], Error::UnknownFlag { token: b"--fore" });
        assert_eq!(
            message,
            "error: unexpected argument '--fore' found\n\
             \n\
             \x20 tip: a similar argument exists: '--force'\n\
             \n\
             Usage: ex use [-f --force] [--jobs <JOBS>] <TOOL> [SHELLS]…\n\
             \n\
             For more information, try '--help'.\n"
        );
    }

    #[test]
    fn a_value_attached_to_a_flag_is_not_part_of_its_name() {
        // `--fore=1`. The parser splits on the `=` before looking the name up, so the flag the
        // user named is `--fore` and an error about `--fore=1` is about something nobody typed.
        //
        // Both halves matter, and clap 4 was run to check both rather than remembered:
        //
        //     error: unexpected argument '--fore' found
        //       tip: a similar argument exists: '--force'
        //
        // The tip is the half that would have gone quietly: `fore=1` against `force` falls under
        // the 0.7 bar, so leaving the value on loses the suggestion exactly where a mistyped
        // value-taking flag is most likely to be written.
        assert_eq!(
            rendered(&["use"], Error::UnknownFlag { token: b"--fore=1" }),
            "error: unexpected argument '--fore' found\n\
             \n\
             \x20 tip: a similar argument exists: '--force'\n\
             \n\
             Usage: ex use [-f --force] [--jobs <JOBS>] <TOOL> [SHELLS]…\n\
             \n\
             For more information, try '--help'.\n"
        );

        // A short cluster is refused whole — `-xy` is not `-x` with a `y` attached — and clap
        // keeps the `=` in a short flag's value, so the rule is for long flags only.
        let message = rendered(&["use"], Error::UnknownFlag { token: b"-j=4" });
        assert!(
            message.starts_with("error: unexpected argument '-j=4' found"),
            "{message}"
        );
    }

    #[test]
    fn a_negation_is_suggested_like_any_other_spelling() {
        // `--no-force` is a name the parser accepts, through `find_negation`, and one the
        // completions already offer. Scoring only `longs` left it out, so a near miss of a name
        // that works got silence — and clap, which has no separate notion of a negation and
        // sees two arguments, suggests it. Measured:
        //
        //     error: unexpected argument '--no-colr' found
        //       tip: a similar argument exists: '--no-color'
        let message = rendered(
            &["use"],
            Error::UnknownFlag {
                token: b"--no-forc",
            },
        );
        assert!(
            message.contains("tip: a similar argument exists: '--no-force'"),
            "{message}"
        );

        // And the plain form is still found, which is the half that already worked.
        let message = rendered(&["use"], Error::UnknownFlag { token: b"--fore" });
        assert!(
            message.contains("tip: a similar argument exists: '--force'"),
            "{message}"
        );
    }

    #[test]
    fn nothing_is_suggested_when_nothing_is_close() {
        // Offering `--force` for `--zzz` is worse than offering nothing: a user reads a tip as
        // the CLI having understood them. clap's threshold, so clap's silence — and the rest of
        // the message is the same either way.
        assert_eq!(
            rendered(&["use"], Error::UnknownFlag { token: b"--zzz" }),
            "error: unexpected argument '--zzz' found\n\
             \n\
             Usage: ex use [-f --force] [--jobs <JOBS>] <TOOL> [SHELLS]…\n\
             \n\
             For more information, try '--help'.\n"
        );
    }

    #[test]
    fn the_scores_are_the_ones_clap_would_compute() {
        // Spot values for the algorithm itself, so a rewrite cannot quietly change which words
        // count as similar. `fore` against `force` is the ordinary case: five of six characters,
        // in order.
        assert!(
            (jaro("fore", "force") - 0.933).abs() < 0.001,
            "{}",
            jaro("fore", "force")
        );
        assert_eq!(jaro("same", "same"), 1.0);
        assert_eq!(jaro("", ""), 1.0);
        assert_eq!(jaro("abc", ""), 0.0);
        // No characters in common at all.
        assert_eq!(jaro("abc", "xyz"), 0.0);

        // And *no* prefix bonus, which is the whole difference from Jaro-Winkler: Jaro counts
        // matching characters and their order, not where the agreement falls, so dropping a
        // word's last letter and dropping its first score alike. Under Winkler the first would
        // win, and a different set of words would clear the bar than clap's.
        assert_eq!(jaro("forc", "force"), jaro("orce", "force"));
    }

    #[test]
    fn a_subcommand_and_a_value_get_the_same_treatment() {
        // Two are close, so the plural — and in clap's order, which is ascending by score, so
        // the *closest* comes last. That reads oddly and is what clap does.
        let message = rendered(&[], Error::UnexpectedArg { token: b"usse" });
        assert!(
            message.contains("tip: some similar subcommands exist: 'user', 'use'"),
            "{message}"
        );

        // The singular is covered by the flag and value cases in this module, which name one
        // each — here both commands begin `us`, so anything close to one is close to both.

        let owned = [
            std::ffi::OsString::from("use"),
            std::ffi::OsString::from("nod"),
        ];
        let argv: Vec<&std::ffi::OsStr> = owned.iter().map(|o| o.as_os_str()).collect();
        let message = render(
            &SPEC,
            &argv,
            &Error::InvalidChoice {
                name: "TOOL",
                choices: &["node", "python"],
            },
            Style::PLAIN,
        );
        assert!(
            message.contains("invalid value 'nod' for '<TOOL>'"),
            "{message}"
        );
        assert!(
            message.contains("tip: a similar value exists: 'node'"),
            "{message}"
        );
    }

    #[test]
    fn a_global_flag_is_suggested_inside_a_subcommand() {
        // What the parser would have accepted there is what should be suggested there — the same
        // rule the completions follow, for the same reason.
        let message = rendered(&["use"], Error::UnknownFlag { token: b"--quie" });
        assert!(
            message.contains("tip: a similar argument exists: '--quiet'"),
            "{message}"
        );
    }
    #[test]
    fn a_dash_prefixed_word_is_a_flag_even_where_subcommands_exist() {
        // The root has subcommands, so a bare word there is a subcommand — but `--forc` is not a
        // subcommand anybody could have meant, and saying "unrecognized subcommand" answers a
        // question the user did not ask. It happens on exactly the commands where the mistake is
        // easiest to make.
        let message = rendered(&[], Error::UnexpectedArg { token: b"--quie" });
        assert!(
            message.starts_with("error: unexpected argument '--quie' found"),
            "{message}"
        );
        assert!(
            message.contains("tip: a similar argument exists: '--quiet'"),
            "{message}"
        );
        assert!(!message.contains("subcommand"), "{message}");

        // A bare word is still a subcommand, which is the other half of the same rule.
        let message = rendered(&[], Error::UnexpectedArg { token: b"usse" });
        assert!(
            message.starts_with("error: unrecognized subcommand 'usse'"),
            "{message}"
        );

        // A lone `-` is a word, not a flag: it is what several tools spell "standard input".
        let message = rendered(&[], Error::UnexpectedArg { token: b"-" });
        assert!(
            message.starts_with("error: unrecognized subcommand '-'"),
            "{message}"
        );
    }
    #[test]
    fn a_siblings_global_is_not_offered_here() {
        // `user` declares a global; it is a sibling of `use`, never an ancestor, so the parser
        // would refuse `--local` inside `use`. A tip naming a flag that does not work is worse
        // than no tip — and the first version of this walked the whole tree, collecting globals
        // from every branch it passed through.
        let message = rendered(&["use"], Error::UnknownFlag { token: b"--locl" });
        assert!(!message.contains("tip:"), "{message}");

        // Inside `user` itself it is offered, which is what makes the absence above a rule
        // rather than an oversight.
        let message = rendered(&["user"], Error::UnknownFlag { token: b"--locl" });
        assert!(
            message.contains("tip: a similar argument exists: '--local'"),
            "{message}"
        );

        // And the root's global still reaches a subcommand, which is the case globals exist for.
        let message = rendered(&["use"], Error::UnknownFlag { token: b"--quie" });
        assert!(
            message.contains("tip: a similar argument exists: '--quiet'"),
            "{message}"
        );

        // An ancestor's *non*-global flag does not: the root declares `--setup` for itself, and
        // the parser would refuse it inside `use` exactly as it refuses a sibling's.
        let message = rendered(&["use"], Error::UnknownFlag { token: b"--setu" });
        assert!(!message.contains("tip:"), "{message}");
        let message = rendered(&[], Error::UnknownFlag { token: b"--setu" });
        assert!(
            message.contains("tip: a similar argument exists: '--setup'"),
            "{message}"
        );
    }
}
