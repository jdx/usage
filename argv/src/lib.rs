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

/// A value's filesystem completion class for `#[usage(value_hint = ...)]`.
///
/// This lives in the runtime crate so a declaration never needs clap merely to describe what
/// kind of path a shell should offer. It is metadata only and adds no work to a successful
/// parse.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum ValueHint {
    /// A path to a file.
    FilePath,
    /// A path to either a file or a directory.
    AnyPath,
    /// A path to a directory.
    DirPath,
}

#[cfg(feature = "complete")]
pub mod complete;
#[cfg(feature = "diagnostics")]
pub mod diagnostic;
#[cfg(feature = "complete")]
pub mod script;

/// Checks that the `complete` feature is on, with an explanation when it is not.
///
/// `#[usage(completion)]` generates code that reaches into [`complete`], which is behind a
/// feature the *depending* crate enables — a derive cannot turn on a feature of another crate.
/// Without this, the failure was `unresolved module complete`, which says nothing about the
/// attribute that caused it.
#[cfg(feature = "complete")]
#[macro_export]
macro_rules! __usage_needs_complete_feature {
    () => {};
}

/// See [`__usage_needs_complete_feature`].
#[cfg(not(feature = "complete"))]
#[macro_export]
macro_rules! __usage_needs_complete_feature {
    () => {
        ::core::compile_error!(
            "`#[usage(completion)]` needs usage-argv's `complete` feature. Add it where \
             usage-argv is depended on: usage-argv = { version = \"…\", features = \
             [\"spec\", \"complete\"] }"
        );
    };
}
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
    /// Whether an unmatched word is forwarded as an external command plus the rest of argv.
    ///
    /// clap's `allow_external_subcommands`. Known subcommands still win; a
    /// [`default_subcommand`](Self::default_subcommand) still catches first. Once the
    /// unmatched word is taken, remaining tokens — including `--help` — are not parsed
    /// as this command's flags.
    pub external_subcommand: bool,
    /// Accept an unambiguous prefix of a subcommand name or alias.
    /// Inherited by nested commands.
    pub infer_subcommands: bool,
    /// Accept an unambiguous prefix of a long flag or alias.
    /// Inherited by nested commands.
    pub infer_long_args: bool,
    /// What an unrecognized flag-like token means here, or `None` to keep whatever the
    /// enclosing command said. See [`UnknownFlags`].
    ///
    /// Inherited rather than resolved per command, which is what usage-lib does — its
    /// `effective_unknown_flags` walks outward from the command that ran and falls back to
    /// the spec's. Resolving it in the tables instead was possible only for a builder that
    /// can see the whole tree: a derive expands one struct at a time and cannot see its
    /// parent, so `#[usage(unknown_flags = "error")]` on the root reached the root alone and
    /// a subcommand had no way to say it at all.
    ///
    /// The parser carries the effective value down as it descends, so a command that states
    /// nothing costs nothing.
    pub unknown_flags: ::core::option::Option<UnknownFlags>,
    /// Whether this command answers to `--version` and `-V`.
    ///
    /// Set on the root, and only when the CLI declares a version: clap adds the flag exactly
    /// then, and a `--version` that answers with nothing is worse than one that is not there.
    /// A field rather than a rule about depth, so a CLI that wants it on a subcommand — clap's
    /// `propagate_version` — has somewhere to say so.
    pub version: bool,
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
        external_subcommand: false,
        infer_subcommands: false,
        infer_long_args: false,
        unknown_flags: ::core::option::Option::None,
        version: false,
        key: 0,
    };
}

/// Basename of argv[0] for a multicall CLI: last path component, with a trailing
/// `.exe` stripped so Windows and Unix agree.
pub fn multicall_basename(argv0: &str) -> &str {
    let name = argv0.rsplit(['/', '\\']).next().unwrap_or(argv0);
    match name.get(name.len().saturating_sub(4)..) {
        Some(ext) if ext.eq_ignore_ascii_case(".exe") => &name[..name.len() - 4],
        _ => name,
    }
}

