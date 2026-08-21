//! Every KDL example on the config and flagset reference pages parses.
//!
//! That page used to document a vocabulary which had never existed — `file`, `findup`,
//! `default "k" "v"`, `alias`, `config_file` — while the parser accepted only `prop`, which
//! the page never mentioned. Nobody noticed because nothing checked. This checks.
//!
//! Only that page, for now. The other reference pages are *catalogues*: a block lists half
//! a dozen alternative spellings of an `arg` or a `flag`, which is not a spec and does not
//! parse as one (one of them puts an argument after a variadic, which the parser refuses
//! for good reason). Covering those means checking a catalogue line by line, which is worth
//! doing and is not this change. Running this test against them today reports five
//! failures, one of which — `flag "flag1"` in `cmd.md`, missing its dashes — is a real
//! documentation bug of exactly the kind this test exists to catch.

use std::path::Path;

/// A fenced block, and what has to be true of it.
enum Example {
    /// A whole spec: parse it as-is.
    Spec(String),
    /// The inside of a `config` block: wrap it before parsing.
    ConfigBody(String),
    /// Top-level nodes shown without the `name`/`bin` a real spec carries.
    Fragment(String),
    /// Reads another file, so it cannot be parsed from a string. Named so the reason is
    /// recorded rather than the block quietly skipped.
    NeedsAFileOnDisk,
}

fn classify(block: &str) -> Example {
    let first = block
        .lines()
        .map(str::trim)
        .find(|l| !l.is_empty() && !l.starts_with("//"))
        .unwrap_or_default();
    if block.contains("include file=") {
        Example::NeedsAFileOnDisk
    } else if first.starts_with("prop ")
        || first.starts_with("source ")
        || first.starts_with("file ")
    {
        Example::ConfigBody(block.to_string())
    } else if first.starts_with("name ") || first.starts_with("bin ") {
        Example::Spec(block.to_string())
    } else {
        // A fragment: nodes that belong at the top level of a spec, shown without the
        // `name`/`bin` a real file would have.
        Example::Fragment(block.to_string())
    }
}

/// The ```kdl blocks of a markdown file.
fn kdl_blocks(markdown: &str) -> Vec<String> {
    let mut blocks = Vec::new();
    let mut current: Option<String> = None;
    for line in markdown.lines() {
        match (&mut current, line.trim()) {
            (None, "```kdl") => current = Some(String::new()),
            (Some(block), "```") => blocks.push(std::mem::take(block)),
            (Some(block), _) => {
                block.push_str(line);
                block.push('\n');
            }
            (None, _) => {}
        }
        if matches!((&current, line.trim()), (Some(b), "```") if b.is_empty()) {
            current = None;
        }
    }
    blocks
}

#[test]
fn every_kdl_example_on_the_checked_pages_parses() {
    let pages = [
        Path::new(env!("CARGO_MANIFEST_DIR")).join("../docs/spec/reference/config.md"),
        // Every block on the flagset page is a whole spec or a top-level fragment, so the
        // page can be held to parsing from the day it was written.
        Path::new(env!("CARGO_MANIFEST_DIR")).join("../docs/spec/reference/flagset.md"),
    ];
    let mut checked = 0;
    let mut failures = Vec::new();

    for page in pages {
        let name = page
            .file_name()
            .unwrap_or_default()
            .to_string_lossy()
            .to_string();
        for (i, block) in kdl_blocks(&std::fs::read_to_string(&page).expect("readable"))
            .iter()
            .enumerate()
        {
            let source = match classify(block) {
                Example::NeedsAFileOnDisk => continue,
                Example::Spec(spec) => spec,
                Example::ConfigBody(body) => {
                    format!("name \"ex\"\nbin \"ex\"\nconfig {{\n{body}}}\n")
                }
                Example::Fragment(body) => format!("name \"ex\"\nbin \"ex\"\n{body}"),
            };
            checked += 1;
            if let Err(err) = source.parse::<usage::Spec>() {
                failures.push(format!("{name} block {i}: {err}\n{source}"));
            }
        }
    }

    assert!(
        failures.is_empty(),
        "{} example(s) in the reference do not parse:\n\n{}",
        failures.len(),
        failures.join("\n---\n")
    );
    // A lower bound rather than a count: pages gain examples, and a test that fails when
    // somebody documents something is worse than useless. Zero would mean the extractor
    // broke.
    assert!(checked >= 5, "only found {checked} examples to check");
}
