//! Checks usage-argv against the corpus.
//!
//! This is the point of writing the grammar down. usage-argv is expected to match
//! every binding vector — including the fifteen where usage-lib does not, since
//! those record the grammar's intent rather than the reference's behavior.
//!
//! Vectors that turn on something decided after the last token (`required`,
//! `choices`, `env`, defaults, `var_min`/`var_max`, `overrides`) are out of scope
//! for this crate by design. Their count is asserted below so the exempt set
//! cannot quietly grow.

use usage_conformance::argv::{run, Outcome};
use usage_conformance::{load, Expect, Reference, Vector};

/// Vectors that exercise binding, which is what usage-argv implements.
const IN_SCOPE: usize = 64;

fn corpus() -> Vec<Vector> {
    let files = load(usage_conformance::corpus_dir()).expect("corpus should load");
    files.into_iter().flat_map(|f| f.vectors).collect()
}

#[test]
fn every_binding_vector_passes() {
    let mut failures = Vec::new();
    let mut in_scope = 0;

    for vector in corpus() {
        let outcome = run(&vector);
        if let Outcome::OutOfScope(_) = outcome {
            continue;
        }
        in_scope += 1;
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

    assert_eq!(
        in_scope, IN_SCOPE,
        "the number of vectors usage-argv answers changed; if that was the point \
         of your change, update IN_SCOPE"
    );
}

#[test]
fn agrees_where_usage_lib_does_not() {
    // The divergences are the reason the corpus exists, so make it explicit that
    // the new parser resolves them rather than inheriting them.
    let mut resolved = 0;

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
        resolved += 1;
    }

    // No floor on `resolved`: every divergence fixed in usage-lib removes a label
    // and shrinks this set, so an empty one would mean the reference has caught up
    // entirely — a success, not a broken test. The per-vector assertion above is
    // what carries the weight.
    let _ = resolved;
}

#[test]
fn out_of_scope_vectors_say_why() {
    // A vector skipped without a reason would be a silent hole in coverage.
    for vector in corpus() {
        if let Outcome::OutOfScope(reason) = run(&vector) {
            assert!(
                !reason.is_empty(),
                "{} was skipped without a reason",
                vector.id
            );
        }
    }
}

#[test]
fn no_vector_has_an_unloadable_spec() {
    for vector in corpus() {
        if let Outcome::BadSpec(e) = run(&vector) {
            panic!("vector {} has an invalid spec: {e}", vector.id);
        }
    }
}

#[test]
fn error_expectations_are_reachable() {
    // Guards against the codes usage-argv claims to produce drifting away from
    // the ones the corpus asks for.
    let produced: Vec<_> = corpus()
        .iter()
        .filter_map(|v| match (run(v), &v.expect) {
            (Outcome::Failed(code), Expect::Error(_)) => Some(code),
            _ => None,
        })
        .collect();
    assert!(
        produced.len() >= 6,
        "expected the binding subset to exercise several error classes, saw {}",
        produced.len()
    );
}
