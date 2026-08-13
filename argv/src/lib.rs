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
//! allocating or `unsafe`. Bytes are what is left, and they turn out to be the
//! honest interface anyway.
//!
//! The reverse conversion is [`os_string_from_bytes`], which lets a `PathBuf`
//! field hold a filename that is not UTF-8 rather than a mangled copy of one. On
//! Unix that is lossless and safe; on Windows, where WTF-8 makes it partial, a
//! value that will not convert is reported. Either way this crate contains no
//! `unsafe`, which a conversion that guessed would have cost.
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

use std::ffi::{OsStr, OsString};

#[cfg(feature = "spec")]
pub mod help;
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
    /// Where a word goes when it names no subcommand of this one.
    ///
    /// The spec's `default_subcommand`. `mise build` means `mise run build`: the word names
    /// no command, so the parser descends into `run` and lets *`run`* have it — even where
    /// this command declares an argument of its own, which is what makes the property worth
    /// having rather than a synonym for a positional.
    ///
    /// Applied at most once per parse, so a CLI cannot loop through it, and only where a
    /// subcommand could still be selected.
    ///
    /// Resolve it with [`find_subcommand`], which turns a name that no subcommand answers to
    /// into a compile error.
    pub default_subcommand: ::core::option::Option<&'a Command<'a>>,
    /// What an unrecognized flag-like token means here. Already resolved — see
    /// [`UnknownFlags`].
    pub unknown_flags: UnknownFlags,
    /// Caller-assigned identifier, echoed back in [`Event::Command`].
    ///
    /// Wide enough for a derive to make these unique without coordination: two
    /// macro expansions cannot see each other, so the generated keys carry a hash
    /// of the type they came from in the high half and a per-type index in the low
    /// half. A parse dispatches on this, so a collision would bind the wrong field
    /// — [`Spec::to_kdl`](crate::spec::Spec::to_kdl) checks the tree for duplicates
    /// in debug builds.
    pub key: u64,
}

impl Command<'_> {
    /// A command with nothing declared, for use with struct update syntax.
    pub const EMPTY: Command<'static> = Command {
        name: "",
        aliases: &[],
        flags: &[],
        args: &[],
        subcommands: &[],
        default_subcommand: ::core::option::Option::None,
        unknown_flags: UnknownFlags::Value,
        key: 0,
    };
}