/// The applet name to parse as the first word, when argv[0] is not the dispatcher.
///
/// `None` means a dispatcher invocation (`busybox ls`): skip argv[0] and parse the
/// rest. `Some` is a symlink invocation (`ls -l`): inject the basename.
pub fn multicall_applet<'a>(argv0: &'a str, name: &str, bin: Option<&str>) -> Option<&'a str> {
    let base = multicall_basename(argv0);
    if !name.is_empty() && base == multicall_basename(name) {
        return None;
    }
    if let Some(bin) = bin {
        if !bin.is_empty() && base == multicall_basename(bin) {
            return None;
        }
    }
    Some(base)
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
    /// The byte that makes one word several values, if the flag declares one.
    ///
    /// Here rather than with the metadata for the same reason [`var_max`](Self::var_max)
    /// is: it decides *where* a word lands. A bound counts values, and a delimiter is what
    /// makes a word stop being one of them — `--include a,b,c` is three, so a `var_max` of
    /// two is already past its bound on the single word it was entitled to take. Binding
    /// cannot count without it.
    pub delimiter: ::core::option::Option<u8>,
    /// Whether a detached value may itself look like a flag.
    ///
    /// The default is to refuse: `--jobs --force` is far more likely a forgotten
    /// value than a jobs of `"--force"`. Declared, the next token is taken
    /// whatever it looks like — including `--` — which is clap's
    /// `allow_hyphen_values` and the spec's property of the same name. A variadic
    /// occurrence still stops collecting at a later flag-like token, so a second
    /// occurrence of the flag is not eaten as a value.
    pub allow_hyphen_values: bool,
    /// Whether the value must be attached with `=`.
    ///
    /// `--flag=value` is accepted and `--flag value` is not, which is clap's
    /// `require_equals` and the spec's property of the same name. A short's
    /// attached form (`-i9229`, `-i=9229`) still binds: only the following word
    /// is refused.
    pub require_equals: bool,
    /// Value used when the flag is present but no value is given.
    ///
    /// clap's `default_missing_value` and the spec's `default_missing`. `--color`
    /// binds this, `--color=never` binds `never`, and an absent flag is not bound.
    /// Combined with [`Self::require_equals`], a following word is still refused.
    pub default_missing: ::core::option::Option<&'a [u8]>,
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
        delimiter: ::core::option::Option::None,
        allow_hyphen_values: false,
        require_equals: false,
        default_missing: ::core::option::Option::None,
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
    /// The byte that makes one word several values, if the argument declares one.
    ///
    /// See [`Flag::delimiter`]: a bound counts values, and only this says how many values a
    /// word carries.
    pub delimiter: ::core::option::Option<u8>,
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
        delimiter: ::core::option::Option::None,
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
pub enum Event<'t, 'a, 'v> {
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
    /// An unmatched word was forwarded as an external command: the name, then
    /// every remaining token, including flags.
    External { values: &'a [&'v OsStr] },
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
    /// A flag that is not repeatable was given more than once.
    DuplicateFlag {
        /// The flag's name, as the spec calls it.
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
    /// A required group had none of its members given.
    ///
    /// Carries the members as members rather than as a rendered sentence: the caller
    /// decides how to say it, and a completion asking what would satisfy this needs the
    /// list rather than the prose.
    MissingGroup {
        /// The group's declared name, which appears in the message so a command with
        /// several groups does not report the same sentence twice.
        group: &'t str,
        /// The flags that would satisfy it, as the declaration spells them.
        members: &'t [&'t str],
    },
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
    /// `--version` was asked for. Not a failure either — the caller prints and leaves.
    ///
    /// Carries nothing: the version string lives in the spec rather than the parse tables,
    /// and the caller that answers this has it.
    Version,
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

/// See [`HELP_LONG_KEY`].
pub const VERSION_LONG_KEY: u64 = u64::MAX - 2;
/// See [`HELP_LONG_KEY`].
pub const VERSION_SHORT_KEY: u64 = u64::MAX - 3;

/// `--version`, where the CLI declared one.
///
/// In the parse table and not in the metadata, exactly as `--help` is: a spec does not declare
/// `--version`, so listing one would make the rendered page disagree with the spec it came from.
pub static VERSION_LONG: Flag<'static> = Flag {
    key: VERSION_LONG_KEY,
    name: "version",
    longs: &["version"],
    ..Flag::BOOL
};

/// `-V`, which clap also supplies.
pub static VERSION_SHORT: Flag<'static> = Flag {
    key: VERSION_SHORT_KEY,
    name: "version",
    shorts: b"V",
    ..Flag::BOOL
};

/// `-h`, which prints the shorter form.
pub static HELP_SHORT: Flag<'static> = Flag {
    key: HELP_SHORT_KEY,
    name: "help",
    shorts: b"h",
    ..Flag::BOOL
};

/// A named subcommand of a given command, by name or alias.
///
/// Free rather than a method because `help` resolves a path *without* descending: the words
/// after it are a question about a command rather than a walk into one.
///
/// Names across every subcommand before any alias, the precedence the grammar states — and
/// the reason this is the only implementation of it on argv's side. `ex run` and `ex help run`
/// selecting different commands would be exactly the divergence this rule was written to end.
pub(crate) fn find_named<'t>(cmd: &'t Command<'t>, name: &[u8]) -> Option<&'t Command<'t>> {
    let subcommands = || cmd.subcommands.iter().copied();
    subcommands()
        .find(|c| c.name.as_bytes() == name)
        .or_else(|| subcommands().find(|c| c.aliases.iter().any(|a| a.as_bytes() == name)))
}

fn find_prefixed<'t>(cmd: &'t Command<'t>, name: &[u8]) -> Option<&'t Command<'t>> {
    if let Some(exact) = find_named(cmd, name) {
        return Some(exact);
    }
    if name.is_empty() {
        return None;
    }
    let mut found = None;
    for command in cmd.subcommands {
        if !command.name.as_bytes().starts_with(name)
            && !command
                .aliases
                .iter()
                .any(|alias| alias.as_bytes().starts_with(name))
        {
            continue;
        }
        if found.is_some() {
            return None;
        }
        found = Some(*command);
    }
    found
}

/// What a caller should print for a parse failure, and what to exit with.
///
/// The one entry point a generated `parse()` reaches for, and the reason it exists here rather
/// than in the derive: whether the good rendering is available is a *feature of this crate* in
/// the adopter's dependency graph, and a `#[cfg]` written into generated code is evaluated in
/// the adopter's crate, where the feature is not theirs to see. That is how a metadata field
/// once got silently dropped; the answer is that the cfg lives beside the thing it gates.
///
/// With `diagnostics` on, this is the clap-shaped message. Without it, the error's `Debug`
/// form — which is still better than nothing and is what a parser-only build asked for.
///
/// [`Error::Help`] and [`Error::Version`] are not failures and must be handled before this.
#[cfg(feature = "diagnostics")]
pub fn render_failure(spec: &spec::Spec<'_>, argv: &[&OsStr], error: &Error<'_, '_>) -> String {
    diagnostic::render(spec, argv, error, diagnostic::Style::auto())
}

