//! How much a CLI says, and whether it colors it, declared on the flags it already has.
//!
//! Every CLI in the fleet hand-rolls this: mise turns six flags into a level in a
//! forty-nine-line function, hk has the same shape with three, aube spells quiet as a value
//! of `--loglevel`, fnox has a lone `--no-color`. None of them could say so in a spec, so
//! help, documentation and anything else reading the spec saw six ordinary booleans.
//!
//! Two implementations answer the question — usage-lib interpreting the emitted spec, and the
//! compiled parser reading its own tables — so the point of this file is that they agree. The
//! shapes exercised are the fleet's, unchanged: nobody had to respell a flag to declare what
//! it already meant.

use std::ffi::OsStr;

use usage::parse::parse;
use usage::{ColorChoice, Spec as LibSpec, Verbosity};
use usage_argv::policy::{ColorPolicy, VerbosityPolicy};
use usage_derive::{Cli, ValueEnum};

/// mise's root, with the six-flag override lattice it really declares.
#[derive(Cli)]
#[usage(bin = "mise", name = "mise")]
struct Mise {
    /// Show extra output (use -vv for even more)
    #[usage(
        long,
        short = 'v',
        global,
        count,
        verbosity = "verbose",
        overrides("--quiet", "--silent", "--trace", "--debug", "--log-level")
    )]
    verbose: u8,
    /// Suppress non-error messages
    #[usage(
        long,
        short = 'q',
        global,
        verbosity = "error",
        overrides("--verbose", "--silent", "--trace", "--debug", "--log-level")
    )]
    quiet: bool,
    /// Suppress all task output and mise non-error messages
    #[usage(
        long,
        global,
        verbosity = "silent",
        overrides("--verbose", "--quiet", "--trace", "--debug", "--log-level")
    )]
    silent: bool,
    /// Sets log level to debug
    #[usage(
        long,
        global,
        hide,
        verbosity = "debug",
        overrides("--verbose", "--quiet", "--silent", "--trace", "--log-level")
    )]
    debug: bool,
    /// Sets log level to trace
    #[usage(
        long,
        global,
        hide,
        verbosity = "trace",
        overrides("--verbose", "--quiet", "--silent", "--debug", "--log-level")
    )]
    trace: bool,
    #[usage(
        long,
        global,
        hide,
        value_name = "LEVEL",
        // mise's own list, `warning` and all.
        choices("trace", "debug", "info", "warning", "error"),
        verbosity = "level",
        overrides("--verbose", "--quiet", "--silent", "--debug", "--trace")
    )]
    log_level: Option<String>,
}

/// aube's, where quiet is a *value* of the level flag and color is two switches.
#[derive(Debug, PartialEq, Eq, ValueEnum)]
enum Loglevel {
    Trace,
    Debug,
    Info,
    Warn,
    Error,
    Silent,
}

#[derive(Cli)]
#[usage(bin = "aube", name = "aube")]
struct Aube {
    /// Enable verbose/debug logging (shortcut for `--loglevel debug`)
    #[usage(long, short = 'v', global, verbosity = "debug")]
    verbose: bool,
    /// Set the log level. Logs at or above this level are shown.
    #[usage(long, global, value_enum, value_name = "LEVEL", verbosity = "level")]
    loglevel: Option<Loglevel>,
    /// Suppress all non-error output (alias for `--loglevel silent`)
    #[usage(long, global, verbosity = "silent")]
    silent: bool,
    /// Force colored output even when stderr is not a TTY
    #[usage(long, global, conflicts = "--no-color", color = "always")]
    color: bool,
    /// Disable colored output
    #[usage(long, global, color = "never")]
    no_color: bool,
}

/// hk's triangle: a counted `-v` and two switches, all mutually overriding.
#[derive(Cli)]
#[usage(bin = "hk", name = "hk")]
struct Hk {
    /// Enables verbose output
    #[usage(
        long,
        short = 'v',
        global,
        count,
        verbosity = "verbose",
        overrides("--quiet", "--silent")
    )]
    verbose: u8,
    /// Suppresses non-essential output
    #[usage(
        long,
        short = 'q',
        global,
        verbosity = "quiet",
        overrides("--verbose", "--silent")
    )]
    quiet: bool,
    /// Suppresses all output including warnings
    #[usage(long, global, verbosity = "silent", overrides("--quiet", "--verbose"))]
    silent: bool,
    /// Enable tracing spans and performance diagnostics
    //
    // Deliberately unannotated: hk's `--trace` turns on spans, not a level. A role is opt-in
    // per flag and never claims a spelling, which is what lets this stay what it is.
    #[usage(long, global)]
    trace: bool,
}

