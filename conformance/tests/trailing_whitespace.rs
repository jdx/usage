//! A description that ends in a newline should not add a blank line to the page.
//!
//! clap's `long_about` often ends with one — a `///` block whose last line is empty, or an
//! examples section written with a trailing break — and it reaches the spec verbatim. Wherever
//! a renderer writes its own blank line after a description, one already in the text doubled it:
//! a stray blank in the middle of `Commands:` back when that list printed long descriptions, and
//! a second gap under the program's own description, which it still would.
//!
//! Found on pitchfork's `daemons add` and mise's `plugins ls-remote`, which is why the fix is in
//! both renderers rather than one — trimming either side alone traded one CLI's divergence for
//! another's. Recorded as a decision: the page loses whitespace nobody wrote on purpose, rather
//! than one renderer reproducing it faithfully.

use usage::Spec as LibSpec;
use usage_argv::help;
use usage_derive::{Args, Cli, Subcommands};

/// Short one
///
/// First.
///
/// Second.
#[derive(Args)]
struct One {
    #[usage(long)]
    thing: bool,
}

/// Short two
#[derive(Args)]
struct Two {
    #[usage(long)]
    other: bool,
}

#[derive(Subcommands)]
enum Commands {
    /// Short one
    ///
    /// A doc comment cannot carry a trailing break — Rust drops it — so the case has to be
    /// declared. This is also the shape `gen-shadow` emits when a spec's two descriptions are
    /// independent, which is how pitchfork's reached the fixture at all.
    #[usage(name = "one", help = "Short one", long_help = "First.\n\nSecond.\n")]
    One(One),
    /// Short two
    Two(Two),
}

/// A tool
///
/// The program's own description, which also ends in a break.
#[derive(Cli)]
#[usage(
    bin = "ex",
    about = "A tool",
    long_about = "A tool\n\nThe program's own description, which ends in a break.\n"
)]
struct Ex {
    #[usage(subcommand)]
    command: Option<Commands>,
}

#[test]
fn a_description_ending_in_a_break_adds_no_blank_line() {
    // Two blanks in a row would mean the trailing newline reached the page. One is the
    // separator each entry gets; two is the bug.
    let page = help::render(Ex::spec(), Ex::command(), true).expect("a page");
    assert!(
        !page.contains("\n\n\n"),
        "no run of blank lines should survive:\n{page:?}"
    );
}

#[test]
fn the_two_renderers_agree_about_it() {
    // The invariant: whatever the rule is, both sides hold it. Trimming one alone is what
    // turned pitchfork green and mise red.
    let spec: LibSpec = Ex::to_kdl().parse().expect("valid spec");
    for long in [false, true] {
        let ours = help::render(Ex::spec(), Ex::command(), long).expect("a page");
        let theirs = usage::docs::cli::render_help(&spec, &spec.cmd, long);
        assert_eq!(ours, theirs, "long={long}");
    }
}

#[test]
fn the_commands_still_parse() {
    // Also what keeps the fixture's fields read: these tests only render, and an unread field
    // in a generated CLI is a warning the adopter cannot silence.
    let argv = [std::ffi::OsStr::new("one"), std::ffi::OsStr::new("--thing")];
    let parsed = Ex::parse_from(&argv).expect("parses");
    match parsed.command {
        Some(Commands::One(one)) => assert!(one.thing),
        _ => panic!("routed to one"),
    }

    let argv = [std::ffi::OsStr::new("two"), std::ffi::OsStr::new("--other")];
    let parsed = Ex::parse_from(&argv).expect("parses");
    match parsed.command {
        Some(Commands::Two(two)) => assert!(two.other),
        _ => panic!("routed to two"),
    }
}

#[test]
fn the_entries_sit_on_consecutive_lines() {
    // The long page lists commands the way the short one does — one line each, in one column,
    // with no separator between them — so a description ending in a break has nowhere to leave
    // a blank behind, and the entries must not gain one either.
    let page = help::render(Ex::spec(), Ex::command(), true).expect("a page");
    let commands = page
        .split_once("Commands:\n")
        .expect("a commands section")
        .1;
    assert!(
        commands.starts_with("  one   Short one\n  two   Short two\n"),
        "{commands:?}"
    );
}
