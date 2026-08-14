//! Splitting a command line the way the shell that typed it would.
//!
//! A completion request arrives as a line and a cursor, not as an argv: the user has pressed
//! Tab in the middle of something the shell has not run and may never run. Four of the five
//! shells hand that over directly — bash's `COMP_LINE`/`COMP_POINT`, zsh's `$BUFFER`/`$CURSOR`,
//! fish's `commandline -cp`, PowerShell's `$commandAst.Extent.Text` with `$cursorPosition` —
//! and nushell, whose external completer only ever sees spans, re-quotes them into a line.
//!
//! Taking the line rather than the shell's own word split is what lets `mise use "my tool<TAB>`
//! complete inside a quoted word at all: a word split has already thrown away the quote that
//! says the space is part of the word. The cost is that the splitting is ours to get right,
//! which is what this module and its tests are.
//!
//! The words that come out are *unquoted*, because they are what the shell would have passed
//! as argv had the line been run — the parser downstream should see exactly what it would see
//! in a real invocation.

use crate::spec::{ArgMeta, CommandMeta, FlagMeta, Spec};
use crate::{Arg, Command, Error, Flag, Parser};

/// Where the cursor is, in the grammar rather than in the line.
///
/// What can be typed at a cursor is decided by everything to its left: which command the words
/// reached, whether a flag is still waiting for its value, which positional comes next, and
/// whether flag interpretation is still on at all. This is that state, read off a real parse
/// of the words before the cursor rather than guessed at — so what is offered and what would
/// be accepted are decided by the same tables.
#[derive(Debug, Clone)]
pub struct Position<'t> {
    /// The command in scope: the deepest one the words selected.
    pub cmd: &'t Command<'t>,
    /// Whether a dash-prefixed word here would still be read as a flag.
    ///
    /// False past a `--`, and past the first value of an `automatic` argument — a wrapper
    /// forwarding to another tool. There is no flag of *this* CLI to offer in either place.
    pub flags_possible: bool,
    /// A flag whose value the cursor is at, if the last word was one that takes a value.
    pub awaiting_value: Option<&'t Flag<'t>>,
    /// The positional a word here would fill, if any are left.
    pub next_arg: Option<&'t Arg<'t>>,
    /// Whether a `--` has been typed.
    ///
    /// Narrower than `flags_possible`, which is also false past an `automatic` argument. An
    /// argument that requires a separator is asking for *this* one specifically.
    pub separator_seen: bool,
    /// Whether the word here names a command to *read about* rather than one to run.
    ///
    /// True after `help`, where the only thing that belongs is a command name — not the
    /// arguments of whatever the root would otherwise fall back to.
    pub help_topic: bool,
    /// The flags a word here could name: this command's own, then any ancestor's globals.
    ///
    /// Taken from the parser rather than gathered again, so what is offered is what would be
    /// accepted — shadowing included.
    pub flags: Vec<&'t Flag<'t>>,
}

/// Walk the words before the cursor, and report what the cursor is at.
///
/// Errors are not failures here. A line being completed is by definition unfinished — a flag
/// with no value yet, a word that names nothing yet — so a parse error means "the grammar runs
/// out here", which is exactly the position being asked about. The walk stops at the first one
/// and reports the state it reached, rather than discarding it the way a real parse must.
pub fn walk<'t>(root: &'t Command<'t>, words: &[String]) -> Position<'t> {
    let argv: Vec<&std::ffi::OsStr> = words.iter().map(std::ffi::OsStr::new).collect();
    let mut parser = Parser::new(root, &argv);
    let mut awaiting_value = None;

    while let Some(event) = parser.next_event() {
        match event {
            Ok(_) => {}
            // The one error that says something about the cursor rather than about the line:
            // the last word was a flag that takes a value, so the cursor is standing in it.
            Err(Error::MissingFlagValue { flag }) => {
                awaiting_value = Some(flag);
                break;
            }
            // `ex help config ⌶` is asking which command to read about, and the answer is a
            // command under `config` — the one the help request already resolved. The parser
            // never descended into it, on purpose (a topic is a question, not an invocation),
            // so the position has to be taken from the request rather than from the parser.
            //
            // Nothing else can be typed there: a topic takes no flags and fills no argument.
            Err(Error::Help { cmd, .. }) => {
                return Position {
                    cmd,
                    flags_possible: false,
                    awaiting_value: None,
                    next_arg: None,
                    separator_seen: false,
                    help_topic: true,
                    flags: Vec::new(),
                }
            }
            Err(_) => break,
        }
    }

    Position {
        cmd: parser.command(),
        flags_possible: !parser.flags_stopped(),
        // A variadic flag still claiming words is standing in the same place a flag waiting
        // for its first value is: the next word belongs to it, not to the positional after it.
        awaiting_value: awaiting_value.or_else(|| parser.collecting()),
        next_arg: parser.pending_arg(),
        separator_seen: parser.double_dash_seen(),
        help_topic: false,
        flags: parser.flags_in_scope().collect(),
    }
}

/// Something a shell could offer at the cursor.
///
/// The description is what fish, zsh, nu and PowerShell show beside a candidate; bash shows
/// only the value. It is borrowed from the spec rather than built, because it is already
/// there — the help text a page would print for the same thing.
#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord)]
pub struct Candidate<'a> {
    pub value: String,
    pub description: Option<&'a str>,
}

/// Paths a shell should offer, on top of whatever this crate knows about.
///
/// Listing them is the shell's job, not ours. It already does it better than a CLI can — the
/// user's own completion styles, colours, escaping, directory-aware widgets — and doing it here
/// would put a directory read in a binary whose whole claim is that it does not touch the
/// filesystem to parse a command line. So the answer says *that* paths belong here, and the
/// generated script hands the position to `_files`, `__fish_complete_path` or `compgen -f`.
///
/// This is a deliberate divergence from usage-lib, which reads the directory itself and returns
/// the names. The conformance comparison holds the two equivalent rather than equal: where the
/// reference answers with a listing, this answers with the marker.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Files {
    /// Anything: files, directories, whatever the shell shows for a path.
    Any,
    /// Directories only.
    Dirs,
}

/// Everything a shell needs to answer one Tab.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct Completions<'a> {
    /// What this CLI knows the word could be.
    pub candidates: Vec<Candidate<'a>>,
    /// Paths the shell should add, if the position admits them.
    pub files: Option<Files>,
}

/// The line a shell reads to mean "paths belong here too".
///
/// A whole line rather than a flag on the protocol, because every one of the five shells can
/// already split output into lines and look at the last one. `\x01` opens it because no
/// candidate this crate produces can contain a control character, so it cannot be mistaken for
/// one.
pub const FILES_MARKER: &str = "\u{1}files";
/// See [`FILES_MARKER`]. Directories only.
pub const DIRS_MARKER: &str = "\u{1}dirs";

/// Write an answer the way `shell` reads it.
///
/// One line per candidate, in the shape the shell's own completion machinery expects — which is
/// where the five differ. bash reads values only; fish, nu and PowerShell take a description
/// after a tab; zsh takes a third field, the text to insert, because what it displays and what
/// it types are not always the same string.
///
/// A trailing [`FILES_MARKER`] says the generated script should hand the position to the
/// shell's own path completion afterwards.
pub fn render(answer: &Completions<'_>, shell: Shell) -> String {
    let mut out = String::new();
    // Descriptions are all-or-nothing per answer: a column that appears on some rows and not
    // others reads as missing data rather than as an absent description, which is the reason
    // the reference decides this per answer too.
    let described = answer.candidates.iter().any(|c| c.description.is_some());

    for candidate in &answer.candidates {
        let description = candidate.description.unwrap_or_default();
        match shell {
            Shell::Bash => out.push_str(&candidate.value),
            Shell::Zsh => {
                // Display, then description, then what to type: a candidate containing a space
                // or a quote has to reach the command line intact.
                out.push_str(&candidate.value);
                out.push('\t');
                out.push_str(description);
                out.push('\t');
                out.push_str(&zsh_quote(&candidate.value));
            }
            Shell::Fish | Shell::Nu | Shell::PowerShell => {
                out.push_str(&candidate.value);
                if described {
                    out.push('\t');
                    out.push_str(description);
                }
            }
        }
        out.push('\n');
    }

    match answer.files {
        Some(Files::Any) => {
            out.push_str(FILES_MARKER);
            out.push('\n');
        }
        Some(Files::Dirs) => {
            out.push_str(DIRS_MARKER);
            out.push('\n');
        }
        None => {}
    }
    out
}

