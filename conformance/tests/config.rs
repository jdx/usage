//! Checks usage-config against the config corpus.
//!
//! The corpus is the definition of what a resolution is; this is the reference implementation
//! answering it. Every vector applies — there is no layer of this that an implementation may skip,
//! because a resolution is one thing rather than a pipeline whose halves can be implemented apart.
//!
//! Corpus well-formedness — unique ids, no empty sections — is checked here too, since this is the
//! only test that loads the config corpus.

use std::collections::BTreeSet;

use usage_conformance::config::{matches, run, VectorFile};

fn corpus() -> Vec<VectorFile> {
    usage_conformance::config::load(usage_conformance::corpus_dir().join("config"))
        .expect("the config corpus should load")
}

#[test]
fn every_vector_resolves_the_way_it_says() {
    let mut failures = Vec::new();
    for file in corpus() {
        for vector in &file.vectors {
            match run(vector) {
                Ok(actual) if matches(&vector.expect, &actual) => {}
                Ok(actual) => failures.push(format!(
                    "{}: {}\n     expected: {:?}\n     got:      {actual:?}",
                    vector.id, vector.doc, vector.expect
                )),
                Err(err) => failures.push(format!("{}: {}\n     {err}", vector.id, vector.doc)),
            }
        }
    }
    assert!(
        failures.is_empty(),
        "{} of the corpus's vectors do not resolve as written:\n\n{}",
        failures.len(),
        failures.join("\n\n")
    );
}

#[test]
fn the_corpus_is_well_formed() {
    let files = corpus();
    assert!(!files.is_empty(), "the corpus should have files in it");

    let mut ids = BTreeSet::new();
    for file in &files {
        assert!(
            !file.about.is_empty(),
            "{} has nothing to say",
            file.section
        );
        assert!(
            !file.vectors.is_empty(),
            "{} has no vectors in it",
            file.section
        );
        for vector in &file.vectors {
            assert!(
                ids.insert(vector.id.clone()),
                "two vectors are called `{}`; reports quote these, so they have to be unique",
                vector.id
            );
            assert!(!vector.doc.is_empty(), "{} says nothing", vector.id);
            assert!(
                !vector.settings.is_empty(),
                "{} declares no settings, so there is nothing for it to resolve",
                vector.id
            );
        }
    }
}

#[test]
fn a_vector_the_harness_cannot_read_stops_rather_than_resolving() {
    // A number too large for the value model used to become zero, so the vector went on to pass or
    // fail for a reason that has nothing to do with what it says. A harness that quietly misreads
    // its own corpus is worse than one that cannot read it.
    let vector = usage_conformance::config::parse_vector(
        r#"vector "too-big" doc="An integer past what the value model holds." {
            setting "jobs" type="int" default=9223372036854775808
            expect {}
        }"#,
    )
    .expect("the vector itself is well-formed KDL");
    let err = run(&vector).expect_err("the default cannot be read");
    assert!(err.contains("9223372036854775808"), "{err}");

    // The same in a layer, which reaches it by the other road.
    let vector = usage_conformance::config::parse_vector(
        r#"vector "too-big-supplied" doc="The same number, supplied by a layer." {
            setting "jobs" type="int"
            layer "file" {
                shaped "jobs" 9223372036854775808
            }
            expect {}
        }"#,
    )
    .expect("the vector itself is well-formed KDL");
    let err = run(&vector).expect_err("the value cannot be read");
    assert!(err.contains("jobs"), "{err}");
}
