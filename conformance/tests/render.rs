//! The rendering corpus, run against both Rust implementations.
//!
//! usage-lib renders from a spec through tera templates; usage-argv renders from `static`
//! tables through hand-written code. They must produce the same text, and the mise fixture in
//! `benches/gate/tests/help.rs` proves that at scale for the shapes mise happens to contain.
//! These vectors cover the shapes it does not.

use usage_conformance::render::{self, Outcome, Reference, VectorFile};

fn corpus() -> Vec<VectorFile> {
    render::load(render::corpus_dir()).expect("the rendering corpus should load")
}

fn vectors(files: &[VectorFile]) -> impl Iterator<Item = &render::Vector> {
    files.iter().flat_map(|f| &f.vectors)
}

#[test]
fn every_id_is_unique() {
    // Reports quote ids, and two vectors sharing one makes a report ambiguous about which
    // case failed.
    let files = corpus();
    let mut seen: Vec<&str> = vectors(&files).map(|v| v.id.as_str()).collect();
    let before = seen.len();
    seen.sort_unstable();
    seen.dedup();
    assert_eq!(before, seen.len(), "two vectors share an id");
    assert!(before > 0, "the corpus should not be empty");
}

#[test]
fn usage_argv_renders_what_the_corpus_expects() {
    let files = corpus();
    let mut differences = Vec::new();
    for vector in vectors(&files) {
        if let Some(diff) = render::argv(vector).difference(&vector.expect) {
            differences.push(format!("{}: {}\n  {diff}", vector.id, vector.doc));
        }
    }
    assert!(
        differences.is_empty(),
        "{} vector(s) render differently in usage-argv:\n\n{}",
        differences.len(),
        differences.join("\n\n")
    );
}

/// How many vectors usage-argv is not asked to answer.
///
/// Asserted rather than counted, for the reason the argv corpus asserts its own: an exemption
/// is a claim that a question does not reach an implementation, and a set that can grow without
/// anybody noticing is a set that will. Every one of these is a word usage-lib reads and the
/// derive has no spelling for, so raising this number means the asymmetry got wider.
const OUT_OF_SCOPE_FOR_ARGV: usize = 1;

#[test]
fn only_the_declared_vectors_are_out_of_usage_argvs_scope() {
    let files = corpus();
    let exempt: Vec<String> = vectors(&files)
        .filter_map(|v| match render::argv(v) {
            Outcome::OutOfScope(why) => Some(format!("{}: {why}", v.id)),
            _ => None,
        })
        .collect();
    assert_eq!(
        exempt.len(),
        OUT_OF_SCOPE_FOR_ARGV,
        "the out-of-scope set changed:\n  {}",
        exempt.join("\n  ")
    );
}

#[test]
fn the_reference_label_is_true_in_both_directions() {
    // The same check `reference.rs` makes of the argv corpus, and for the same reason: a
    // vector claiming agreement must agree, and a vector claiming divergence must still
    // diverge. So a divergence that gets fixed fails here with an instruction to delete the
    // label, and the list cannot quietly rot into folklore.
    let files = corpus();
    let mut wrong = Vec::new();
    for vector in vectors(&files) {
        let diff = render::reference(vector).difference(&vector.expect);
        match (&vector.reference, diff) {
            (Reference::Agrees, Some(diff)) => wrong.push(format!(
                "{}: labelled as agreeing, but usage-lib differs on {diff}",
                vector.id
            )),
            (Reference::Diverges(note), None) => wrong.push(format!(
                "{}: labelled as diverging ({note}), but usage-lib now agrees — delete the label",
                vector.id
            )),
            _ => {}
        }
    }
    assert!(
        wrong.is_empty(),
        "{} reference label(s) are wrong:\n\n{}",
        wrong.len(),
        wrong.join("\n\n")
    );
}

#[test]
fn a_vector_that_pins_a_page_pins_a_whole_one() {
    // A page given as lines is easy to truncate by accident, and a short expectation would
    // pass against a page that carries on. Every pinned page must end where the renderer's
    // does, which the comparison already enforces — this asserts the corpus bothers to pin
    // some, since a corpus of usage lines alone would not have caught a Flags-section bug.
    let files = corpus();
    let pinned = vectors(&files)
        .filter(|v| v.expect.short_help.is_some() || v.expect.long_help.is_some())
        .count();
    assert!(
        pinned >= 3,
        "only {pinned} vector(s) pin a whole page; the sections need covering too"
    );
}

#[test]
fn the_two_implementations_agree_with_each_other() {
    // Implied by the two tests above for any vector labelled as agreeing, and stated anyway:
    // this is the claim the corpus exists to make, and it should fail by name rather than as
    // a consequence.
    let files = corpus();
    let mut differences = Vec::new();
    for vector in vectors(&files) {
        if matches!(vector.reference, Reference::Diverges(_)) {
            continue;
        }
        let (Outcome::Rendered(ours), Outcome::Rendered(theirs)) =
            (render::argv(vector), render::reference(vector))
        else {
            // A spec that will not load is the other tests' complaint to make, and a vector
            // out of usage-argv's scope has nothing for the two to agree or disagree about.
            continue;
        };
        if ours != theirs {
            differences.push(vector.id.clone());
        }
    }
    assert!(
        differences.is_empty(),
        "usage-argv and usage-lib disagree on: {}",
        differences.join(", ")
    );
}
