//! The completion corpus, run against `usage-argv`.
//!
//! `corpus/complete/README.md` says what a vector means and why the corpus exists. This is the
//! half that makes it a gate rather than a document.

use std::collections::BTreeSet;

use usage_conformance::complete::{load, run, Reference};

#[test]
fn every_vector_is_offered_what_it_expects() {
    let mut failures = Vec::new();
    let vectors = load();
    assert!(
        !vectors.is_empty(),
        "the corpus loaded nothing, so this would pass by measuring nothing"
    );

    for (file, vector) in &vectors {
        match run(vector) {
            Err(why) => failures.push(format!("  {} [{file}]: {why}", vector.id)),
            Ok(offered) => {
                if !offered.matches(&vector.expect) {
                    failures.push(format!(
                        "  {} [{file}]\n      wanted: {:?}{}\n      got:    {:?}{}",
                        vector.id,
                        vector.expect.candidates,
                        if vector.expect.files { " + files" } else { "" },
                        offered.candidates,
                        if offered.files { " + files" } else { "" },
                    ));
                }
            }
        }
    }

    assert!(
        failures.is_empty(),
        "{} of {} vector(s) failed:\n{}",
        failures.len(),
        vectors.len(),
        failures.join("\n")
    );
}

#[test]
fn every_id_is_unique() {
    // Failures quote the id, and two vectors sharing one makes a report ambiguous about which
    // case moved. The render corpus checks the same thing for the same reason.
    let mut seen = BTreeSet::new();
    let mut duplicates = Vec::new();
    for (_, vector) in load() {
        if !seen.insert(vector.id.clone()) {
            duplicates.push(vector.id);
        }
    }
    assert!(duplicates.is_empty(), "duplicate ids: {duplicates:?}");
}

#[test]
fn every_vector_says_something() {
    // A vector with an empty `doc` is a case nobody can review: the expectation may be right and
    // there is no way to tell. And one expecting nothing at all — no candidates, no files — is
    // almost always a vector whose spec did not say what its author thought.
    for (file, vector) in load() {
        assert!(
            !vector.doc.trim().is_empty(),
            "{} [{file}] has no doc",
            vector.id
        );
        assert!(
            !vector.expect.candidates.is_empty() || vector.expect.files,
            "{} [{file}] expects nothing at all — if that is really the claim, say so in `doc` \
             and relax this check",
            vector.id
        );
    }
}

#[test]
fn a_divergence_from_the_reference_is_labelled() {
    // The corpus's rule, kept the same way the argv corpus keeps it: a vector the reference
    // disagrees with must say so, so a divergence is a recorded decision rather than a mystery.
    //
    // Only the label is checked here, not the reference's answer: `usage-cli` completes through
    // a `Command` with its own shell splitting, and re-deriving that inside a test would be a
    // second implementation of the thing under test. `benches/gate` compares the two over mise's
    // real spec, which is where the reference is actually held to account.
    for (file, vector) in load() {
        if let Reference::Diverges(note) = &vector.reference {
            assert!(
                !note.trim().is_empty(),
                "{} [{file}] is labelled as diverging with no note saying how",
                vector.id
            );
        }
    }
}
