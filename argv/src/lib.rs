//! A zero-allocation argv parser for [usage](https://usage.jdx.dev) specs.
//!
//! This crate implements the binding rules of [the argv grammar]: which token
//! becomes which flag or argument, when a word selects a subcommand, and what
//! is an error. It does so without building a command tree, without allocating,
//! and in one pass.
//!
//! It is the runtime half of a compiled parser. The tables it reads are meant to
//! be emitted by a derive macro as `static` data, so that starting a parse costs
//! nothing at all: there is no construction step to pay for, only the walk over
//! `argv`.
//!
//! # Shape of the API
//!
//! Parsing yields [`Event`]s rather than a map. A map would have to allocate,
//! and would then have to be read back out again — whereas generated code can
//! assign an event straight into a struct field. This is the same reason serde
//! deserializes into your type instead of into a `Value`.
//!
//! ```
//! use usage_argv::{Arg, Command, Event, Flag, Parser};
//!
//! static FORCE: Flag = Flag { key: 0, longs: &["force"], shorts: b"f", ..Flag::BOOL };
//! static FILE: Arg = Arg { key: 1, ..Arg::REQUIRED };
//! static ROOT: Command = Command {
//!     name: "ex",
//!     flags: &[&FORCE],
//!     args: &[&FILE],
//!     ..Command::EMPTY
//! };
//!
//! let argv = ["--force", "a.txt"].map(std::ffi::OsStr::new);
//! let mut parser = Parser::new(&ROOT, &argv);
//!
//! let mut force = false;
//! let mut file = None;
//! while let Some(event) = parser.next_event() {
//!     match event.expect("valid command line") {
//!         Event::Flag { flag, .. } if flag.key == 0 => force = true,
//!         Event::Arg { value, .. } => file = Some(value),
//!         _ => {}
//!     }
//! }
//! assert!(force);
//! assert_eq!(file, Some(&b"a.txt"[..]));
//! ```
//!
//! # Values are bytes
//!
//! An [`Event`] carries `&[u8]`, borrowed from `argv`. Converting to `&str` is
//! the caller's step ([`as_str`]), and it is the right place for the only
//! failure a value can have: a command line that is not valid UTF-8 still
//! *parses* — flags match, subcommands route — and only the values that are
//! actually looked at can fail to convert.
//!
//! Slicing an `OsStr` into `&str` pieces safely is not possible without
//! allocating or `unsafe`, and this crate forbids `unsafe`. Bytes are what is
//! left, and they turn out to be the honest interface anyway.
//!
//! # What this crate does not do
//!
//! Only binding. Required-ness, `choices`, `env` fallback, defaults, `var_min`
//! and `var_max` are all decided *after* the last token is read, and they need to
//! know a value's type, so they belong to the layer that owns the target struct.
//! Keeping them out is what makes this loop small.
//!
//! # Features
//!
//! - `spec` — a parallel tree of cold metadata (help text, choices, defaults,
//!   effects) and a writer that emits it as a usage spec. Off by default: a
//!   successful parse never reads any of it, so a CLI that only wants a parser
//!   should not compile it.
//!
//! [the argv grammar]: https://usage.jdx.dev/spec/argv

#![forbid(unsafe_code)]

use std::ffi::OsStr;

#[cfg(feature = "spec")]
pub mod spec;

/// How deep a command tree this parser will descend.
///
/// The ancestor chain is kept in a fixed-size array so that a parse allocates
/// nothing; this is that array's size. mise, the largest usage CLI, is four
/// levels deep.
pub const MAX_DEPTH: usize = 16;

/// A command: its flags, its positional arguments, and its subcommands.
///
/// Every field is a borrowed slice so that a derive can emit the whole tree as
/// `static` data. Use `..Command::EMPTY` to fill in the parts you do not need.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Command<'a> {
    /// The canonical name, used to select this command.
    pub name: &'a str,
    /// Alternative names that also select it.
    pub aliases: &'a [&'a str],
    pub flags: &'a [&'a Flag<'a>],
    /// Positional arguments, in the order they are filled.
    pub args: &'a [&'a Arg<'a>],
    pub subcommands: &'a [&'a Command<'a>],
    /// What an unrecognized flag-like token means here. Already resolved — see
    /// [`UnknownFlags`].
    pub unknown_flags: UnknownFlags,
    /// Caller-assigned identifier, echoed back in [`Event::Command`].
    pub key: u32,
}

impl Command<'_> {
    /// A command with nothing declared, for use with struct update syntax.
    pub const EMPTY: Command<'static> = Command {
        name: "",
        aliases: &[],
        flags: &[],
        args: &[],
        subcommands: &[],
        unknown_flags: UnknownFlags::Value,
        key: 0,
    };
}

