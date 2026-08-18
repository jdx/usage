//! The completion corpus: its format, a loader, and the runner.
//!
//! The argv corpus pins what a command line *binds*; `render` pins what a spec *reads as*. This
//! one pins what could go where the cursor is.
//!
//! # Why it exists
//!
//! Completion was the one area with three implementations and no shared fixture.
//! `argv/src/complete.rs` has its own unit tests, `cli/src/cli/complete_word.rs` has its own, and
//! a Go implementation landed with a third set. Three sets of tests written against three
//! readings of the same rules is the arrangement that produced every drift this project has had
//! to chase: the help renderers agreed on mise and differed on five of the other six jdx CLIs
//! until one fixture held them together.
//!
//! It also closes two thirds of PLAN.md's "not covered by the corpus yet" — completion parsing,
//! which is `parse_partial` over deliberately incomplete input, and restart tokens, which only
//! matter at a cursor. Mounts stay uncovered on purpose: resolving one *runs a command*, which a
//! corpus cannot do hermetically.
//!
//! # Every expectation here was measured first
//!
//! Two of the first vectors written asserted behaviour neither implementation has — a restart
//! token offering itself, and an attached `--format=j` completing its value. Both were plausible
//! and both were invented. The corpus is the definition of correct, so a vector may not assert a
//! rule on one author's opinion: what goes in is measured on both implementations, and where they
//! agree on something that looks wrong, the vector says so rather than pinning it silently.

use std::collections::BTreeSet;
use std::path::{Path, PathBuf};

use serde::{Deserialize, Serialize};
use usage::Spec;
use usage_argv::complete::{complete, split, Shell};

use crate::tables;

/// One `corpus/complete/*.json` file: a themed group of vectors.
#[derive(Debug, Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
pub struct VectorFile {
    /// What this file covers, e.g. `"positions"`.
    pub section: String,
    /// What the group establishes, and anything a reader needs in order to judge whether these
    /// expectations are the right ones.
    pub about: String,
    pub vectors: Vec<Vector>,
}

/// A single case: ask `spec` what completes at the cursor in `line` and you must get `expect`.
#[derive(Debug, Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
pub struct Vector {
    /// Stable identifier, unique across the corpus. Failures quote it, so renaming one breaks
    /// anybody tracking known failures.
    pub id: String,
    /// What this vector pins down, in one sentence.
    pub doc: String,
    /// A complete spec, as KDL.
    pub spec: String,
    /// The command line as typed, program name included.
    pub line: String,
    /// Where the cursor is, as a byte offset into `line`. Defaults to its end, which is where a
    /// shell asks from; a vector completing mid-line says so.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub cursor: Option<usize>,
    pub expect: Expect,
    /// Whether `usage-cli`, the reference implementation, agrees with `expect`.
    #[serde(default)]
    pub reference: Reference,
}

impl Vector {
    fn cursor(&self) -> usize {
        self.cursor.unwrap_or(self.line.len())
    }
}

/// What completion must produce.
#[derive(Debug, Default, Deserialize, Serialize, PartialEq, Eq)]
#[serde(deny_unknown_fields)]
pub struct Expect {
    /// The words offered. Order-insensitive unless `ordered`: which order a shell shows them in
    /// is the shell's business, and two implementations sorting differently is not a
    /// disagreement about what completes.
    #[serde(default)]
    pub candidates: Vec<String>,
    /// Whether the answer defers to the shell's own path completion. A vector cannot state what
    /// the filesystem holds without becoming a test of the machine it runs on, so it states that
    /// paths belong here and stops.
    #[serde(default, skip_serializing_if = "std::ops::Not::not")]
    pub files: bool,
    /// Whether the order of `candidates` is itself the claim.
    #[serde(default, skip_serializing_if = "std::ops::Not::not")]
    pub ordered: bool,
}

/// Whether the reference implementation matches a vector's expectation.
#[derive(Debug, Default, Deserialize, Serialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum Reference {
    /// `usage-cli` produces exactly `expect`.
    #[default]
    Agrees,
    /// It produces something else. The note says what, and why the corpus keeps its own
    /// expectation regardless.
    Diverges(String),
}

/// What one implementation offered.
#[derive(Debug, PartialEq, Eq)]
pub struct Offered {
    pub candidates: Vec<String>,
    pub files: bool,
}

impl Offered {
    /// Whether this satisfies a vector. Sorted unless the vector claims an order, so a failure
    /// is about the set rather than about two implementations' sort stability.
    pub fn matches(&self, expect: &Expect) -> bool {
        if self.files != expect.files {
            return false;
        }
        if expect.ordered {
            return self.candidates == expect.candidates;
        }
        let mine: BTreeSet<&str> = self.candidates.iter().map(String::as_str).collect();
        let theirs: BTreeSet<&str> = expect.candidates.iter().map(String::as_str).collect();
        mine == theirs
    }
}

/// What `usage-argv` offers for a vector.
///
/// Its tables are built from the vector's KDL by `tables::build_spec`, which is the same bridge
/// the render corpus uses: the alternative is a Rust type per vector, and a corpus a reader
/// cannot extend is not one.
pub fn run(vector: &Vector) -> Result<Offered, String> {
    let spec: Spec = vector
        .spec
        .parse()
        .map_err(|e| format!("the spec would not load: {e}"))?;
    let tables = tables::build_spec(&spec);
    // Bash because the split has to be *some* shell and this one has no quoting rules of its
    // own to fold in; which shell renders the answer is `complete::render`'s business, tested
    // separately.
    let at = split(&vector.line, vector.cursor(), Shell::Bash);
    let answer = complete(tables, &at);
    Ok(Offered {
        candidates: answer.candidates.iter().map(|c| c.value.clone()).collect(),
        files: answer.files.is_some(),
    })
}

/// What the reference `usage-cli` implementation offers for a vector.
pub fn reference(vector: &Vector) -> Result<Offered, String> {
    let spec: Spec = vector
        .spec
        .parse()
        .map_err(|e| format!("the spec would not load: {e}"))?;
    let at = split(&vector.line, vector.cursor(), Shell::Bash);
    let answer = usage_cli::complete_answer(&spec, &at.words, at.cword, "bash")
        .map_err(|e| format!("the reference would not complete it: {e}"))?;
    Ok(Offered {
        candidates: if answer.files {
            Vec::new()
        } else {
            answer
                .candidates
                .into_iter()
                .map(|(value, _)| value)
                .collect()
        },
        files: answer.files,
    })
}

/// Every vector in the corpus, in file order.
pub fn load() -> Vec<(String, Vector)> {
    let mut out = Vec::new();
    let mut files: Vec<PathBuf> = std::fs::read_dir(dir())
        .expect("the completion corpus should be readable")
        .map(|entry| {
            entry
                .expect("a completion corpus directory entry should be readable")
                .path()
        })
        .filter(|p| p.extension().is_some_and(|e| e == "json"))
        .collect();
    files.sort();
    for path in files {
        let text = std::fs::read_to_string(&path).expect("a corpus file should be readable");
        let file: VectorFile = serde_json::from_str(&text)
            .unwrap_or_else(|e| panic!("{} is not a valid corpus file: {e}", path.display()));
        let name = path
            .file_name()
            .and_then(|n| n.to_str())
            .unwrap_or_default()
            .to_string();
        for vector in file.vectors {
            out.push((name.clone(), vector));
        }
    }
    out
}

fn dir() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR")).join("../corpus/complete")
}
