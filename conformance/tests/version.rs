//! `--version`, which a CLI declaring one should answer to.
//!
//! Measured from clap 4 rather than remembered — three rules, and the third is the one that is
//! easy to get wrong:
//!
//! ```text
//! $ ex --version      ex 1.2.3
//! $ ex -V             ex 1.2.3
//! $ ex other --version    error: unexpected argument '--version' found
//! ```
//!
//! The root only. clap propagates it to subcommands only when asked (`propagate_version`), and
//! a subcommand that declares its own `-V` keeps it.
//!
//! Supplied by the parser and *not* listed in help, exactly as `--help` is: a spec does not
//! declare either, so listing one would make the rendered page disagree with the spec it came
//! from. That is this crate's existing answer for `--help` and there is no reason for the two
//! to differ.

use std::ffi::OsStr;

use usage_argv::Error;
use usage_derive::{Args, Cli, Subcommands};

/// Do a thing
#[derive(Args)]
struct Go {
    /// Its own `-V`, which is not a version request
    #[usage(long, short = 'V')]
    verbose: bool,
}

/// Something else
#[derive(Args)]
struct Other {}

#[derive(Subcommands)]
enum Command {
    /// Do a thing
    Go(Box<Go>),
    /// Something else
    Other(Box<Other>),
}

/// A tool that knows its version
#[derive(Cli)]
#[usage(bin = "ex", version = "1.2.3")]
struct Versioned {
    #[usage(subcommand)]
    command: Option<Command>,
}

/// A tool that does not
#[derive(Cli)]
#[usage(bin = "plain")]
struct Unversioned {
    #[usage(long)]
    quiet: bool,
}

/// A tool that wants `-V` for itself
///
/// clap refuses this outright — it panics at startup saying `-V` is in use by both, and points
/// at `disable_version_flag`. Nothing has to break here: the CLI's own declaration wins for the
/// spelling it declares, and the supplied flag fills in what is left, which is exactly how
/// `--help` and `-h` already behave.
#[derive(Cli)]
#[usage(bin = "own", version = "9.9")]
struct OwnShort {
    /// Say more
    #[usage(long = "verbose", short = 'V')]
    verbose: bool,
}

/// A tool for which `--version` means something else
///
/// Not far-fetched: plenty of CLIs take a `--version <VERSION>` to select one. The declaration
/// wins, and `-V` still answers the question nobody else claimed.
#[derive(Cli)]
#[usage(bin = "picker", version = "9.9")]
struct OwnLong {
    /// Which version to use
    #[usage(long = "version")]
    wanted: Option<String>,
}

/// The text of a rendered message, without whatever the terminal asked for.
///
/// A tiny CSI stripper rather than a dependency: every escape this crate emits is `\x1b[`…`m`,
/// and a test that reads words should not care which of them arrived.
fn strip_ansi(s: &str) -> String {
    let mut out = String::with_capacity(s.len());
    let mut rest = s;
    while let Some(i) = rest.find('\u{1b}') {
        out.push_str(&rest[..i]);
        match rest[i..].find('m') {
            Some(end) => rest = &rest[i + end + 1..],
            None => {
                rest = "";
                break;
            }
        }
    }
    out.push_str(rest);
    out
}