/// fnox: one word, and it is the only change fnox makes.
#[derive(Cli)]
#[usage(bin = "fnox", name = "fnox")]
struct Fnox {
    /// Enable verbose logging
    #[usage(long, short = 'v', global, verbosity = "debug")]
    verbose: bool,
    /// Disable colored output
    #[usage(long, global, color = "never")]
    no_color: bool,
}

/// The other color shape: one negatable switch rather than two flags.
#[derive(Cli)]
#[usage(bin = "paired", name = "paired")]
struct Paired {
    /// Colorize output
    //
    // The default is what an absent flag means, and a negatable switch has to declare it:
    // a `bool` holds the answer rather than whether one was given, so without it `false`
    // would read as `--no-color` rather than as silence. The spec and the derive both
    // refuse the flag without one.
    #[usage(long, global, negate = "no-color", default = "true", color = "always")]
    color: bool,
}

/// A plain switch carrying an explicit `default`, which is where "absent" and "said no"
/// are easiest to confuse: the parse output holds the default for a flag nobody typed.
#[derive(Cli)]
#[usage(bin = "defaulted", name = "defaulted")]
struct Defaulted {
    /// Disable colored output
    #[usage(long, global, color = "never", default = "false")]
    no_color: bool,
}

/// The roles a command gets from somewhere else: a group it flattens, which the derive now
/// lowers into a `flagset`. hk's shape, written once and given to several commands.
#[derive(usage_derive::Args)]
struct Loudness {
    /// Enables verbose output
    #[usage(long, short = 'v', global, count, verbosity = "verbose")]
    verbose: u8,
    /// Disable colored output
    #[usage(long, global, color = "never")]
    no_color: bool,
}

#[derive(Cli)]
#[usage(bin = "borrowed", name = "borrowed")]
struct Borrowed {
    #[usage(flatten)]
    loudness: Loudness,
    #[usage(long, global)]
    jobs: Option<String>,
}

/// A CLI that declares no roles at all, which most of the fleet is.
#[derive(Cli)]
#[usage(bin = "tak", name = "tak")]
struct Tak {
    #[usage(long, global)]
    runner: Option<String>,
}

/// The level usage-lib resolves, interpreting the spec the derive emitted.
fn interpreted(kdl: &str, argv: &[&str]) -> (Verbosity, ColorChoice) {
    let spec: LibSpec = kdl.parse().expect("usage-lib should read the emitted spec");
    let words: Vec<String> = std::iter::once(spec.bin.clone())
        .chain(argv.iter().map(|w| (*w).to_string()))
        .collect();
    let parsed = parse(&spec, &words).expect("valid command line");
    (parsed.verbosity(), parsed.color())
}

/// The same question, put to the compiled parser through the built struct.
fn compiled<T>(argv: &[&str]) -> (Verbosity, ColorChoice)
where
    T: VerbosityPolicy + ColorPolicy,
    T: for<'v> TypedParse<'v>,
{
    let words: Vec<&OsStr> = argv.iter().map(OsStr::new).collect();
    let cli = T::parse_words(&words);
    (
        match VerbosityPolicy::verbosity(&cli) {
            usage_argv::policy::Verbosity::Silent => Verbosity::Silent,
            usage_argv::policy::Verbosity::Error => Verbosity::Error,
            usage_argv::policy::Verbosity::Warn => Verbosity::Warn,
            usage_argv::policy::Verbosity::Info => Verbosity::Info,
            usage_argv::policy::Verbosity::Debug => Verbosity::Debug,
            usage_argv::policy::Verbosity::Trace => Verbosity::Trace,
        },
        match ColorPolicy::color(&cli) {
            usage_argv::policy::ColorChoice::Auto => ColorChoice::Auto,
            usage_argv::policy::ColorChoice::Always => ColorChoice::Always,
            usage_argv::policy::ColorChoice::Never => ColorChoice::Never,
        },
    )
}