/// A candidate as zsh would have to see it typed.
///
/// The same rule the reference uses: left alone when every character is safe, and otherwise
/// single-quoted with the close-open dance around any apostrophe.
fn zsh_quote(value: &str) -> String {
    let safe = |c: char| {
        c.is_ascii_alphanumeric()
            || matches!(c, '_' | '-' | '.' | '/' | ':' | '@' | '+' | '=' | '%' | ',')
    };
    if !value.is_empty() && value.chars().all(safe) {
        return value.to_string();
    }
    format!("'{}'", value.replace('\'', "'\\''"))
}

/// The paths an argument or value name asks for, by the name itself.
///
/// The reference resolves a completer by the *lowercased name* and falls back to treating that
/// name as the type, so an argument called `<FILE>` completes files and `<DIR>` directories
/// without a spec saying so. Reimplemented rather than modelled as new vocabulary, because it
/// is the same rule read off the same names.
fn files_for(name: &str) -> Option<Files> {
    // Compared without allocating a lowercased copy: a name is short, and this is a parser.
    let matches = |want: &str| name.eq_ignore_ascii_case(want);
    if matches("file") || matches("path") || matches("config_file") {
        Some(Files::Any)
    } else if matches("dir") || matches("directory") {
        Some(Files::Dirs)
    } else {
        None
    }
}

/// What could be typed at the cursor, given a spec and a split line.
///
/// The rules are usage-lib's, because a CLI's completions should not change with the
/// implementation that answers them:
///
/// - a lone `-` offers both forms, `--…` offers longs, `-…` offers shorts;
/// - a flag waiting for its value offers that flag's choices, and nothing else;
/// - otherwise the next positional's choices and the subcommands, aliases included.
///
/// Hidden things are never offered, in any branch. What is *not* here yet: the file fallback
/// for a word nothing is known about, and the `run=` completions a spec can declare.
pub fn complete<'a>(spec: &'a Spec<'a>, split: &Split) -> Completions<'a> {
    let position = walk(spec.root.cmd, split.argv());
    let meta = crate::help::find(spec, position.cmd).map(|(_, meta)| meta);
    let token = split.prefix.as_str();
    let candidates = candidates(spec, split);

    // Which argument the cursor is at — the same question `candidates` answers, asked once so
    // that the two halves cannot disagree. Past a restart token it is the command's *first*
    // argument, whatever the words before the token filled, and everything below follows from
    // that: whether paths belong, whether the set is declared, whether a separator is owed.
    let at_cursor = if restarted(meta, split) {
        meta.and_then(|m| m.args.first()).map(|m| m.arg)
    } else {
        position.next_arg
    };

    // A dash-prefixed word is a flag or nothing: no path starts with one, and the reference
    // suppresses its own listing there for the same reason.
    let flag_like = position.flags_possible && token.starts_with('-');

    // The name a value here would have, which is what says whether paths belong, and whether
    // that value declares its own set.
    let (named, declares_choices) = if let Some(flag) = position.awaiting_value {
        let meta = flag_meta(spec.root, flag);
        (
            meta.and_then(|m| m.value_name).or(Some(flag.name)),
            meta.is_some_and(|m| !m.choices.is_empty()),
        )
    } else if let Some(arg) = at_cursor {
        let meta = arg_meta(spec.root, arg);
        (Some(arg.name), meta.is_some_and(|m| !m.choices.is_empty()))
    } else {
        (None, false)
    };
    let asked_for = named.and_then(files_for);

    // An argument that requires a separator is not fillable yet, so nothing else belongs here —
    // not even a path, which the parser would reject exactly as it rejects a value.
    //
    // Only when the cursor is *at* that argument, though. A flag waiting for its value is a
    // different position that happens to have an unfilled positional behind it, and a rule about
    // the positional has nothing to say about the flag: `ex --from ⌶` takes a path whatever the
    // argument after it needs.
    let needs_separator = position.awaiting_value.is_none()
        && at_cursor.is_some_and(|arg| arg.double_dash == crate::DoubleDash::Required)
        && !position.separator_seen;

    // Two questions, and the reference asks both. Was anything found — because a position that
    // answered does not need help. And does the position *declare* its set — because then an
    // unmatched prefix means no matches rather than "ask somebody else", which is the difference
    // between "there is nothing else this can be" and "nothing matched what you typed".
    // Offering the working directory for a mistyped choice answers the second as though it were
    // the first.
    let closed = !candidates.is_empty() || declares_choices || position.help_topic;

    let files = if flag_like || needs_separator {
        None
    } else if asked_for.is_some() {
        asked_for
    } else if closed {
        None
    } else {
        Some(Files::Any)
    };

    Completions { candidates, files }
}

/// Just the candidates this CLI knows about, without the question of paths.
pub fn candidates<'a>(spec: &'a Spec<'a>, split: &Split) -> Vec<Candidate<'a>> {
    let position = walk(spec.root.cmd, split.argv());
    let meta = crate::help::find(spec, position.cmd).map(|(_, meta)| meta);
    let token = split.prefix.as_str();

    let mut out = if position.flags_possible && token == "-" {
        // Both forms, because a lone dash says nothing about which was meant.
        let mut both = short_flags(spec, &position, "");
        both.extend(long_flags(spec, &position, ""));
        both
    } else if position.flags_possible && token.starts_with("--") {
        long_flags(spec, &position, token)
    } else if position.flags_possible && token.starts_with('-') {
        short_flags(spec, &position, token)
    } else if restarted(meta, split) {
        // Past a restart token — mise's `:::`, which starts a fresh invocation of the same
        // command — the cursor is at that command's *first* argument again, whatever the
        // words before the token filled.
        meta.and_then(|m| m.args.first())
            .map(|m| positional(m, &position, token))
            .unwrap_or_default()
    } else if let Some(flag) = position.awaiting_value {
        flag_meta(spec.root, flag)
            .map(|m| choices(m.choices, token))
            .unwrap_or_default()
    } else {
        let mut found = Vec::new();
        if let Some(arg) = position.next_arg {
            if let Some(m) = arg_meta(spec.root, arg) {
                found.extend(positional(m, &position, token));
            }
        }
        if let Some(meta) = meta {
            found.extend(subcommands(meta, token));
        }
        // At the root, a word may also be meant for the command the root falls back to —
        // `mise build` is `mise run build` — so what that command's first argument accepts is
        // a candidate here too. Only its first: the words that would fill the rest have not
        // been typed, since the subcommand name itself was elided.
        if core::ptr::eq(position.cmd, spec.root.cmd) && !position.help_topic {
            if let Some(default) = spec.default_subcommand {
                // By any name it answers to: `default_subcommand` names a command, and a
                // spec may name it the way its author refers to it rather than canonically.
                let target = spec
                    .root
                    .subcommands
                    .iter()
                    .find(|sub| sub.cmd.name == default || sub.cmd.aliases.contains(&default));
                if let Some(arg) = target.and_then(|sub| sub.args.first()) {
                    found.extend(positional(arg, &position, token));
                }
            }
        }
        found
    };

    out.sort();
    out.dedup_by(|a, b| a.value == b.value);
    out
}

/// Whether the word before the cursor is this command's restart token.
///
/// mise writes `:::` between two runs of the same command, and what follows one is a fresh
/// invocation — so the cursor is back at the first argument rather than wherever the previous
/// words had reached.
fn restarted(meta: Option<&CommandMeta<'_>>, split: &Split) -> bool {
    let Some(token) = meta.and_then(|m| m.restart_token) else {
        return false;
    };
    split.cword > 0 && split.words[split.cword - 1] == token
}