/// What a caller should print for a parse failure, without the renderer that makes it readable.
///
/// See the other half. A caller that wants the clap-shaped message turns on `diagnostics`;
/// this is what a parser-only build asked for, and it still says which error it was.
#[cfg(all(feature = "spec", not(feature = "diagnostics")))]
pub fn render_failure(spec: &spec::Spec<'_>, argv: &[&OsStr], error: &Error<'_, '_>) -> String {
    let _ = (spec, argv);
    ::std::format!("error: {error:?}\n")
}

/// Whether a flag is one of the two the parser supplies rather than the CLI declaring it.
pub fn is_help_flag(flag: &Flag<'_>) -> bool {
    flag.key == HELP_LONG_KEY || flag.key == HELP_SHORT_KEY
}

/// Whether a flag is one of the two the parser supplies for `--version`.
pub fn is_version_flag(flag: &Flag<'_>) -> bool {
    flag.key == VERSION_LONG_KEY || flag.key == VERSION_SHORT_KEY
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
    // Names first, then aliases: a command's own name outranks another command's alias, so
    // the answer does not depend on the order the table happens to list them in. Checking
    // each candidate's name *and* aliases in one pass instead let whichever command came
    // first win, and usage-lib resolved the same spec to the last one.
    let mut i = 0;
    while i < subcommands.len() {
        if str_eq(subcommands[i].name, name) {
            return subcommands[i];
        }
        i += 1;
    }
    // Aliases answer too, because usage-lib resolves the name against names, aliases and
    // hidden aliases alike — so a spec may point `default_subcommand` at any of them.
    let mut i = 0;
    while i < subcommands.len() {
        let candidate = subcommands[i];
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

/// Refuse two subcommands that answer to the same name, aliases included.
///
/// A derive expansion can validate aliases written on one enum, but aliases may also live on
/// the independently expanded `Args` structs its variants wrap. This final, joined-table check
/// is where both declarations are visible.
pub const fn assert_unique_subcommand_names(subcommands: &[&Command<'_>]) {
    const fn form<'a>(cmd: &'a Command<'a>, at: usize) -> Option<&'a str> {
        if at == 0 {
            Some(cmd.name)
        } else if at <= cmd.aliases.len() {
            Some(cmd.aliases[at - 1])
        } else {
            None
        }
    }

    let mut command = 0;
    while command < subcommands.len() {
        let mut at = 0;
        while let Some(name) = form(subcommands[command], at) {
            let mut other_command = command;
            while other_command < subcommands.len() {
                let mut other_at = if other_command == command { at + 1 } else { 0 };
                while let Some(other) = form(subcommands[other_command], other_at) {
                    assert!(
                        !str_eq(name, other),
                        "two subcommands answer to the same name, counting aliases"
                    );
                    other_at += 1;
                }
                other_command += 1;
            }
            at += 1;
        }
        command += 1;
    }
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
pub struct Parser<'t, 'a, 'v> {
    argv: &'a [&'v OsStr],
    /// Index of the next token to read.
    pos: usize,
    /// The command currently in scope.
    cmd: &'t Command<'t>,
    /// What an unrecognized flag-like token means in the command currently in scope.
    ///
    /// Carried rather than looked up, because it is inherited: a command that states
    /// nothing keeps what the enclosing one said, and walking back up the ancestors on
    /// every unrecognized token would pay for the inheritance at the wrong moment.
    unknown_flags: UnknownFlags,
    infer_subcommands: bool,
    infer_long_args: bool,
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
    /// Where the command in scope began, as an index into `argv`.
    cmd_start: usize,
    /// Where each ancestor's own words began, in step with `ancestors`.
    starts: [usize; MAX_DEPTH],
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
    /// The `argv` range the `help` *word* resolved as a command path, if one was typed.
    ///
    /// Empty for `--help`, which asks about wherever the parse had got to. For the word, the
    /// question is about a command deeper than the parse reached, and only this walk knows
    /// which tokens named it: a caller re-scanning `argv` would count a flag's detached value
    /// that happens to spell a sibling's name. Two indices rather than the commands
    /// themselves, so the parser keeps allocating nothing.
    help_span: (usize, usize),
}

impl<'t: 'v, 'a, 'v> Parser<'t, 'a, 'v> {
    /// Begin parsing `argv` against `root`.
    ///
    /// `argv` excludes the program name.
    pub fn new(root: &'t Command<'t>, argv: &'a [&'v OsStr]) -> Self {
        Parser {
            argv,
            pos: 0,
            cmd: root,
            unknown_flags: match root.unknown_flags {
                ::core::option::Option::Some(mode) => mode,
                // Nothing above the root to inherit from, so the default stands.
                ::core::option::Option::None => UnknownFlags::Value,
            },
            infer_subcommands: root.infer_subcommands,
            infer_long_args: root.infer_long_args,
            ancestors: [None; MAX_DEPTH],
            depth: 0,
            bundle: &[],
            bundle_token: &[],
            collecting: None,
            cmd_start: 0,
            starts: [0; MAX_DEPTH],
            collected: 0,
            arg_pos: 0,
            arg_taken: 0,
            arg_filled: false,
            flags_stopped: false,
            separator_seen: false,
            default_taken: false,
            done: false,
            help_span: (0, 0),
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

    /// Every command entered so far, and where each one's own words begin.
    ///
    /// The ancestors are already kept for flag scoping; this is the same chain with the offsets,
    /// which is what lets a completion hand a callback the words of *its* command rather than of
    /// the deepest one — a global flag is declared on an ancestor.
    pub fn command_path(&self) -> Vec<(&'t Command<'t>, usize)> {
        let mut out = Vec::with_capacity(self.depth + 1);
        for (i, ancestor) in self.ancestors[..self.depth].iter().enumerate() {
            if let Some(cmd) = ancestor {
                // An ancestor's own words start where the one before it descended, and the
                // root's start at the beginning.
                out.push((*cmd, self.starts[i]));
            }
        }
        out.push((self.cmd, self.cmd_start));
        out
    }

    /// The `argv` range the `help` word resolved as a command path.
    ///
    /// Empty unless the word was typed. Every token in it named a subcommand of the one before
    /// it — the parser resolved them itself, so nothing here is a flag or a flag's value.
    pub fn help_span(&self) -> (usize, usize) {
        self.help_span
    }

    /// Where the command in scope began: the index in `argv` just after its name.
    ///
    /// `argv[command_start()..]` is what that command was given, which is what a completion
    /// callback needs to be handed its own command's half-parsed struct rather than the root's.
    pub fn command_start(&self) -> usize {
        self.cmd_start
    }

    /// Whether flag interpretation has stopped, for any reason.
    ///
    /// Wider than [`double_dash_seen`](Self::double_dash_seen), and the question completion
    /// asks: past a separator *or* past the first value of an `automatic` argument, a
    /// dash-prefixed word is a value, so there is no flag there to offer.
    pub fn flags_stopped(&self) -> bool {
        self.flags_stopped
    }

    /// A variadic flag that is still claiming words.
    ///
    /// Asked *between* events, because the answer is gone by the end: the call that finds argv
    /// exhausted is the one that clears it. A completion needs it — the next word after
    /// `--tools a ⌶` is another tool, not the positional that follows.
    pub fn collecting(&self) -> Option<&'t Flag<'t>> {
        self.collecting
    }

    /// The positional the next word would fill, if there is one left.
    ///
    /// A variadic stays here until it reaches its bound, which is what makes it the answer to
    /// "what could go where the cursor is" as many times as it can be filled.
    pub fn pending_arg(&self) -> Option<&'t Arg<'t>> {
        self.next_arg()
    }

    /// Flags a word here could name: this command's own, then any ancestor's globals.
    ///
    /// The same set the parser itself would look in, so what is offered and what is accepted
    /// cannot disagree — including the shadowing rule, where a subcommand redeclaring an
    /// inherited name hides it.
    pub fn flags_in_scope(&self) -> impl Iterator<Item = &'t Flag<'t>> + '_ {
        self.in_scope()
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
    pub fn next_event(&mut self) -> Option<Result<Event<'t, 'a, 'v>, Error<'t, 'v>>> {
        if self.done {
            return None;
        }
        let event = self.step();
        if let Some(Err(_)) = event {
            self.done = true;
        }
        event
    }

    fn step(&mut self) -> Option<Result<Event<'t, 'a, 'v>, Error<'t, 'v>>> {
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
                    self.collected += values_in(bytes(next), flag.delimiter);
                    // Same rule as a positional: a bounded occurrence takes that many and
                    // leaves the rest to whatever follows.
                    if flag.var_max.is_some_and(|max| self.collected >= max) {
                        self.collecting = None;
                    }
                    // Stopping is only the same as staying within the bound while one word
                    // is one value. A delimited word can carry the occurrence past it in a
                    // single step, and that word cannot be split between two owners, so the
                    // overshoot is an error rather than a place to stop.
                    if let Some(max) = flag.var_max.filter(|max| self.collected > *max) {
                        return Some(Err(Error::VarTooMany {
                            name: flag.name,
                            max: max as usize,
                            got: self.collected as usize,
                        }));
                    }
                    return Some(Ok(Event::Flag {
                        flag,
                        value: Some(bytes(next)),
                        negated: false,
                    }));
                }
                // A token that could be something else ends the run — but the *end of argv*
                // decides nothing. Clearing there threw away the answer to "would the next
                // word be claimed?", which is the question a completion asks and no parse
                // ever does: once argv is exhausted there are no more events either way.
                Some(_) => self.collecting = None,
                None => {}
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
                Err(e) if self.unknown_flags == UnknownFlags::Error => {
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

    fn long_flag(&mut self, token: &'v [u8]) -> Result<Event<'t, 'a, 'v>, Error<'t, 'v>> {
        let body = &token[2..];
        let (name, attached) = match body.iter().position(|&b| b == b'=') {
            Some(i) => (&body[..i], Some(&body[i + 1..])),
            None => (body, None),
        };

        if let Some((flag, negated)) = self.find_long_form(name) {
            let value = if flag.takes_value {
                Some(match attached {
                    Some(v) => v,
                    None => self.take_detached_value(flag)?,
                })
            } else {
                None
            };
            if flag.variadic {
                self.start_collecting(flag, value.unwrap_or(b""))?;
            }
            return Ok(Event::Flag {
                flag,
                value,
                negated,
            });
        }

        // Where the CLI declared a version, `--version` answers with it — asked after the
        // command's own flags, so a CLI declaring its own keeps it.
        if name == b"version" && self.cmd.version {
            return Ok(Event::Flag {
                flag: &VERSION_LONG,
                value: None,
                negated: false,
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

        if self.unknown_flags == UnknownFlags::Error {
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

    fn short_flag(&mut self) -> Result<Event<'t, 'a, 'v>, Error<'t, 'v>> {
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
            self.start_collecting(flag, value)?;
        }
        Ok(Event::Flag {
            flag,
            value: Some(value),
            negated: false,
        })
    }

    /// Take the following token as a flag's value.
    ///
    /// Refuses a flag-like token unless [`Flag::allow_hyphen_values`] is set:
    /// `--jobs --force` is far more likely a forgotten value than a deliberate
    /// one, and the attached form is available for the deliberate case. Declared,
    /// the next token is taken whatever it looks like, including `--`.
    fn take_detached_value(&mut self, flag: &'t Flag<'t>) -> Result<&'v [u8], Error<'t, 'v>> {
        if flag.require_equals {
            return self.missing_or_default(flag);
        }
        match self.argv.get(self.pos) {
            Some(next) if flag.allow_hyphen_values || !is_flag_like(bytes(next)) => {
                self.pos += 1;
                Ok(bytes(next))
            }
            _ => self.missing_or_default(flag),
        }
    }

    fn missing_or_default(&self, flag: &'t Flag<'t>) -> Result<&'v [u8], Error<'t, 'v>> {
        match flag.default_missing {
            Some(value) => Ok(value),
            None => Err(Error::MissingFlagValue { flag }),
        }
    }

    fn word(&mut self, token: &'v [u8]) -> Result<Event<'t, 'a, 'v>, Error<'t, 'v>> {
        // Subcommands are only matched where descent is still possible: once a
        // positional of this command has taken a word, a later word that happens
        // to equal a subcommand name is just a value.
        if !self.arg_filled && !self.flags_stopped {
            if let Some(sub) = self.find_subcommand(token) {
                self.descend(sub)?;
                return Ok(Event::Command(sub));
            }

            // `ex help config ls` — the line every page with a Commands section has printed
            // all along ("help  Print this message or the help of the given subcommand(s)"),
            // and which until now did nothing. The page is what decides the condition here:
            // it prints that line where there are subcommands, so that is where the word is
            // answered, and to a leaf `help` is a word like any other.
            //
            // Asked *after* the subcommand lookup, so a CLI that declares a `help` of its own
            // keeps it — the same rule the two help flags follow.
            //
            // The words after it name a command, resolved here rather than descended into:
            // descending would bind them, and they are a question rather than an invocation.
            if token == b"help" && !self.cmd.subcommands.is_empty() {
                let mut cmd = self.cmd;
                let mut infer_subcommands = self.infer_subcommands;
                let from = self.pos;
                while let Some(next) = self.argv.get(self.pos) {
                    let Some(sub) = (if infer_subcommands {
                        find_prefixed(cmd, bytes(next))
                    } else {
                        find_named(cmd, bytes(next))
                    }) else {
                        break;
                    };
                    cmd = sub;
                    infer_subcommands |= sub.infer_subcommands;
                    self.pos += 1;
                }
                // Kept for `help::route_to`: which mount was asked about is not recoverable
                // from `cmd`, since two mounts of one `Subcommands` type are one address.
                self.help_span = (from, self.pos);
                // The long form, as `ex config --help` gives: someone who typed a whole word to
                // ask for help wants the fuller answer.
                return Err(Error::Help { cmd, long: true });
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
                // `-` joins `--` in being excluded, and for the reason already written above:
                // a value was never a candidate to *select* anything. `is_flag_like` calls a
                // lone `-` a value — conventionally stdin — so it passed this guard and
                // descended, where mise's `run` has no positional and the parse failed.
                // usage-lib and clap both bind it to the root's own `[TASK]` instead.
                if !self.default_taken && !is_flag_like(token) && token != b"--" && token != b"-" {
                    self.default_taken = true;
                    self.descend(default)?;
                    self.pos -= 1;
                    return Ok(Event::Command(default));
                }
            }

            // An unmatched word that names no subcommand is forwarded as an external
            // command: this word, then every token after it, including flags. Known
            // subcommands already won above, and a default_subcommand already caught.
            if self.cmd.external_subcommand
                && !is_flag_like(token)
                && token != b"--"
                && token != b"-"
            {
                let from = self.pos - 1;
                self.pos = self.argv.len();
                return Ok(Event::External {
                    values: &self.argv[from..],
                });
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
            self.arg_taken += values_in(token, arg.delimiter);
            // Before advancing, which resets the count: as with a variadic flag, reaching
            // the bound and passing it are the same event once a word can carry several
            // values, and only the second is a mistake.
            if let Some(max) = arg.var_max.filter(|max| self.arg_taken > *max) {
                return Err(Error::VarTooMany {
                    name: arg.name,
                    max: max as usize,
                    got: self.arg_taken as usize,
                });
            }
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
        self.starts[self.depth] = self.cmd_start;
        self.depth += 1;
        self.cmd = sub;
        self.infer_subcommands |= sub.infer_subcommands;
        self.infer_long_args |= sub.infer_long_args;
        // Only a command that says something changes it, which is what inheriting means.
        if let ::core::option::Option::Some(mode) = sub.unknown_flags {
            self.unknown_flags = mode;
        }
        // Where this command's own words start, which is what lets a completion hand a callback
        // the half-parsed struct of the command it was declared on rather than of the root.
        self.cmd_start = self.pos;
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
    /// The value it was given on the same token counts, which is why this starts at what
    /// that value holds: `--include a b` with `var_max=2` takes `a` and `b`, not three
    /// words — and `--include a,b` has already taken both on the one token.
    fn start_collecting(&mut self, flag: &'t Flag<'t>, first: &[u8]) -> Result<(), Error<'t, 'v>> {
        self.collected = values_in(first, flag.delimiter);
        if let Some(max) = flag.var_max.filter(|max| self.collected > *max) {
            return Err(Error::VarTooMany {
                name: flag.name,
                max: max as usize,
                got: self.collected as usize,
            });
        }
        self.collecting = if flag.var_max.is_some_and(|max| self.collected >= max) {
            None
        } else {
            Some(flag)
        };
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

    fn find_long_form(&self, name: &[u8]) -> Option<(&'t Flag<'t>, bool)> {
        if let Some(flag) = self
            .in_scope()
            .find(|f| f.longs.iter().any(|long| long.as_bytes() == name))
        {
            return Some((flag, false));
        }
        if let Some(flag) = self
            .in_scope()
            .find(|f| f.negate.is_some_and(|negate| negate.as_bytes() == name))
        {
            return Some((flag, true));
        }
        if !self.infer_long_args || name.is_empty() {
            return None;
        }

        let mut found: Option<(&Flag<'_>, bool)> = None;
        for flag in self.in_scope() {
            let positive = flag.longs.iter().any(|long| {
                long.as_bytes().starts_with(name)
                    && !self.long_form_is_shadowed(flag, long.as_bytes())
            });
            let negative = !positive
                && flag.negate.is_some_and(|negate| {
                    negate.as_bytes().starts_with(name)
                        && !self.long_form_is_shadowed(flag, negate.as_bytes())
                });
            if !positive && !negative {
                continue;
            }
            if found.is_some_and(|(prior, _)| !::core::ptr::eq(prior, flag)) {
                return None;
            }
            found = Some((flag, negative));
        }
        found
    }

    /// Whether a nearer declaration already owns this exact long spelling.
    ///
    /// `in_scope` is ordered by precedence. Prefix inference must apply the same
    /// shadowing as exact lookup: redeclaring `--verbose` on a child does not make
    /// `--verb` ambiguous merely because an inherited global also spells it that way.
    fn long_form_is_shadowed(&self, flag: &Flag<'_>, form: &[u8]) -> bool {
        for prior in self.in_scope() {
            if ::core::ptr::eq(prior, flag) {
                return false;
            }
            if prior.longs.iter().any(|long| long.as_bytes() == form)
                || prior.negate.is_some_and(|negate| negate.as_bytes() == form)
            {
                return true;
            }
        }
        false
    }

    fn find_short(&self, byte: u8) -> Option<&'t Flag<'t>> {
        self.in_scope()
            .find(|f| f.shorts.contains(&byte))
            // As for `--help`: supplied by the parser, and only where the command has not
            // declared a `-h` of its own.
            .or(if byte == b'h' {
                Some(&HELP_SHORT)
            } else if byte == b'V' && self.cmd.version {
                Some(&VERSION_SHORT)
            } else {
                None
            })
    }

    fn find_subcommand(&self, name: &[u8]) -> Option<&'t Command<'t>> {
        // Shared with `help` rather than spelled out again, so descending into a command and
        // asking about one cannot drift apart.
        if self.infer_subcommands {
            find_prefixed(self.cmd, name)
        } else {
            find_named(self.cmd, name)
        }
    }
}

/// View a token as bytes.
///
/// `as_encoded_bytes` is a plain accessor with no conversion and no allocation.
/// The reverse direction is the one with a cost — see [`os_string_from_bytes`] —
/// which is why values come back as bytes.
fn bytes<'v>(s: &&'v OsStr) -> &'v [u8] {
    s.as_encoded_bytes()
}

/// How many values one word carries.
///
/// One, until a delimiter is declared — and then one per separator, counting the same way
/// splitting on it does: `a,b` is two, `a,` is two with an empty second, and `` is one.
/// Counted rather than split because binding only needs the number, and the split itself
/// belongs to the layer that owns the values.
fn values_in(word: &[u8], delimiter: ::core::option::Option<u8>) -> u32 {
    match delimiter {
        Some(d) => 1 + word.iter().filter(|b| **b == d).count() as u32,
        None => 1,
    }
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
    /// Same shape as ROOT, but a CLI that owns all of its flags. The subcommand says
    /// nothing and inherits it, which is the point: only the root declares the mode.
    static STRICT_INSTALL: Command = Command {
        name: "install",
        aliases: &["i"],
        flags: &[&FORCE],
        key: 100,
        ..Command::EMPTY
    };
    static STRICT: Command = Command {
        name: "ex",
        flags: &[&FORCE, &JOBS, &COLOR, &VERBOSE],
        args: &[&FILE, &REST],
        subcommands: &[&STRICT_INSTALL],
        unknown_flags: Some(UnknownFlags::Error),
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
    fn parse<'t: 'v, 'v>(
        root: &'t Command<'t>,
        argv: &'v [&'v OsStr],
    ) -> Result<Vec<Event<'t, 'v, 'v>>, Error<'t, 'v>> {
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
    fn an_unmatched_word_is_forwarded_when_external_subcommand_is_set() {
        static CATCH: Command = Command {
            name: "ex",
            flags: &[&VERBOSE],
            subcommands: &[&INSTALL],
            external_subcommand: true,
            unknown_flags: Some(UnknownFlags::Error),
            ..Command::EMPTY
        };
        let a = argv(["foo", "--help", "bar"]);
        assert_eq!(
            parse(&CATCH, &a).unwrap(),
            vec![Event::External { values: &a[..] }]
        );

        let a = argv(["install"]);
        assert_eq!(parse(&CATCH, &a).unwrap(), vec![Event::Command(&INSTALL)]);

        let a = argv(["--verbose", "foo", "--verbose"]);
        assert_eq!(
            parse(&CATCH, &a).unwrap(),
            vec![
                Event::Flag {
                    flag: &VERBOSE,
                    value: None,
                    negated: false
                },
                Event::External { values: &a[1..] }
            ]
        );

        let a = argv(["--wat"]);
        assert_eq!(
            parse(&CATCH, &a),
            Err(Error::UnknownFlag { token: b"--wat" })
        );

        // A negative number is a value, not a flag, so it can be the unmatched word.
        let a = argv(["-1", "rest"]);
        assert_eq!(
            parse(&CATCH, &a).unwrap(),
            vec![Event::External { values: &a[..] }]
        );
    }

    #[test]
    fn a_default_subcommand_outranks_an_external_one() {
        static CATCH_DEFAULT: Command = Command {
            name: "ex",
            subcommands: &[&RUN],
            default_subcommand: Some(&RUN),
            external_subcommand: true,
            ..Command::EMPTY
        };
        let a = argv(["build"]);
        assert_eq!(
            parse(&CATCH_DEFAULT, &a).unwrap(),
            vec![
                Event::Command(&RUN),
                Event::Arg {
                    arg: &RUN_TASK,
                    value: b"build"
                }
            ]
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
    fn a_name_outranks_another_commands_alias() {
        // A spec `assert_unique_subcommand_names` would reject, resolved anyway: a parser
        // handed a table nothing validated still has to answer, and the answer is the
        // command whose own name it is. Both orders, because taking the first candidate
        // that matched on either name or alias made this depend on which was listed first
        // — and usage-lib, building a map, took the last.
        static ALPHA: Command = Command {
            name: "alpha",
            aliases: &["run"],
            key: 300,
            ..Command::EMPTY
        };
        static PLAIN_RUN: Command = Command {
            name: "run",
            key: 301,
            ..Command::EMPTY
        };
        for subcommands in [&[&ALPHA, &PLAIN_RUN] as &[&Command], &[&PLAIN_RUN, &ALPHA]] {
            assert!(::core::ptr::eq(
                find_subcommand(subcommands, "run"),
                &PLAIN_RUN
            ));
            let root: Command = Command {
                name: "ex",
                subcommands,
                ..Command::EMPTY
            };
            let a = argv(["run"]);
            assert_eq!(parse(&root, &a).unwrap(), vec![Event::Command(&PLAIN_RUN)]);
            // The alias still reaches its own command by every name it does not share.
            let a = argv(["alpha"]);
            assert_eq!(parse(&root, &a).unwrap(), vec![Event::Command(&ALPHA)]);
            // `ex help run` asks about the command `ex run` selects. These are separate
            // lookups — help resolves a path without descending — and answering differently
            // for a colliding word is the divergence this rule exists to end.
            let a = argv(["help", "run"]);
            match parse(&root, &a) {
                Err(Error::Help { cmd, .. }) => {
                    assert!(
                        ::core::ptr::eq(cmd, &PLAIN_RUN),
                        "got help for {}",
                        cmd.name
                    )
                }
                other => panic!("expected a help request, got {other:?}"),
            }
        }
    }

    #[test]
    #[should_panic(expected = "two subcommands answer to the same name")]
    fn an_alias_cannot_shadow_a_sibling_command() {
        static ADD: Command = Command {
            name: "add",
            aliases: &["install"],
            ..Command::EMPTY
        };
        assert_unique_subcommand_names(&[&INSTALL, &ADD]);
    }

    #[test]
    #[cfg(feature = "spec")]
    fn inferred_help_keeps_the_route_it_resolved() {
        let root = Command {
            name: "ex",
            subcommands: &[&INSTALL],
            infer_subcommands: true,
            ..Command::EMPTY
        };
        let a = argv(["help", "insta"]);
        let Err(Error::Help { cmd, .. }) = parse(&root, &a) else {
            panic!("expected inferred help")
        };
        let route = crate::help::route_to(&root, &a, cmd).expect("the inferred route");
        assert_eq!(route.len(), 2);
        assert!(::core::ptr::eq(route[1], &INSTALL));
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
    fn allow_hyphen_values_takes_a_flaglike_detached_value() {
        static ARGS: Flag = Flag {
            key: 6,
            name: "args",
            longs: &["args"],
            shorts: b"a",
            takes_value: true,
            allow_hyphen_values: true,
            ..Flag::BOOL
        };
        static DIR: Flag = Flag {
            key: 7,
            name: "working-dir",
            longs: &["working-dir"],
            shorts: b"d",
            ..Flag::VALUE
        };
        static HYPHEN: Command = Command {
            name: "ex",
            flags: &[&ARGS, &DIR],
            args: &[&REST],
            ..Command::EMPTY
        };

        let a = argv(["-a", "-destroy"]);
        assert_eq!(
            parse(&HYPHEN, &a).unwrap(),
            vec![Event::Flag {
                flag: &ARGS,
                value: Some(b"-destroy"),
                negated: false
            }]
        );

        let a = argv(["--args", "--", "-x"]);
        assert_eq!(
            parse(&HYPHEN, &a).unwrap(),
            vec![
                Event::Flag {
                    flag: &ARGS,
                    value: Some(b"--"),
                    negated: false
                },
                Event::Arg {
                    arg: &REST,
                    value: b"-x"
                },
            ]
        );
    }

    #[test]
    fn require_equals_refuses_a_detached_value() {
        static INSPECT: Flag = Flag {
            key: 8,
            name: "inspect",
            longs: &["inspect"],
            shorts: b"i",
            takes_value: true,
            require_equals: true,
            ..Flag::BOOL
        };
        static EQ: Command = Command {
            name: "ex",
            flags: &[&INSPECT],
            ..Command::EMPTY
        };

        let a = argv(["--inspect=9229"]);
        assert_eq!(
            parse(&EQ, &a).unwrap(),
            vec![Event::Flag {
                flag: &INSPECT,
                value: Some(b"9229"),
                negated: false
            }]
        );

        let a = argv(["--inspect", "9229"]);
        assert!(matches!(
            parse(&EQ, &a),
            Err(Error::MissingFlagValue { .. })
        ));

        let a = argv(["-i9229"]);
        assert_eq!(
            parse(&EQ, &a).unwrap(),
            vec![Event::Flag {
                flag: &INSPECT,
                value: Some(b"9229"),
                negated: false
            }]
        );

        static ALL: Flag = Flag {
            key: 9,
            name: "all",
            longs: &["all"],
            shorts: b"a",
            ..Flag::BOOL
        };
        static BUNDLE: Command = Command {
            name: "ex",
            flags: &[&ALL, &INSPECT],
            ..Command::EMPTY
        };
        let a = argv(["-ai", "9229"]);
        assert!(
            matches!(parse(&BUNDLE, &a), Err(Error::MissingFlagValue { .. })),
            "a require_equals short reached through a bundle still refuses the following word"
        );
    }

    #[test]
    fn default_missing_binds_when_the_value_is_left_off() {
        static COLOR: Flag = Flag {
            key: 9,
            name: "color",
            longs: &["color"],
            takes_value: true,
            default_missing: Some(b"always"),
            ..Flag::BOOL
        };
        static VERBOSE: Flag = Flag {
            key: 10,
            name: "verbose",
            longs: &["verbose"],
            ..Flag::BOOL
        };
        static MISSING: Command = Command {
            name: "ex",
            flags: &[&COLOR, &VERBOSE],
            ..Command::EMPTY
        };

        let a = argv(["--color"]);
        assert_eq!(
            parse(&MISSING, &a).unwrap(),
            vec![Event::Flag {
                flag: &COLOR,
                value: Some(b"always"),
                negated: false
            }]
        );

        let a = argv(["--color=never"]);
        assert_eq!(
            parse(&MISSING, &a).unwrap(),
            vec![Event::Flag {
                flag: &COLOR,
                value: Some(b"never"),
                negated: false
            }]
        );

        let a = argv(["--color", "--verbose"]);
        assert_eq!(
            parse(&MISSING, &a).unwrap(),
            vec![
                Event::Flag {
                    flag: &COLOR,
                    value: Some(b"always"),
                    negated: false
                },
                Event::Flag {
                    flag: &VERBOSE,
                    value: None,
                    negated: false
                },
            ]
        );

        let a = argv(["--color="]);
        assert_eq!(
            parse(&MISSING, &a).unwrap(),
            vec![Event::Flag {
                flag: &COLOR,
                value: Some(b""),
                negated: false
            }]
        );
    }

    #[test]
    fn default_missing_with_require_equals_leaves_the_following_word() {
        static INSPECT: Flag = Flag {
            key: 11,
            name: "inspect",
            longs: &["inspect"],
            takes_value: true,
            require_equals: true,
            default_missing: Some(b"9229"),
            ..Flag::BOOL
        };
        static BOTH: Command = Command {
            name: "ex",
            flags: &[&INSPECT],
            args: &[&REST],
            ..Command::EMPTY
        };

        let a = argv(["--inspect"]);
        assert_eq!(
            parse(&BOTH, &a).unwrap(),
            vec![Event::Flag {
                flag: &INSPECT,
                value: Some(b"9229"),
                negated: false
            }]
        );

        let a = argv(["--inspect", "80"]);
        assert_eq!(
            parse(&BOTH, &a).unwrap(),
            vec![
                Event::Flag {
                    flag: &INSPECT,
                    value: Some(b"9229"),
                    negated: false
                },
                Event::Arg {
                    arg: &REST,
                    value: b"80"
                },
            ]
        );

        let a = argv(["--inspect="]);
        assert_eq!(
            parse(&BOTH, &a).unwrap(),
            vec![Event::Flag {
                flag: &INSPECT,
                value: Some(b""),
                negated: false
            }]
        );
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

    #[test]
    fn a_multicall_applet_is_the_basename_unless_it_is_the_dispatcher() {
        assert_eq!(multicall_basename("/usr/bin/ls"), "ls");
        assert_eq!(multicall_basename(r"C:\busybox\ls.exe"), "ls");
        assert_eq!(
            multicall_applet("/usr/bin/ls", "busybox", Some("busybox")),
            Some("ls")
        );
        assert_eq!(
            multicall_applet("/usr/bin/busybox", "busybox", Some("busybox")),
            None
        );
        assert_eq!(
            multicall_applet("ls.exe", "busybox", Some("busybox")),
            Some("ls")
        );
        assert_eq!(
            multicall_applet("/usr/bin/busybox", "BusyBox", Some("/opt/bin/busybox")),
            None
        );
        assert_eq!(
            multicall_applet("busybox.exe", "BusyBox", Some("busybox.exe")),
            None
        );
    }
}
