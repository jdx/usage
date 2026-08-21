//! What an adopter's own tests look like, written against the harness they get from the facade.
//!
//! Each test here is the shape a CLI's test suite is expected to take, so the file doubles as
//! the worked example the documentation points at.
#![cfg(all(feature = "test", feature = "spec"))]

use usage_rs::test::{self as harness, Outcome, Page};
use usage_rs::{Args, Cli, Subcommands};

/// A tool that does things.
///
/// Longer prose, so the short and long pages differ.
#[derive(Cli, Debug, PartialEq, Eq)]
#[usage(bin = "ex", version = "1.2.3")]
struct Ex {
    /// How many jobs to run at once
    #[usage(short = 'j', long)]
    jobs: Option<u8>,
    #[usage(subcommand)]
    command: Command,
}

#[derive(Subcommands, Debug, PartialEq, Eq)]
enum Command {
    Build(Build),
    Secret(Secret),
}

/// Build the thing
#[derive(Args, Debug, PartialEq, Eq)]
#[usage(visible_alias = "b")]
struct Build {
    /// Where to write it
    #[usage(long)]
    out: Option<String>,
    /// What to build
    target: String,
}

/// Not for you
#[derive(Args, Debug, PartialEq, Eq)]
#[usage(hide)]
struct Secret;

/// A command line with nothing on it, which is its own case.
#[derive(Cli, Debug)]
#[usage(bin = "bare", arg_required_else_help)]
struct Bare {
    /// A file to read
    file: Option<String>,
}

#[test]
fn a_command_line_parses_to_the_struct_it_declares() {
    let words = harness::argv(["-j", "4", "build", "--out", "dist", "release"]);
    let parsed = harness::parse(Ex::spec(), &words.words(), Ex::parse_from)
        .expect("the command line should parse");

    assert_eq!(
        parsed,
        Ex {
            jobs: Some(4),
            command: Command::Build(Build {
                out: Some("dist".to_string()),
                target: "release".to_string(),
            }),
        }
    );
}

#[test]
fn a_failure_comes_back_as_the_text_a_user_reads() {
    let words = harness::argv(["--jobs", "many", "build", "release"]);
    let message = harness::parse(Ex::spec(), &words.words(), Ex::parse_from)
        .expect_err("`many` is not a number of jobs");

    // The rendered diagnostic, not a debug-printed error code: what the user is shown is what
    // a test about the user's experience has to be able to assert on.
    assert!(message.contains("invalid value 'many'"), "{message}");
    assert!(message.contains("invalid digit"), "{message}");
}

#[test]
fn a_failure_is_plain_text_wherever_the_test_runs() {
    // The process asks the environment whether to colour, and gets a different answer under
    // `cargo test` in a terminal than in CI. A string a test asserts on cannot turn on that:
    // this is the regression guard for the harness rendering plain rather than `auto`.
    //
    // Set for the whole process rather than for one call, because that is the only way the
    // renderer can be asked, and nothing else in this binary reads it: the harness never calls
    // `Style::auto`.
    unsafe { std::env::set_var("CLICOLOR_FORCE", "1") };
    let words = harness::argv(["--jobs", "many", "build", "release"]);
    let message = harness::parse(Ex::spec(), &words.words(), Ex::parse_from)
        .expect_err("`many` is not a number of jobs");
    unsafe { std::env::remove_var("CLICOLOR_FORCE") };

    assert!(
        !message.contains('\u{1b}'),
        "the message should carry no escape sequences: {message:?}"
    );
}

#[test]
fn an_unknown_flag_falls_through_where_a_cli_stays_lax() {
    // usage is lax by default, so this is a test about what *does* happen rather than about a
    // rejection: `--nope` is left alone, and the word after it is the one with nowhere to go.
    let words = harness::argv(["build", "--nope", "release"]);
    let message = harness::parse(Ex::spec(), &words.words(), Ex::parse_from)
        .expect_err("`release` has already filled the one argument `build` declares");

    assert!(message.contains("'release'"), "{message}");
}

#[test]
fn a_missing_required_argument_names_itself() {
    let words = harness::argv(["build"]);
    let message = harness::parse(Ex::spec(), &words.words(), Ex::parse_from)
        .expect_err("`build` requires a target");

    assert!(message.contains("<TARGET>"), "{message}");
}

#[test]
fn help_asked_for_goes_to_stdout_with_a_zero_status() {
    let words = harness::argv(["build", "--help"]);
    let outcome = harness::outcome(Ex::spec(), &words.words(), Ex::parse_from);

    let Outcome::Help(printed) = outcome else {
        panic!("`--help` is a help request: {outcome:?}");
    };
    assert!(!printed.stderr, "a question's answer is not an error");
    assert_eq!(printed.code, 0);
    assert!(printed.text.contains("Build the thing"), "{}", printed.text);
    assert!(printed.text.contains("--out"), "{}", printed.text);
}