/// The subcommands a word here could name, each under every name it answers to.
///
/// Aliases are offered beside canonical names — someone who types `mise ls<TAB>` means `list`
/// and should see it — except the hidden ones, which the parser answers to but nothing
/// advertises. A hidden command is offered under none of its names at all.
fn subcommands<'a>(meta: &'a CommandMeta<'a>, token: &str) -> Vec<Candidate<'a>> {
    let mut out = Vec::new();
    for sub in meta.subcommands {
        if sub.hide {
            continue;
        }
        for name in core::iter::once(&sub.cmd.name).chain(sub.cmd.aliases.iter()) {
            // A hidden alias is one the parser answers to but nothing advertises — usually an
            // old name kept working after a rename. The parse table holds it beside the
            // visible ones because both must be *accepted*; only the metadata says which are
            // meant to be *offered*.
            if sub.hidden_aliases.contains(name) {
                continue;
            }
            if name.starts_with(token) {
                out.push(Candidate {
                    value: (*name).to_string(),
                    description: sub.about,
                });
            }
        }
    }
    out
}

/// The long forms of every flag in scope, and the negations of those that have one.
fn long_flags<'a>(spec: &'a Spec<'a>, position: &Position<'_>, token: &str) -> Vec<Candidate<'a>> {
    let mut out = Vec::new();
    for flag in &position.flags {
        let meta = flag_meta(spec.root, flag);
        if meta.is_some_and(|m| m.hide) {
            continue;
        }
        let description = meta.and_then(|m| m.help);
        for long in flag.longs {
            let value = format!("--{long}");
            if value.starts_with(token) {
                out.push(Candidate { value, description });
            }
        }
        // A negation is a way to write the same flag, so it carries the same help — the
        // reference leaves it bare, which reads as a flag nobody documented.
        if let Some(negate) = flag.negate {
            // The table holds it the way the parser matches it — with the dashes already
            // taken off — so a candidate has to put them back. Offered bare, it never matched
            // a `--` the user had typed, and a lone `-` offered a word no shell would accept.
            let value = format!("--{negate}");
            if value.starts_with(token) {
                out.push(Candidate { value, description });
            }
        }
    }
    out
}

/// The short forms of every flag in scope.
///
/// A token of `-x` is asking about the letter `x`, so only that letter's flag is offered:
/// bundling means anything else would be a candidate for a different position in the token.
fn short_flags<'a>(spec: &'a Spec<'a>, position: &Position<'_>, token: &str) -> Vec<Candidate<'a>> {
    let wanted = token.as_bytes().get(1).copied();
    let mut out = Vec::new();
    for flag in &position.flags {
        let meta = flag_meta(spec.root, flag);
        if meta.is_some_and(|m| m.hide) {
            continue;
        }
        for &short in flag.shorts {
            // Written out rather than with `is_none_or`, which this crate's MSRV predates.
            let asked_about = match wanted {
                None => true,
                Some(letter) => letter == short,
            };
            if asked_about {
                out.push(Candidate {
                    value: format!("-{}", short as char),
                    description: meta.and_then(|m| m.help),
                });
            }
        }
    }
    out
}

/// What a positional accepts here — its choices, or the separator that has to come first.
///
/// An argument declared `double_dash = "required"` is not fillable until a `--` has been
/// typed, so its values are not candidates yet: the parser would reject every one of them. The
/// only useful thing to offer is the separator itself, and only when nothing has been typed to
/// filter it away.
fn positional<'a>(
    meta: &'a ArgMeta<'a>,
    position: &Position<'_>,
    token: &str,
) -> Vec<Candidate<'a>> {
    if meta.arg.double_dash == crate::DoubleDash::Required && !position.separator_seen {
        if token.is_empty() {
            return vec![Candidate {
                value: "--".to_string(),
                description: None,
            }];
        }
        return Vec::new();
    }
    choices(meta.choices, token)
}

/// The declared values of a flag or argument, filtered by what has been typed.
fn choices<'a>(declared: &'a [&'a str], token: &str) -> Vec<Candidate<'a>> {
    declared
        .iter()
        .filter(|c| c.starts_with(token))
        .map(|c| Candidate {
            value: (*c).to_string(),
            description: None,
        })
        .collect()
}

/// The metadata for a flag, wherever in the tree it was declared.
///
/// By identity rather than by name: a global flag's metadata sits on the command that declared
/// it, which is an ancestor of the one being completed, and two commands may declare different
/// flags under one name.
fn flag_meta<'a>(meta: &'a CommandMeta<'a>, flag: &Flag<'_>) -> Option<&'a FlagMeta<'a>> {
    meta.flags
        .iter()
        .find(|m| core::ptr::eq(m.flag, flag))
        .or_else(|| meta.subcommands.iter().find_map(|sub| flag_meta(sub, flag)))
}

/// The metadata for an argument, wherever in the tree it was declared.
fn arg_meta<'a>(meta: &'a CommandMeta<'a>, arg: &Arg<'_>) -> Option<&'a ArgMeta<'a>> {
    meta.args
        .iter()
        .find(|m| core::ptr::eq(m.arg, arg))
        .or_else(|| meta.subcommands.iter().find_map(|sub| arg_meta(sub, arg)))
}

/// Which shell's quoting rules a line follows.
///
/// Only two rule sets, not five: bash, zsh, fish and nushell all follow the POSIX shape
/// closely enough that a completion request cannot tell them apart, while PowerShell escapes
/// with a backtick and doubles a quote to escape it. The distinction is kept per *shell*
/// rather than per rule set so that a shell whose rules turn out to differ can be given its
/// own without changing this type's public shape.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[non_exhaustive]
pub enum Shell {
    Bash,
    Zsh,
    Fish,
    Nu,
    PowerShell,
}

impl Shell {
    /// The spelling a shell is named by, on the command line and in a spec.
    pub fn as_str(self) -> &'static str {
        match self {
            Shell::Bash => "bash",
            Shell::Zsh => "zsh",
            Shell::Fish => "fish",
            Shell::Nu => "nu",
            Shell::PowerShell => "powershell",
        }
    }

    /// Read a shell by name, as it is spelled on the command line.
    pub fn from_name(name: &str) -> Option<Self> {
        match name {
            "bash" => Some(Shell::Bash),
            "zsh" => Some(Shell::Zsh),
            "fish" => Some(Shell::Fish),
            "nu" | "nushell" => Some(Shell::Nu),
            "powershell" | "pwsh" => Some(Shell::PowerShell),
            _ => None,
        }
    }

    /// Whether an escape is written with a backtick rather than a backslash.
    fn backtick_escapes(self) -> bool {
        matches!(self, Shell::PowerShell)
    }

    /// Whether a quote inside a quoted string is written by doubling it.
    fn doubles_quotes(self) -> bool {
        matches!(self, Shell::PowerShell)
    }
}

/// A command line as the shell would have passed it, plus where the cursor was.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Split {
    /// The words, unquoted — what argv would hold had the line been run.
    ///
    /// Always at least one: a cursor sitting after a space is completing a word that does not
    /// exist yet, and an empty word is how that is said. Candidates for "anything at all" and
    /// candidates for "something starting with `no`" are the same question with a different
    /// prefix, and a caller should not have to special-case the empty one.
    pub words: Vec<String>,
    /// Which of `words` the cursor is in.
    pub cword: usize,
    /// The part of that word before the cursor, unquoted — what a candidate must start with.
    pub prefix: String,
}

impl Split {
    /// The words up to and including the one being completed.
    ///
    /// What the parser should walk: everything after the cursor describes a command line the
    /// user has not finished thinking about, and a half-typed tail behind the cursor should
    /// not decide what can be typed at it.
    pub fn walked(&self) -> &[String] {
        &self.words[..=self.cword]
    }

    /// The words a parser should walk: after the program name, before the cursor's word.
    ///
    /// Two things dropped for two reasons. The program name, because argv does not contain it
    /// and the parse tables describe what comes *after* it. The word being completed, because
    /// it is half-typed by definition — feeding it in would ask what can follow a word the
    /// user has not finished, when the question is what that word could be.
    pub fn argv(&self) -> &[String] {
        let start = 1.min(self.cword);
        &self.words[start..self.cword]
    }
}

