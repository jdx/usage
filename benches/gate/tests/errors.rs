//! Does a failure read the way clap's reads, at mise's scale?
//!
//! An adopter's users read clap's errors today. The help either side of this is already
//! byte-identical to usage-lib's, so the error text is the last thing they would notice changing
//! — and the way to know is to fail the same command line on both sides and compare.
//!
//! Not byte-equality, deliberately. The usage line is *ours*, rendered from the spec exactly as
//! `--help` renders it, because an error that disagrees with the help about how a command is
//! spelled is worse than one that disagrees with clap. So what is compared is the first line —
//! the sentence a user actually reads — plus the shape around it.

use clap::Parser;
use std::ffi::{OsStr, OsString};
use usage_argv::diagnostic::{render, Style};

/// clap's message for a command line, without colour.
fn clap_error(words: &[&str]) -> Option<String> {
    let mut argv = vec!["mise"];
    argv.extend_from_slice(words);
    match shadow_mise_clap::Cli::try_parse_from(&argv) {
        Ok(_) => None,
        Err(e) => Some(e.render().to_string()),
    }
}

/// Ours for the same command line.
fn our_error(words: &[&str]) -> Option<String> {
    let owned: Vec<OsString> = words.iter().map(OsString::from).collect();
    let argv: Vec<&OsStr> = owned.iter().map(|o| o.as_os_str()).collect();
    match shadow_mise::Cli::parse_from(&argv) {
        Ok(_) => None,
        Err(error) => Some(render(
            shadow_mise::Cli::spec(),
            &argv,
            &error,
            Style::PLAIN,
        )),
    }
}

fn first_line(message: &str) -> &str {
    message.lines().next().unwrap_or_default()
}

#[test]
fn the_sentence_a_user_reads_is_the_one_clap_wrote() {
    // Each of these fails on both sides, and the failure is the same failure — so the line naming
    // it should be the same line.
    for words in [
        vec!["use", "--jobs"],
        vec!["config", "nonesuch"],
        vec!["plugins", "link"],
        vec!["activate", "zsx"],
        vec!["settings", "get"],
        vec!["alias", "get"],
    ] {
        let theirs = clap_error(&words).unwrap_or_else(|| panic!("clap parsed {words:?}"));
        let ours = our_error(&words).unwrap_or_else(|| panic!("we parsed {words:?}"));
        assert_eq!(
            first_line(&ours),
            first_line(&theirs),
            "\n{words:?}\n  ours:   {ours}\n  theirs: {theirs}"
        );
    }
}

#[test]
fn the_message_ends_the_way_clap_ends_one() {
    let words = ["config", "nonesuch"];
    let ours = our_error(&words).expect("a failure");
    let theirs = clap_error(&words).expect("a failure");
    let footer = "For more information, try '--help'.";
    assert!(ours.trim_end().ends_with(footer), "{ours}");
    assert!(theirs.trim_end().ends_with(footer), "{theirs}");
}

#[test]
fn a_usage_block_appears_where_clap_shows_one() {
    // The line differs — ours is the spec's — but whether there *is* one is a decision about how
    // much to say, and that should match.
    for words in [
        vec!["config", "nonesuch"],
        vec!["plugins", "link"],
        vec!["use", "--jobs"],
        vec!["activate", "zsx"],
    ] {
        let ours = our_error(&words).expect("a failure");
        let theirs = clap_error(&words).expect("a failure");
        assert_eq!(
            ours.contains("\nUsage: "),
            theirs.contains("\nUsage: "),
            "{words:?}\n  ours:   {ours}\n  theirs: {theirs}"
        );
    }
}

#[test]
fn the_usage_line_is_the_one_the_help_prints() {
    // Which is the deliberate difference from clap: an error and a help page describing the same
    // command differently is the confusing kind of inconsistency.
    let ours = our_error(&["config", "nonesuch"]).expect("a failure");
    let line = ours
        .lines()
        .find_map(|l| l.strip_prefix("Usage: "))
        .expect("a usage line");
    let root = shadow_mise::Cli::spec().root;
    let config = root
        .subcommands
        .iter()
        .find(|s| s.cmd.name == "config")
        .expect("mise config");
    assert_eq!(
        line,
        usage_argv::help::usage_line(&["mise", "config"], config)
    );
}

#[test]
fn the_suggestion_is_the_one_clap_would_make() {
    // Jaro-Winkler above 0.7, which is clap's rule — so the two suggest in the same cases and
    // suggest the same thing. Checked against mise's real flags rather than a fixture, because
    // what makes a suggestion good is the size of the set it was chosen from.
    let cases = [
        (vec!["activate", "zsx"], "a similar value exists: 'zsh'"),
        (
            vec!["config", "lss"],
            "some similar subcommands exist: 'list', 'ls'",
        ),
    ];
    for (words, tip) in cases {
        let ours = our_error(&words).unwrap_or_else(|| panic!("we parsed {words:?}"));
        assert!(ours.contains(tip), "{words:?}\n{ours}");

        // And clap says the same thing, where clap fails at all.
        if let Some(theirs) = clap_error(&words) {
            assert!(theirs.contains(tip), "clap on {words:?}:\n{theirs}");
        }
    }
}

#[test]
fn a_word_nothing_resembles_gets_no_tip() {
    // The threshold earns its keep on a set this size: mise has 711 flags, and something is
    // always vaguely similar to anything if the bar is low enough.
    let ours = our_error(&["config", "zzzzzzzz"]).expect("a failure");
    assert!(!ours.contains("tip:"), "{ours}");
    if let Some(theirs) = clap_error(&["config", "zzzzzzzz"]) {
        assert!(!theirs.contains("tip:"), "{theirs}");
    }
}

#[test]
fn mise_accepts_an_unknown_flag_where_clap_refuses_one() {
    // Not a rendering question, but the reason no flag typo appears above: mise's spec leaves
    // `unknown_flags` at its default, which means a dash-prefixed word nothing matches is a
    // *value*. clap refuses it. So `mise use --globa` is an error today and a tool named
    // `--globa` under this parser.
    //
    // Recorded rather than worked around, because it is a decision for the adopting CLI —
    // declaring `unknown_flags=error` on a command restores clap's behaviour and, with it, the
    // typo suggestions this module produces for a flag.
    let words = ["use", "--globa"];
    assert!(
        clap_error(&words).is_some(),
        "clap should refuse an unknown flag"
    );
    assert!(
        our_error(&words).is_none(),
        "mise's spec should accept it as a value — if this fails, the spec changed and the \
         suggestion cases above can cover a flag again"
    );

    // The flag itself is there — it is the refusal that is not, which is what makes this a
    // property of the spec rather than of the parser.
    let has_global = shadow_mise::Cli::spec()
        .root
        .subcommands
        .iter()
        .find(|s| s.cmd.name == "use")
        .expect("mise use")
        .flags
        .iter()
        .any(|f| f.flag.longs.contains(&"global"));
    assert!(has_global, "mise use declares --global");
}