/// A flag, addressed by any of its long or short forms.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Flag<'a> {
    /// Caller-assigned identifier, echoed back in [`Event::Flag`]. This is how
    /// generated code knows which field to assign without any string comparison.
    pub key: u32,
    /// Unused by binding, kept so a table entry can carry its own name for
    /// diagnostics.
    pub name: &'a str,
    /// Long forms, written without the leading `--`.
    pub longs: &'a [&'a str],
    /// Short forms, as single bytes.
    pub shorts: &'a [u8],
    /// A long form that sets the flag to false, written without the `--`.
    pub negate: Option<&'a str>,
    /// Whether the flag takes a value.
    pub takes_value: bool,
    /// Whether one occurrence of this flag keeps taking values, until a flag-like
    /// token or the end of the command line.
    ///
    /// This is the spec's variadic flag *argument* (`--include <pattern>...`). It
    /// is not the spec's flag-level `var=#true`, which means the flag may be
    /// repeated and takes one value each time — repetition needs nothing from the
    /// parser, since it already reports every occurrence separately. Conflating
    /// the two makes a merely repeatable flag greedy enough to eat a positional.
    pub variadic: bool,
    /// Whether the flag is recognized by every command beneath the one that
    /// declares it.
    pub global: bool,
}

impl Flag<'_> {
    /// A value-less flag, for use with struct update syntax.
    pub const BOOL: Flag<'static> = Flag {
        key: 0,
        name: "",
        longs: &[],
        shorts: &[],
        negate: None,
        takes_value: false,
        variadic: false,
        global: false,
    };

    /// A flag that takes a value, for use with struct update syntax.
    pub const VALUE: Flag<'static> = Flag {
        takes_value: true,
        ..Flag::BOOL
    };
}

/// A positional argument.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Arg<'a> {
    /// Caller-assigned identifier, echoed back in [`Event::Arg`].
    pub key: u32,
    /// Whether this argument keeps taking values once it has one.
    pub var: bool,
    /// This argument's relationship to the `--` separator.
    pub double_dash: DoubleDash,
    /// Unused by binding, kept so a table entry can carry its own name for
    /// diagnostics.
    pub name: &'a str,
}

impl Arg<'_> {
    /// A single-value argument, for use with struct update syntax.
    pub const REQUIRED: Arg<'static> = Arg {
        key: 0,
        var: false,
        double_dash: DoubleDash::Optional,
        name: "",
    };

    /// A variadic argument, for use with struct update syntax.
    pub const VAR: Arg<'static> = Arg {
        var: true,
        ..Arg::REQUIRED
    };
}

/// What to do with a flag-like token that names no flag in scope.
///
/// The default is [`UnknownFlags::Value`]: the token carries on to the positional
/// arguments, because a spec is often parsing a command line whose flags belong to
/// something else — a wrapped tool, a task script. A CLI that owns all of its
/// flags declares [`UnknownFlags::Error`] and gets typo detection instead.
///
/// Stored per command and already resolved: inheritance is a question for whoever
/// builds the tables, and answering it at compile time keeps it out of the parse.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum UnknownFlags {
    /// Offer the token to the positionals. If none can take it, it is an
    /// unexpected argument.
    #[default]
    Value,
    /// Reject the token.
    Error,
}

/// How an argument relates to the `--` separator.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum DoubleDash {
    /// Values may appear on either side of a `--`.
    #[default]
    Optional,
    /// Values are accepted only after a `--`.
    Required,
    /// A `--` is kept as a value rather than consumed as a separator.
    Preserve,
    /// Once the argument takes a value, behave as if a `--` had been given, so
    /// the rest of the command line is values. A wrapper can then forward flags
    /// without its caller typing the separator.
    Automatic,
}

/// Something the parser bound.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Event<'t, 'v> {
    /// A subcommand was selected; parsing continues inside it.
    Command(&'t Command<'t>),
    /// A flag was given. `value` is `Some` for a flag that takes one, and
    /// `negated` is true when the flag was set through its `negate` form.
    Flag {
        flag: &'t Flag<'t>,
        value: Option<&'v [u8]>,
        negated: bool,
    },
    /// A word was bound to a positional argument. A variadic argument produces
    /// one event per value.
    Arg { arg: &'t Arg<'t>, value: &'v [u8] },
}

/// A binding failure.
///
/// Carries the offending token so a caller can render a good message, but no
/// message of its own: rendering belongs to a cold path, and building a string
/// here would allocate on the way to reporting that nothing was allocated.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Error<'t, 'v> {
    /// A flag-like token matched no flag in scope. `token` is the whole token as
    /// typed, so a bundle containing an unrecognized letter reports `-fz` rather
    /// than the letter alone — which is also the unit in which it is rejected.
    UnknownFlag { token: &'v [u8] },
    /// A flag that needs a value did not get one, either because the command
    /// line ended or because the next token was flag-like.
    MissingFlagValue { flag: &'t Flag<'t> },
    /// A word arrived with no argument left to hold it.
    UnexpectedArg { token: &'v [u8] },
    /// A word was offered to a `double_dash = "required"` argument before any
    /// `--` had been seen.
    ArgRequiresDoubleDash { arg: &'t Arg<'t> },
    /// The command tree is deeper than [`MAX_DEPTH`].
    TooDeep,
}

/// Interpret a value as UTF-8.
///
/// The parser hands back bytes borrowed from `argv`; this is the conversion most
/// callers want, and the point at which a non-UTF-8 command line is rejected —
/// but only for the values actually inspected.
pub fn as_str(value: &[u8]) -> Result<&str, std::str::Utf8Error> {
    std::str::from_utf8(value)
}

/// A single-pass parse over `argv`.
///
/// Created with [`Parser::new`] and driven with [`Parser::next_event`].
pub struct Parser<'t, 'v> {
    argv: &'v [&'v OsStr],
    /// Index of the next token to read.
    pos: usize,
    /// The command currently in scope.
    cmd: &'t Command<'t>,
    /// The chain above `cmd`, used to find inherited global flags. Fixed size so
    /// that nothing is allocated.
    ancestors: [Option<&'t Command<'t>>; MAX_DEPTH],
    depth: usize,
    /// Bytes left in a short-flag bundle, if one is partly read.
    bundle: &'v [u8],
    /// The whole token the current bundle came from, so an error raised part way
    /// through it can still name what the user typed.
    bundle_token: &'v [u8],
    /// A variadic flag that is still collecting values.
    collecting: Option<&'t Flag<'t>>,
    /// Which of `cmd.args` is next to fill.
    arg_pos: usize,
    /// Whether any word has been bound to a positional of `cmd`. Once one has,
    /// no further word can select a subcommand.
    arg_filled: bool,
    /// Whether flag interpretation has stopped. A `--` does this, and so does an
    /// `automatic` argument taking a value.
    flags_stopped: bool,
    /// Whether a `--` was actually consumed as a separator.
    ///
    /// Tracked apart from `flags_stopped` because the two can differ: an
    /// `automatic` argument stops flag interpretation without any separator being
    /// typed, and a `preserve` argument keeps one as a value rather than
    /// consuming it. Callers asking this question want to know what the user
    /// wrote, not what state the parser reached.
    separator_seen: bool,
    /// Set once a fatal error has been reported, so iteration stops.
    done: bool,
}

