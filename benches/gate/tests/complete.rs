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
fn a_prefix_matching_no_declared_choice_is_answered_the_same_way() {
    // `mise activate zsx⌶` — the argument declares its whole set, so nothing matching means no
    // matches, not "here is the working directory". Both sides must agree, and both must be
    // empty: this is the case where a filtered-list test would have passed while the rule was
    // wrong, because the rule is about what the position *declares*.
    let spec = mise_spec();
    for line in ["mise activate zs", "mise activate zsx"] {
        let s = split(line, line.len(), Shell::Bash);
        let ours = usage_argv::complete::complete(shadow_mise::Cli::spec(), &s);
        let theirs = usage_cli::complete_candidates(&spec, &s.words, s.cword, "bash")
            .expect("the reference should answer");
        let theirs: Vec<String> = theirs.into_iter().map(|(v, _)| v).collect();
        let ours_values: Vec<String> = ours.candidates.iter().map(|c| c.value.clone()).collect();
        assert_eq!(ours_values, theirs, "{line:?}");
        assert_eq!(
            ours.files, None,
            "{line:?} declares its set, so no paths belong"
        );
    }
}

#[test]
fn the_short_flags_offered_are_the_reference_s() {
    // A lone dash offers both forms; a letter narrows to the flag that has it.
    assert_same("mise -");
    assert_same("mise use -");
    assert_same("mise use -g");
    assert_same("mise install -f");
}

/// Where the reference reads the directory, this says the shell should.
///
/// The one deliberate divergence in the completion path, so it is checked as an equivalence
/// rather than waived: for a line whose answer is paths, the reference's candidates must all be
/// entries that exist in the working directory, and ours must be the marker saying so. A
/// difference either way — a listing where we claim to know the answer, or a marker where the
/// reference had real candidates — fails.
#[test]
fn where_the_reference_lists_files_this_asks_the_shell_for_them() {
    use usage_argv::complete::{complete, Files};

    let spec = mise_spec();
    let cwd = std::env::current_dir().expect("a working directory");
    let entries: Vec<String> = std::fs::read_dir(&cwd)
        .expect("readable")
        .filter_map(|e| e.ok())
        .map(|e| e.file_name().to_string_lossy().to_string())
        .collect();
    assert!(
        !entries.is_empty(),
        "the fixture needs a non-empty directory"
    );

    // `mise trust ⌶` takes a `[CONFIG_FILE]`, which the reference completes as a path.
    for line in ["mise trust ", "mise config get --file "] {
        let s = split(line, line.len(), Shell::Bash);
        let ours = complete(shadow_mise::Cli::spec(), &s);
        let theirs = usage_cli::complete_candidates(&spec, &s.words, s.cword, "bash")
            .expect("the reference should answer");

        assert_eq!(
            ours.files,
            Some(Files::Any),
            "{line:?} should hand paths to the shell, got {ours:?}"
        );
        // Every name the reference offered is something in this directory — i.e. it answered
        // with a listing, which is the thing we are replacing rather than contradicting.
        for (value, _) in &theirs {
            let bare = value.trim_end_matches('/');
            assert!(
                entries.iter().any(|e| e == bare),
                "{line:?}: the reference offered {value:?}, which is not a directory entry — so \
                 it was not doing file completion and the marker is wrong here"
            );
        }
    }
}
