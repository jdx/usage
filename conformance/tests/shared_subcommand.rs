//! One `Subcommands` type mounted under two parents.
//!
//! Legal, and the tables say nothing about which mount you are in: both parents splice the same
//! `&'static [Command]`, so `alpha shared` and `beta shared` are one `Command` at one address.
//! A lookup that searches the metadata tree for that address finds whichever mount comes first,
//! which is not the one the user typed.
//!
//! The route is the only thing that tells them apart, so diagnostics resolve by the route the
//! parser took rather than by the command it ended on.

use std::ffi::OsStr;

use usage_argv::diagnostic::{render, Style};
use usage_derive::{Args, Cli, Subcommands};

/// Do the shared thing
#[derive(Args)]
struct Shared {
    /// A flag of its own
    #[usage(long)]
    thing: bool,
}

#[derive(Subcommands)]
enum Both {
    /// Do the shared thing
    Shared(Shared),
}

/// The first parent
#[derive(Args)]
struct Alpha {
    /// Only alpha declares this
    #[usage(long, global)]
    alphaglobal: bool,
    #[usage(subcommand)]
    command: Option<Both>,
}

/// The second parent
#[derive(Args)]
struct Beta {
    /// Only beta declares this
    #[usage(long, global)]
    betaglobal: bool,
    #[usage(subcommand)]
    command: Option<Both>,
}

#[derive(Subcommands)]
enum Top {
    /// The first parent
    Alpha(Box<Alpha>),
    /// The second parent
    Beta(Box<Beta>),
}

/// A tool that mounts one set of subcommands twice
#[derive(Cli)]
#[usage(bin = "ex", unknown_flags = "error")]
struct Ex {
    #[usage(subcommand)]
    command: Option<Top>,
}

#[test]
fn the_two_mounts_really_are_one_command() {
    // The premise, asserted rather than assumed: if these ever stop sharing an address the test
    // below still passes, and would stop testing anything.
    let root = Ex::command();
    let alpha = root.subcommands.iter().find(|c| c.name == "alpha").unwrap();
    let beta = root.subcommands.iter().find(|c| c.name == "beta").unwrap();
    let a = alpha
        .subcommands
        .iter()
        .find(|c| c.name == "shared")
        .unwrap();
    let b = beta
        .subcommands
        .iter()
        .find(|c| c.name == "shared")
        .unwrap();
    assert!(
        core::ptr::eq(a, b),
        "the two mounts no longer share a command, so this file tests nothing"
    );
}

#[test]
fn an_error_under_the_second_mount_describes_the_second_mount() {
    let owned: Vec<&OsStr> = ["beta", "shared", "--betaglobl"]
        .iter()
        .map(|s| OsStr::new(*s))
        .collect();
    let Err(err) = Ex::parse_from(&owned) else {
        panic!("no such flag")
    };
    let message = render(Ex::spec(), &owned, &err, Style::PLAIN);

    // The usage line named the wrong command entirely: `ex alpha shared`, for a line that says
    // `beta`.
    assert!(
        message.contains("Usage: ex beta shared"),
        "the usage line is for the command that was typed: {message}"
    );
    // And the tip came from the wrong ancestor's globals, so the flag that would have worked
    // went unmentioned while a flag the parser refuses here was on offer.
    assert!(
        message.contains("tip: a similar argument exists: '--betaglobal'"),
        "{message}"
    );
    assert!(
        !message.contains("alphaglobal"),
        "a global from an unrelated branch: {message}"
    );
}

#[test]
fn the_first_mount_is_unaffected() {
    // The half that passed before, and has to keep passing: resolving by route must not make the
    // first mount resolve like the second.
    let owned: Vec<&OsStr> = ["alpha", "shared", "--alphaglobl"]
        .iter()
        .map(|s| OsStr::new(*s))
        .collect();
    let Err(err) = Ex::parse_from(&owned) else {
        panic!("no such flag")
    };
    let message = render(Ex::spec(), &owned, &err, Style::PLAIN);
    assert!(message.contains("Usage: ex alpha shared"), "{message}");
    assert!(
        message.contains("tip: a similar argument exists: '--alphaglobal'"),
        "{message}"
    );
}

#[test]
fn the_fields_are_bound_under_either_parent() {
    // Also what keeps every field read, which CI requires of a test CLI: a field nothing looks
    // at is dead code, and silencing that would let a declaration rot unnoticed.
    let owned: Vec<&OsStr> = ["beta", "--betaglobal", "shared", "--thing"]
        .iter()
        .map(|s| OsStr::new(*s))
        .collect();
    let ex = Ex::parse_from(&owned).expect("should parse");
    let Some(Top::Beta(beta)) = ex.command else {
        panic!("expected beta")
    };
    assert!(beta.betaglobal, "a global binds after its own command");
    let Some(Both::Shared(shared)) = beta.command else {
        panic!("expected shared")
    };
    assert!(shared.thing);

    // And the same enum under the other parent, which is the point of the file.
    let owned: Vec<&OsStr> = ["alpha", "--alphaglobal", "shared"]
        .iter()
        .map(|s| OsStr::new(*s))
        .collect();
    let ex = Ex::parse_from(&owned).expect("should parse");
    let Some(Top::Alpha(alpha)) = ex.command else {
        panic!("expected alpha")
    };
    assert!(alpha.alphaglobal);
    assert!(matches!(alpha.command, Some(Both::Shared(_))));
}

#[test]
fn a_page_follows_the_route_the_words_took() {
    // The half the diagnostics fixed and help did not. `render` has only a `&Command` to go on
    // and finds it by address — and both mounts *are* one address, so it returned whichever
    // came first. `ex beta shared --help` printed `Usage: ex alpha shared`, with alpha's
    // globals, for as long as this crate has had help.
    //
    // Both spellings of a help request, because they reach it differently: `--help` stops
    // where the parse got to, and `help beta shared` asks about a command deeper than that.
    for words in [
        vec!["beta", "shared", "--help"],
        vec!["help", "beta", "shared"],
    ] {
        let owned: Vec<&OsStr> = words.iter().map(|s| OsStr::new(*s)).collect();
        let Err(usage_argv::Error::Help { cmd, long }) = Ex::parse_from(&owned) else {
            panic!("{words:?} should ask for help")
        };
        let route = usage_argv::help::route_to(Ex::command(), &owned, cmd)
            .unwrap_or_else(|| panic!("{words:?}: no route"));
        let page = usage_argv::help::render_at(Ex::spec(), &route, long).expect("a page");

        assert!(page.contains("Usage: ex beta shared"), "{words:?}: {page}");
        assert!(page.contains("--betaglobal"), "{words:?}: {page}");
        assert!(!page.contains("alphaglobal"), "{words:?}: {page}");
    }
}

#[test]
fn the_first_mount_is_still_its_own() {
    // The other half: resolving by route must not make every page beta's.
    let owned: Vec<&OsStr> = ["alpha", "shared", "--help"]
        .iter()
        .map(|s| OsStr::new(*s))
        .collect();
    let Err(usage_argv::Error::Help { cmd, long }) = Ex::parse_from(&owned) else {
        panic!("should ask for help")
    };
    let route = usage_argv::help::route_to(Ex::command(), &owned, cmd).expect("a route");
    let page = usage_argv::help::render_at(Ex::spec(), &route, long).expect("a page");
    assert!(page.contains("Usage: ex alpha shared"), "{page}");
    assert!(page.contains("--alphaglobal"), "{page}");
}
