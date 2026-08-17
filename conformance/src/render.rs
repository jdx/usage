//! The rendering corpus: its format, a loader, and the two runners.
//!
//! The argv corpus pins what a command line *binds*. This one pins what a spec *reads as* —
//! the usage line, `-h` and `--help` — for the same reason and in the same shape: the rules
//! are now implemented three times over (usage-lib's templates, `usage_argv::help`, and the Go
//! emitter's help table), and three implementations of a rendering rule drift exactly the way
//! three implementations of a parsing rule do.
//!
//! # Why this exists beside the mise fixture
//!
//! `benches/gate/tests/help.rs` compares every one of mise's 211 commands against usage-lib,
//! byte for byte, and it is the check that decides whether an adopter's help output changes.
//! What it cannot do is cover a shape mise does not use. A flag whose *value* is optional is
//! one: every flag value mise declares is required and undefaulted, so `[--opt [n]]` rendered
//! as `[--opt <n>]` for as long as usage-argv existed and the 211-command comparison passed
//! throughout.
//!
//! So the two are complements. The fixture answers "does a real CLI still render the same",
//! at a scale no hand-written case reaches. The corpus answers "does every shape a spec can
//! declare render the same", including the ones no single CLI happens to contain.
//!
//! # Scope
//!
//! Presentation only. What a page *says* — that a flag exists, what its help text is — is the
//! spec's business and is checked elsewhere; what this pins is how it is written down.

use std::path::{Path, PathBuf};

use serde::{Deserialize, Serialize};
use usage::Spec;
use usage_argv::help::{long_help, short_help, usage_line};
use usage_argv::spec::CommandMeta;

use crate::tables;

/// One `corpus/render/*.json` file: a themed group of vectors.
#[derive(Debug, Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
pub struct VectorFile {
    /// What this file covers, e.g. `"flag-values"`.
    pub section: String,
    /// What the group establishes, and anything a reader needs in order to judge whether these
    /// expectations are the right ones.
    pub about: String,
    pub vectors: Vec<Vector>,
}

/// A single case: render `cmd` out of `spec` and you must get `expect`.
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
    /// Which command's page, as the path below the root. Empty means the root's own.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub cmd: Vec<String>,
    pub expect: Expect,
    /// Whether usage-lib, the reference implementation, agrees with `expect`.
    #[serde(default)]
    pub reference: Reference,
}

/// What rendering must produce.
///
/// The usage line is required, because it is the one line every vector has an opinion about
/// and the shortest thing that can carry a shape. The pages are optional and given as lines
/// rather than one escaped string: a JSON file holding a `\n\n  --flag  Help\n` is not
/// something a reviewer can read, and a diff over it says nothing about which line moved.
#[derive(Debug, Deserialize, Serialize, PartialEq, Eq)]
#[serde(deny_unknown_fields)]
pub struct Expect {
    /// The `Usage:` line's body, including the binary.
    pub usage: String,
    /// Every line of `-h`, if the vector pins the whole page.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub short_help: Option<Vec<String>>,
    /// Every line of `--help`, if the vector pins the whole page. Rendered at 80 columns,
    /// which is what both implementations fall back to when `COLUMNS` is unset.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub long_help: Option<Vec<String>>,
}

/// Whether the reference implementation matches a vector's expectation.
#[derive(Debug, Default, Deserialize, Serialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum Reference {
    /// usage-lib produces exactly `expect`.
    #[default]
    Agrees,
    /// usage-lib produces something else. The note says what, and why the corpus keeps its own
    /// expectation regardless.
    Diverges(String),
}

/// What one implementation rendered.
#[derive(Debug, PartialEq, Eq)]
pub enum Outcome {
    Rendered(Rendered),
    /// The vector turns on something this implementation deliberately cannot express. The
    /// string says which.
    OutOfScope(&'static str),
    /// The spec would not load, or names no such command. A bug in the vector.
    Bad(String),
}

/// Why a vector is not usage-argv's to answer, if it isn't.
///
/// One word so far. `disable_help` turns the parser's answer to `-h` off, and usage-lib drops
/// the supplied entry accordingly; usage-argv has no equivalent, and `lib/src/docs/cli/mod.rs`
/// says why — it is a KDL-only word, so no spec *the derive* can produce ever carries one and
/// the two renderers cannot disagree about it.
///
/// This harness breaks that premise, since it builds usage-argv's tables from KDL rather than
/// from a Rust type. So a vector declaring it is answered by the reference alone and skipped
/// here, rather than being recorded as a divergence: nothing usage-argv could render would be
/// right, because the question does not reach it.
fn out_of_scope(spec: &Spec) -> Option<&'static str> {
    (spec.disable_help == Some(true)).then_some(
        "`disable_help` is a KDL-only word with no derive spelling, so usage-argv's tables \
         cannot carry it",
    )
}