/// A flag, addressed by any of its long or short forms.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Flag<'a> {
    /// Caller-assigned identifier, echoed back in [`Event::Flag`]. This is how
    /// generated code knows which field to assign without any string comparison.
    /// See [`Command::key`] on why it is this wide.
    pub key: u64,
    /// Unused by binding, kept so a table entry can carry its own name for
    /// diagnostics.
    pub name: &'a str,
    /// Long forms, written without the leading `--`.
    pub longs: &'a [&'a str],
    /// Short forms, as single bytes.
    ///
    /// **Should be ASCII.** A cluster like `-xyz` is walked one byte at a time, so a
    /// non-ASCII short can never be matched, and the remainder after a value-taking one —
    /// which becomes its value — would begin in the middle of a character.
    /// `#[derive(Cli)]` rejects a non-ASCII `short`; a table written by hand should keep to
    /// it. Nothing is unsound if it does not: the value would simply be cut in a place that
    /// makes no sense, and on Windows would then fail to convert.
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
    /// How many values one variadic occurrence may take, after which the next word
    /// belongs to whatever comes next.
    ///
    /// Only for [`variadic`](Self::variadic). A merely repeatable flag — the spec's
    /// `var=#true` — is bounded on how many times it was *given*, which no single token
    /// can decide, so that bound stays with the metadata and is checked after the parse.
    pub var_max: ::core::option::Option<u32>,
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
        var_max: ::core::option::Option::None,
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
    /// Caller-assigned identifier, echoed back in [`Event::Arg`]. See
    /// [`Command::key`] on why it is this wide.
    pub key: u64,
    /// Whether this argument keeps taking values once it has one.
    pub var: bool,
    /// How many words a variadic may take before the next argument gets the rest.
    ///
    /// A bound belongs here, in the table binding reads, rather than with the metadata:
    /// it decides *where* a word lands, not whether what landed is acceptable. clap's
    /// `num_args` works the same way, and every spec in the wild is generated from a clap
    /// command. `u32` rather than `usize` because a CLI that bounds a variadic above four
    /// billion has other problems, and this table is read on the hot path.
    pub var_max: ::core::option::Option<u32>,
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
        var_max: ::core::option::Option::None,
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
///
/// `non_exhaustive`, because an error enum grows: a caller matching on it needs a
/// fallback arm so that recognizing a new failure is never a breaking change.
// No `Copy`: one variant owns its message. `Clone` stays, and the enum is still 40 bytes
// because that variant is boxed, so nothing on the hot path grew.
#[derive(Debug, Clone, PartialEq, Eq)]
#[non_exhaustive]
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

    // The rest are raised *after* the parse, by whoever owns the target type: they
    // need to know a value's declared type, which the parser deliberately does not.
    // They share this enum so that a caller has one error to handle rather than two.
    /// Something the command requires was never given.
    MissingRequired {
        /// The flag or argument's name, as the spec calls it.
        name: &'t str,
    },
    /// A value was given that is not among the declared choices.
    ///
    /// Carries the choices rather than the offending value: rendering the value means
    /// owning it, and an error that allocates on a path this crate promises not to
    /// allocate on would be a poor trade for a better message. Diagnostics are a
    /// separate layer.
    InvalidChoice {
        name: &'t str,
        choices: &'t [&'t str],
    },
    /// Fewer values than `var_min`.
    VarTooFew {
        name: &'t str,
        min: usize,
        got: usize,
    },
    /// More values than `var_max`.
    VarTooMany {
        name: &'t str,
        max: usize,
        got: usize,
    },
    /// Two flags declared to conflict were both given.
    ///
    /// Carries both names because either one alone reads as a puzzle: which flag is
    /// unwelcome depends entirely on what else is on the command line.
    ConflictingFlags {
        /// The flag whose declaration names the conflict.
        name: &'t str,
        /// The flag it cannot be given with, as the declaration spells it.
        other: &'t str,
    },
    /// A value was given that the field's type could not be built from.
    ///
    /// Boxed, and the only error here that owns anything. Everything else borrows the
    /// tables or argv, which is what keeps a *successful* parse allocation-free — and the
    /// box keeps `Error` the size it was, so the `Result` this rides in on the hot path
    /// does not grow. A value that will not convert has already failed, and a message
    /// worth reading is worth one allocation.
    InvalidValue(::std::boxed::Box<InvalidValue<'t>>),
    /// A subcommand was required, and none was given.
    MissingSubcommand,
    /// `--help` or `-h` was given, and `cmd` is what it was asked about.
    ///
    /// Not a failure, and returned as one anyway: a parse that stops to print help has not
    /// produced a value, and every caller already handles the "no value" shape. clap does the
    /// same thing for the same reason.
    ///
    /// `long` distinguishes the two: `-h` prints the short form and `--help` the long one, as
    /// clap has them. The caller renders — this crate does not print, because a library that
    /// writes to stdout on its own is one an adopter cannot embed.
    Help { cmd: &'t Command<'t>, long: bool },
}

/// The high half of every key one declaration's items get.
///
/// A derive cannot see other expansions, so it cannot hand out keys from a shared
/// counter: it hashes the declaration it was given instead. It cannot see a module path
/// either, which is why the module is mixed in *here* — `module_path!()` is available to
/// the generated code as a compile-time string, so two byte-identical declarations in
/// different modules end up with different keys rather than colliding.
///
/// `declaration` is a hash the derive computed over the item's own tokens.
pub const fn key_base(module: &str, declaration: u32) -> u64 {
    // FNV-1a, continuing from the declaration's hash rather than starting over, so both
    // halves contribute. Spelled out rather than taken from a `Hasher`, which is not
    // guaranteed to be stable between compilations — and these are baked into a binary.
    let mut hash: u32 = declaration;
    let bytes = module.as_bytes();
    let mut i = 0;
    while i < bytes.len() {
        hash ^= bytes[i] as u32;
        hash = hash.wrapping_mul(0x0100_0193);
        i += 1;
    }
    (hash as u64) << 32
}

/// Why a value would not convert into the type its field holds.
///
/// Separate from [`Error`] so that the enum stays small: this is reached through a `Box`.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct InvalidValue<'t> {
    /// The flag or argument's name, as the spec calls it.
    pub name: &'t str,
    /// The text that would not convert.
    pub value: ::std::string::String,
    /// What the type's own conversion complained about.
    pub reason: ::std::string::String,
}

