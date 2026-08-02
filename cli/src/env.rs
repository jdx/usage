use std::collections::{BTreeMap, HashSet};
use std::process::Command;

pub use std::env::*;

pub fn var_true(key: &str) -> bool {
    matches!(var(key), Ok(v) if v == "1" || v == "true")
}

/// Hand the parsed spec's variables to a command we are about to spawn.
///
/// On Windows this is not just `Command::env`. The executable search order there puts the
/// system directory ahead of `PATH`, so `bash` resolves to `C:\Windows\System32\bash.exe` —
/// the WSL launcher — on any machine with WSL installed, whatever else is on `PATH`. WSL only
/// carries a Win32 variable across the boundary if `WSLENV` names it, so without this the
/// script runs with every `usage_*` variable unset, silently and with no error.
pub fn apply_parsed_env(cmd: &mut Command, env: &BTreeMap<String, String>) {
    for (key, val) in env {
        cmd.env(key, val);
    }
    if env.is_empty() {
        return;
    }
    // `cfg!` rather than `#[cfg(windows)]`: CI only runs on Linux, so a `#[cfg]` block here
    // would never be compiled, type-checked or linted anywhere. This compiles everywhere and
    // optimizes away off Windows.
    if cfg!(windows) {
        let existing = var("WSLENV").ok();
        let keys = env.keys().map(String::as_str);
        cmd.env("WSLENV", append_to_wslenv(existing.as_deref(), keys));
    }
}

/// Whether an entry already in `WSLENV` delivers its value to WSL unchanged.
///
/// Only a bare name or `/u` does. Measured against WSL rather than read off the flag list,
/// because two of them lose the value outright in this direction:
///
/// | entry     | `FOO=bar`        | `FOO=C:\Windows` |
/// | --------- | ---------------- | ---------------- |
/// | `FOO`     | `bar`            | `C:\Windows`     |
/// | `FOO/u`   | `bar`            | `C:\Windows`     |
/// | `FOO/w`   | *unset*          | *unset*          |
/// | `FOO/uw`  | *unset*          | *unset*          |
/// | `FOO/p`   | *unset*          | `/mnt/c/Windows` |
/// | `FOO/l`   | *unset*          | `/mnt/c/Windows` |
///
/// `/w` is the other direction only, and `/p` and `/l` translate the value as a path — which
/// drops anything that is not one. usage's values are arbitrary strings off a command line, so
/// a `/p` entry would silently swallow almost all of them.
fn carries_value_verbatim(entry: &str) -> bool {
    match entry.split_once('/') {
        None => true,
        Some((_, flags)) => !flags.is_empty() && flags.chars().all(|flag| flag == 'u'),
    }
}

/// Add `keys` to a `WSLENV` value, preserving whatever was already there.
///
/// `WSLENV` is a `:`-separated list of *variable names*, each optionally suffixed with flags.
/// Names are added bare: usage has no idea whether a given value is a path, and `/p` would
/// silently rewrite anything that merely looks like one. Unflagged names copy the value
/// verbatim, so a script sees the same bytes it would on Unix.
///
/// Existing entries are never rewritten or dropped — a name the caller configured is theirs.
/// But an existing entry for a name usage is about to set does not stop usage adding its own
/// bare one unless it [carries the value verbatim](carries_value_verbatim): a `usage_foo/p`
/// inherited from somewhere would otherwise mean the script sees nothing at all. Listing the
/// name twice is how it is fixed rather than a problem to avoid — WSL takes the entry that
/// transfers, so `FOO/p:FOO` arrives as plain `FOO`.
///
/// Takes the current value as an argument instead of reading the environment so it stays a
/// pure function, testable on every platform rather than only where it does anything.
pub fn append_to_wslenv<'a>(
    existing: Option<&str>,
    keys: impl IntoIterator<Item = &'a str>,
) -> String {
    let mut entries: Vec<&str> = vec![];
    let mut names: HashSet<&str> = HashSet::new();

    for entry in existing.unwrap_or_default().split(':') {
        // Absorbs a leading, trailing or doubled `:`, either inherited or left by a caller
        // that built the list by naive concatenation.
        if entry.is_empty() {
            continue;
        }
        if carries_value_verbatim(entry) {
            names.insert(entry.split('/').next().unwrap_or(entry));
        }
        entries.push(entry);
    }

    for key in keys {
        // A name carrying `:` or `/` would not just fail to transfer, it would corrupt the
        // rest of the list and take the caller's own entries down with it. `as_env` derives
        // names from `to_snake_case`, which cannot produce either, so this is a guard against
        // that changing out from under us rather than a case we expect.
        if key.is_empty() || key.contains(':') || key.contains('/') {
            continue;
        }
        if names.insert(key) {
            entries.push(key);
        }
    }

    entries.join(":")
}

