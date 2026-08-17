//! A `global` flag may be given once per command, not once per line.
//!
//! It is in scope for every descendant, so `ex -y sub -y` names it twice and means it once —
//! the inner occurrence wins. clap works that way, and mise ships clap, so `mise -y install -y`
//! is a line that works today; usage-argv refused it as a duplicate.
//!
//! Repeating it at *one* level is still an error, which is why the check is per level rather
//! than simply off for globals. clap draws the same line, and both halves are measured here.

use std::ffi::OsStr;

use usage_argv::Error;
use usage_derive::{Args, Cli, Subcommands};

/// Do the thing
#[derive(Args)]
struct Sub {
    /// Its own flag
    #[usage(long)]
    thing: bool,
    #[usage(subcommand)]
    command: Option<Deeper>,
}

/// Deeper still
#[derive(Args)]
struct Leaf {
    #[usage(long)]
    leafy: bool,
}

#[derive(Subcommands)]
enum Deeper {
    /// A third level
    Leaf(Leaf),
}

#[derive(Subcommands)]
enum Commands {
    /// A second level
    Sub(Sub),
}

#[derive(Cli)]
#[usage(bin = "ex")]
struct Ex {
    /// Say yes to everything
    #[usage(long, short = 'y', global)]
    yes: bool,
    /// Not global, for contrast
    #[usage(long, short = 'q')]
    quiet: bool,
    #[usage(subcommand)]
    command: Option<Commands>,
}

fn parse(words: &[&str]) -> Result<Ex, String> {
    let owned: Vec<&OsStr> = words.iter().map(|s| OsStr::new(*s)).collect();
    Ex::parse_from(&owned).map_err(|e| format!("{e:?}"))
}

/// The failure a line produced, or `None` if it parsed. `Ex` derives no `Debug`, which a
/// generated struct has no reason to, so this reports the error rather than the value.
fn failure(words: &[&str]) -> Option<String> {
    parse(words).err()
}

#[test]
fn across_a_command_boundary_the_inner_occurrence_wins() {
    let parsed = parse(&["-y", "sub", "-y"]).expect("a global may be repeated deeper");
    assert!(parsed.yes);

    // Three levels, given at each: still one flag, meant once.
    let parsed = parse(&["-y", "sub", "-y", "leaf", "-y"]).expect("at every level");
    assert!(parsed.yes);

    // And the long form, which is the same flag by another name.
    let parsed = parse(&["--yes", "sub", "--yes"]).expect("long form too");
    assert!(parsed.yes);
}

#[test]
fn twice_at_one_level_is_still_a_duplicate() {
    // The half a blanket exemption for globals would lose. clap refuses these too.
    for words in [
        &["-y", "-y"][..],
        &["sub", "-y", "-y"][..],
        &["-y", "sub", "-y", "-y"][..],
        &["--yes", "--yes"][..],
    ] {
        let err = failure(words)
            .unwrap_or_else(|| panic!("{words:?} repeats at one level and should be refused"));
        assert!(err.contains("DuplicateFlag"), "{words:?}: {err}");
    }
}

#[test]
fn a_flag_that_is_not_global_is_unaffected() {
    // `--quiet` belongs to the root alone, so there is no second level at which to give it and
    // nothing about this rule should reach it.
    let err = failure(&["-q", "-q"]).expect("a non-global repeat is a duplicate");
    assert!(err.contains("DuplicateFlag"), "{err}");

    let parsed = parse(&["-q", "sub"]).expect("once is fine");
    assert!(parsed.quiet);
}

#[test]
fn the_levels_bind_their_own_flags_too() {
    // Not only about `-y`: the words at each level still land where they belong. This is also
    // what keeps the fixture's fields read — an unread field in a generated CLI is a warning the
    // adopter cannot silence.
    let parsed = parse(&["-y", "sub", "--thing", "-y", "leaf", "--leafy"]).expect("parses");
    assert!(parsed.yes);
    let Some(Commands::Sub(sub)) = parsed.command else {
        panic!("routed to sub")
    };
    assert!(sub.thing);
    let Some(Deeper::Leaf(leaf)) = sub.command else {
        panic!("routed to leaf")
    };
    assert!(leaf.leafy);
}

#[test]
fn the_error_still_names_the_flag() {
    let owned: Vec<&OsStr> = ["-y", "-y"].iter().map(|s| OsStr::new(*s)).collect();
    match Ex::parse_from(&owned) {
        Err(Error::DuplicateFlag { name }) => assert_eq!(name, "yes"),
        Err(other) => panic!("expected a duplicate naming `yes`, got {other:?}"),
        Ok(_) => panic!("expected a duplicate naming `yes`, and it parsed"),
    }
}
