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
    let declared = declared_names(root);
    let mut unknown: Vec<String> = names
        .into_iter()
        .map(OsStr::to_string_lossy)
        .filter(|name| starts_with_env(name, prefix))
        .filter(|name| {
            !declared
                .iter()
                .chain(known)
                .any(|k| same_env(k, name.as_ref()))
        })
        .map(|name| name.into_owned())
        .collect();
    unknown.sort_unstable();
    unknown.dedup();
    unknown
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

/// Windows variable names are case-insensitive — `std::env::var("PATH")` finds `Path` — so a
/// variable set as `Ex_Port` is the one a CLI declares as `EX_PORT`. Folded with Unicode case
/// rules rather than ASCII ones, as `usage_config::EnvLayer` does, because Windows folds `ä` and
/// `Ä` together too.
fn same_env(a: &str, b: &str) -> bool {
    if cfg!(windows) {
        a.to_uppercase() == b.to_uppercase()
    } else {
        a == b
    }
}

fn starts_with_env(name: &str, prefix: &str) -> bool {
    if cfg!(windows) {
        name.to_uppercase().starts_with(&prefix.to_uppercase())
    } else {
        name.starts_with(prefix)
    }
}