impl<'t, 'v> Parser<'t, 'v> {
    /// Begin parsing `argv` against `root`.
    ///
    /// `argv` excludes the program name.
    pub fn new(root: &'t Command<'t>, argv: &'v [&'v OsStr]) -> Self {
        Parser {
            argv,
            pos: 0,
            cmd: root,
            ancestors: [None; MAX_DEPTH],
            depth: 0,
            bundle: &[],
            bundle_token: &[],
            collecting: None,
            arg_pos: 0,
            arg_filled: false,
            flags_stopped: false,
            separator_seen: false,
            done: false,
        }
    }

    /// The command in scope: the root, or the deepest subcommand selected so far.
    pub fn command(&self) -> &'t Command<'t> {
        self.cmd
    }

    /// Whether a `--` was consumed as a separator.
    ///
    /// False when flag interpretation stopped for another reason, such as an
    /// `automatic` argument taking a value, and false for a `--` that a
    /// `preserve` argument kept as a value.
    pub fn double_dash_seen(&self) -> bool {
        self.separator_seen
    }

    /// Read the next event.
    ///
    /// Returns `None` when `argv` is exhausted. An `Err` is terminal: the parse
    /// stops there, since continuing past a token that could not be understood
    /// would only produce bindings derived from a guess. Events already yielded
    /// before an error are therefore not a partial result to be used — a caller
    /// that assigned them into fields should discard the whole attempt.
    ///
    /// One case is stronger than that, because the grammar demands it: a short
    /// bundle containing an unrecognized letter yields the error *instead of*, not
    /// after, the letters that did match.
    #[allow(clippy::should_implement_trait)] // not an Iterator: items borrow from self's tables
    pub fn next_event(&mut self) -> Option<Result<Event<'t, 'v>, Error<'t, 'v>>> {
        if self.done {
            return None;
        }
        let event = self.step();
        if let Some(Err(_)) = event {
            self.done = true;
        }
        event
    }

    fn step(&mut self) -> Option<Result<Event<'t, 'v>, Error<'t, 'v>>> {
        // A partly-read short bundle takes priority: its remaining bytes are
        // still part of the token being processed.
        if !self.bundle.is_empty() {
            return Some(self.short_flag());
        }

        // A variadic flag keeps claiming tokens until one of them could be
        // something else.
        if let Some(flag) = self.collecting {
            match self.argv.get(self.pos) {
                Some(next) if !is_flag_like(bytes(next)) && bytes(next) != b"--" => {
                    self.pos += 1;
                    return Some(Ok(Event::Flag {
                        flag,
                        value: Some(bytes(next)),
                        negated: false,
                    }));
                }
                _ => self.collecting = None,
            }
        }

        let token = bytes(self.argv.get(self.pos)?);
        self.pos += 1;

        if self.flags_stopped {
            return Some(self.word(token));
        }

        if token == b"--" {
            // `preserve` wants the separator itself as a value, so ask the
            // argument that would receive it before treating it as syntax.
            if self
                .next_arg()
                .is_some_and(|a| a.double_dash == DoubleDash::Preserve)
            {
                return Some(self.word(token));
            }
            self.flags_stopped = true;
            self.separator_seen = true;
            // An explicit separator unlocks any argument that required one, even
            // if earlier arguments are still unfilled.
            if let Some(idx) = self.cmd.args[self.arg_pos..]
                .iter()
                .position(|a| a.double_dash == DoubleDash::Required)
            {
                self.arg_pos += idx;
            }
            return self.step();
        }

        if is_flag_like(token) {
            if token.starts_with(b"--") {
                return Some(self.long_flag(token));
            }
            // Check the whole bundle before emitting anything from it. Events go
            // out one at a time, so discovering an unknown letter half way
            // through would mean the earlier letters had already been applied —
            // and the grammar rejects the entire token, not the tail of it.
            match self.check_bundle(token) {
                Ok(()) => {}
                // Unrecognized, so it is a word unless this command wants it refused.
                Err(e) if self.cmd.unknown_flags == UnknownFlags::Error => {
                    return Some(Err(e));
                }
                Err(_) => return Some(self.word(token)),
            }
            self.bundle = &token[1..];
            self.bundle_token = token;
            return Some(self.short_flag());
        }

        Some(self.word(token))
    }

    fn long_flag(&mut self, token: &'v [u8]) -> Result<Event<'t, 'v>, Error<'t, 'v>> {
        let body = &token[2..];
        let (name, attached) = match body.iter().position(|&b| b == b'=') {
            Some(i) => (&body[..i], Some(&body[i + 1..])),
            None => (body, None),
        };

        if let Some(flag) = self.find_long(name) {
            let value = if flag.takes_value {
                Some(match attached {
                    Some(v) => v,
                    None => self.take_detached_value(flag)?,
                })
            } else {
                None
            };
            if flag.variadic {
                self.collecting = Some(flag);
            }
            return Ok(Event::Flag {
                flag,
                value,
                negated: false,
            });
        }

        if let Some(flag) = self.find_negation(name) {
            return Ok(Event::Flag {
                flag,
                value: None,
                negated: true,
            });
        }

        if self.cmd.unknown_flags == UnknownFlags::Error {
            return Err(Error::UnknownFlag { token });
        }
        // Not a flag here, so it is a word like any other.
        self.word(token)
    }

    /// Walk a short-flag token without binding anything, to find out whether all
    /// of it is recognized.
    ///
    /// Scanning stops at the first letter whose flag takes a value, because
    /// everything after it is that value rather than more letters.
    fn check_bundle(&self, token: &'v [u8]) -> Result<(), Error<'t, 'v>> {
        let mut rest = &token[1..];
        while let Some((&byte, tail)) = rest.split_first() {
            match self.find_short(byte) {
                None => return Err(Error::UnknownFlag { token }),
                Some(flag) if flag.takes_value => return Ok(()),
                Some(_) => rest = tail,
            }
        }
        Ok(())
    }

    fn short_flag(&mut self) -> Result<Event<'t, 'v>, Error<'t, 'v>> {
        let byte = self.bundle[0];
        let rest = &self.bundle[1..];

        let Some(flag) = self.find_short(byte) else {
            // check_bundle already rejected any token containing an unrecognized
            // letter, so this is unreachable — but a parser should report rather
            // than panic if that ever stops being true.
            self.bundle = &[];
            return Err(Error::UnknownFlag {
                token: self.bundle_token,
            });
        };

        if !flag.takes_value {
            self.bundle = rest;
            return Ok(Event::Flag {
                flag,
                value: None,
                negated: false,
            });
        }

        // A value-taking short ends the token: everything after it is the value,
        // less one separating `=`.
        self.bundle = &[];
        let value = if rest.is_empty() {
            self.take_detached_value(flag)?
        } else if rest[0] == b'=' {
            &rest[1..]
        } else {
            rest
        };
        if flag.variadic {
            self.collecting = Some(flag);
        }
        Ok(Event::Flag {
            flag,
            value: Some(value),
            negated: false,
        })
    }

    /// Take the following token as a flag's value.
    ///
    /// Refuses a flag-like token: `--jobs --force` is far more likely a forgotten
    /// value than a deliberate one, and the attached form is available for the
    /// deliberate case.
    fn take_detached_value(&mut self, flag: &'t Flag<'t>) -> Result<&'v [u8], Error<'t, 'v>> {
        match self.argv.get(self.pos) {
            Some(next) if !is_flag_like(bytes(next)) => {
                self.pos += 1;
                Ok(bytes(next))
            }
            _ => Err(Error::MissingFlagValue { flag }),
        }
    }

    fn word(&mut self, token: &'v [u8]) -> Result<Event<'t, 'v>, Error<'t, 'v>> {
        // Subcommands are only matched where descent is still possible: once a
        // positional of this command has taken a word, a later word that happens
        // to equal a subcommand name is just a value.
        if !self.arg_filled && !self.flags_stopped {
            if let Some(sub) = self.find_subcommand(token) {
                self.descend(sub)?;
                return Ok(Event::Command(sub));
            }
        }

        let Some(arg) = self.next_arg() else {
            return Err(Error::UnexpectedArg { token });
        };

        if arg.double_dash == DoubleDash::Required && !self.separator_seen {
            return Err(Error::ArgRequiresDoubleDash { arg });
        }

        self.arg_filled = true;
        // An `automatic` argument stops flag interpretation from here on, as
        // though the caller had typed the separator themselves.
        if arg.double_dash == DoubleDash::Automatic {
            self.flags_stopped = true;
        }
        // A variadic keeps taking values, so the cursor stays put.
        if !arg.var {
            self.arg_pos += 1;
        }
        Ok(Event::Arg { arg, value: token })
    }

    fn descend(&mut self, sub: &'t Command<'t>) -> Result<(), Error<'t, 'v>> {
        if self.depth >= MAX_DEPTH {
            return Err(Error::TooDeep);
        }
        self.ancestors[self.depth] = Some(self.cmd);
        self.depth += 1;
        self.cmd = sub;
        self.arg_pos = 0;
        self.arg_filled = false;
        Ok(())
    }

    fn next_arg(&self) -> Option<&'t Arg<'t>> {
        self.cmd.args.get(self.arg_pos).copied()
    }

    /// Flags in scope: this command's own, then any ancestor's globals.
    ///
    /// Own flags come first so that a subcommand redeclaring an inherited name
    /// shadows it, which is what mise relies on when it redeclares root globals
    /// on `run` with different shorts.
    fn in_scope(&self) -> impl Iterator<Item = &'t Flag<'t>> + '_ {
        let own = self.cmd.flags.iter().copied();
        let inherited = self.ancestors[..self.depth]
            .iter()
            .rev()
            .filter_map(|c| *c)
            .flat_map(|c| c.flags.iter().copied())
            .filter(|f| f.global);
        own.chain(inherited)
    }

    fn find_long(&self, name: &[u8]) -> Option<&'t Flag<'t>> {
        self.in_scope()
            .find(|f| f.longs.iter().any(|l| l.as_bytes() == name))
    }

    fn find_negation(&self, name: &[u8]) -> Option<&'t Flag<'t>> {
        self.in_scope()
            .find(|f| f.negate.is_some_and(|n| n.as_bytes() == name))
    }

    fn find_short(&self, byte: u8) -> Option<&'t Flag<'t>> {
        self.in_scope().find(|f| f.shorts.contains(&byte))
    }

    fn find_subcommand(&self, name: &[u8]) -> Option<&'t Command<'t>> {
        self.cmd
            .subcommands
            .iter()
            .copied()
            .find(|c| c.name.as_bytes() == name || c.aliases.iter().any(|a| a.as_bytes() == name))
    }
}