fn argv<'a>(words: &'a [&'a str]) -> Vec<&'a OsStr> {
    words.iter().map(|w| OsStr::new(*w)).collect()
}

#[test]
fn a_version_is_answered_in_both_spellings() {
    for word in ["--version", "-V"] {
        let words = [word];
        let a = argv(&words);
        assert!(
            matches!(Versioned::parse_from(&a), Err(Error::Version)),
            "`{word}` should be a version request"
        );
    }
}

#[test]
fn a_cli_with_no_version_has_no_flag_to_answer_with() {
    // clap adds the flag exactly when a version is declared, and a `--version` that answers
    // with nothing is worse than one that is not there. Here it is an ordinary unknown flag —
    // and gets the ordinary treatment, tip and all.
    let a = argv(&["--version"]);
    assert!(matches!(
        Unversioned::parse_from(&a),
        Err(Error::UnknownFlag { .. }) | Err(Error::UnexpectedArg { .. })
    ));
}

#[test]
fn a_subcommand_does_not_inherit_it() {
    // clap's default, and the reason is that a subcommand's version is the program's: asking
    // `ex other --version` is asking a question the subcommand has no answer to.
    let a = argv(&["other", "--version"]);
    assert!(
        !matches!(Versioned::parse_from(&a), Err(Error::Version)),
        "a subcommand should not answer the root's question"
    );
}

#[test]
fn a_declared_short_wins_over_the_supplied_one() {
    // On the *root*, where the supplied flag would otherwise be in scope — a subcommand never
    // has one, so testing it there would prove nothing. The command's own flags are looked up
    // first, the same rule that lets a CLI declare its own `-h`.
    let a = argv(&["-V"]);
    let parsed = OwnShort::parse_from(&a).expect("its own flag, not a version request");
    assert!(parsed.verbose);

    // And the spelling it did *not* take still answers, which is what makes this better than
    // clap's answer of refusing to start: nothing is lost, only what was claimed is claimed.
    let a = argv(&["--version"]);
    assert!(matches!(OwnShort::parse_from(&a), Err(Error::Version)));
}

#[test]
fn a_declared_long_takes_the_word_and_its_value() {
    // The long half of the same rule. Nothing supplies a second `--version`, so the value binds.
    let a = argv(&["--version", "20"]);
    let parsed = OwnLong::parse_from(&a).expect("its own flag, which takes a value");
    assert_eq!(parsed.wanted.as_deref(), Some("20"));

    // And `-V`, unclaimed, still answers.
    let a = argv(&["-V"]);
    assert!(matches!(OwnLong::parse_from(&a), Err(Error::Version)));
}

#[test]
fn a_declared_long_wins_too() {
    // The other half of the same rule, on `go`'s parent: a CLI that wants `--version` for its
    // own purposes takes it, and nothing supplies a second one.
    let a = argv(&["go", "-V"]);
    let parsed = Versioned::parse_from(&a).expect("the subcommand's own flag");
    let Some(Command::Go(go)) = parsed.command else {
        panic!("expected go")
    };
    assert!(
        go.verbose,
        "a subcommand's `-V` is its own; none is supplied there"
    );
}

#[test]
fn the_flag_is_not_in_the_help_page_or_the_spec() {
    // Supplied rather than declared, exactly as `--help` is. A page listing a flag the spec
    // does not declare is a page that disagrees with the spec it was rendered from.
    let page = usage_argv::help::render(Versioned::spec(), Versioned::spec().root.cmd, true)
        .expect("a page");
    assert!(!page.contains("--version"), "{page}");

    // Asserted on the metadata rather than by searching the page for `-V`: `go` declares one of
    // its own, and it is listed — correctly — in the commands summary. What must be absent is a
    // *root* flag nobody declared.
    assert!(
        !Versioned::spec()
            .root
            .flags
            .iter()
            .any(|f| f.flag.shorts.contains(&b'V') || f.flag.longs.contains(&"version")),
        "the root lists a flag it does not declare"
    );

    let kdl = Versioned::to_kdl();
    assert!(!kdl.contains("--version"), "{kdl}");
    // But the version itself is declared, which is what the flag answers with.
    assert!(kdl.contains(r#"version "1.2.3""#), "{kdl}");
}

#[test]
fn the_fields_are_bound() {
    let a = argv(&["--quiet"]);
    assert!(Unversioned::parse_from(&a).expect("should parse").quiet);
    // Destructured rather than matched with `_`, which leaves the payload unread and is a
    // `dead_code` warning — an error in this workspace.
    let a = argv(&["other"]);
    let Some(Command::Other(other)) = Versioned::parse_from(&a).expect("should parse").command
    else {
        panic!("expected other")
    };
    let Other {} = *other;
}

#[test]
fn a_failure_renders_the_way_a_user_should_read_it() {
    // What `parse()` prints. It exits the process, so this checks the function it calls —
    // which is the point of that function existing: whether the good rendering is available is
    // a feature of usage-argv in the *adopter's* graph, and a `#[cfg]` in generated code is
    // evaluated in the adopter's crate, where the feature is not theirs to see.
    let a = argv(&["--nope"]);
    let Err(err) = Unversioned::parse_from(&a) else {
        panic!("no such flag")
    };
    // Stripped of colour before reading, because `render_failure` styles for the terminal it
    // finds itself in — under `CLICOLOR_FORCE=1`, or a TTY, the same words arrive wrapped in
    // escapes and a plain `starts_with` fails on a message that is perfectly correct. What is
    // asserted here is the wording; the colouring has its own tests.
    let message = strip_ansi(&usage_argv::render_failure(Unversioned::spec(), &a, &err));
    assert!(
        message.starts_with("error: unexpected argument '--nope' found"),
        "{message}"
    );
    assert!(
        message.contains("For more information, try '--help'."),
        "{message}"
    );
}