/// The three renderings, as an implementation produced them.
#[derive(Debug, PartialEq, Eq)]
pub struct Rendered {
    pub usage: String,
    pub short_help: Vec<String>,
    pub long_help: Vec<String>,
}

impl Outcome {
    /// How this differs from what the vector expects, or `None` if it does not.
    ///
    /// A vector that pins only the usage line is not asserting the pages are empty, so an
    /// absent expectation is not compared rather than compared against nothing.
    pub fn difference(&self, expect: &Expect) -> Option<String> {
        let got = match self {
            Outcome::Bad(why) => return Some(why.clone()),
            // Not a difference: the vector was never this implementation's to answer, and a
            // caller that cares checks for the variant rather than reading it as agreement.
            Outcome::OutOfScope(_) => return None,
            Outcome::Rendered(got) => got,
        };
        if got.usage != expect.usage {
            return Some(format!(
                "usage line\n      ours: {}\n  expected: {}",
                got.usage, expect.usage
            ));
        }
        for (label, want, have) in [
            ("-h", expect.short_help.as_ref(), &got.short_help),
            ("--help", expect.long_help.as_ref(), &got.long_help),
        ] {
            let Some(want) = want else { continue };
            if want != have {
                return Some(format!("{label}\n{}", first_diff(have, want)));
            }
        }
        None
    }
}

/// The first line that differs, with a little context — a whole page twice over is not
/// something anyone reads.
fn first_diff(ours: &[String], theirs: &[String]) -> String {
    for (i, (a, b)) in ours.iter().zip(theirs).enumerate() {
        if a != b {
            return format!("  line {}:\n      ours: {a:?}\n  expected: {b:?}", i + 1);
        }
    }
    format!(
        "  same for {} lines, then ours has {} and the vector expects {}",
        ours.len().min(theirs.len()),
        ours.len(),
        theirs.len()
    )
}

/// Render a vector with usage-lib, the reference.
pub fn reference(vector: &Vector) -> Outcome {
    let spec: Spec = match vector.spec.parse() {
        Ok(spec) => spec,
        Err(e) => return Outcome::Bad(format!("the spec would not parse: {e}")),
    };
    let mut cmd = &spec.cmd;
    for name in &vector.cmd {
        cmd = match cmd.subcommands.get(name) {
            Some(sub) => sub,
            None => return Outcome::Bad(format!("the spec has no command `{name}`")),
        };
    }
    // usage-lib's `usage()` starts at the command path and omits the binary, which the
    // template puts back after `Usage: `.
    Outcome::Rendered(Rendered {
        usage: format!("{} {}", spec.bin, cmd.usage()).trim().to_string(),
        short_help: lines(&usage::docs::cli::render_help(&spec, cmd, false)),
        long_help: lines(&usage::docs::cli::render_help(&spec, cmd, true)),
    })
}

/// Render a vector with usage-argv, from tables built out of the same spec.
pub fn argv(vector: &Vector) -> Outcome {
    let spec: Spec = match vector.spec.parse() {
        Ok(spec) => spec,
        Err(e) => return Outcome::Bad(format!("the spec would not parse: {e}")),
    };
    if let Some(reason) = out_of_scope(&spec) {
        return Outcome::OutOfScope(reason);
    }
    let built = tables::build_spec(&spec);

    // The path a user types and the chain of metadata down to it. Both are needed: the path is
    // what the line prints, and the chain is what says which flags are inherited.
    let mut path: Vec<&'static str> = vec![built.bin.unwrap_or(built.name)];
    let mut chain: Vec<&'static CommandMeta<'static>> = vec![built.root];
    for name in &vector.cmd {
        let here = chain.last().expect("a chain always has its root");
        let next = here.subcommands.iter().find(|sub| sub.cmd.name == *name);
        match next {
            Some(sub) => {
                path.push(sub.cmd.name);
                chain.push(sub);
            }
            None => return Outcome::Bad(format!("the tables have no command `{name}`")),
        }
    }
    let meta = chain.last().expect("a chain always has its root");

    Outcome::Rendered(Rendered {
        usage: usage_line(&path, meta),
        short_help: lines(&short_help(built, &path, &chain)),
        long_help: lines(&long_help(built, &path, &chain)),
    })
}

/// A rendered page as lines, without the trailing empty one every page ends with.
///
/// Both implementations trim the document and put back a single newline, so `lines()` on the
/// result is exactly the page's lines with nothing added or lost.
fn lines(page: &str) -> Vec<String> {
    page.lines().map(str::to_string).collect()
}

/// The rendering corpus directory, resolved against this crate rather than the process's
/// working directory.
pub fn corpus_dir() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR")).join("../corpus/render")
}

/// Load every `*.json` file in a rendering corpus directory, sorted by file name.
pub fn load(dir: impl AsRef<Path>) -> Result<Vec<VectorFile>, String> {
    crate::load_as(dir)
}
