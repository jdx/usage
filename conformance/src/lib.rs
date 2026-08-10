//! The argv conformance corpus: its format, a loader, and the reference runner.
//!
//! The corpus is the executable half of [the argv grammar]. Each vector pairs a
//! spec and an `argv` with the result that parsing one against the other must
//! produce. The files are plain JSON so implementations in other languages can
//! run the same cases without reimplementing a test format, and so the grammar
//! has a mechanical definition rather than only a prose one.
//!
//! [the argv grammar]: https://usage.jdx.dev/spec/argv

use std::collections::BTreeMap;
use std::path::{Path, PathBuf};

use serde::{Deserialize, Serialize};

pub mod reference;

/// One `corpus/*.json` file: a themed group of vectors.
#[derive(Debug, Deserialize, Serialize)]
pub struct VectorFile {
    /// Which part of the grammar this file covers, e.g. `"short-flags"`.
    pub section: String,
    /// What the group establishes, plus anything a reader needs in order to
    /// judge whether these expectations are the right ones.
    pub about: String,
    pub vectors: Vec<Vector>,
}

/// A single case: parse `argv` against `spec` and you must get `expect`.
#[derive(Debug, Deserialize, Serialize)]
pub struct Vector {
    /// Stable identifier, unique across the corpus. Failures and
    /// cross-implementation reports quote it, so renaming one breaks anybody
    /// tracking known failures.
    pub id: String,
    /// What behavior this pins down, in one sentence.
    pub doc: String,
    /// A complete spec, as KDL.
    pub spec: String,
    /// The command line, excluding the program name.
    pub argv: Vec<String>,
    /// The environment the parse sees. Only vectors about `env` fallback set it;
    /// the harness never consults the real environment, so no vector's result
    /// can depend on the machine running it.
    #[serde(default, skip_serializing_if = "BTreeMap::is_empty")]
    pub env: BTreeMap<String, String>,
    pub expect: Expect,
    /// Whether usage-lib, the reference implementation, agrees with `expect`.
    ///
    /// Recorded per vector rather than assumed. The corpus describes the grammar
    /// the spec intends; usage-lib is one implementation of it, and where the two
    /// differ that is worth writing down instead of hiding. These notes are the
    /// compatibility matrix any new implementation has to read.
    #[serde(default)]
    pub reference: Reference,
}

/// The result of a parse: a binding, or a class of failure.
#[derive(Debug, Deserialize, Serialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum Expect {
    Ok(Parsed),
    /// Only the *class* of error is pinned, never its wording. Message text is a
    /// diagnostics concern and is expected to differ between implementations.
    Error(ErrorCode),
}

/// What a successful parse binds.
///
/// Keyed by the name the spec gives each flag and argument rather than by the
/// token that set it, so `-j`, `--jobs`, and `JOBS=8` all land under `jobs`.
/// Anything left unset is omitted rather than recorded as null.
#[derive(Debug, Default, Deserialize, Serialize, PartialEq, Eq)]
pub struct Parsed {
    /// The subcommand path selected, outermost first; empty for the root.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub cmd: Vec<String>,
    #[serde(default, skip_serializing_if = "BTreeMap::is_empty")]
    pub flags: BTreeMap<String, Value>,
    #[serde(default, skip_serializing_if = "BTreeMap::is_empty")]
    pub args: BTreeMap<String, Value>,
}

/// A bound value.
///
/// Deliberately small. The grammar decides which tokens bind where, not what
/// they mean: turning `"8"` into a number is the caller's business, so the
/// corpus records the string.
#[derive(Debug, Deserialize, Serialize, PartialEq, Eq)]
#[serde(untagged)]
pub enum Value {
    Bool(bool),
    Str(String),
    Bools(Vec<bool>),
    Strs(Vec<String>),
}

/// The classes of failure the grammar distinguishes.
///
/// Coarse on purpose. An implementation should produce something far more
/// specific; what the corpus pins is that a command line fails for a given
/// *reason*, which is what lets a strict parser and a lenient one be told apart
/// mechanically.
#[derive(Debug, Clone, Copy, Deserialize, Serialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum ErrorCode {
    /// A token looked like a flag, but no flag by that name is in scope here.
    UnknownFlag,
    /// A flag needing a value was last, or was followed by something that cannot
    /// be its value.
    MissingFlagValue,
    /// A required flag never appeared.
    MissingRequiredFlag,
    /// A required positional was never filled.
    MissingRequiredArg,
    /// More positionals were given than the command accepts.
    UnexpectedArg,
    /// A value was given that is not among the declared choices.
    InvalidChoice,
    /// A positional declared `double_dash="required"` was given before `--`.
    ArgRequiresDoubleDash,
    /// A variadic got fewer values than `var_min`.
    VarTooFew,
    /// A variadic got more values than `var_max`.
    VarTooMany,
}

/// Whether the reference implementation matches a vector's expectation.
#[derive(Debug, Default, Deserialize, Serialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum Reference {
    /// usage-lib produces exactly `expect`.
    #[default]
    Agrees,
    /// usage-lib produces something else. The note says what, and why the corpus
    /// keeps its own expectation regardless.
    Diverges(String),
}

/// Load every `*.json` file in a corpus directory, sorted by file name.
pub fn load(dir: impl AsRef<Path>) -> Result<Vec<VectorFile>, String> {
    let dir = dir.as_ref();
    let mut paths: Vec<_> = std::fs::read_dir(dir)
        .map_err(|e| format!("reading {}: {e}", dir.display()))?
        .filter_map(Result::ok)
        .map(|e| e.path())
        .filter(|p| p.extension().is_some_and(|x| x == "json"))
        .collect();
    paths.sort();

    paths
        .iter()
        .map(|p| {
            let text =
                std::fs::read_to_string(p).map_err(|e| format!("reading {}: {e}", p.display()))?;
            serde_json::from_str(&text).map_err(|e| format!("parsing {}: {e}", p.display()))
        })
        .collect()
}

/// The corpus directory, resolved against this crate rather than the process's
/// working directory.
pub fn corpus_dir() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR")).join("../corpus")
}
