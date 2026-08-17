//! Report what each implementation renders for every vector in the rendering corpus.
//!
//! Authoring aid, not a test. `cargo run -p usage-conformance --bin render-oracle` prints what
//! usage-lib and usage-argv produce beside what the vector expects, which is how an
//! expectation gets filled in with a measurement rather than a guess — and how the `reference`
//! label gets set honestly when the two disagree.
//!
//! `--json` emits the same thing machine-readably, which is what makes it usable for filling in
//! a new vector: an array of `{"id", "usage-lib", "usage-argv"}`, where each of the two is a
//! vector's `expect` object exactly as written. So it is not the file's own shape — pick the
//! renderer you have decided is right and copy that object, which `corpus/render/README.md`
//! shows with `jq`. Copying a measurement beats transcribing a page by hand either way.
//!
//! The test suite (`conformance/tests/render.rs`) is what actually enforces agreement in CI.

use usage_conformance::render::{self, Outcome, Rendered};

fn main() -> Result<(), String> {
    let json = std::env::args().any(|a| a == "--json");
    let filter = std::env::args()
        .skip(1)
        .find(|a| !a.starts_with("--"))
        .unwrap_or_default();

    let files = render::load(render::corpus_dir())?;
    let mut rows = Vec::new();

    for file in &files {
        for vector in &file.vectors {
            if !filter.is_empty() && !vector.id.contains(&filter) {
                continue;
            }
            rows.push((vector, render::reference(vector), render::argv(vector)));
        }
    }

    if json {
        // One row per vector, each renderer's result in the shape a vector's `expect` takes, so
        // a new one can be filled in by copying the object of whichever renderer is right.
        let out: Vec<serde_json::Value> = rows
            .iter()
            .map(|(vector, lib, argv)| {
                serde_json::json!({
                    "id": vector.id,
                    "usage-lib": value(lib),
                    "usage-argv": value(argv),
                })
            })
            .collect();
        println!(
            "{}",
            serde_json::to_string_pretty(&out).map_err(|e| e.to_string())?
        );
        return Ok(());
    }

    for (vector, lib, argv) in &rows {
        let lib_diff = lib.difference(&vector.expect);
        let argv_diff = argv.difference(&vector.expect);
        let mark = match (&lib_diff, &argv_diff) {
            // A vector usage-argv is not asked to answer reports as agreeing, so it is called
            // what it is rather than being counted as a pass.
            _ if matches!(argv, Outcome::OutOfScope(_)) => "skip",
            (None, None) => "ok  ",
            (Some(_), Some(_)) => "BOTH",
            (Some(_), None) => "LIB ",
            (None, Some(_)) => "ARGV",
        };
        println!("{mark}  {}", vector.id);
        if let Some(diff) = lib_diff {
            println!("        usage-lib:  {}", indent(&diff));
        }
        if let Some(diff) = argv_diff {
            println!("        usage-argv: {}", indent(&diff));
        }
    }
    Ok(())
}

/// What an implementation produced, in the vector's own shape.
fn value(outcome: &Outcome) -> serde_json::Value {
    match outcome {
        Outcome::Bad(why) => serde_json::json!({ "error": why }),
        Outcome::OutOfScope(why) => serde_json::json!({ "out_of_scope": why }),
        Outcome::Rendered(Rendered {
            usage,
            short_help,
            long_help,
        }) => serde_json::json!({
            "usage": usage,
            "short_help": short_help,
            "long_help": long_help,
        }),
    }
}

fn indent(text: &str) -> String {
    text.replace('\n', "\n        ")
}
