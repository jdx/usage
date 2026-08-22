//! Every KDL example on the introductory, command, config, and flagset pages parses.
//!
//! That page used to document a vocabulary which had never existed — `file`, `findup`,
//! `default "k" "v"`, `alias`, `config_file` — while the parser accepted only `prop`, which
//! the page never mentioned. Nobody noticed because nothing checked. This checks.
//!
//! Only pages whose blocks can each stand alone, for now. Some reference pages are *catalogues*:
//! a block lists half
//! a dozen alternative spellings of an `arg` or a `flag`, which is not a spec and does not
//! parse as one (one of them puts an argument after a variadic, which the parser refuses
//! for good reason). Covering those means checking a catalogue line by line.
//!
//! A block that reads another file is parsed from disk rather than skipped, because the
//! `include` examples are the ones a reader is least able to check for themselves: the
//! whole point of one is that the meaning is spread over two blocks.

use std::path::{Path, PathBuf};

/// A fenced block, and what has to be true of it.
enum Example {
    /// A whole spec: parse it as-is.
    Spec(String),
    /// The inside of a `config` block: wrap it before parsing.
    ConfigBody(String),
    /// Top-level nodes shown without the `name`/`bin` a real spec carries.
    Fragment(String),
    /// A spec that only means something as a file: it reads another one, or a block that
    /// does names it. Written under `name` in a scratch directory and parsed from there,
    /// so a relative `include` resolves the way the page says it does.
    File { name: String, body: String },
}

/// The file a `// name.usage.kdl` header claims, if the block opens with one.
fn declared_name(block: &str) -> Option<String> {
    let first = block.lines().map(str::trim).find(|l| !l.is_empty())?;
    let name = first.strip_prefix("//")?.trim();
    let is_file = name.ends_with(".kdl") && !name.contains(char::is_whitespace);
    is_file.then(|| name.to_string())
}

/// Every file an `include` in this block reads, as written.
fn included_files(block: &str) -> Vec<String> {
    block
        .lines()
        .filter_map(|l| l.split_once("include file="))
        .filter_map(|(_, rest)| rest.trim_start().strip_prefix('"'))
        .filter_map(|rest| rest.split_once('"'))
        .map(|(file, _)| file.to_string())
        .collect()
}

fn classify(block: &str, index: usize) -> Example {
    let first = block
        .lines()
        .map(str::trim)
        .find(|l| !l.is_empty() && !l.starts_with("//"))
        .unwrap_or_default();
    if let Some(name) = declared_name(block) {
        Example::File {
            name,
            body: block.to_string(),
        }
    } else if !included_files(block).is_empty() {
        // Nothing named it, so the name is ours to pick; it only has to be somewhere an
        // `include` written relative to it can resolve.
        Example::File {
            name: format!("block-{index}.usage.kdl"),
            body: block.to_string(),
        }
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

/// Lay a page's file-shaped blocks out in `dir`, and return where each one landed.
///
/// Written before anything is parsed rather than as each block is reached: a page is free
/// to show the including file first, and a test that depended on the order would be a
/// documentation rule nobody agreed to.
fn write_files(dir: &Path, blocks: &[String]) -> Vec<(usize, PathBuf)> {
    let mut written = Vec::new();
    for (i, block) in blocks.iter().enumerate() {
        if let Example::File { name, body } = classify(block, i) {
            let path = dir.join(&name);
            std::fs::write(&path, &body).expect("writable");
            written.push((i, path));
        }
    }
    // A page may show the file that includes without showing what it includes — `config.md`
    // points at a `settings.usage.kdl` whose contents are the rest of that page. An empty
    // stand-in keeps the including block honestly parsed; what the target says is checked
    // wherever the page shows it.
    for block in blocks {
        for file in included_files(block) {
            let path = dir.join(&file);
            if !path.exists() {
                std::fs::write(&path, "").expect("writable");
            }
        }
    }
    written
}

#[test]
fn every_kdl_example_on_the_checked_pages_parses() {
    let pages = [
        Path::new(env!("CARGO_MANIFEST_DIR")).join("../docs/spec/index.md"),
        Path::new(env!("CARGO_MANIFEST_DIR")).join("../docs/spec/reference/cmd.md"),
        Path::new(env!("CARGO_MANIFEST_DIR")).join("../docs/spec/reference/config.md"),
        // Every block on the flagset page is a whole spec, a top-level fragment or a file
        // another block reads, so the page can be held to parsing from the day it was
        // written.
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
        let blocks = kdl_blocks(&std::fs::read_to_string(&page).expect("readable"));
        assert!(!blocks.is_empty(), "{name} contains no KDL examples");
        let dir = tempfile::tempdir().expect("temp dir");
        let files = write_files(dir.path(), &blocks);

        for (i, block) in blocks.iter().enumerate() {
            checked += 1;
            if let Some((_, path)) = files.iter().find(|(at, _)| *at == i) {
                if let Err(err) = usage::Spec::parse_file(path) {
                    failures.push(format!("{name} block {i}: {err}\n{block}"));
                }
                continue;
            }
            let source = match classify(block, i) {
                Example::File { .. } => unreachable!("written above"),
                Example::Spec(spec) => spec,
                Example::ConfigBody(body) => {
                    format!("name \"ex\"\nbin \"ex\"\nconfig {{\n{body}}}\n")
                }
                Example::Fragment(body) => format!("name \"ex\"\nbin \"ex\"\n{body}"),
            };
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
