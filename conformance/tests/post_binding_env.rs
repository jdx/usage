//! Where the environment fills in what argv left out.
//!
//! Its own test binary, and that is the point. Environment variables are
//! process-wide, so a test that sets one races every other test in the same binary
//! that reads it — including one that only reads it *implicitly*, by parsing a CLI
//! with an `env` field. Consolidating the cases into a single `#[test]` fixes the
//! race between those cases and not the race with their neighbours. A separate file
//! is a separate process, which does fix it.

use std::ffi::OsStr;

use usage_derive::Cli;

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