/// Split a line at a byte cursor, the way `shell` would have split it.
///
/// `cursor` is a byte offset into `line`; anything past the end is treated as the end, and an
/// offset landing inside a multi-byte character is moved back to that character's start rather
/// than panicking — a completion request is not a place to be strict about a shell's arithmetic.
pub fn split(line: &str, cursor: usize, shell: Shell) -> Split {
    let cursor = floor_char_boundary(line, cursor.min(line.len()));

    let mut words: Vec<String> = Vec::new();
    let mut word = String::new();
    // Whether anything has been written into `word` — including a quote that so far contains
    // nothing, so that `mise ""` is a word and not a gap between two.
    let mut started = false;
    let mut cword = None;
    let mut prefix = None;
    // Whether the cursor sat inside a word rather than in the gap before one. A gap is a word
    // the user is about to type, so one has to be made for them.
    let mut cursor_in_word = false;

    let mut chars = line.char_indices().peekable();
    let mut quote: Option<char> = None;

    // The cursor is reached before the character it sits in front of is read, so the word in
    // hand is the word being completed and what is in it is the prefix.
    //
    // A macro rather than a closure because it writes to four locals the loop also writes to,
    // and it has to run at *two* places: at the top of each character, and again before an
    // escape swallows the one after it. Only checking the top meant a cursor sitting on an
    // escaped character was never noticed, and the split described the last word of the line
    // instead of the one being typed.
    macro_rules! reached {
        ($idx:expr) => {
            if $idx == cursor && prefix.is_none() {
                cword = Some(words.len());
                prefix = Some(word.clone());
                cursor_in_word = started;
            }
        };
    }

    while let Some((i, c)) = chars.next() {
        reached!(i);

        match quote {
            Some('\'') => {
                if c == '\'' {
                    // PowerShell writes a quote inside a quoted string by doubling it; the
                    // POSIX-shaped shells have no such rule, and there a second quote always
                    // ends the string.
                    if shell.doubles_quotes() && chars.peek().map(|&(_, n)| n) == Some('\'') {
                        reached!(chars.peek().expect("peeked just above").0);
                        word.push('\'');
                        chars.next();
                    } else {
                        quote = None;
                    }
                } else {
                    word.push(c);
                }
            }
            Some(q) => {
                if c == q {
                    if shell.doubles_quotes() && chars.peek().map(|&(_, n)| n) == Some(q) {
                        reached!(chars.peek().expect("peeked just above").0);
                        word.push(q);
                        chars.next();
                    } else {
                        quote = None;
                    }
                } else if is_escape(c, shell) {
                    // Inside double quotes an escape is only an escape before a character it
                    // could mean something to; before anything else it is a literal, which is
                    // why a Windows path in double quotes survives.
                    match chars.peek() {
                        Some(&(j, next)) if escapable_in_quotes(next, shell) => {
                            reached!(j);
                            word.push(next);
                            chars.next();
                        }
                        _ => word.push(c),
                    }
                } else {
                    word.push(c);
                }
            }
            None => {
                if c == '\'' || c == '"' {
                    quote = Some(c);
                    started = true;
                } else if is_escape(c, shell) {
                    // Before the check, because the escape has already started the word: a
                    // cursor on the escaped character is inside it, not in the gap before it.
                    started = true;
                    if let Some(&(j, next)) = chars.peek() {
                        reached!(j);
                        word.push(next);
                        chars.next();
                    } else {
                        // A trailing escape is a line the user is still typing, not a mistake
                        // to report: it escapes the character they have not typed yet.
                        started = true;
                    }
                } else if c.is_whitespace() {
                    if started {
                        words.push(core::mem::take(&mut word));
                        started = false;
                    }
                } else {
                    word.push(c);
                    started = true;
                }
            }
        }
    }

    // The cursor at the very end of the line: the loop above only sees positions it reads a
    // character at, and there is no character there.
    if prefix.is_none() {
        cword = Some(words.len());
        prefix = Some(word.clone());
        cursor_in_word = started;
    }

    if started {
        words.push(word);
    }

    let cword = cword.unwrap_or(0);
    // A cursor in a gap is completing a word that is not in the line yet — at the end of it,
    // or between two that are already there. `mise ⌶use` is asking what can go *before* `use`,
    // and answering about `use` itself would complete the wrong word.
    if !cursor_in_word {
        words.insert(cword, String::new());
    }
    Split {
        words,
        cword,
        prefix: prefix.unwrap_or_default(),
    }
}

/// Whether a character starts an escape in this shell.
fn is_escape(c: char, shell: Shell) -> bool {
    if shell.backtick_escapes() {
        c == '`'
    } else {
        c == '\\'
    }
}

/// Whether an escape inside double quotes applies to the character after it.
fn escapable_in_quotes(c: char, shell: Shell) -> bool {
    if shell.backtick_escapes() {
        matches!(c, '"' | '`' | '$')
    } else {
        matches!(c, '"' | '\\' | '$' | '`')
    }
}

/// The start of the character containing `index`.
///
/// `str::floor_char_boundary` is still unstable, and this crate takes no dependencies.
fn floor_char_boundary(s: &str, index: usize) -> usize {
    let mut i = index;
    while i > 0 && !s.is_char_boundary(i) {
        i -= 1;
    }
    i
}

#[cfg(test)]
mod tests {
    use super::*;

    /// A small tree with the shapes that make a cursor's position non-obvious: a global flag,
    /// a flag that takes a value, a subcommand of a subcommand, and a wrapper that forwards.
    static GLOBAL: Flag = Flag {
        key: 1,
        name: "verbose",
        longs: &["verbose"],
        shorts: b"v",
        global: true,
        // Held without its dashes, which is how the parser matches it.
        negate: Some("quiet"),
        ..Flag::BOOL
    };
    static JOBS: Flag = Flag {
        key: 2,
        name: "jobs",
        longs: &["jobs"],
        ..Flag::VALUE
    };
    /// A flag that keeps claiming words once it has one.
    static TOOLS: Flag = Flag {
        key: 8,
        name: "tools",
        longs: &["tools"],
        variadic: true,
        ..Flag::VALUE
    };
    static TOOL: Arg = Arg {
        key: 3,
        name: "TOOL",
        ..Arg::REQUIRED
    };
    static USE: Command = Command {
        name: "use",
        aliases: &["u"],
        flags: &[&JOBS, &TOOLS],
        args: &[&TOOL],
        ..Command::EMPTY
    };
    static FORWARDED: Arg = Arg {
        key: 4,
        name: "ARGS",
        double_dash: crate::DoubleDash::Automatic,
        ..Arg::VAR
    };
    static EXEC: Command = Command {
        name: "exec",
        args: &[&FORWARDED],
        ..Command::EMPTY
    };
    static LS: Command = Command {
        name: "ls",
        ..Command::EMPTY
    };
    /// Two arguments and a restart token, so that "back at the first" is a different answer
    /// from "wherever the words reached".
    static FIRST: Arg = Arg {
        key: 5,
        name: "FIRST",
        ..Arg::REQUIRED
    };
    static SECOND: Arg = Arg {
        key: 6,
        name: "SECOND",
        ..Arg::REQUIRED
    };
    static TASK: Command = Command {
        name: "task",
        args: &[&FIRST, &SECOND],
        ..Command::EMPTY
    };
    /// A restarting command whose first argument takes paths and whose second does not, so that
    /// "which argument is the cursor at" has two different answers.
    static SCRIPT_ARG: Arg = Arg {
        key: 13,
        name: "FILE",
        ..Arg::REQUIRED
    };
    static MODE: Arg = Arg {
        key: 14,
        name: "MODE",
        ..Arg::REQUIRED
    };
    static SHIP: Command = Command {
        name: "ship",
        // The choices-bearing one first, so that "which argument is the cursor at" has two
        // different *answers* rather than two routes to the same one.
        args: &[&MODE, &SCRIPT_ARG],
        ..Command::EMPTY
    };
    static FILE: Arg = Arg {
        key: 9,
        name: "FILE",
        ..Arg::REQUIRED
    };
    /// A path-named argument that is not fillable until a `--` is typed.
    static PIPED: Arg = Arg {
        key: 11,
        name: "PATH",
        double_dash: crate::DoubleDash::Required,
        ..Arg::REQUIRED
    };
    static FROM: Flag = Flag {
        key: 12,
        name: "from",
        longs: &["from"],
        ..Flag::VALUE
    };
    static PIPE: Command = Command {
        name: "pipe",
        flags: &[&FROM],
        args: &[&PIPED],
        ..Command::EMPTY
    };
    static EDIT: Command = Command {
        name: "edit",
        flags: &[&INTO],
        args: &[&FILE],
        ..Command::EMPTY
    };
    static INTO: Flag = Flag {
        key: 10,
        name: "into",
        longs: &["into"],
        ..Flag::VALUE
    };
    static AFTER: Arg = Arg {
        key: 7,
        name: "AFTER",
        double_dash: crate::DoubleDash::Required,
        ..Arg::REQUIRED
    };
    static WRAP: Command = Command {
        name: "wrap",
        args: &[&AFTER],
        ..Command::EMPTY
    };
    static PLUGINS: Command = Command {
        name: "plugins",
        subcommands: &[&LS],
        ..Command::EMPTY
    };
    static ROOT: Command = Command {
        name: "mise",
        flags: &[&GLOBAL],
        subcommands: &[&USE, &EXEC, &PLUGINS],
        ..Command::EMPTY
    };

