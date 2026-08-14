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

use crate::{Arg, Command, Error, Flag, Parser};

/// Where the cursor is, in the grammar rather than in the line.
///
/// What can be typed at a cursor is decided by everything to its left: which command the words
/// reached, whether a flag is still waiting for its value, which positional comes next, and
/// whether flag interpretation is still on at all. This is that state, read off a real parse
/// of the words before the cursor rather than guessed at — so what is offered and what would
/// be accepted are decided by the same tables.
#[derive(Debug, Clone, Copy)]
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
                }
            }
            Err(_) => break,
        }
    }

    Position {
        cmd: parser.command(),
        flags_possible: !parser.flags_stopped(),
        awaiting_value,
        next_arg: parser.pending_arg(),
    }
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
    static USE: Command = Command {
        name: "use",
        flags: &[&JOBS],
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
}
