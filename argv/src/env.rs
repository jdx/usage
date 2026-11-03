//! Environment variables under a CLI's own prefix that nothing declares.
//!
//! `unknown_flags = "error"` turns a misspelled flag into an error. A misspelled or stale
//! environment variable gets no such check by default: `MYAPP_LISTEN_TLS_CRET` is read by
//! nobody, and the program runs on its defaults. That failure is worst in deployments, where
//! configuration arrives through the environment and nobody watches the command line.
//!
//! A derive opts in with `#[usage(unknown_env(prefix = "MYAPP_", allow("MYAPP_LIBRARY")))]`
//! on its root, and its generated `check` calls [`refuse_unknown`]. The prefix only scopes the
//! check; it never builds a name, so every variable stays spelled in full where it is declared.
//!
//! usage-lib answers the same question over a parsed spec, in `usage::SpecUnknownEnv`. The two
//! read different models — compiled tables here, a `Spec` there — and
//! `conformance/tests/unknown_env.rs` holds them to one answer for the same CLI.

use std::ffi::OsStr;

use crate::Error;
use crate::spec::CommandMeta;

/// Every environment variable the command tree under `root` reads.
///
/// Each flag's and argument's `env`, `env_fallback` and `deprecated_env`, in every command
/// rather than only the one a parse reached — a variable meant for `serve` is not a typo when
/// the user runs `publish` — clauses and flattened groups included. Deprecated names count, so
/// they keep producing their deprecation warning instead of becoming unknown.
pub fn declared_names<'a>(root: &CommandMeta<'a>) -> Vec<&'a str> {
    let mut names = Vec::new();
    let mut seen: Vec<*const CommandMeta<'a>> = Vec::new();
    collect(root, &mut names, &mut seen);
    names.sort_unstable();
    names.dedup();
    names
}

fn collect<'a>(
    meta: &CommandMeta<'a>,
    names: &mut Vec<&'a str>,
    seen: &mut Vec<*const CommandMeta<'a>>,
) {
    // A subcommand type may be shared by several parents, or reach back to one: each
    // command's names are collected once.
    let ptr: *const CommandMeta<'a> = meta;
    if seen.contains(&ptr) {
        return;
    }
    seen.push(ptr);
    let clause = meta.extra.clause;
    let flags = meta
        .flags
        .iter()
        .chain(clause.into_iter().flat_map(|c| c.flags.iter()));
    for flag in flags {
        names.extend(flag.env());
        names.extend_from_slice(flag.env_fallback());
        names.extend_from_slice(flag.deprecated_env());
    }
    let args = meta
        .args
        .iter()
        .chain(clause.into_iter().flat_map(|c| c.args.iter()));
    for arg in args {
        names.extend(arg.env());
        names.extend_from_slice(arg.env_fallback());
        names.extend_from_slice(arg.deprecated_env());
    }
    for sub in meta.subcommands {
        collect(sub, names, seen);
    }
}

/// The variables in `names` that start with `prefix` and that neither the tree under `root`
/// nor `known` declares, sorted and each once.
///
/// `known` is everything read outside the command tree: an allow list for variables a library
/// reads directly, and a settings registry's names. A name that is not valid UTF-8 cannot be a
/// declared one, so it is reported in its lossy spelling rather than skipped. On Windows, where
/// variable names are case-insensitive, so is the comparison.
pub fn unknown<'n>(
    root: &CommandMeta<'_>,
    prefix: &str,
    known: &[&str],
    names: impl IntoIterator<Item = &'n OsStr>,
) -> Vec<String> {
    // An empty prefix owns nothing: the derive refuses one, and a caller passing one must not
    // make `PATH` an error.
    if prefix.is_empty() {
        return Vec::new();
    }
    let declared = declared_names(root);
    let mut unknown: Vec<&OsStr> = names
        .into_iter()
        .filter(|name| {
            // A name that is not UTF-8 is never a declared one, even when its lossy spelling
            // happens to equal a declaration that contains U+FFFD.
            starts_with_env(valid_head(name), prefix)
                && name
                    .to_str()
                    .is_none_or(|name| !declared.iter().chain(known).any(|k| same_env(k, name)))
        })
        .collect();
    // On Windows `EX_TYPO` and `ex_typo` are one variable, and listing it twice would read as two
    // mistakes. Two names that are not UTF-8 can share a lossy spelling and still be two
    // variables, so those are kept apart by their bytes.
    unknown.sort_by_cached_key(|name| (fold_env(&name.to_string_lossy()).into_owned(), *name));
    unknown.dedup_by(|a, b| match (a.to_str(), b.to_str()) {
        (Some(a), Some(b)) => same_env(a, b),
        _ => a == b,
    });
    unknown
        .into_iter()
        .map(|name| name.to_string_lossy().into_owned())
        .collect()
}

/// [`unknown`] against the process environment, as the error a parse returns.
///
/// Every offending variable is named at once: a deployment with two typos should not need two
/// restarts to find them.
pub fn refuse_unknown<'t, 'v>(
    root: &CommandMeta<'_>,
    prefix: &'t str,
    known: &[&str],
) -> Result<(), Error<'t, 'v>> {
    let process: Vec<_> = std::env::vars_os().map(|(name, _)| name).collect();
    let names = unknown(root, prefix, known, process.iter().map(|n| n.as_os_str()));
    if names.is_empty() {
        return Ok(());
    }
    Err(Error::UnknownEnv(::std::boxed::Box::new(
        crate::UnknownEnv { prefix, names },
    )))
}

/// A name as the platform compares it.
///
/// Windows variable names are case-insensitive — `std::env::var("PATH")` finds `Path` — so a
/// variable set as `Ex_Port` is the one a CLI declares as `EX_PORT`. Windows folds one character
/// at a time, `ä` with `Ä` but `ß` never with `SS`, so a character whose uppercase is longer than
/// itself is kept as it is rather than expanded.
fn fold_env(name: &str) -> std::borrow::Cow<'_, str> {
    if !cfg!(windows) {
        return name.into();
    }
    name.chars()
        .map(|c| {
            let mut upper = c.to_uppercase();
            match (upper.next(), upper.next()) {
                (Some(u), None) => u,
                _ => c,
            }
        })
        .collect::<String>()
        .into()
}

fn same_env(a: &str, b: &str) -> bool {
    fold_env(a) == fold_env(b)
}

/// Folding keeps one character for one, so a folded prefix is a prefix of the folded name
/// exactly when the original is.
fn starts_with_env(name: &str, prefix: &str) -> bool {
    fold_env(name).starts_with(&*fold_env(prefix))
}

/// The part of `name` before its first byte that is not UTF-8.
///
/// The prefix is UTF-8, so a name can only start with it there. Matching the lossy spelling
/// instead would let a prefix containing U+FFFD claim a name whose bytes are something else.
fn valid_head(name: &OsStr) -> &str {
    let bytes = name.as_encoded_bytes();
    match std::str::from_utf8(bytes) {
        Ok(name) => name,
        Err(e) => std::str::from_utf8(&bytes[..e.valid_up_to()]).unwrap_or_default(),
    }
}
