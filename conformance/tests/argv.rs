//! Checks usage-argv against the corpus.
//!
//! This is the point of writing the grammar down: usage-argv is expected to match
//! every vector it answers, including the ones where usage-lib does not, since those
//! record the grammar's intent rather than the reference's behavior.
//!
//! Vectors that turn on something decided after the last token — `required`,
//! `choices`, `env`, defaults, `var_min`/`var_max`, `overrides` — are out of scope
//! for this crate by design, and [`the_exempt_vectors`] is the record of which.
//!
//! Corpus well-formedness is checked once, in `reference.rs`, rather than again
//! here.

use usage_conformance::argv::{run, Outcome};
use usage_conformance::{load, Reference, Vector};

fn corpus() -> Vec<Vector> {
    let files = load(usage_conformance::corpus_dir()).expect("corpus should load");
    files.into_iter().flat_map(|f| f.vectors).collect()
}

#[test]
fn every_binding_vector_passes() {
    let mut failures = Vec::new();

    for vector in corpus() {
        let outcome = run(&vector);
        if let Outcome::OutOfScope(_) = outcome {
            continue;
        }
        if !outcome.matches(&vector.expect) {
            failures.push(format!(
                "{}: {}\n     expected: {:?}\n     got:      {outcome:?}",
                vector.id, vector.doc, vector.expect
            ));
        }
    }

    assert!(
        failures.is_empty(),
        "{} binding vector(s) failed:\n  - {}",
        failures.len(),
        failures.join("\n  - ")
    );
}

/// Which vectors usage-argv does not answer, and the reason each gives.
///
/// A snapshot rather than a count. A count asserts something nobody can check by
/// reading it, and it collides whenever two changes touch the corpus — two pull
/// requests each adding an ordinary vector both passed alone and broke `main`
/// together, because each saw only its own increment. A list fails as a reviewable
/// diff naming the vector that changed sides, which is the thing actually worth
/// knowing, and adding an ordinary vector does not touch it at all.
#[test]
fn the_exempt_vectors() {
    let mut exempt: Vec<String> = corpus()
        .iter()
        .filter_map(|vector| match run(vector) {
            Outcome::OutOfScope(reason) => Some(format!("{}: {reason}", vector.id)),
            _ => None,
        })
        .collect();
    exempt.sort();

    insta::assert_snapshot!(exempt.join("\n"));
}

#[test]
fn agrees_where_usage_lib_does_not() {
    // The divergences are the reason the corpus exists, so make it explicit that the
    // new parser resolves them rather than inheriting them.
    //
    // No lower bound on how many: every divergence fixed in usage-lib deletes a
    // label and shrinks this set, so an empty one would mean the reference has caught
    // up entirely — a success, not a broken test.
    for vector in corpus() {
        let Reference::Diverges(_) = vector.reference else {
            continue;
        };
        let outcome = run(&vector);
        if let Outcome::OutOfScope(_) = outcome {
            continue;
        }
        assert!(
            outcome.matches(&vector.expect),
            "{} is a known usage-lib divergence that usage-argv should fix, but it \
             produced {outcome:?} instead of {:?}",
            vector.id,
            vector.expect
        );
    }
}
