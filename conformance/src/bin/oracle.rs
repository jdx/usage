//! Report what usage-lib does with each corpus vector.
//!
//! Authoring aid, not a test. `cargo run -p usage-conformance --bin oracle` prints
//! every vector's observed result next to its expectation, which is how the
//! `reference` field on each vector gets filled in with a measurement rather than
//! a guess. `--json` emits the same thing machine-readably.
//!
//! The test suite (`conformance/tests/reference.rs`) is what actually enforces
//! agreement in CI.

use usage_conformance::reference::run;
use usage_conformance::{load, Reference};

fn main() -> Result<(), String> {
    let json = std::env::args().any(|a| a == "--json");
    let filter = std::env::args()
        .skip(1)
        .find(|a| !a.starts_with("--"))
        .unwrap_or_default();

    let files = load(usage_conformance::corpus_dir())?;
    let mut rows = Vec::new();

    for file in &files {
        for vector in &file.vectors {
            if !filter.is_empty() && !vector.id.contains(&filter) {
                continue;
            }
            let observed = run(vector);
            let agrees = observed.matches(&vector.expect);
            let declared_agrees = matches!(vector.reference, Reference::Agrees);
            rows.push((
                file.section.clone(),
                vector.id.clone(),
                agrees,
                declared_agrees,
                format!("{observed:?}"),
                format!("{:?}", vector.expect),
            ));
        }
    }

    if json {
        let out: Vec<_> = rows
            .iter()
            .map(|(section, id, agrees, declared, observed, expect)| {
                serde_json::json!({
                    "section": section,
                    "id": id,
                    "reference_agrees": agrees,
                    "declared_agrees": declared,
                    "observed": observed,
                    "expected": expect,
                })
            })
            .collect();
        println!(
            "{}",
            serde_json::to_string_pretty(&out).map_err(|e| e.to_string())?
        );
        return Ok(());
    }

    let mut mismatched = 0;
    for (section, id, agrees, declared, observed, expect) in &rows {
        let mark = match (agrees, declared) {
            (true, true) => "ok  ",
            (false, false) => "div ",
            _ => {
                mismatched += 1;
                "MISM"
            }
        };
        println!("{mark} {section}/{id}");
        if !agrees {
            println!("       expected: {expect}");
            println!("       observed: {observed}");
        }
    }

    println!(
        "\n{} vectors, {} declared divergences, {mismatched} mislabeled",
        rows.len(),
        rows.iter().filter(|r| !r.3).count()
    );
    Ok(())
}