/// Interpret a value as UTF-8.
///
/// The parser hands back bytes borrowed from `argv`; this is the conversion most
/// callers want, and the point at which a non-UTF-8 command line is rejected —
/// but only for the values actually inspected.
pub fn as_str(value: &[u8]) -> Result<&str, std::str::Utf8Error> {
    std::str::from_utf8(value)
}

/// How many entries a group of tables holds in total.
///
/// The length for [`concat_flags`] and [`concat_args`], which need it as a const generic — so
/// it has to be computable separately from the concatenation itself.
///
/// ```
/// use usage_argv::{table_len, Flag};
///
/// static A: Flag = Flag { name: "a", ..Flag::BOOL };
/// static B: Flag = Flag { name: "b", ..Flag::BOOL };
/// const GROUPS: &[&[&Flag]] = &[&[&A], &[], &[&B]];
/// const N: usize = table_len(GROUPS);
/// assert_eq!(N, 2);
/// ```
pub const fn table_len<T>(groups: &[&[T]]) -> usize {
    let mut total = 0;
    let mut i = 0;
    while i < groups.len() {
        total += groups[i].len();
        i += 1;
    }
    total
}

/// Join groups of flag tables into one, at compile time.
///
/// This is how `#[usage(flatten)]` stays free. A flattened struct's flags have to appear in
/// the parent's own table, and the parent's macro expansion cannot see them — it has only a
/// type. But it can name that type's [`CommandArgs::COMMAND`](crate::spec::CommandArgs::COMMAND),
/// and a `const fn` can read through it, so the two lists become one `static` array before the
/// program runs. The parser then walks a single flat slice, exactly as it does for a command
/// that declared everything itself: flatten costs nothing at run time.
///
/// Groups are laid out in the order given, which is what lets a flattened group sit *between*
/// two of the parent's own declarations — necessary for positional arguments, where order is
/// the meaning.
///
/// `N` must be [`table_len`] of the same groups. It cannot be inferred, and a wrong one fails
/// to compile rather than leaving the difference filled with padding.
///
/// ```
/// use usage_argv::{concat_flags, table_len, Flag};
///
/// static FORCE: Flag = Flag { name: "force", longs: &["force"], ..Flag::BOOL };
/// static QUIET: Flag = Flag { name: "quiet", longs: &["quiet"], ..Flag::BOOL };
/// static SHARED: &[&Flag] = &[&QUIET];
///
/// const GROUPS: &[&[&Flag]] = &[&[&FORCE], SHARED];
/// static FLAGS: [&Flag; table_len(GROUPS)] = concat_flags(GROUPS);
///
/// assert_eq!(FLAGS.iter().map(|f| f.name).collect::<Vec<_>>(), ["force", "quiet"]);
/// ```
pub const fn concat_flags<const N: usize>(
    groups: &[&[&'static Flag<'static>]],
) -> [&'static Flag<'static>; N] {
    // Every slot is written below, but an array has to start somewhere and `MaybeUninit`
    // would mean `unsafe`. A `Flag` nobody can reach is cheaper than that.
    static PLACEHOLDER: Flag<'static> = Flag::BOOL;
    let mut out = [&PLACEHOLDER; N];
    let mut at = 0;
    let mut g = 0;
    while g < groups.len() {
        let group = groups[g];
        let mut i = 0;
        while i < group.len() {
            out[at] = group[i];
            at += 1;
            i += 1;
        }
        g += 1;
    }
    assert!(
        at == N,
        "`N` must be `table_len` of the same groups, or the table would keep a placeholder \
         that answers to nothing"
    );
    out
}

/// Join groups of argument tables into one, at compile time.
///
/// The positional counterpart of [`concat_flags`] — see there for why this exists. Order
/// matters more here: an argument's position *is* its identity, so a flattened group has to
/// land exactly where the field was written.
///
/// Two functions rather than one generic: each needs a value to fill an array with before
/// overwriting it, and there is no way to ask a type parameter for one in a `const fn`.
pub const fn concat_args<const N: usize>(
    groups: &[&[&'static Arg<'static>]],
) -> [&'static Arg<'static>; N] {
    static PLACEHOLDER: Arg<'static> = Arg::REQUIRED;
    let mut out = [&PLACEHOLDER; N];
    let mut at = 0;
    let mut g = 0;
    while g < groups.len() {
        let group = groups[g];
        let mut i = 0;
        while i < group.len() {
            out[at] = group[i];
            at += 1;
            i += 1;
        }
        g += 1;
    }
    assert!(
        at == N,
        "`N` must be `table_len` of the same groups, or the table would keep a placeholder \
         that answers to nothing"
    );
    out
}

