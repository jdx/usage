//! A spec's `config` block, as the registry `usage-config` resolves against.
//!
//! Every CLI in the jdx fleet declares its settings in one file and resolves them in another, and
//! the two are kept in step by hand. They drift every time: hk declares eighteen `sources.cli`
//! bindings and reads five, pitchfork generates five settings that its own `settings get` cannot
//! reach, fnox's docs describe a layer that does not exist. This crate is the join. The spec is
//! read here, at build time, and what comes out is `const` — so a setting that is declared is a
//! setting that resolves, and there is no second place to forget to update.
//!
//! ```no_run
//! // build.rs
//! usage_config_build::generate("mycli.usage.kdl").expect("settings");
//! ```
//!
//! ```ignore
//! // src/settings.rs
//! include!(concat!(env!("OUT_DIR"), "/settings.rs"));
//! ```
//!
//! Nothing generated here allocates or parses at run time: `PropMeta` is `const`-constructible
//! down to its defaults, so a binary that reads no settings pays nothing for having them, and one
//! that reads all of them pays a `static`.
//!
//! # What it refuses
//!
//! A build script is the right place to be strict, because the alternative is a warning on every
//! run of a shipped binary for a mistake only the author can fix. So a registry is refused when it
//! cannot mean what it says: a `renamed_to` naming a setting that is not there, renames that form a
//! cycle, an old name carrying a default its replacement lacks, a `parse` nobody implements, a
//! `map` keyed by something a config file cannot spell, or two keys whose generated names collide.
//! All of them at once, rather than the first — an author fixing a registry wants the list.

use std::fmt;
use std::path::{Path, PathBuf};
use std::str::FromStr as _;

mod emit;
mod settings;

/// Read `spec` and write the registry to `$OUT_DIR/settings.rs`.
///
/// Returns the path written, and tells cargo to re-run when the spec changes.
pub fn generate(spec: impl AsRef<Path>) -> Result<PathBuf, Error> {
    let out_dir = std::env::var("OUT_DIR").map_err(|_| Error::NoOutDir)?;
    let out = PathBuf::from(out_dir).join("settings.rs");
    generate_to(spec, &out)?;
    Ok(out)
}

/// Read `spec` and write the registry to `out`.
///
/// For a caller that keeps generated code in the repository rather than in `OUT_DIR` — which is
/// how this crate tests itself, and how a CLI that wants its registry reviewable does it.
pub fn generate_to(spec: impl AsRef<Path>, out: impl AsRef<Path>) -> Result<(), Error> {
    let spec = spec.as_ref();
    // Before anything can fail, so a spec that does not parse is still watched and the next build
    // is not a stale success.
    println!("cargo::rerun-if-changed={}", spec.display());
    let source = source(spec)?;
    // And every file the spec *included*, which is where a CLI with many settings keeps them — so
    // watching only the file the build script names left editing the settings rebuilding nothing.
    for watched in watched(spec)?.into_iter().skip(1) {
        println!("cargo::rerun-if-changed={}", watched.display());
    }
    let out = out.as_ref();
    if let Some(parent) = out.parent() {
        std::fs::create_dir_all(parent).map_err(|err| Error::Io {
            path: parent.to_path_buf(),
            why: err.to_string(),
        })?;
    }
    // Only when it differs, so a checked-in registry keeps its mtime and nothing downstream
    // rebuilds for a generator that produced the same bytes.
    if std::fs::read_to_string(out).is_ok_and(|existing| existing == source) {
        return Ok(());
    }
    std::fs::write(out, source).map_err(|err| Error::Io {
        path: out.to_path_buf(),
        why: err.to_string(),
    })
}

/// Every file a build should watch: the spec, then each `include`, recursively.
///
/// [`generate_to`] prints these for cargo. A build script doing something more elaborate — writing
/// the registry somewhere of its own, or generating other things from the same spec — wants the list
/// rather than the printing.
pub fn watched(spec: impl AsRef<Path>) -> Result<Vec<PathBuf>, Error> {
    let parsed =
        usage::Spec::parse_file(spec.as_ref()).map_err(|err| Error::Spec(err.to_string()))?;
    Ok(parsed.sources)
}

/// The Rust source a spec's `config` block becomes.
pub fn source(spec: impl AsRef<Path>) -> Result<String, Error> {
    let spec = spec.as_ref();
    let parsed = usage::Spec::parse_file(spec).map_err(|err| Error::Spec(err.to_string()))?;
    let name = spec
        .file_name()
        .map(|n| n.to_string_lossy().into_owned())
        .unwrap_or_else(|| spec.display().to_string());
    source_of(&parsed.config, &name)
}

/// The same, from a spec that is already in memory.
///
/// A relative `include` resolves against the file a spec was read from, so a spec whose settings
/// live in their own file has to go through [`source`]; this is for a caller holding one text.
pub fn source_of_spec(spec: &str, name: &str) -> Result<String, Error> {
    let parsed = usage::Spec::from_str(spec).map_err(|err| Error::Spec(err.to_string()))?;
    source_of(&parsed.config, name)
}

fn source_of(config: &usage::spec::config::SpecConfig, name: &str) -> Result<String, Error> {
    if config.props.is_empty() {
        return Err(Error::NoSettings);
    }
    emit::registry(config, name).map_err(Error::Registry)
}

/// Why a registry could not be generated.
#[derive(Debug)]
pub enum Error {
    /// The spec itself did not parse.
    Spec(String),
    /// The spec parsed and declares no settings, which is not something to generate an empty
    /// registry for: a build script asking for one has been pointed at the wrong file.
    NoSettings,
    /// Everything wrong with the settings, rather than the first thing.
    Registry(Vec<String>),
    /// [`generate`] was called outside a build script.
    NoOutDir,
    Io {
        path: PathBuf,
        why: String,
    },
}

impl fmt::Display for Error {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Spec(why) => write!(f, "{why}"),
            Self::NoSettings => f.write_str("this spec declares no `config` settings"),
            Self::Registry(problems) => {
                for (i, problem) in problems.iter().enumerate() {
                    if i > 0 {
                        f.write_str("\n")?;
                    }
                    f.write_str(problem)?;
                }
                Ok(())
            }
            Self::NoOutDir => f.write_str("OUT_DIR is not set; call this from a build script"),
            Self::Io { path, why } => write!(f, "{}: {why}", path.display()),
        }
    }
}

impl std::error::Error for Error {}