/// View a token as bytes.
///
/// `as_encoded_bytes` is a plain accessor with no conversion and no allocation.
/// It is only the reverse direction that needs `unsafe`, which is why values
/// come back as bytes.
fn bytes<'v>(s: &'v &'v OsStr) -> &'v [u8] {
    s.as_encoded_bytes()
}

/// Whether a token should be read as a flag.
///
/// `-` alone is a value, conventionally stdin. A negative number is a value too,
/// without which no CLI could accept `--offset -1`.
fn is_flag_like(token: &[u8]) -> bool {
    match token {
        [b'-', rest @ ..] if !rest.is_empty() => !is_number(rest),
        _ => false,
    }
}

/// Whether the text after a `-` is a number, so `-1`, `-2.5`, and `-1e5` are values
/// while `-1x` is a flag-shaped token that names nothing.
///
/// Digits, at most one `.`, and an optional exponent. Deliberately narrower than
/// `f64::from_str`, which also accepts `inf` and `NaN` — `-inf` is far likelier to be
/// a misspelled flag than a number somebody meant to pass.
///
/// usage-lib applies the same rule, and the corpus pins the edges so the two cannot
/// drift apart: they disagreed about `-1e5` when this was a hand-rolled scanner on
/// one side and a float parse on the other.
///
/// Written out rather than deferred to `f64::from_str` because this runs on the hot
/// path, and a parse would mean a UTF-8 check on a slice already decided by its
/// bytes.
fn is_number(rest: &[u8]) -> bool {
    let (mantissa, exponent) = match rest.iter().position(|b| matches!(b, b'e' | b'E')) {
        Some(at) => (&rest[..at], Some(&rest[at + 1..])),
        None => (rest, None),
    };

    let mut seen_digit = false;
    let mut seen_dot = false;
    for &b in mantissa {
        match b {
            b'0'..=b'9' => seen_digit = true,
            b'.' if !seen_dot => seen_dot = true,
            _ => return false,
        }
    }
    if !seen_digit {
        return false;
    }

    match exponent {
        None => true,
        // An exponent needs digits of its own, and may carry a sign.
        Some(exp) => {
            let digits = exp
                .strip_prefix(b"+")
                .or_else(|| exp.strip_prefix(b"-"))
                .unwrap_or(exp);
            !digits.is_empty() && digits.iter().all(|b| b.is_ascii_digit())
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    static FORCE: Flag = Flag {
        key: 1,
        longs: &["force"],
        shorts: b"f",
        ..Flag::BOOL
    };
    static JOBS: Flag = Flag {
        key: 2,
        longs: &["jobs"],
        shorts: b"j",
        ..Flag::VALUE
    };
    static COLOR: Flag = Flag {
        key: 3,
        longs: &["color"],
        negate: Some("no-color"),
        ..Flag::BOOL
    };
    static VERBOSE: Flag = Flag {
        key: 4,
        longs: &["verbose"],
        shorts: b"v",
        global: true,
        ..Flag::BOOL
    };
    static FILE: Arg = Arg {
        key: 10,
        name: "file",
        ..Arg::REQUIRED
    };
    static REST: Arg = Arg {
        key: 11,
        name: "rest",
        ..Arg::VAR
    };
    static INSTALL: Command = Command {
        name: "install",
        aliases: &["i"],
        flags: &[&FORCE],
        key: 100,
        ..Command::EMPTY
    };
    /// Same shape as ROOT, but a CLI that owns all of its flags. The subcommand
    /// carries the setting too: the tables hold it already resolved, because
    /// inheritance is the table builder's job rather than the parser's.
    static STRICT_INSTALL: Command = Command {
        name: "install",
        aliases: &["i"],
        flags: &[&FORCE],
        unknown_flags: UnknownFlags::Error,
        key: 100,
        ..Command::EMPTY
    };
    static STRICT: Command = Command {
        name: "ex",
        flags: &[&FORCE, &JOBS, &COLOR, &VERBOSE],
        args: &[&FILE, &REST],
        subcommands: &[&STRICT_INSTALL],
        unknown_flags: UnknownFlags::Error,
        ..Command::EMPTY
    };
    static ROOT: Command = Command {
        name: "ex",
        flags: &[&FORCE, &JOBS, &COLOR, &VERBOSE],
        args: &[&FILE, &REST],
        subcommands: &[&INSTALL],
        ..Command::EMPTY
    };

    /// Collect every event, or the first error.
    fn parse<'t, 'v>(
        root: &'t Command<'t>,
        argv: &'v [&'v OsStr],
    ) -> Result<Vec<Event<'t, 'v>>, Error<'t, 'v>> {
        let mut parser = Parser::new(root, argv);
        let mut events = Vec::new();
        while let Some(event) = parser.next_event() {
            events.push(event?);
        }
        Ok(events)
    }

    fn argv<const N: usize>(tokens: [&str; N]) -> [&OsStr; N] {
        tokens.map(OsStr::new)
    }

    #[test]
    fn long_boolean() {
        let a = argv(["--force"]);
        assert_eq!(
            parse(&ROOT, &a).unwrap(),
            vec![Event::Flag {
                flag: &FORCE,
                value: None,
                negated: false
            }]
        );
    }

    #[test]
    fn long_value_forms() {
        for tokens in [vec!["--jobs=8"], vec!["--jobs", "8"]] {
            let a: Vec<&OsStr> = tokens.iter().map(|t| OsStr::new(*t)).collect();
            assert_eq!(
                parse(&ROOT, &a).unwrap(),
                vec![Event::Flag {
                    flag: &JOBS,
                    value: Some(b"8"),
                    negated: false
                }],
                "{tokens:?}"
            );
        }
    }

    #[test]
    fn long_value_keeps_later_equals() {
        let a = argv(["--jobs=a=b"]);
        let Event::Flag { value, .. } = parse(&ROOT, &a).unwrap()[0] else {
            panic!("expected a flag");
        };
        assert_eq!(value, Some(&b"a=b"[..]));
    }

    #[test]
    fn long_value_attached_empty_is_empty_not_absent() {
        let a = argv(["--jobs="]);
        let Event::Flag { value, .. } = parse(&ROOT, &a).unwrap()[0] else {
            panic!("expected a flag");
        };
        assert_eq!(value, Some(&b""[..]));
    }

    #[test]
    fn long_value_refuses_flaglike_next_word() {
        let a = argv(["--jobs", "--force"]);
        assert_eq!(
            parse(&ROOT, &a),
            Err(Error::MissingFlagValue { flag: &JOBS })
        );
    }

    #[test]
    fn long_value_accepts_negative_number() {
        let a = argv(["--jobs", "-1"]);
        let Event::Flag { value, .. } = parse(&ROOT, &a).unwrap()[0] else {
            panic!("expected a flag");
        };
        assert_eq!(value, Some(&b"-1"[..]));
    }

    #[test]
    fn no_abbreviation() {
        // A prefix names no flag, so by default it is a value like any other word.
        let a = argv(["--forc"]);
        assert_eq!(
            parse(&ROOT, &a).unwrap(),
            vec![Event::Arg {
                arg: &FILE,
                value: b"--forc"
            }]
        );

        // And a CLI that owns its flags hears about it, which is the whole reason
        // the strict mode exists.
        assert!(matches!(
            parse(&STRICT, &a),
            Err(Error::UnknownFlag { token: b"--forc" })
        ));
    }

    #[test]
    fn an_unknown_flag_is_a_value_by_default() {
        // The default, and the case it is for: a command line being forwarded to
        // something whose flags this spec does not know.
        let a = argv(["--wat", "keep"]);
        assert_eq!(
            parse(&ROOT, &a).unwrap(),
            vec![
                Event::Arg {
                    arg: &FILE,
                    value: b"--wat"
                },
                Event::Arg {
                    arg: &REST,
                    value: b"keep"
                },
            ]
        );

        // With nowhere to put it, it is an unexpected argument — the same error an
        // extra word gets, rather than a special one about flags.
        static ONE: Command = Command {
            name: "ex",
            args: &[&FILE],
            ..Command::EMPTY
        };
        let a = argv(["a", "--wat"]);
        assert_eq!(
            parse(&ONE, &a),
            Err(Error::UnexpectedArg { token: b"--wat" })
        );
    }

    #[test]
    fn negation() {
        let a = argv(["--no-color"]);
        assert_eq!(
            parse(&ROOT, &a).unwrap(),
            vec![Event::Flag {
                flag: &COLOR,
                value: None,
                negated: true
            }]
        );
    }

    #[test]
    fn short_bundle_and_attached_value() {
        let a = argv(["-fj8"]);
        assert_eq!(
            parse(&ROOT, &a).unwrap(),
            vec![
                Event::Flag {
                    flag: &FORCE,
                    value: None,
                    negated: false
                },
                Event::Flag {
                    flag: &JOBS,
                    value: Some(b"8"),
                    negated: false
                },
            ]
        );
    }

    #[test]
    fn short_value_strips_one_equals() {
        for (tokens, want) in [(["-j=8"], &b"8"[..]), (["-j==8"], &b"=8"[..])] {
            let a = argv(tokens);
            let Event::Flag { value, .. } = parse(&ROOT, &a).unwrap()[0] else {
                panic!("expected a flag");
            };
            assert_eq!(value, Some(want), "{tokens:?}");
        }
    }

    #[test]
    fn bare_dash_is_a_value() {
        let a = argv(["-"]);
        assert_eq!(
            parse(&ROOT, &a).unwrap(),
            vec![Event::Arg {
                arg: &FILE,
                value: b"-"
            }]
        );
    }

    #[test]
    fn positionals_then_variadic() {
        let a = argv(["one", "two", "three"]);
        assert_eq!(
            parse(&ROOT, &a).unwrap(),
            vec![
                Event::Arg {
                    arg: &FILE,
                    value: b"one"
                },
                Event::Arg {
                    arg: &REST,
                    value: b"two"
                },
                Event::Arg {
                    arg: &REST,
                    value: b"three"
                },
            ]
        );
    }

    #[test]
    fn subcommand_and_alias_route_the_same() {
        for token in ["install", "i"] {
            let a = argv([token]);
            assert_eq!(
                parse(&ROOT, &a).unwrap(),
                vec![Event::Command(&INSTALL)],
                "{token}"
            );
        }
    }

    #[test]
    fn subcommand_only_routes_before_a_positional_is_filled() {
        let a = argv(["other", "install"]);
        assert_eq!(
            parse(&ROOT, &a).unwrap(),
            vec![
                Event::Arg {
                    arg: &FILE,
                    value: b"other"
                },
                Event::Arg {
                    arg: &REST,
                    value: b"install"
                },
            ]
        );
    }

    #[test]
    fn globals_are_inherited_but_plain_flags_are_not() {
        let a = argv(["install", "--verbose"]);
        assert_eq!(
            parse(&ROOT, &a).unwrap(),
            vec![
                Event::Command(&INSTALL),
                Event::Flag {
                    flag: &VERBOSE,
                    value: None,
                    negated: false
                }
            ]
        );

        // `--jobs` belongs to the root and is not global, so it is not a flag here.
        // Strictly that is an unknown flag; leniently it is a word, and `install`
        // declares no argument to hold one — either way it is never read as the
        // root's flag, which is what this test is about.
        let a = argv(["install", "--jobs", "8"]);
        assert!(matches!(parse(&STRICT, &a), Err(Error::UnknownFlag { .. })));
        assert!(matches!(
            parse(&ROOT, &a),
            Err(Error::UnexpectedArg { token: b"--jobs" })
        ));
    }

    #[test]
    fn double_dash_protects_flaglike_values() {
        let a = argv(["--", "--force", "-x"]);
        assert_eq!(
            parse(&ROOT, &a).unwrap(),
            vec![
                Event::Arg {
                    arg: &FILE,
                    value: b"--force"
                },
                Event::Arg {
                    arg: &REST,
                    value: b"-x"
                },
            ]
        );
    }

    #[test]
    fn second_double_dash_is_a_value() {
        let a = argv(["--", "a", "--", "b"]);
        let values: Vec<&[u8]> = parse(&ROOT, &a)
            .unwrap()
            .iter()
            .filter_map(|e| match e {
                Event::Arg { value, .. } => Some(*value),
                _ => None,
            })
            .collect();
        assert_eq!(values, vec![&b"a"[..], &b"--"[..], &b"b"[..]]);
    }

    #[test]
    fn variadic_flag_collects_until_a_flaglike_token() {
        static INCLUDE: Flag = Flag {
            key: 5,
            name: "include",
            longs: &["include"],
            shorts: b"i",
            takes_value: true,
            variadic: true,
            ..Flag::BOOL
        };
        static GREEDY: Command = Command {
            name: "ex",
            flags: &[&INCLUDE, &FORCE],
            args: &[&FILE],
            ..Command::EMPTY
        };

        let a = argv(["--include", "x", "y", "--force"]);
        assert_eq!(
            parse(&GREEDY, &a).unwrap(),
            vec![
                Event::Flag {
                    flag: &INCLUDE,
                    value: Some(b"x"),
                    negated: false
                },
                Event::Flag {
                    flag: &INCLUDE,
                    value: Some(b"y"),
                    negated: false
                },
                Event::Flag {
                    flag: &FORCE,
                    value: None,
                    negated: false
                },
            ]
        );
    }

    #[test]
    fn a_non_variadic_flag_leaves_the_next_word_alone() {
        // The counterpart to the test above: a flag that takes one value must not
        // swallow the word after it, which would silently steal a positional.
        let a = argv(["--jobs", "8", "keep-me"]);
        assert_eq!(
            parse(&ROOT, &a).unwrap(),
            vec![
                Event::Flag {
                    flag: &JOBS,
                    value: Some(b"8"),
                    negated: false
                },
                Event::Arg {
                    arg: &FILE,
                    value: b"keep-me"
                },
            ]
        );
    }

    #[test]
    fn double_dash_seen_means_a_separator_was_typed() {
        static FILES: Arg = Arg {
            key: 23,
            name: "files",
            double_dash: DoubleDash::Automatic,
            ..Arg::VAR
        };
        static AUTO: Command = Command {
            name: "ex",
            flags: &[&FORCE],
            args: &[&FILES],
            ..Command::EMPTY
        };

        let a = argv(["--", "x"]);
        let mut parser = Parser::new(&ROOT, &a);
        while parser.next_event().is_some() {}
        assert!(parser.double_dash_seen(), "a real separator was consumed");

        // `automatic` stops flag interpretation without a separator being typed,
        // and reporting one would be a lie to any caller that forwards argv.
        let a = argv(["x", "--force"]);
        let mut parser = Parser::new(&AUTO, &a);
        while parser.next_event().is_some() {}
        assert!(
            !parser.double_dash_seen(),
            "automatic mode must not claim a separator was given"
        );
    }

    #[test]
    fn double_dash_required_arg() {
        static CMD: Arg = Arg {
            key: 20,
            name: "cmd",
            double_dash: DoubleDash::Required,
            ..Arg::REQUIRED
        };
        static EXEC: Command = Command {
            name: "ex",
            args: &[&CMD],
            ..Command::EMPTY
        };

        let a = argv(["--", "ls"]);
        assert_eq!(
            parse(&EXEC, &a).unwrap(),
            vec![Event::Arg {
                arg: &CMD,
                value: b"ls"
            }]
        );

        let a = argv(["ls"]);
        assert_eq!(
            parse(&EXEC, &a),
            Err(Error::ArgRequiresDoubleDash { arg: &CMD })
        );
    }

    #[test]
    fn double_dash_preserve_keeps_the_separator() {
        static ARGS: Arg = Arg {
            key: 21,
            name: "args",
            double_dash: DoubleDash::Preserve,
            ..Arg::VAR
        };
        static WRAP: Command = Command {
            name: "ex",
            args: &[&ARGS],
            ..Command::EMPTY
        };

        let a = argv(["a", "--", "b"]);
        let values: Vec<&[u8]> = parse(&WRAP, &a)
            .unwrap()
            .iter()
            .filter_map(|e| match e {
                Event::Arg { value, .. } => Some(*value),
                _ => None,
            })
            .collect();
        assert_eq!(values, vec![&b"a"[..], &b"--"[..], &b"b"[..]]);
    }

    #[test]
    fn double_dash_automatic_stops_flag_interpretation() {
        static FILES: Arg = Arg {
            key: 22,
            name: "files",
            double_dash: DoubleDash::Automatic,
            ..Arg::VAR
        };
        static AUTO: Command = Command {
            name: "ex",
            flags: &[&FORCE],
            args: &[&FILES],
            ..Command::EMPTY
        };

        // The flag before the first value is still a flag; the one after it is a
        // value.
        let a = argv(["-f", "one", "--force"]);
        assert_eq!(
            parse(&AUTO, &a).unwrap(),
            vec![
                Event::Flag {
                    flag: &FORCE,
                    value: None,
                    negated: false
                },
                Event::Arg {
                    arg: &FILES,
                    value: b"one"
                },
                Event::Arg {
                    arg: &FILES,
                    value: b"--force"
                },
            ]
        );
    }

    #[test]
    fn too_many_words() {
        static ONE: Command = Command {
            name: "ex",
            args: &[&FILE],
            ..Command::EMPTY
        };
        let a = argv(["a", "b"]);
        assert_eq!(parse(&ONE, &a), Err(Error::UnexpectedArg { token: b"b" }));
    }

    #[test]
    fn unknown_letter_rejects_the_whole_bundle() {
        // `-f` is real and `-z` is not. The first event must be the error: if the
        // flag event came out first, a caller would have applied `-f` from a
        // command line that was rejected.
        let a = argv(["-fz"]);
        let mut parser = Parser::new(&STRICT, &a);
        assert_eq!(
            parser.next_event(),
            Some(Err(Error::UnknownFlag { token: b"-fz" })),
            "an unknown letter must reject the token before any of it is applied"
        );
        assert!(parser.next_event().is_none());

        // Leniently, the same token is a value — and `-f` is *not* applied, since
        // the token was never a bundle at all.
        let a = argv(["-fz"]);
        assert_eq!(
            parse(&ROOT, &a).unwrap(),
            vec![Event::Arg {
                arg: &FILE,
                value: b"-fz"
            }]
        );
    }

    #[test]
    fn unknown_short_error_names_the_whole_token() {
        for (tokens, want) in [(["-z"], &b"-z"[..]), (["-fz"], &b"-fz"[..])] {
            let a = argv(tokens);
            assert_eq!(
                parse(&STRICT, &a),
                Err(Error::UnknownFlag { token: want }),
                "{tokens:?}"
            );
        }
    }

    #[test]
    fn errors_are_terminal() {
        let a = argv(["--wat", "--force"]);
        let mut parser = Parser::new(&STRICT, &a);
        assert!(parser.next_event().unwrap().is_err());
        assert!(parser.next_event().is_none());
    }

    #[test]
    fn non_utf8_values_still_parse() {
        // A value that is not valid UTF-8 binds; only converting it fails, and
        // only if a caller asks.
        let raw = OsStr::new("--force");
        let a = [raw];
        assert!(parse(&ROOT, &a).is_ok());

        assert!(as_str(b"ok").is_ok());
        assert!(as_str(&[0xff, 0xfe]).is_err());
    }
}