/// The key `--help` answers to, and the one `-h` does.
///
/// Reserved rather than generated: a derive builds keys from a hash of the type they came from
/// in the high half and an index in the low half, so the top of the range belongs to nobody.
/// Generated code compares against these to tell a help request from a flag of its own.
pub const HELP_LONG_KEY: u64 = u64::MAX;
/// See [`HELP_LONG_KEY`].
pub const HELP_SHORT_KEY: u64 = u64::MAX - 1;

/// `--help`, which every command answers to.
///
/// In the parse table and *not* in the metadata, which is the whole trick: the parser has to
/// recognise the flag, and help output must not list it — a spec does not declare `--help`, so
/// showing one would make the rendered page disagree with the spec it came from.
pub static HELP_LONG: Flag<'static> = Flag {
    key: HELP_LONG_KEY,
    name: "help",
    longs: &["help"],
    ..Flag::BOOL
};

/// `-h`, which prints the shorter form.
pub static HELP_SHORT: Flag<'static> = Flag {
    key: HELP_SHORT_KEY,
    name: "help",
    shorts: b"h",
    ..Flag::BOOL
};

/// Whether a flag is one of the two the parser supplies rather than the CLI declaring it.
pub fn is_help_flag(flag: &Flag<'_>) -> bool {
    flag.key == HELP_LONG_KEY || flag.key == HELP_SHORT_KEY
}

/// Resolve a subcommand by name or alias, at compile time.
///
/// For [`Command::default_subcommand`], which names a command that a derive cannot see: the
/// variants of a subcommand enum are a different macro expansion, so the name is all the
/// parent has. Searching the list in a `const fn` closes that gap — the answer is the same
/// `&'static` the table already holds, found before the program runs.
///
/// A name no subcommand answers to is a **compile error**, since this panics during const
/// evaluation. That is the whole point of doing it here rather than at startup.
///
/// ```
/// use usage_argv::{find_subcommand, Command};
///
/// static RUN: Command = Command { name: "run", ..Command::EMPTY };
/// static SUBS: &[&Command] = &[&RUN];
/// static ROOT: Command = Command {
///     name: "ex",
///     subcommands: SUBS,
///     default_subcommand: Some(find_subcommand(SUBS, "run")),
///     ..Command::EMPTY
/// };
/// assert_eq!(ROOT.default_subcommand.unwrap().name, "run");
/// ```
pub const fn find_subcommand<'a>(
    subcommands: &'a [&'a Command<'a>],
    name: &str,
) -> &'a Command<'a> {
    let mut i = 0;
    while i < subcommands.len() {
        let candidate = subcommands[i];
        if str_eq(candidate.name, name) {
            return candidate;
        }
        // Aliases answer too, because usage-lib resolves the name against names, aliases and
        // hidden aliases alike — so a spec may point `default_subcommand` at any of them.
        let mut a = 0;
        while a < candidate.aliases.len() {
            if str_eq(candidate.aliases[a], name) {
                return candidate;
            }
            a += 1;
        }
        i += 1;
    }
    panic!("`default_subcommand` names a command that this one does not have")
}

/// `==` on strings, in a `const fn`.
const fn str_eq(a: &str, b: &str) -> bool {
    let (a, b) = (a.as_bytes(), b.as_bytes());
    if a.len() != b.len() {
        return false;
    }
    let mut i = 0;
    while i < a.len() {
        if a[i] != b[i] {
            return false;
        }
        i += 1;
    }
    true
}