    /// The same tree's metadata: help text, choices, and what is hidden.
    static SECRET: Command = Command {
        name: "secret",
        ..Command::EMPTY
    };
    static LIST: Command = Command {
        name: "list",
        // `l` is answered to but never advertised — an old name kept working.
        aliases: &["ls", "l"],
        ..Command::EMPTY
    };
    static META_LS: CommandMeta = CommandMeta {
        cmd: &LS,
        about: Some("List them"),
        ..CommandMeta::EMPTY
    };
    static META_PLUGINS: CommandMeta = CommandMeta {
        cmd: &PLUGINS,
        about: Some("Manage plugins"),
        subcommands: &[&META_LS],
        ..CommandMeta::EMPTY
    };
    static META_USE: CommandMeta = CommandMeta {
        cmd: &USE,
        about: Some("Use a tool"),
        flags: &[FlagMeta {
            flag: &JOBS,
            help: Some("How many at once"),
            choices: &["1", "2", "4"],
            ..FlagMeta::EMPTY
        }],
        args: &[ArgMeta {
            arg: &TOOL,
            help: Some("Which tool"),
            choices: &["node", "python"],
            ..ArgMeta::EMPTY
        }],
        ..CommandMeta::EMPTY
    };
    static META_SHIP: CommandMeta = CommandMeta {
        cmd: &SHIP,
        about: Some("Ship a file"),
        restart_token: Some(":::"),
        args: &[
            ArgMeta {
                arg: &MODE,
                choices: &["fast", "slow"],
                ..ArgMeta::EMPTY
            },
            ArgMeta {
                arg: &SCRIPT_ARG,
                help: Some("Which file"),
                ..ArgMeta::EMPTY
            },
        ],
        ..CommandMeta::EMPTY
    };
    static META_PIPE: CommandMeta = CommandMeta {
        cmd: &PIPE,
        about: Some("Pipe a file"),
        flags: &[FlagMeta {
            flag: &FROM,
            help: Some("Read from"),
            value_name: Some("FILE"),
            ..FlagMeta::EMPTY
        }],
        args: &[ArgMeta {
            arg: &PIPED,
            help: Some("Where from"),
            ..ArgMeta::EMPTY
        }],
        ..CommandMeta::EMPTY
    };
    static META_EDIT: CommandMeta = CommandMeta {
        cmd: &EDIT,
        about: Some("Edit a file"),
        flags: &[FlagMeta {
            flag: &INTO,
            help: Some("Where to write it"),
            value_name: Some("DIR"),
            ..FlagMeta::EMPTY
        }],
        args: &[ArgMeta {
            arg: &FILE,
            help: Some("Which file"),
            // Two well-known ones, so the position has candidates *and* wants paths — which is
            // what makes the name the deciding rule rather than the emptiness.
            choices: &["mise.toml", "mise.local.toml"],
            ..ArgMeta::EMPTY
        }],
        ..CommandMeta::EMPTY
    };
    static META_WRAP: CommandMeta = CommandMeta {
        cmd: &WRAP,
        about: Some("Wrap something"),
        args: &[ArgMeta {
            arg: &AFTER,
            choices: &["red", "blue"],
            ..ArgMeta::EMPTY
        }],
        ..CommandMeta::EMPTY
    };
    static META_TASK: CommandMeta = CommandMeta {
        cmd: &TASK,
        about: Some("Do two things"),
        restart_token: Some(":::"),
        args: &[
            ArgMeta {
                arg: &FIRST,
                choices: &["one", "two"],
                ..ArgMeta::EMPTY
            },
            ArgMeta {
                arg: &SECOND,
                choices: &["alpha", "beta"],
                ..ArgMeta::EMPTY
            },
        ],
        ..CommandMeta::EMPTY
    };
    static META_EXEC: CommandMeta = CommandMeta {
        cmd: &EXEC,
        about: Some("Run something"),
        args: &[ArgMeta {
            arg: &FORWARDED,
            help: Some("What to run"),
            choices: &["one", "two"],
            ..ArgMeta::EMPTY
        }],
        ..CommandMeta::EMPTY
    };
    static META_SECRET: CommandMeta = CommandMeta {
        cmd: &SECRET,
        about: Some("Not for you"),
        hide: true,
        ..CommandMeta::EMPTY
    };
    static META_LIST: CommandMeta = CommandMeta {
        cmd: &LIST,
        about: Some("List everything"),
        hidden_aliases: &["l"],
        ..CommandMeta::EMPTY
    };
    static META_ROOT: CommandMeta = CommandMeta {
        cmd: &ROOT_WITH_META,
        flags: &[FlagMeta {
            flag: &GLOBAL,
            help: Some("Say more"),
            ..FlagMeta::EMPTY
        }],
        subcommands: &[
            &META_USE,
            &META_EXEC,
            &META_PLUGINS,
            &META_SECRET,
            &META_LIST,
            &META_TASK,
            &META_WRAP,
            &META_EDIT,
            &META_PIPE,
            &META_SHIP,
        ],
        ..CommandMeta::EMPTY
    };
    static ROOT_WITH_META: Command = Command {
        name: "mise",
        flags: &[&GLOBAL],
        subcommands: &[
            &USE, &EXEC, &PLUGINS, &SECRET, &LIST, &TASK, &WRAP, &EDIT, &PIPE, &SHIP,
        ],
        ..Command::EMPTY
    };
    static SPEC: Spec = Spec {
        name: "mise",
        bin: Some("mise"),
        root: &META_ROOT,
        // What a word at the root falls back to, as mise's `run` does.
        default_subcommand: Some("u"),
        ..Spec::EMPTY
    };

    /// What a shell would be offered at the end of a line.
    fn offered(line: &str) -> Vec<String> {
        candidates(&SPEC, &at_end(line))
            .into_iter()
            .map(|c| c.value)
            .collect()
    }

