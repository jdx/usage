//! Checks the corpus against usage-lib.
//!
//! Two distinct things are enforced here. First, the corpus is well formed: it
//! loads, ids are unique, every spec parses. Second, every vector's `reference`
//! label is accurate — a vector claiming the reference agrees must actually
//! agree, and one claiming a divergence must actually diverge.
//!
//! That second check is the point. It means a change in usage-lib's parsing
//! shows up here as a failing label rather than as silent drift, and it keeps the
//! recorded divergences from becoming stale folklore.

use usage_conformance::reference::{duplicate_ids, run, Observed};
use usage_conformance::{load, Reference, Vector};

fn corpus() -> Vec<Vector> {
    let files = load(usage_conformance::corpus_dir()).expect("corpus should load");
    assert!(!files.is_empty(), "corpus directory should not be empty");
    files.into_iter().flat_map(|f| f.vectors).collect()
}

#[test]
fn ids_are_unique() {
    let vectors = corpus();
    let dupes = duplicate_ids(vectors.iter());
    assert!(dupes.is_empty(), "duplicate vector ids: {dupes:?}");
}

#[test]
fn specs_are_valid() {
    for vector in corpus() {
        if let Observed::BadSpec(e) = run(&vector) {
            panic!("vector {} has an invalid spec: {e}", vector.id);
        }
    }
}

#[test]
fn every_vector_has_a_doc_comment() {
    // The corpus doubles as documentation of the grammar, so a vector without an
    // explanation is only half of one.
    for vector in corpus() {
        assert!(
            !vector.doc.trim().is_empty(),
            "vector {} needs a doc string",
            vector.id
        );
    }
}

#[test]
fn reference_labels_are_accurate() {
    let mut wrong = Vec::new();

    for vector in corpus() {
        let observed = run(&vector);
        let agrees = observed.matches(&vector.expect);

        match (&vector.reference, agrees) {
            (Reference::Agrees, true) => {}
            (Reference::Diverges(_), false) => {}
            (Reference::Agrees, false) => wrong.push(format!(
                "{}: labeled `agrees`, but usage-lib disagrees\n     expected: {:?}\n     observed: {observed:?}",
                vector.id, vector.expect
            )),
            (Reference::Diverges(note), true) => wrong.push(format!(
                "{}: labeled `diverges` ({note}), but usage-lib now agrees — delete the label",
                vector.id
            )),
        }
    }

    assert!(
        wrong.is_empty(),
        "{} vector(s) mislabeled:\n  - {}",
        wrong.len(),
        wrong.join("\n  - ")
    );
}