/// Rebuild an [`OsString`] from bytes the parser handed back.
///
/// This is the reverse of [`OsStr::as_encoded_bytes`], and it is how a `PathBuf` field
/// receives a filename the operating system accepts but UTF-8 does not — `/tmp/\xff` stays
/// `/tmp/\xff` rather than becoming a *different* filename with `U+FFFD` in it.
///
/// Where the platform cannot hold those bytes, they are handed back in the `Err` — as
/// `String::from_utf8` does — so the caller can name the value in its error without this
/// having to copy it for a case that is nearly never taken.
///
/// # Why this is not `unsafe`, and why it is not lossless everywhere
///
/// On **Unix** an `OsString` is an arbitrary byte sequence, so the conversion is total and
/// uses the safe [`OsStringExt::from_vec`]. Every byte survives, which is the case that
/// matters: non-UTF-8 filenames are ordinary there.
///
/// [`OsStringExt::from_vec`]: std::os::unix::ffi::OsStringExt::from_vec
///
/// On **Windows** the encoding is WTF-8, where not every byte sequence is valid, and the only
/// constructor that accepts one is `OsString::from_encoded_bytes_unchecked` — whose
/// precondition this function cannot enforce. It takes a `Vec<u8>` from a safe caller, so
/// there is no way to know the bytes came from `as_encoded_bytes` rather than from anywhere
/// else, and a safe function with a precondition that can be violated is unsound however
/// carefully its callers behave today.
///
/// So on Windows the bytes go through UTF-8, and one that is not valid UTF-8 is refused
/// rather than assumed. What that gives up is a Windows argument containing an unpaired
/// surrogate, which is reported instead of accepted; what it buys is that this crate needs no
/// `unsafe` at all.
pub fn os_string_from_bytes(value: Vec<u8>) -> Result<OsString, Vec<u8>> {
    #[cfg(unix)]
    {
        Ok(std::os::unix::ffi::OsStringExt::from_vec(value))
    }
    #[cfg(not(unix))]
    {
        match String::from_utf8(value) {
            Ok(text) => Ok(OsString::from(text)),
            Err(bad) => Err(bad.into_bytes()),
        }
    }
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
    /// How many values it has taken, so a bound can stop it.
    collected: u32,
    /// Which of `cmd.args` is next to fill.
    arg_pos: usize,
    /// How many words the variadic at `arg_pos` has taken, for the same reason.
    arg_taken: u32,
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
    /// Whether the default subcommand has already been taken.
    ///
    /// Once, per parse: a default subcommand that itself declares one would otherwise
    /// descend on every word until the tree ran out.
    default_taken: bool,
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
            collected: 0,
            arg_pos: 0,
            arg_taken: 0,
            arg_filled: false,
            flags_stopped: false,
            separator_seen: false,
            default_taken: false,
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
                    self.collected += 1;
                    // Same rule as a positional: a bounded occurrence takes that many and
                    // leaves the rest to whatever follows.
                    if flag.var_max.is_some_and(|max| self.collected >= max) {
                        self.collecting = None;
                    }
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
                // The count belongs to the argument at `arg_pos`, so jumping past it has
                // to leave the count behind: a bounded variadic before the separator would
                // otherwise lend its total to the argument after it, which then stops
                // early or at once.
                self.arg_pos += idx;
                self.arg_taken = 0;
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
                self.start_collecting(flag);
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

        // Every CLI answers to `--help`, and none of them declares it. Asked *after* the
        // command's own flags, so a CLI that declares its own `--help` keeps it.
        if name == b"help" {
            return Ok(Event::Flag {
                flag: &HELP_LONG,
                value: None,
                negated: false,
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
            self.start_collecting(flag);
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

            // A word that names no subcommand goes to the default one, if there is one.
            //
            // Only a word, though. A dash-prefixed token that named no flag arrives here as a
            // value — that is what `unknown_flags = value` means — and it was never a
            // candidate to *select* anything, so it binds where it was typed. usage-lib stops
            // looking for subcommands at an unrecognised flag for the same reason. `--` is
            // excluded on the same grounds: it reaches this function only when a `preserve`
            // argument wants it as a value.
            //
            // The token is *not* consumed: the cursor steps back so the next event reads it
            // again, now against the command just descended into. That is what lets it be a
            // subcommand of the default (`mise build` where `build` is a task the mount
            // added) as easily as an argument of it, without this function having to decide
            // which — and without yielding two events for one word.
            if let Some(default) = self.cmd.default_subcommand {
                if !self.default_taken && !is_flag_like(token) && token != b"--" {
                    self.default_taken = true;
                    self.descend(default)?;
                    self.pos -= 1;
                    return Ok(Event::Command(default));
                }
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
        // A variadic keeps taking values, so the cursor stays put — until it reaches its
        // bound, at which point the words after it belong to whatever comes next. That is
        // what makes `[a]… [b]` expressible at all.
        if arg.var {
            self.arg_taken += 1;
            if arg.var_max.is_some_and(|max| self.arg_taken >= max) {
                self.advance_arg();
            }
        } else {
            self.advance_arg();
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
        self.arg_taken = 0;
        self.arg_filled = false;
        Ok(())
    }

    /// Move to the next positional, forgetting what the last one took.
    fn advance_arg(&mut self) {
        self.arg_pos += 1;
        self.arg_taken = 0;
    }

    /// A variadic flag occurrence begins, counting from zero.
    ///
    /// The value it was given on the same token counts, which is why this starts at one:
    /// `--include a b` with `var_max=2` takes `a` and `b`, not three words.
    fn start_collecting(&mut self, flag: &'t Flag<'t>) {
        self.collected = 1;
        self.collecting = if flag.var_max.is_some_and(|max| max <= 1) {
            None
        } else {
            Some(flag)
        };
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
        self.in_scope()
            .find(|f| f.shorts.contains(&byte))
            // As for `--help`: supplied by the parser, and only where the command has not
            // declared a `-h` of its own.
            .or(if byte == b'h' {
                Some(&HELP_SHORT)
            } else {
                None
            })
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
/// The reverse direction is the one with a cost — see [`os_string_from_bytes`] —
/// which is why values come back as bytes.
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

    // A CLI shaped exactly like mise's root: a default subcommand, a positional of its own,
    // and a subcommand under the default — which is the arrangement that tells routing from
    // a plain positional.
    static TASK: Arg = Arg {
        key: 20,
        name: "task",
        ..Arg::REQUIRED
    };
    static RUN_TASK: Arg = Arg {
        key: 21,
        name: "run_task",
        ..Arg::REQUIRED
    };
    static DEEP: Command = Command {
        name: "deep",
        args: &[&RUN_TASK],
        key: 203,
        ..Command::EMPTY
    };
    static LINT: Command = Command {
        name: "lint",
        subcommands: &[&DEEP],
        // A default of its own, so that a parse which forgot it had already taken one would
        // have somewhere to go. Nothing else in these fixtures can show the latch working.
        default_subcommand: Some(&DEEP),
        key: 202,
        ..Command::EMPTY
    };
    static RUN: Command = Command {
        name: "run",
        args: &[&RUN_TASK],
        subcommands: &[&LINT],
        key: 200,
        ..Command::EMPTY
    };
    static DEFAULTING: Command = Command {
        name: "mise",
        flags: &[&VERBOSE],
        args: &[&TASK],
        subcommands: &[&RUN, &INSTALL],
        default_subcommand: Some(find_subcommand(&[&RUN, &INSTALL], "run")),
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
    fn a_word_naming_no_subcommand_goes_to_the_default_one() {
        // usage-lib's answer, which this reproduces: `mise build` comes back as commands
        // `["mise", "run"]` with the word bound to *run's* argument — not to `mise`'s own
        // `[TASK]`, which is what makes this more than a synonym for a positional.
        let a = argv(["build"]);
        assert_eq!(
            parse(&DEFAULTING, &a).unwrap(),
            vec![
                Event::Command(&RUN),
                Event::Arg {
                    arg: &RUN_TASK,
                    value: b"build"
                },
            ]
        );
    }

    // A shared table, as a flattened struct's would be. Declared outside the tests so both
    // can splice it, which is the arrangement it exists to model.
    static SHARED_QUIET: Flag = Flag {
        key: 300,
        name: "quiet",
        longs: &["quiet"],
        ..Flag::BOOL
    };
    static SHARED_FLAGS: &[&Flag] = &[&SHARED_QUIET];
    static SHARED_WHAT: Arg = Arg {
        key: 301,
        name: "what",
        ..Arg::REQUIRED
    };
    static SHARED_ARGS: &[&Arg] = &[&SHARED_WHAT];

    #[test]
    fn concatenating_tables_keeps_the_order_they_were_given_in() {
        // The property positional arguments depend on: a flattened group lands where the
        // field was written, not at the end. `[&FILE], SHARED, [&REST]` has to stay in that
        // order or `ex a b c` binds the wrong words.
        const ARGS: &[&[&Arg]] = &[&[&FILE], SHARED_ARGS, &[&REST]];
        static TABLE: [&Arg; table_len(ARGS)] = concat_args(ARGS);
        assert_eq!(
            TABLE.iter().map(|a| a.name).collect::<Vec<_>>(),
            ["file", "what", "rest"]
        );

        // Empty groups contribute nothing and disturb nothing, which is what lets the derive
        // emit a group per field without checking whether it is empty first.
        const WITH_GAPS: &[&[&Flag]] = &[&[], &[&FORCE], &[], SHARED_FLAGS, &[]];
        static FLAGS: [&Flag; table_len(WITH_GAPS)] = concat_flags(WITH_GAPS);
        // By long form: these fixtures do not all set `name`, and the placeholder's is also
        // empty — so comparing names could not tell a real entry from a leftover slot.
        assert_eq!(
            FLAGS.iter().map(|f| f.longs).collect::<Vec<_>>(),
            [&["force"], &["quiet"]]
        );
    }

    #[test]
    fn a_concatenated_table_parses_like_a_declared_one() {
        // The point of doing this at compile time: what the parser walks is one flat slice,
        // indistinguishable from a command that declared everything itself.
        const FLAG_GROUPS: &[&[&Flag]] = &[&[&FORCE], SHARED_FLAGS];
        const ARG_GROUPS: &[&[&Arg]] = &[SHARED_ARGS, &[&REST]];
        static FLAGS: [&Flag; table_len(FLAG_GROUPS)] = concat_flags(FLAG_GROUPS);
        static ARGS: [&Arg; table_len(ARG_GROUPS)] = concat_args(ARG_GROUPS);
        static JOINED: Command = Command {
            name: "joined",
            flags: &FLAGS,
            args: &ARGS,
            ..Command::EMPTY
        };

        let a = argv(["--quiet", "one", "two", "--force"]);
        assert_eq!(
            parse(&JOINED, &a).unwrap(),
            vec![
                Event::Flag {
                    flag: &SHARED_QUIET,
                    value: None,
                    negated: false
                },
                Event::Arg {
                    arg: &SHARED_WHAT,
                    value: b"one"
                },
                Event::Arg {
                    arg: &REST,
                    value: b"two"
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
    fn an_unknown_flag_is_not_routed() {
        // A dash-prefixed token that names no flag becomes a value here (the default for
        // `unknown_flags`), and it must not thereby become a *subcommand* word: usage-lib
        // stops looking for subcommands at an unrecognised flag, and binds it to the command
        // still in scope. Verified against usage-lib, where `ex --wat` comes back as commands
        // `["ex"]` with `ROOT_TASK = "--wat"`.
        for token in ["--wat", "-x"] {
            let a = argv([token]);
            assert_eq!(
                parse(&DEFAULTING, &a).unwrap(),
                vec![Event::Arg {
                    arg: &TASK,
                    value: token.as_bytes()
                }],
                "{token} should bind where it was typed, not in the default subcommand"
            );
        }
    }

    #[test]
    fn a_named_subcommand_is_not_routed() {
        // The default is for words that name nothing. A word that names a sibling still
        // selects it, and the root's own argument is still reachable behind one.
        let a = argv(["install"]);
        assert_eq!(
            parse(&DEFAULTING, &a).unwrap(),
            vec![Event::Command(&INSTALL)]
        );
    }

    #[test]
    fn the_default_can_be_named_by_an_alias() {
        // usage-lib resolves the name against subcommand names, aliases and hidden aliases
        // alike, so a spec may point `default_subcommand` at any of them.
        static BY_ALIAS: Command = Command {
            name: "mise",
            args: &[&TASK],
            subcommands: &[&INSTALL],
            // `INSTALL` answers to "i" as well as to its name.
            default_subcommand: Some(find_subcommand(&[&INSTALL], "i")),
            ..Command::EMPTY
        };
        assert!(::core::ptr::eq(
            BY_ALIAS.default_subcommand.expect("declared"),
            &INSTALL
        ));
    }

    #[test]
    fn the_word_is_re_examined_against_the_command_it_reached() {
        // The reason the cursor steps back rather than the token being consumed: `lint` names
        // nothing at the root, and once inside `run` it names a subcommand. mise's mounted
        // task names arrive exactly this way.
        let a = argv(["lint"]);
        assert_eq!(
            parse(&DEFAULTING, &a).unwrap(),
            vec![Event::Command(&RUN), Event::Command(&LINT)]
        );
    }

    #[test]
    fn the_default_is_taken_at_most_once_per_parse() {
        // usage-lib latches this for the whole parse rather than per command, and the shape
        // that shows the difference needs two of them: `lint` routes through `run`, and `lint`
        // declares a default too. A second word there would descend again — walking a CLI
        // deeper than anything the user typed — so the answer is that it does not.
        let a = argv(["lint", "zzz"]);
        assert_eq!(
            parse(&DEFAULTING, &a),
            Err(Error::UnexpectedArg { token: b"zzz" }),
            "the second word must not reach `deep`"
        );

        // Reached explicitly, the same command still takes it: the latch bounds routing, not
        // the tree.
        let a = argv(["lint", "deep", "zzz"]);
        assert_eq!(
            parse(&DEFAULTING, &a).unwrap(),
            vec![
                Event::Command(&RUN),
                Event::Command(&LINT),
                Event::Command(&DEEP),
                Event::Arg {
                    arg: &RUN_TASK,
                    value: b"zzz"
                },
            ]
        );
    }

    #[test]
    fn a_flag_before_the_word_still_belongs_to_the_root() {
        // Routing happens at the word, so anything typed before it was addressed to the
        // command the user was actually at.
        let a = argv(["--verbose", "build"]);
        assert_eq!(
            parse(&DEFAULTING, &a).unwrap(),
            vec![
                Event::Flag {
                    flag: &VERBOSE,
                    value: None,
                    negated: false
                },
                Event::Command(&RUN),
                Event::Arg {
                    arg: &RUN_TASK,
                    value: b"build"
                },
            ]
        );
    }

    #[test]
    fn nothing_routes_after_the_separator() {
        // Past `--` there are no subcommands left to select, so there is no default to reach
        // either: the words are values of whatever the command declares.
        let a = argv(["--", "build"]);
        assert_eq!(
            parse(&DEFAULTING, &a).unwrap(),
            vec![Event::Arg {
                arg: &TASK,
                value: b"build"
            }]
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
    fn a_wrapper_still_forwards_a_help_flag() {
        // Supplying `--help` must not take the two forwarding mechanisms away from a wrapper,
        // which is the one place a CLI means to hand the token on rather than answer it.
        static ARGS: Arg = Arg {
            key: 24,
            name: "args",
            ..Arg::VAR
        };
        static WRAP: Command = Command {
            name: "wrap",
            args: &[&ARGS],
            ..Command::EMPTY
        };

        // A typed separator: everything after it is a value, `--help` included.
        let a = argv(["--", "--help", "-h"]);
        assert_eq!(
            parse(&WRAP, &a).unwrap(),
            vec![
                Event::Arg {
                    arg: &ARGS,
                    value: b"--help"
                },
                Event::Arg {
                    arg: &ARGS,
                    value: b"-h"
                },
            ]
        );

        // And `automatic`, for the wrapper whose caller should not have to type one: the
        // first value stops flag interpretation, so the flags after it forward.
        static AUTO_ARGS: Arg = Arg {
            key: 25,
            name: "args",
            double_dash: DoubleDash::Automatic,
            ..Arg::VAR
        };
        static AUTO_WRAP: Command = Command {
            name: "wrap",
            args: &[&AUTO_ARGS],
            ..Command::EMPTY
        };

        let a = argv(["node", "--help"]);
        assert_eq!(
            parse(&AUTO_WRAP, &a).unwrap(),
            vec![
                Event::Arg {
                    arg: &AUTO_ARGS,
                    value: b"node"
                },
                Event::Arg {
                    arg: &AUTO_ARGS,
                    value: b"--help"
                },
            ]
        );

        // Before either takes effect, though, the wrapper's own help is what `--help` asks
        // for — `mise run --help` is a question about `run`, not a value for it.
        let a = argv(["--help"]);
        assert_eq!(
            parse(&AUTO_WRAP, &a).unwrap(),
            vec![Event::Flag {
                flag: &HELP_LONG,
                value: None,
                negated: false
            }]
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