/// Parsing one of the CLIs above, without naming each of them at every call site.
trait TypedParse<'v>: Sized {
    fn parse_words(argv: &'v [&'v OsStr]) -> Self;
}

macro_rules! typed_parse {
    ($ty:ty) => {
        impl<'v> TypedParse<'v> for $ty {
            fn parse_words(argv: &'v [&'v OsStr]) -> Self {
                <$ty>::parse_from(argv).expect("valid command line")
            }
        }
    };
}

typed_parse!(Mise);
typed_parse!(Aube);
typed_parse!(Hk);
typed_parse!(Fnox);
typed_parse!(Paired);
typed_parse!(Defaulted);
typed_parse!(Borrowed);
typed_parse!(Tak);

/// Both implementations, held to the same answer.
macro_rules! agree {
    ($ty:ty, $kdl:expr, $argv:expr, $level:expr, $color:expr) => {{
        let argv: &[&str] = &$argv;
        let want = ($level, $color);
        assert_eq!(compiled::<$ty>(argv), want, "compiled: {argv:?}");
        assert_eq!(interpreted(&$kdl, argv), want, "interpreted: {argv:?}");
    }};
}

#[test]
fn mise_six_flag_lattice() {
    let kdl = Mise::to_kdl();
    agree!(Mise, kdl, [], Verbosity::Info, ColorChoice::Auto);
    agree!(Mise, kdl, ["-v"], Verbosity::Debug, ColorChoice::Auto);
    agree!(Mise, kdl, ["-vv"], Verbosity::Trace, ColorChoice::Auto);
    // Past the end of the scale is still the end of the scale.
    agree!(Mise, kdl, ["-vvvvv"], Verbosity::Trace, ColorChoice::Auto);
    agree!(Mise, kdl, ["-q"], Verbosity::Error, ColorChoice::Auto);
    agree!(
        Mise,
        kdl,
        ["--silent"],
        Verbosity::Silent,
        ColorChoice::Auto
    );
    agree!(Mise, kdl, ["--debug"], Verbosity::Debug, ColorChoice::Auto);
    agree!(Mise, kdl, ["--trace"], Verbosity::Trace, ColorChoice::Auto);
    agree!(
        Mise,
        kdl,
        ["--log-level", "warning"],
        Verbosity::Warn,
        ColorChoice::Auto
    );

    // The lattice, not the resolver, settles a contradiction: `overrides` removes the
    // displaced flag during the parse, so only one of these ever reaches the question.
    agree!(
        Mise,
        kdl,
        ["-vv", "--quiet"],
        Verbosity::Error,
        ColorChoice::Auto
    );
    agree!(
        Mise,
        kdl,
        ["--quiet", "-vv"],
        Verbosity::Trace,
        ColorChoice::Auto
    );
    agree!(
        Mise,
        kdl,
        ["--silent", "--log-level", "trace"],
        Verbosity::Trace,
        ColorChoice::Auto
    );
}

#[test]
fn aube_level_as_a_value_and_color_as_a_pair() {
    let kdl = Aube::to_kdl();
    agree!(Aube, kdl, [], Verbosity::Info, ColorChoice::Auto);
    agree!(Aube, kdl, ["-v"], Verbosity::Debug, ColorChoice::Auto);
    // `silent` is a point on the scale rather than a separate concept, which is exactly why
    // `--loglevel silent` and `--silent` resolve the same way with no special case.
    agree!(
        Aube,
        kdl,
        ["--loglevel", "silent"],
        Verbosity::Silent,
        ColorChoice::Auto
    );
    agree!(
        Aube,
        kdl,
        ["--silent"],
        Verbosity::Silent,
        ColorChoice::Auto
    );
    // No lattice here, so both arrive: the value pins over the switch.
    agree!(
        Aube,
        kdl,
        ["-v", "--loglevel", "trace"],
        Verbosity::Trace,
        ColorChoice::Auto
    );
    // And where two switches disagree, the more restrictive one wins.
    agree!(
        Aube,
        kdl,
        ["-v", "--silent"],
        Verbosity::Silent,
        ColorChoice::Auto
    );

    agree!(Aube, kdl, ["--color"], Verbosity::Info, ColorChoice::Always);
    agree!(
        Aube,
        kdl,
        ["--no-color"],
        Verbosity::Info,
        ColorChoice::Never
    );
}

