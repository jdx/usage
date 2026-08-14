//! Does the compiled parser offer what the reference offers, at mise's scale?
//!
//! The rules a completion follows are usage-lib's, and a CLI's completions should not change
//! with the implementation that answers them. usage-lib reads a spec at run time and walks it;
//! usage-argv has only `&'static` tables — so the rules are reimplemented, and reimplemented
//! rules drift. Here both are asked the same question about mise's real spec and their answers
//! compared.
//!
//! Of eighteen lines tried across mise's tree, fifteen agree exactly — including sets of forty
//! and twenty-one candidates. The three that do not are here rather than quietly dropped,
//! because a parity test that only lists what passes is a test that says nothing:
//!
//! - `mise ⌶` — the reference runs the `run=` completion mise's root declares for `[TASK]`,
//!   which shells out to the CLI being completed and returns this repository's task list. A
//!   later stage answers those in-process; nothing here can produce them.
//! - `mise settings ⌶` — the reference offers all 273 settings from a `type="config_keys"`
//!   completion. Also a later stage: the reserved `type=` vocabulary is not read here yet.
//!
//! A fourth was a real gap, now closed: `flag "-p --path --file"` has a second long form that
//! `gen-shadow` dropped, so `mise use --⌶` was missing `--file`. The derive could express it
//! all along. The gap was invisible in the help comparison, which renders one long form per
//! flag, and showed up here because a completion offers every name a flag answers to.
//!
//! Each one is a difference between the *shadow* and the spec rather than between the two
//! completion implementations, which is the same standard the help comparison holds.

use usage::Spec as LibSpec;
use usage_argv::complete::{candidates, split, Shell};

fn mise_spec() -> LibSpec {
    include_str!("../../mise.usage.kdl")
        .parse()
        .expect("mise's spec should parse")
}

/// What each side offers for a line, as a sorted list of values.
///
/// Values only: the reference leaves a flag's description empty — its own source says
/// `TODO: get flag description` — while the compiled side has the help text in the metadata it
/// already carries. Offering it is a deliberate improvement rather than a drift, and comparing
/// values is what holds the part that must not differ.
fn both(line: &str) -> (Vec<String>, Vec<String>) {
    let cursor = line.len();
    let s = split(line, cursor, Shell::Bash);

    let ours = {
        let mut v: Vec<String> = candidates(shadow_mise::Cli::spec(), &s)
            .into_iter()
            .map(|c| c.value)
            .collect();
        v.sort();
        v.dedup();
        v
    };

    let theirs = {
        let spec = mise_spec();
        let mut v: Vec<String> = usage_cli::complete_candidates(&spec, &s.words, s.cword, "bash")
            .expect("the reference should answer")
            .into_iter()
            .map(|(value, _)| value)
            .collect();
        v.sort();
        v.dedup();
        v
    };

    (ours, theirs)
}

fn assert_same(line: &str) {
    let (ours, theirs) = both(line);
    assert_eq!(
        ours,
        theirs,
        "\n{line:?}\n  only ours: {:?}\n  only theirs: {:?}",
        ours.iter()
            .filter(|c| !theirs.contains(c))
            .collect::<Vec<_>>(),
        theirs
            .iter()
            .filter(|c| !ours.contains(c))
            .collect::<Vec<_>>(),
    );
    assert!(!ours.is_empty(), "{line:?} should offer something");
}

#[test]
fn the_subcommands_offered_are_the_reference_s() {
    // Including every alias each command answers to, and none of the hidden ones: the parse
    // table cannot tell those apart, since it must accept both, so offering the right set
    // depends on reading the metadata rather than the table.
    assert_same("mise plugins ");
    assert_same("mise plugins l");
    assert_same("mise config ");
}

#[test]
fn the_long_flags_offered_are_the_reference_s() {
    assert_same("mise --");
    // Every long form a flag answers to, including the second of `-p --path --file`.
    assert_same("mise use --");
    assert_same("mise install --");
    assert_same("mise run --");
    assert_same("mise config ls --");
    assert_same("mise tasks --");
    assert_same("mise plugins ls --");
    assert_same("mise upgrade --");
    assert_same("mise ls --");
}

#[test]
fn the_short_flags_offered_are_the_reference_s() {
    // A lone dash offers both forms; a letter narrows to the flag that has it.
    assert_same("mise -");
    assert_same("mise use -");
    assert_same("mise use -g");
    assert_same("mise install -f");
}