    /// The position at the cursor of a line, which is what a completion asks about.
    fn position_at(line: &str) -> Position<'static> {
        let s = at_end(line);
        walk(&ROOT, s.argv())
    }

    #[test]
    fn the_cursor_is_in_the_command_the_words_reached() {
        assert_eq!(position_at("mise ").cmd.name, "mise");
        assert_eq!(position_at("mise plugins ").cmd.name, "plugins");
        assert_eq!(position_at("mise plugins ls ").cmd.name, "ls");
        // A half-typed word names nothing yet, so it is not descended into: the cursor is
        // still in the command before it.
        assert_eq!(position_at("mise plug").cmd.name, "mise");
    }

    #[test]
    fn a_flag_that_takes_a_value_puts_the_cursor_in_it() {
        // The parser calls this a missing value; to a completion it is the question being
        // asked, and the flag is what decides the answer.
        let p = position_at("mise use --jobs ");
        assert_eq!(p.awaiting_value.map(|f| f.name), Some("jobs"));

        // A flag that takes none does not, and neither does one already given its value.
        assert!(position_at("mise use --jobs 4 ").awaiting_value.is_none());
        assert!(position_at("mise --verbose ").awaiting_value.is_none());
    }

    #[test]
    fn a_variadic_flag_still_claiming_words_holds_the_cursor() {
        // `mise use --tools node ⌶` — the next word is another tool, because the parser would
        // bind it to the flag rather than to the positional after it. The state saying so is
        // cleared by the very call that ends the walk, so it has to be read on the way.
        let p = position_at("mise use --tools node ");
        assert_eq!(p.awaiting_value.map(|f| f.name), Some("tools"));

        // Until something that could not be a value comes along, which is when the parser
        // stops claiming too.
        let p = position_at("mise use --tools node -- ");
        assert!(p.awaiting_value.is_none());
    }

    #[test]
    fn the_next_positional_is_the_one_a_word_would_fill() {
        assert_eq!(
            position_at("mise use ").next_arg.map(|a| a.name),
            Some("TOOL")
        );
        // Filled, so there is nothing left for a second word to go into.
        assert!(position_at("mise use node ").next_arg.is_none());
        // A variadic keeps taking values, so it is still the answer.
        assert_eq!(
            position_at("mise exec a b ").next_arg.map(|a| a.name),
            Some("ARGS")
        );
    }

    #[test]
    fn flags_stop_being_possible_where_the_parser_stops_reading_them() {
        assert!(position_at("mise use ").flags_possible);
        assert!(!position_at("mise use -- ").flags_possible);
        // And past an `automatic` argument's first value, which is a wrapper forwarding to
        // another tool: the flags there are the other tool's, and this CLI has none to offer.
        assert!(!position_at("mise exec node ").flags_possible);
        assert!(position_at("mise exec ").flags_possible);
    }

    #[test]
    fn a_help_topic_puts_the_cursor_under_the_command_it_named() {
        // `mise help plugins ⌶` asks which command to read about, and the candidates are
        // `plugins`'s own — not the root's, which is where the parser stayed, since a topic is
        // resolved without being descended into.
        let p = position_at("mise help plugins ");
        assert_eq!(p.cmd.name, "plugins");
        assert!(!p.flags_possible, "a topic takes no flags");
        assert!(p.next_arg.is_none(), "and fills no argument");

        // And with no topic yet, the cursor is under the command `help` was typed in.
        assert_eq!(position_at("mise help ").cmd.name, "mise");
    }

    #[test]
    fn a_global_flag_is_in_scope_inside_a_subcommand() {
        // What the parser would accept there is what should be offered there — one rule, read
        // from the same tables, rather than two that can disagree.
        let names: Vec<_> = {
            let argv = [std::ffi::OsStr::new("plugins")];
            let mut parser = Parser::new(&ROOT, &argv);
            while parser.next_event().is_some() {}
            parser.flags_in_scope().map(|f| f.name).collect()
        };
        assert!(names.contains(&"verbose"), "{names:?}");
    }

    fn at_end(line: &str) -> Split {
        split(line, line.len(), Shell::Bash)
    }

    #[test]
    fn a_line_splits_into_the_words_the_shell_would_have_passed() {
        let s = at_end("mise use node");
        assert_eq!(s.words, ["mise", "use", "node"]);
        assert_eq!(s.cword, 2);
        assert_eq!(s.prefix, "node");
    }

    #[test]
    fn a_cursor_after_a_space_is_completing_a_word_that_does_not_exist_yet() {
        // The common case — Tab pressed to ask "what can go here?" — and the one an empty
        // word exists for: without it a caller cannot tell "anything" from "nothing".
        let s = at_end("mise use ");
        assert_eq!(s.words, ["mise", "use", ""]);
        assert_eq!(s.cword, 2);
        assert_eq!(s.prefix, "");
    }

    #[test]
    fn a_cursor_inside_a_word_completes_that_word_and_keeps_the_rest_of_the_line() {
        // `mise us|e node` — the tail is still part of the line, because the parser walking
        // left to right may need it, but the word being completed is `us`.
        let line = "mise use node";
        let s = split(line, 7, Shell::Bash);
        assert_eq!(s.words, ["mise", "use", "node"]);
        assert_eq!(s.cword, 1);
        assert_eq!(s.prefix, "us");
        assert_eq!(s.walked(), ["mise", "use"]);
    }

    #[test]
    fn a_quoted_space_stays_inside_its_word() {
        // The reason for taking a line rather than the shell's own split: by the time a shell
        // has split, the quote that made this one word is gone.
        let s = at_end(r#"mise run "my task"#);
        assert_eq!(s.words, ["mise", "run", "my task"]);
        assert_eq!(s.prefix, "my task");

        let s = at_end("mise run 'my task");
        assert_eq!(s.words, ["mise", "run", "my task"]);
        assert_eq!(s.prefix, "my task");
    }

    #[test]
    fn an_empty_quote_is_a_word() {
        let s = at_end(r#"mise run "" "#);
        assert_eq!(s.words, ["mise", "run", "", ""]);
        assert_eq!(s.cword, 3);
    }

    #[test]
    fn a_backslash_escapes_the_character_after_it() {
        let s = at_end(r"mise run my\ task");
        assert_eq!(s.words, ["mise", "run", "my task"]);

        // And a trailing one escapes the character not yet typed, rather than being an error:
        // the user is mid-word, which is the only state this function is ever called in.
        let s = at_end(r"mise run my\");
        assert_eq!(s.words, ["mise", "run", "my"]);
        assert_eq!(s.prefix, "my");
    }

    #[test]
    fn a_single_quote_keeps_a_backslash_literal() {
        // Which is what makes a Windows path survive being completed.
        let s = at_end(r"mise use 'C:\Users\me");
        assert_eq!(s.words, ["mise", "use", r"C:\Users\me"]);

        // In double quotes a backslash escapes only what it could mean something to, so the
        // same path survives there too.
        let s = at_end(r#"mise use "C:\Users\me"#);
        assert_eq!(s.words, ["mise", "use", r"C:\Users\me"]);

        let s = at_end(r#"mise use "say \"hi"#);
        assert_eq!(s.words, ["mise", "use", r#"say "hi"#]);
    }

    #[test]
    fn powershell_escapes_with_a_backtick() {
        let s = split("mise run my` task", 17, Shell::PowerShell);
        assert_eq!(s.words, ["mise", "run", "my task"]);

        // And a backslash is an ordinary character there, which is the point: a path typed in
        // PowerShell is full of them.
        let s = split(r"mise use C:\Users\me", 20, Shell::PowerShell);
        assert_eq!(s.words, ["mise", "use", r"C:\Users\me"]);
    }

    #[test]
    fn a_cursor_in_a_gap_completes_a_word_that_is_not_there_yet() {
        // `mise ⌶use` asks what can go *before* `use` — a word the line does not contain. It
        // has to be made, or the question is answered about `use`, which is a different one.
        let s = split("mise use", 5, Shell::Bash);
        assert_eq!(s.words, ["mise", "", "use"]);
        assert_eq!(s.cword, 1);
        assert_eq!(s.prefix, "");
        assert_eq!(s.walked(), ["mise", ""]);

        // Same in a run of spaces, where there is no character at the cursor either way.
        let s = split("mise   use", 6, Shell::Bash);
        assert_eq!(s.words, ["mise", "", "use"]);
        assert_eq!(s.cword, 1);
    }

    #[test]
    fn a_cursor_on_an_escaped_character_is_still_in_its_word() {
        // `mise run my\ ⌶task` — the cursor sits on the character the escape swallowed, which
        // the loop never stops at on its own. Missing it made the split describe the last word
        // of the line rather than the one being typed.
        // The backslash is at 11 and the space it escapes at 12, which is where the cursor is.
        let line = r"mise run my\ task and more";
        let s = split(line, 12, Shell::Bash);
        assert_eq!(s.cword, 2);
        assert_eq!(s.prefix, "my");
        assert_eq!(s.words, ["mise", "run", "my task", "and", "more"]);

        // The same inside double quotes, where the escape applies to the quote it precedes.
        let line = r#"mise run "say \"hi" then"#;
        let s = split(line, 15, Shell::Bash);
        assert_eq!(s.cword, 2);
        assert_eq!(s.prefix, "say ");
    }

    #[test]
    fn powershell_writes_a_quote_by_doubling_it() {
        // Documented as PowerShell's rule, and now done: `''` inside a quoted string is one
        // quote rather than the end of the string and the start of another.
        let s = split("mise run 'it''s here", 20, Shell::PowerShell);
        assert_eq!(s.words, ["mise", "run", "it's here"]);
        assert_eq!(s.prefix, "it's here");

        let s = split(r#"mise run "say ""hi"#, 18, Shell::PowerShell);
        assert_eq!(s.words, ["mise", "run", r#"say "hi"#]);

        // And not POSIX's rule, in either quote: there a second quote ends the string, so
        // `'it''s` is two pieces of one word rather than an escaped quote.
        let s = split("mise run 'it''s", 15, Shell::Bash);
        assert_eq!(s.words, ["mise", "run", "its"]);
        let s = split(r#"mise run "it""s"#, 15, Shell::Bash);
        assert_eq!(s.words, ["mise", "run", "its"]);
    }

    #[test]
    fn an_empty_line_is_still_completing_something() {
        let s = at_end("");
        assert_eq!(s.words, [""]);
        assert_eq!(s.cword, 0);
        assert_eq!(s.prefix, "");
        assert_eq!(s.walked(), [""]);
    }

    #[test]
    fn a_cursor_off_the_end_or_inside_a_character_lands_somewhere_sensible() {
        // A shell's idea of a byte offset is not always ours — nushell counts spans, and a
        // reconstructed line can be a byte or two off. Being wrong is better than panicking in
        // a keystroke handler.
        let s = split("mise use", 999, Shell::Bash);
        assert_eq!(s.prefix, "use");

        // `ü` is two bytes, at 5 and 6, so 7 is the boundary after it and 6 is inside it.
        let line = "mise ünicode";
        let s = split(line, 7, Shell::Bash);
        assert_eq!(s.cword, 1);
        assert_eq!(s.prefix, "ü");
        // Mid-character, floored to the start of the `ü` rather than splitting it.
        let s = split(line, 6, Shell::Bash);
        assert_eq!(s.cword, 1);
        assert_eq!(s.prefix, "");
    }
    #[test]
    fn a_word_that_could_name_a_command_offers_the_commands() {
        assert_eq!(
            offered("mise "),
            // `node` and `python` are the fallback command's, which the root offers too — see
            // `the_root_offers_what_the_command_it_falls_back_to_accepts`.
            [
                "edit", "exec", "list", "ls", "node", "pipe", "plugins", "python", "ship", "task",
                "u", "use", "wrap",
            ],
            "sorted, and a hidden command is offered under none of its names"
        );
        assert_eq!(offered("mise pl"), ["plugins"]);
        // An alias is a name someone types meaning the command, so it is offered too — but
        // not a hidden one, which the parser answers to and nothing advertises.
        assert_eq!(offered("mise l"), ["list", "ls"]);
        assert_eq!(offered("mise plugins "), ["ls"]);
    }

    #[test]
    fn a_dash_offers_the_flags_that_would_be_accepted_there() {
        // Both forms for a lone dash, since it says nothing about which was meant.
        assert_eq!(offered("mise -"), ["--quiet", "--verbose", "-v"]);
        assert_eq!(offered("mise --"), ["--quiet", "--verbose"]);

        // Inside a subcommand: its own flags and the globals it inherits, which is exactly
        // what the parser would accept there.
        assert_eq!(
            offered("mise use --"),
            ["--jobs", "--quiet", "--tools", "--verbose"]
        );
        // A short token asks about one letter, because bundling means any other letter would
        // be a candidate for a different position in the token.
        assert_eq!(offered("mise use -v"), ["-v"]);
        assert!(
            offered("mise use -j").is_empty(),
            "--jobs has no short form"
        );
    }

    #[test]
    fn a_flag_waiting_for_its_value_offers_that_flags_choices() {
        assert_eq!(offered("mise use --jobs "), ["1", "2", "4"]);
        assert_eq!(offered("mise use --jobs 2"), ["2"]);
        // And nothing else: the subcommands and the positional are not candidates for a
        // value that a flag has already claimed.
        assert!(!offered("mise use --jobs ").contains(&"node".to_string()));
    }

    #[test]
    fn a_positional_offers_its_choices() {
        assert_eq!(offered("mise use "), ["node", "python"]);
        assert_eq!(offered("mise use p"), ["python"]);
    }

    #[test]
    fn past_a_separator_a_dash_is_not_a_flag() {
        // `--` stops flag interpretation, so there is no flag to offer — the word there is a
        // value, and the only candidates are what a value could be.
        assert!(offered("mise use -- -").is_empty());
        // The fallback command's values are still offered past a separator: `--` says the
        // word is not a flag, not that it is not a word.
        assert_eq!(
            offered("mise -- "),
            [
                "edit", "exec", "list", "ls", "node", "pipe", "plugins", "python", "ship", "task",
                "u", "use", "wrap",
            ]
        );
    }

    #[test]
    fn a_candidate_carries_the_help_a_page_would_print() {
        let found = candidates(&SPEC, &at_end("mise use --"));
        let jobs = found.iter().find(|c| c.value == "--jobs").expect("--jobs");
        assert_eq!(jobs.description, Some("How many at once"));

        let found = candidates(&SPEC, &at_end("mise pl"));
        assert_eq!(found[0].description, Some("Manage plugins"));
    }
    #[test]
    fn a_help_topic_offers_the_commands_under_it() {
        // The candidate half of the same rule: after `mise help plugins ⌶` the useful answer
        // is what can be read about under `plugins`, and nothing else belongs there.
        assert_eq!(offered("mise help plugins "), ["ls"]);
        // And nothing the root would fall back to: `mise help ⌶` is asking for a command to
        // read about, not for a word to run.
        assert_eq!(
            offered("mise help "),
            ["edit", "exec", "list", "ls", "pipe", "plugins", "ship", "task", "u", "use", "wrap"]
        );
    }
    #[test]
    fn a_negation_is_offered_the_way_it_is_typed() {
        // The table holds it without dashes, because that is what the parser matches once a
        // token's `--` has been taken off. A candidate is what the user types, so it needs
        // them back — offered bare it matched no `--` prefix and no shell would accept it.
        assert!(offered("mise --").contains(&"--quiet".to_string()));
        assert_eq!(offered("mise --q"), ["--quiet"]);
    }

    #[test]
    fn a_restart_token_puts_the_cursor_back_at_the_first_argument() {
        // `mise task one ::: ⌶` starts a fresh invocation of the same command, so the words
        // before the token do not decide what comes after it: the cursor is back at the first
        // argument, not at the second one the words had reached.
        assert_eq!(offered("mise task one "), ["alpha", "beta"]);
        assert_eq!(offered("mise task one ::: "), ["one", "two"]);
        assert_eq!(offered("mise task one ::: t"), ["two"]);
    }

    #[test]
    fn the_root_offers_what_the_command_it_falls_back_to_accepts() {
        // `mise build` means `mise run build`, so a word at the root may be meant for the
        // default subcommand — and what its first argument accepts belongs here too, beside
        // the subcommand names themselves.
        let found = offered("mise ");
        assert!(found.contains(&"use".to_string()), "{found:?}");
        assert!(found.contains(&"node".to_string()), "{found:?}");
        assert!(found.contains(&"python".to_string()), "{found:?}");

        // Filtered like everything else, and not offered inside a subcommand: the fallback is
        // the root's rule.
        assert_eq!(offered("mise n"), ["node"]);
        assert!(!offered("mise plugins ").contains(&"node".to_string()));
    }
    #[test]
    fn an_argument_that_needs_a_separator_offers_the_separator() {
        // `mise wrap ⌶` cannot take `red` yet — the parser rejects every value until a `--`
        // has been typed — so offering the choices would be offering words that do not work.
        assert_eq!(offered("mise wrap "), ["--"]);
        // Nothing once something has been typed: the separator no longer matches, and the
        // values are still not fillable.
        assert!(offered("mise wrap r").is_empty());
        // Past the separator they are.
        assert_eq!(offered("mise wrap -- "), ["blue", "red"]);
        assert_eq!(offered("mise wrap -- r"), ["red"]);
    }

    #[test]
    fn the_fallback_command_is_found_by_any_name_it_answers_to() {
        // `default_subcommand` names a command, and a spec may name it the way its author
        // refers to it — here `u` rather than `use`. Resolving only canonical names silently
        // dropped the fallback's values.
        let found = offered("mise ");
        assert!(found.contains(&"node".to_string()), "{found:?}");
    }
    /// The whole answer for a line: what this CLI knows, and whether paths belong.
    fn answer(line: &str) -> Completions<'static> {
        complete(&SPEC, &at_end(line))
    }

    #[test]
    fn a_word_named_like_a_path_asks_the_shell_for_paths() {
        // The reference resolves a completer by the lowercased name and treats that name as the
        // type when nothing else says otherwise, so `<FILE>` completes files without the spec
        // mentioning it. Same rule, read off the same name.
        assert_eq!(answer("mise edit ").files, Some(Files::Any));
        // Even though it has choices of its own, which would otherwise close the position: a
        // path argument that names two well-known files still takes any other path.
        assert_eq!(offered("mise edit "), ["mise.local.toml", "mise.toml"]);
        // And a flag's value is named by its placeholder, not by the flag.
        assert_eq!(answer("mise edit --into ").files, Some(Files::Dirs));
    }

    #[test]
    fn a_position_that_knows_its_answers_does_not_ask_for_paths() {
        // Offering the working directory beside a known set is how a mistyped choice completes
        // to whatever happened to be lying around.
        assert_eq!(answer("mise use ").files, None);
        assert_eq!(answer("mise plugins ").files, None);
        // Nor for a flag, which no path starts with — including one that matches nothing, where
        // there is otherwise no candidate to suppress the fallback.
        assert_eq!(answer("mise use --").files, None);
        assert_eq!(answer("mise -").files, None);
        // And a prefix that matches none of a declared set: the set is still the whole answer,
        // so "nothing matched what you typed" must not become "here is the working directory".
        let a = answer("mise use nodx");
        assert!(a.candidates.is_empty());
        assert_eq!(a.files, None);
        // A mistyped *command*, though, does fall back — nothing declared a set there, and the
        // reference offers paths for the same reason.
        assert_eq!(answer("mise plugni").files, Some(Files::Any));

        let a = answer("mise use --zzz");
        assert!(a.candidates.is_empty());
        assert_eq!(a.files, None);
        // Nor for a help topic, which is a command name and nothing else.
        assert_eq!(answer("mise help ").files, None);
    }

    #[test]
    fn a_position_with_nothing_to_say_lets_the_shell_answer() {
        // `mise edit some-file ⌶` has filled its only argument and has no subcommands, so this
        // CLI has nothing to say — and a path is the shell's best guess, which is the
        // reference's fallback too.
        let a = answer("mise edit some-file ");
        assert!(a.candidates.is_empty(), "{:?}", a.candidates);
        assert_eq!(a.files, Some(Files::Any));
    }

    #[test]
    fn a_path_that_needs_a_separator_is_still_not_a_path_yet() {
        // Named `<PATH>`, so paths are what it takes — but not until the separator is typed,
        // because until then the parser rejects a path exactly as it rejects any other value.
        let a = answer("mise pipe ");
        assert_eq!(a.files, None);
        assert_eq!(a.candidates.len(), 1, "the separator, and nothing else");
        assert_eq!(a.candidates[0].value, "--");

        // Past it, they are.
        assert_eq!(answer("mise pipe -- ").files, Some(Files::Any));

        // And a flag waiting for its value is a different position that happens to have that
        // argument behind it: `--from ⌶` takes a path whatever the positional after it needs.
        assert_eq!(answer("mise pipe --from ").files, Some(Files::Any));
    }

    #[test]
    fn an_argument_that_needs_a_separator_asks_for_nothing_else() {
        // The separator is the only useful candidate, and a path is not one: the parser would
        // reject it until the `--` is typed.
        let a = answer("mise wrap ");
        assert_eq!(a.candidates.len(), 1);
        assert_eq!(a.files, None);
    }
    #[test]
    fn a_restart_asks_about_the_first_argument_for_paths_too() {
        // Both halves of an answer have to agree about which argument the cursor is at. `ship`
        // takes a `MODE` and then a `FILE`, so its two arguments want different things: the first
        // declares its whole set, the second takes any path.
        let first = answer("mise ship ");
        assert_eq!(first.files, None, "MODE declares its set");
        assert_eq!(
            first
                .candidates
                .iter()
                .map(|c| c.value.as_str())
                .collect::<Vec<_>>(),
            ["fast", "slow"]
        );
        assert_eq!(
            answer("mise ship fast ").files,
            Some(Files::Any),
            "FILE takes paths"
        );

        // Past the restart token the cursor is back at the first argument, whatever the words
        // before it filled — so a prefix matching none of `MODE`'s set means no matches, not
        // "here is the working directory".
        let after = answer("mise ship fast ::: ");
        assert_eq!(after.files, None, "back at MODE, which declares its set");
        assert_eq!(
            after
                .candidates
                .iter()
                .map(|c| c.value.as_str())
                .collect::<Vec<_>>(),
            ["fast", "slow"]
        );
        let mistyped = answer("mise ship fast ::: zzz");
        assert!(mistyped.candidates.is_empty());
        assert_eq!(mistyped.files, None, "a mistyped choice is still a choice");
    }

    #[test]
    fn each_shell_is_written_the_way_it_reads() {
        let answer = complete(&SPEC, &at_end("mise pl"));

        // bash shows values and nothing else.
        assert_eq!(render(&answer, Shell::Bash), "plugins\n");

        // fish, nu and PowerShell take a description after a tab.
        assert_eq!(render(&answer, Shell::Fish), "plugins\tManage plugins\n");

        // zsh takes a third field: what to type, which is not always what is shown.
        assert_eq!(
            render(&answer, Shell::Zsh),
            "plugins\tManage plugins\tplugins\n"
        );
    }

    #[test]
    fn a_candidate_a_shell_could_not_read_is_quoted_for_zsh() {
        static ODD: Command = Command {
            name: "with space",
            ..Command::EMPTY
        };
        static ODD_META: CommandMeta = CommandMeta {
            cmd: &ODD,
            about: Some("Odd"),
            ..CommandMeta::EMPTY
        };
        static ODD_ROOT: Command = Command {
            name: "ex",
            subcommands: &[&ODD],
            ..Command::EMPTY
        };
        static ODD_ROOT_META: CommandMeta = CommandMeta {
            cmd: &ODD_ROOT,
            subcommands: &[&ODD_META],
            ..CommandMeta::EMPTY
        };
        static ODD_SPEC: Spec = Spec {
            name: "ex",
            bin: Some("ex"),
            root: &ODD_ROOT_META,
            ..Spec::EMPTY
        };

        let answer = complete(&ODD_SPEC, &split("ex ", 3, Shell::Zsh));
        let line = render(&answer, Shell::Zsh);
        // Shown as it is, typed as the shell needs it.
        assert!(
            line.starts_with("with space\tOdd\t'with space'"),
            "{line:?}"
        );
    }

    #[test]
    fn the_marker_is_the_last_line_when_paths_belong() {
        let answer = complete(&SPEC, &at_end("mise edit "));
        let out = render(&answer, Shell::Bash);
        assert_eq!(out.lines().last(), Some(FILES_MARKER));
        // And the candidates are still there in front of it.
        assert!(out.starts_with("mise.local.toml\nmise.toml\n"), "{out:?}");

        let answer = complete(&SPEC, &at_end("mise edit --into "));
        assert_eq!(
            render(&answer, Shell::Fish).lines().last(),
            Some(DIRS_MARKER)
        );

        // And absent where paths do not belong, rather than saying "none".
        let answer = complete(&SPEC, &at_end("mise use "));
        let out = render(&answer, Shell::Bash);
        assert!(!out.contains('\u{1}'), "{out:?}");
    }
}
