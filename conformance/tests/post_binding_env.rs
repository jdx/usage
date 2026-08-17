//! Where the environment fills in what argv left out.
//!
//! Its own test binary, and that is the point. Environment variables are
//! process-wide, so a test that sets one races every other test in the same binary
//! that reads it — including one that only reads it *implicitly*, by parsing a CLI
//! with an `env` field. Consolidating the cases into a single `#[test]` fixes the
//! race between those cases and not the race with their neighbours. A separate file
//! is a separate process, which does fix it.

use std::ffi::OsStr;

use usage_derive::{Args, Cli};

fn argv<const N: usize>(tokens: [&str; N]) -> [&OsStr; N] {
    tokens.map(OsStr::new)
}

/// A CLI whose values can come from the environment
#[derive(Cli)]
#[usage(bin = "ex")]
struct Ex {
    /// Where to write
    #[usage(long, env = "EX_ENV_OUT")]
    out: Option<String>,
    /// Colorize
    #[usage(long, env = "EX_ENV_COLOR")]
    color: bool,
    /// How loud
    #[usage(short = 'v', long, count, env = "EX_ENV_VERBOSE")]
    verbose: u8,
    /// What to act on
    target: String,
}

#[test]
fn the_environment_fills_what_argv_left_out() {
    // One test, in one process, so the order of these is the order they read.
    unsafe { std::env::set_var("EX_ENV_OUT", "from-env") };
    let a = argv(["x"]);
    let ex = Ex::parse_from(&a).expect("should parse");
    assert_eq!(ex.out.as_deref(), Some("from-env"));

    // What argv says wins.
    let a = argv(["--out", "from-argv", "x"]);
    let ex = Ex::parse_from(&a).expect("should parse");
    assert_eq!(ex.out.as_deref(), Some("from-argv"));

    // A switch reads as on for anything but a spelling of "off".
    unsafe { std::env::set_var("EX_ENV_COLOR", "1") };
    let a = argv(["x"]);
    assert!(Ex::parse_from(&a).expect("should parse").color);

    unsafe { std::env::set_var("EX_ENV_COLOR", "false") };
    let a = argv(["x"]);
    assert!(!Ex::parse_from(&a).expect("should parse").color);

    // A counting field takes a number, since the environment cannot repeat a flag.
    unsafe { std::env::set_var("EX_ENV_VERBOSE", "3") };
    let a = argv(["x"]);
    assert_eq!(Ex::parse_from(&a).expect("should parse").verbose, 3);

    // And something that is not a number leaves it alone rather than counting as
    // given — which was the bug: the value was discarded but the field was marked
    // filled.
    unsafe { std::env::set_var("EX_ENV_VERBOSE", "loud") };
    let a = argv(["x"]);
    assert_eq!(Ex::parse_from(&a).expect("should parse").verbose, 0);

    // An occurrence on the command line still wins.
    let a = argv(["-vv", "x"]);
    let ex = Ex::parse_from(&a).expect("should parse");
    assert_eq!(ex.verbose, 2);
    // The environment fills flags, not positionals that were given.
    assert_eq!(ex.target, "x");

    for var in ["EX_ENV_OUT", "EX_ENV_COLOR", "EX_ENV_VERBOSE"] {
        unsafe { std::env::remove_var(var) };
    }
}

/// A CLI where a flag with an environment variable can lose an override.
#[derive(Cli)]
#[usage(bin = "ovr")]
struct Ovr {
    /// Read from a file
    #[usage(long, env = "OVR_ENV_FILE", overrides = "--stdin")]
    file: Option<String>,
    /// Read from standard input
    #[usage(long)]
    stdin: bool,
    /// Where to write, which has to be given
    #[usage(long, overrides = "--quiet")]
    out: String,
    /// Say nothing
    #[usage(long)]
    quiet: bool,
}

/// An exclusive flag beside a value supplied by the environment.
#[derive(Cli)]
#[usage(bin = "exclusive-env")]
struct ExclusiveEnv {
    /// Dump and leave
    #[usage(long, exclusive)]
    dump: bool,
    /// Where to write
    #[usage(long, env = "EXCLUSIVE_ENV_OUT")]
    out: Option<String>,
}