#[test]
fn help_nobody_asked_for_goes_to_stderr_with_clap_s_status() {
    let words = harness::argv([] as [&str; 0]);
    let outcome = harness::outcome(Bare::spec(), &words.words(), Bare::parse_from);

    let Outcome::Help(printed) = outcome else {
        panic!("`arg_required_else_help` shows help for an empty command line: {outcome:?}");
    };
    assert!(printed.stderr, "unasked-for help is not stdout's business");
    assert_eq!(printed.code, 2);
    assert!(printed.text.contains("Usage: bare"), "{}", printed.text);
}

#[test]
fn a_version_request_is_its_own_outcome() {
    let words = harness::argv(["--version"]);
    let outcome = harness::outcome(Ex::spec(), &words.words(), Ex::parse_from);

    let Outcome::Version(printed) = outcome else {
        panic!("`--version` is a version request: {outcome:?}");
    };
    assert_eq!(printed.text, "ex 1.2.3\n");
    assert_eq!(printed.code, 0);
}

#[test]
fn a_page_by_path_is_the_page_a_user_would_have_been_shown() {
    // The invariant that makes both halves of the harness worth having: asking for a page by
    // the path a user types gives exactly what the command line asking for it produces.
    let words = harness::argv(["build", "--help"]);
    let printed = harness::outcome(Ex::spec(), &words.words(), Ex::parse_from)
        .printed()
        .expect("a help request prints")
        .text
        .clone();

    assert_eq!(harness::help(Ex::spec(), &["build"], Page::Long), printed);
}

#[test]
fn a_page_can_be_asked_for_by_alias() {
    assert_eq!(
        harness::help(Ex::spec(), &["b"], Page::Long),
        harness::help(Ex::spec(), &["build"], Page::Long),
    );
}

#[test]
fn the_short_and_long_pages_are_different_pages() {
    let short = harness::help(Ex::spec(), &[], Page::Short);
    let long = harness::help(Ex::spec(), &[], Page::Long);

    assert!(short.contains("A tool that does things"), "{short}");
    assert!(long.contains("Longer prose"), "{long}");
    assert!(!short.contains("Longer prose"), "{short}");
}

#[test]
#[should_panic(expected = "names no subcommand")]
fn a_path_that_names_nothing_says_so() {
    // A command that was renamed should fail the test that asks about it, rather than quietly
    // asserting about some other page.
    harness::help(Ex::spec(), &["biuld"], Page::Long);
}

#[test]
fn the_whole_tree_is_one_snapshot() {
    let tree = harness::help_tree(Ex::spec(), Page::Long);

    // One entry per command, in declaration order, with the path a user types as its header.
    let headers: Vec<&str> = tree
        .lines()
        .filter(|line| line.starts_with("=== "))
        .collect();
    assert_eq!(
        headers,
        [
            "=== ex ===",
            "=== ex build ===",
            "=== ex secret (hidden) ==="
        ]
    );

    // And the pages themselves, so a flag's help changing anywhere in the tree is a diff here.
    assert!(tree.contains("Where to write it"), "{tree}");
}

#[test]
fn a_recursive_page_covers_the_visible_tree() {
    let all = harness::help(Ex::spec(), &[], Page::All);

    assert!(all.contains("Build the thing"), "{all}");
    assert!(all.contains("Where to write it"), "{all}");
    // `secret` is hidden, and a recursive page is still help.
    assert!(!all.contains("Not for you"), "{all}");
}

#[cfg(feature = "completions")]
#[test]
fn a_half_typed_command_is_offered_the_commands_that_match() {
    assert_eq!(harness::candidates(Ex::spec(), "ex bui"), ["build"]);
}

#[cfg(feature = "completions")]
#[test]
fn a_flag_is_offered_with_the_help_a_shell_shows_beside_it() {
    let offered = harness::described(Ex::spec(), "ex build --o");

    assert_eq!(
        offered,
        [("--out".to_string(), Some("Where to write it".to_string()))]
    );
}

#[cfg(feature = "completions")]
#[test]
fn a_cursor_in_the_middle_of_a_line_completes_the_word_it_sits_in() {
    let line = "ex bui release";
    let candidates = harness::completion_at(
        Ex::spec(),
        line,
        "ex bui".len(),
        usage_rs::test::Shell::Bash,
    );

    let values: Vec<&str> = candidates
        .candidates
        .iter()
        .map(|candidate| candidate.value.as_str())
        .collect();
    assert_eq!(values, ["build"]);
}

#[cfg(feature = "completions")]
#[test]
fn a_value_position_says_whether_it_admits_paths() {
    // The other half of a completion answer: a shell asks what a word could be *and* whether to
    // add filenames to whatever it is told.
    let value = harness::completion(Ex::spec(), "ex build --out ");
    assert_eq!(value.files, Some(usage_rs::test::Files::Any));

    // A flag name is not a path, however many files sit in the directory.
    let flag = harness::completion(Ex::spec(), "ex build --");
    assert_eq!(flag.files, None);
}

#[cfg(feature = "completions")]
#[test]
fn a_hidden_command_is_not_offered() {
    let offered = harness::candidates(Ex::spec(), "ex ");

    assert!(offered.contains(&"build".to_string()), "{offered:?}");
    assert!(!offered.contains(&"secret".to_string()), "{offered:?}");
}