#[test]
fn hk_counted_triangle() {
    let kdl = Hk::to_kdl();
    agree!(Hk, kdl, [], Verbosity::Info, ColorChoice::Auto);
    agree!(Hk, kdl, ["-vv"], Verbosity::Trace, ColorChoice::Auto);
    // hk's `-q` is a step rather than a level, which is what its help says it is.
    agree!(Hk, kdl, ["-q"], Verbosity::Warn, ColorChoice::Auto);
    agree!(Hk, kdl, ["--silent"], Verbosity::Silent, ColorChoice::Auto);
    // `--trace` declares nothing, so it moves nothing.
    agree!(Hk, kdl, ["--trace"], Verbosity::Info, ColorChoice::Auto);
}

#[test]
fn fnox_one_word() {
    let kdl = Fnox::to_kdl();
    agree!(Fnox, kdl, [], Verbosity::Info, ColorChoice::Auto);
    agree!(Fnox, kdl, ["-v"], Verbosity::Debug, ColorChoice::Auto);
    agree!(
        Fnox,
        kdl,
        ["--no-color"],
        Verbosity::Info,
        ColorChoice::Never
    );
}

#[test]
fn a_negatable_switch_says_both_answers() {
    let kdl = Paired::to_kdl();
    // The declared default is the third statement: what the command line means when it
    // mentions neither spelling.
    agree!(Paired, kdl, [], Verbosity::Info, ColorChoice::Always);
    agree!(
        Paired,
        kdl,
        ["--color"],
        Verbosity::Info,
        ColorChoice::Always
    );
    agree!(
        Paired,
        kdl,
        ["--no-color"],
        Verbosity::Info,
        ColorChoice::Never
    );
}

#[test]
fn a_default_is_not_the_same_as_an_answer() {
    let kdl = Defaulted::to_kdl();
    // The flag was not given. A plain switch has no way to say "no" — that is what a
    // negation is for — so its `false`, default or otherwise, says nothing at all.
    agree!(Defaulted, kdl, [], Verbosity::Info, ColorChoice::Auto);
    // And when it is given, it says the one thing it can.
    agree!(
        Defaulted,
        kdl,
        ["--no-color"],
        Verbosity::Info,
        ColorChoice::Never
    );
}

#[test]
fn a_flattened_group_answers_for_the_command_that_holds_it() {
    // The declarations belong to the command, so its answer is theirs — and the group is
    // emitted as a `flagset` the command `use`s, so this also holds the roles to surviving
    // that indirection on the way into the spec and back out.
    let kdl = Borrowed::to_kdl();
    assert!(kdl.contains("verbosity=verbose"), "{kdl}");
    assert!(kdl.contains("color=never"), "{kdl}");
    agree!(Borrowed, kdl, [], Verbosity::Info, ColorChoice::Auto);
    agree!(Borrowed, kdl, ["-vv"], Verbosity::Trace, ColorChoice::Auto);
    agree!(
        Borrowed,
        kdl,
        ["--no-color"],
        Verbosity::Info,
        ColorChoice::Never
    );
    // And a flag of the command's own moves nothing, which is the other half of "the
    // parent's roles and the group's are the same set".
    agree!(
        Borrowed,
        kdl,
        ["--jobs", "4", "-v"],
        Verbosity::Debug,
        ColorChoice::Auto
    );
}

#[test]
fn a_cli_that_declares_nothing_says_nothing() {
    let kdl = Tak::to_kdl();
    agree!(Tak, kdl, [], Verbosity::Info, ColorChoice::Auto);
    agree!(
        Tak,
        kdl,
        ["--runner", "local"],
        Verbosity::Info,
        ColorChoice::Auto
    );
}

#[test]
fn the_roles_survive_the_round_trip_through_kdl() {
    for kdl in [Mise::to_kdl(), Aube::to_kdl(), Hk::to_kdl(), Fnox::to_kdl()] {
        let spec: LibSpec = kdl.parse().expect("usage-lib should read the emitted spec");
        let rendered = spec.to_string();
        let reparsed: LibSpec = rendered.parse().expect("and read what it wrote");
        for (before, after) in spec.cmd.flags.iter().zip(reparsed.cmd.flags.iter()) {
            assert_eq!(before.verbosity, after.verbosity, "{}", before.name);
            assert_eq!(before.color, after.color, "{}", before.name);
        }
    }
}