#[test]
fn an_environment_value_counts_for_exclusivity() {
    unsafe { std::env::set_var("EXCLUSIVE_ENV_OUT", "from-env") };
    let a = argv(["--dump"]);
    assert!(ExclusiveEnv::parse_from(&a).is_err());
    unsafe { std::env::remove_var("EXCLUSIVE_ENV_OUT") };

    let parsed = ExclusiveEnv::parse_from(&a).expect("without the environment it is alone");
    assert!(parsed.dump);
    assert!(parsed.out.is_none());
}

#[derive(Args)]
struct FlattenedEnvOutput {
    /// Where to write
    #[usage(long, env = "FLAT_EXCLUSIVE_ENV_OUT")]
    out: Option<String>,
}

#[derive(Cli)]
#[usage(bin = "flat-exclusive-env")]
struct ExclusiveAcrossFlattenEnv {
    /// Dump and leave
    #[usage(long, exclusive)]
    dump: bool,
    #[usage(flatten)]
    extra: FlattenedEnvOutput,
}

#[derive(Args)]
struct FlattenedExclusiveEnv {
    /// Dump and leave
    #[usage(long, exclusive)]
    dump: bool,
}

#[derive(Cli)]
#[usage(bin = "flat-exclusive-env-reverse")]
struct EnvAcrossFlattenExclusive {
    /// Where to write
    #[usage(long, env = "FLAT_EXCLUSIVE_ENV_REVERSE_OUT")]
    out: Option<String>,
    #[usage(flatten)]
    extra: FlattenedExclusiveEnv,
}

#[test]
fn flattened_environment_values_are_visible_to_cross_boundary_exclusivity() {
    unsafe { std::env::set_var("FLAT_EXCLUSIVE_ENV_OUT", "from-env") };
    let a = argv(["--dump"]);
    assert!(ExclusiveAcrossFlattenEnv::parse_from(&a).is_err());
    unsafe { std::env::remove_var("FLAT_EXCLUSIVE_ENV_OUT") };

    unsafe { std::env::set_var("FLAT_EXCLUSIVE_ENV_REVERSE_OUT", "from-env") };
    let a = argv(["--dump"]);
    assert!(EnvAcrossFlattenExclusive::parse_from(&a).is_err());
    unsafe { std::env::remove_var("FLAT_EXCLUSIVE_ENV_REVERSE_OUT") };

    let a = argv(["--dump"]);
    let parsed = ExclusiveAcrossFlattenEnv::parse_from(&a).expect("alone after cleanup");
    assert!(parsed.dump);
    assert!(parsed.extra.out.is_none());
    let parsed = EnvAcrossFlattenExclusive::parse_from(&a).expect("alone after cleanup");
    assert!(parsed.out.is_none());
    assert!(parsed.extra.dump);
}

#[test]
fn a_displaced_flag_is_not_revived_by_its_environment_variable() {
    // The command line says `--stdin` came last, so `--file` lost. Filling it from the
    // environment afterwards would leave both standing and undo that.
    unsafe { std::env::set_var("OVR_ENV_FILE", "from-env") };

    let a = argv(["--file", "typed", "--stdin", "--out", "o"]);
    let ovr = Ovr::parse_from(&a).expect("should parse");
    assert!(ovr.stdin);
    assert_eq!(ovr.file, None, "displaced, and not refilled from the env");

    // Without the override in play, the environment still fills it.
    let a = argv(["--out", "o"]);
    let ovr = Ovr::parse_from(&a).expect("should parse");
    assert_eq!(ovr.file.as_deref(), Some("from-env"));
}

#[test]
fn a_displaced_flag_is_not_reported_missing() {
    // `--out` is a bare `String`, so it is required — but `--quiet` displaced it, which
    // is an answer rather than an omission.
    let a = argv(["--out", "o", "--quiet"]);
    let ovr = Ovr::parse_from(&a).expect("should parse");
    assert!(ovr.quiet);
    assert_eq!(ovr.out, "", "displaced back to its unset value");
}