#[cfg(test)]
mod tests {
    use super::*;

    fn append(existing: Option<&str>, keys: &[&str]) -> String {
        append_to_wslenv(existing, keys.iter().copied())
    }

    #[test]
    fn wslenv_adds_keys_in_order() {
        assert_eq!(append(None, &["usage_workspace"]), "usage_workspace");
        assert_eq!(
            append(None, &["usage_workspace", "usage_region"]),
            "usage_workspace:usage_region"
        );
    }

    #[test]
    fn wslenv_appends_after_existing_entries() {
        assert_eq!(append(Some("FOO"), &["usage_a"]), "FOO:usage_a");
    }

    #[test]
    fn wslenv_leaves_existing_flags_untouched() {
        assert_eq!(
            append(Some("FOO/p:BAR/l"), &["usage_a"]),
            "FOO/p:BAR/l:usage_a"
        );
    }

    #[test]
    fn wslenv_does_not_repeat_an_existing_name() {
        assert_eq!(
            append(Some("usage_a"), &["usage_a", "usage_b"]),
            "usage_a:usage_b"
        );
    }

    #[test]
    fn wslenv_treats_a_direction_only_entry_as_covering_the_name() {
        // `/u` is this direction — Win32 invoking WSL — so the value already arrives intact.
        assert_eq!(append(Some("usage_a/u"), &["usage_a"]), "usage_a/u");
    }

    #[test]
    fn wslenv_adds_its_own_entry_beside_one_that_would_lose_the_value() {
        // `/p` translates the value as a path and drops anything that is not one; `/w` is the
        // other direction entirely. Neither would deliver a parsed argument, so usage adds a
        // bare entry after it — WSL then takes the one that transfers.
        for flags in ["/p", "/l", "/w", "/uw"] {
            let existing = format!("usage_a{flags}");
            assert_eq!(
                append(Some(&existing), &["usage_a"]),
                format!("{existing}:usage_a"),
                "an inherited {existing} must not swallow the value"
            );
        }
    }

    #[test]
    fn carries_value_verbatim_only_for_bare_names_and_u() {
        assert!(carries_value_verbatim("FOO"));
        assert!(carries_value_verbatim("FOO/u"));
        for entry in [
            "FOO/p", "FOO/l", "FOO/w", "FOO/uw", "FOO/wu", "FOO/pu", "FOO/",
        ] {
            assert!(!carries_value_verbatim(entry), "{entry}");
        }
    }

    #[test]
    fn wslenv_drops_empty_segments() {
        assert_eq!(append(Some(""), &["usage_a"]), "usage_a");
        assert_eq!(append(Some("::FOO::"), &["usage_a"]), "FOO:usage_a");
    }

    #[test]
    fn wslenv_with_no_keys_returns_existing() {
        assert_eq!(append(Some("FOO"), &[]), "FOO");
        assert_eq!(append(None, &[]), "");
    }

    #[test]
    fn wslenv_skips_keys_that_would_corrupt_the_list() {
        assert_eq!(append(None, &["ok", "bad:name"]), "ok");
        assert_eq!(append(None, &["ok", "bad/p"]), "ok");
        assert_eq!(append(None, &["", "ok"]), "ok");
    }

    #[test]
    fn wslenv_dedups_within_the_new_keys() {
        assert_eq!(append(None, &["a", "a"]), "a");
    }

    #[test]
    fn wslenv_adds_no_flags() {
        // Values are arbitrary strings, not known to be paths, so they must cross verbatim.
        assert!(!append(None, &["usage_a"]).contains('/'));
    }

    #[test]
    fn parsed_env_keys_are_safe_for_wslenv() {
        // `append_to_wslenv` skips names containing `:` or `/`. Nothing usage-lib produces
        // should ever hit that path; if `as_env` starts generating such names, variables
        // would go missing on Windows, so pin the invariant here.
        let spec: usage::Spec = r#"
            arg "<some file>"
            flag "--dry-run"
            "#
        .parse()
        .unwrap();
        let args = ["test", "x", "--dry-run"].map(String::from);
        let env = usage::parse(&spec, &args).unwrap().as_env();

        assert!(!env.is_empty());
        for key in env.keys() {
            assert!(
                !key.is_empty() && !key.contains(':') && !key.contains('/'),
                "as_env produced a key that cannot go in WSLENV: {key}"
            );
        }
    }
}
