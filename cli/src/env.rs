use std::collections::{BTreeMap, HashSet};
use std::process::Command;

pub use std::env::*;

pub fn var_true(key: &str) -> bool {
    matches!(var(key), Ok(v) if v == "1" || v == "true")
}

/// The prefix this CLI's own settings live under.
///
/// Not `USAGE_`, which is the same namespace the parser writes a spec's values into
/// (`usage_<arg>`). On Windows environment variable names are case-insensitive, so
/// `USAGE_DEBUG` and a spec's `usage_debug` are one variable there, and two things follow:
/// a script with an ordinary `--debug` flag cannot read `$usage_debug` when the setting is
/// also set, and mise — which clears `usage_*` before a task so its own parsed arguments
/// cannot leak in, comparing the first six characters case-insensitively — takes the
/// settings with it. Six characters is why no `USAGE_…` spelling escapes: `USAGE_CLI_` and
/// `USAGE_SETTING_` are cleared just the same.
const SETTING_PREFIX: &str = "USAGECLI_";

/// The prefix the settings used to live under, still read so nothing that set it breaks.
const LEGACY_SETTING_PREFIX: &str = "USAGE_";

/// The environment variable naming setting `name`, e.g. `SHELL_BASH` -> `USAGECLI_SHELL_BASH`.
pub fn setting_var_name(name: &str) -> String {
    format!("{SETTING_PREFIX}{name}")
}

/// The value of setting `name`, preferring the current spelling over the legacy one.
///
/// First one set wins and the rest are not looked at, which is what makes the old name an
/// alias rather than a second setting — the same order `usage-config` gives `deprecated_envs`.
/// Nothing is warned about: one name set is the ordinary case, and the settings read here are
/// read before there is a logger to warn with.
///
/// An empty or blank value reads as unset at each name, matching the `FOO= cmd` convention for
/// switching something off, so blanking the current name falls through to the legacy one
/// rather than to nothing.
pub fn setting(name: &str, lookup: impl Fn(&str) -> Option<String>) -> Option<String> {
    setting_entry(name, lookup).map(|(_, value)| value)
}

/// As [`setting`], and also which spelling supplied it.
///
/// The name matters to anything that reports back: telling someone their `USAGECLI_SHELL_BASH`
/// could not be started, when what they set was `USAGE_SHELL_BASH`, sends them looking at a
/// variable they never touched.
pub fn setting_entry(
    name: &str,
    lookup: impl Fn(&str) -> Option<String>,
) -> Option<(String, String)> {
    [
        format!("{SETTING_PREFIX}{name}"),
        format!("{LEGACY_SETTING_PREFIX}{name}"),
    ]
    .into_iter()
    .find_map(|key| {
        let value = lookup(&key)?;
        let value = value.trim();
        (!value.is_empty()).then(|| (key, value.to_string()))
    })
}

/// The log filter to use, under either spelling of each setting.
///
/// Resolved rather than read straight by `env_logger`: `Env::filter_or` falls back to its
/// default only when the variable is *unset*, so a blank `USAGECLI_LOG` would be taken as the
/// filter instead of falling through to `USAGE_LOG` — which is the rule [`setting`] promises.
///
/// Precedence: trace over debug over an explicit level, and `info` if none of them says
/// otherwise.
pub fn log_filter(lookup: impl Fn(&str) -> Option<String>) -> String {
    // By reference, so the caller's closure need not be `Copy` — `&F` is itself `Fn` when `F`
    // is, which is what lets one lookup answer all three settings.
    let on = |name: &str| matches!(setting(name, &lookup), Some(v) if v == "1" || v == "true");
    if on("TRACE") {
        return "trace".to_string();
    }
    if on("DEBUG") {
        return "debug".to_string();
    }
    setting("LOG", &lookup).unwrap_or_else(|| "info".to_string())
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

/// Keyed by the *program* rather than the subcommand, because that is what the value names.
/// `usage powershell` runs `pwsh`, so its variable is `USAGECLI_SHELL_PWSH`.
///
/// The current spelling, since this is what error messages tell people to set. The legacy
/// `USAGE_SHELL_*` is still read — see [`setting`].
pub fn shell_var_name(shell: &str) -> String {
    setting_var_name(&shell_setting_name(shell))
}

fn shell_setting_name(shell: &str) -> String {
    format!("SHELL_{}", shell.to_ascii_uppercase())
}

/// The shell program to run in place of `shell`, if one was configured.
///
/// `None` means run `shell` as before. The value is a program path or a name to look up on
/// `PATH` — not a command line: shells on Windows live at paths like
/// `C:\Program Files\Git\bin\bash.exe`, and treating the value as a command line would make
/// usage responsible for quoting rules it has no reason to own. `Command` passes the program
/// and each argument separately, so a path with spaces needs no quoting.
///
/// An empty or blank value reads as unset, matching the `FOO= cmd` convention for switching
/// something off. Nothing checks that the program exists: the value need not be an absolute
/// path, so deciding would mean reimplementing `PATH`, `PATHEXT` and permission lookup, and
/// racing the spawn that follows. A bad value surfaces as a spawn error naming it.
///
/// `lookup` is injected rather than read from the environment so this stays testable without
/// mutating process-wide state — the same shape as `parse_partial_with_env` in usage-lib.
pub fn shell_program_override(
    shell: &str,
    lookup: impl Fn(&str) -> Option<String>,
) -> Option<String> {
    setting(&shell_setting_name(shell), lookup)
}

/// As [`shell_program_override`], and also the variable that named it.
pub fn shell_program_override_entry(
    shell: &str,
    lookup: impl Fn(&str) -> Option<String>,
) -> Option<(String, String)> {
    setting_entry(&shell_setting_name(shell), lookup)
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

    fn from(pairs: &[(&str, &str)]) -> impl Fn(&str) -> Option<String> + use<> {
        let pairs: Vec<(String, String)> = pairs
            .iter()
            .map(|(k, v)| ((*k).to_string(), (*v).to_string()))
            .collect();
        move |key| {
            pairs
                .iter()
                .find(|(k, _)| k == key)
                .map(|(_, v)| v.to_string())
        }
    }

    #[test]
    fn shell_var_names_cover_every_shell_subcommand() {
        // These are the four programs `Cli::run` dispatches to; `powershell` runs `pwsh`.
        assert_eq!(shell_var_name("bash"), "USAGECLI_SHELL_BASH");
        assert_eq!(shell_var_name("zsh"), "USAGECLI_SHELL_ZSH");
        assert_eq!(shell_var_name("fish"), "USAGECLI_SHELL_FISH");
        assert_eq!(shell_var_name("pwsh"), "USAGECLI_SHELL_PWSH");
    }

    #[test]
    fn the_prefix_is_one_mise_does_not_clear() {
        // mise clears a task's `usage_*` so its own parsed arguments cannot leak in, comparing
        // the first six characters case-insensitively. Everything this CLI wants to survive
        // that has to differ inside those six — which is the whole reason for the spelling.
        for shell in ["bash", "zsh", "fish", "pwsh"] {
            let name = shell_var_name(shell);
            assert!(
                !name[.."usage_".len()].eq_ignore_ascii_case("usage_"),
                "{name} would be cleared before a mise task ran"
            );
        }
    }

    #[test]
    fn unset_means_no_override() {
        assert_eq!(shell_program_override("bash", from(&[])), None);
    }

    #[test]
    fn a_blank_value_means_no_override() {
        for blank in ["", "   ", "\t"] {
            assert_eq!(
                shell_program_override("bash", from(&[("USAGE_SHELL_BASH", blank)])),
                None,
                "blank value {blank:?} should read as unset"
            );
        }
    }

    #[test]
    fn surrounding_whitespace_is_trimmed() {
        assert_eq!(
            shell_program_override("bash", from(&[("USAGE_SHELL_BASH", "  /usr/bin/bash  ")])),
            Some("/usr/bin/bash".to_string())
        );
    }

    #[test]
    fn a_path_with_spaces_survives_intact() {
        let path = r"C:\Program Files\Git\bin\bash.exe";
        assert_eq!(
            shell_program_override("bash", from(&[("USAGE_SHELL_BASH", path)])),
            Some(path.to_string())
        );
    }

    #[test]
    fn another_shells_variable_is_not_picked_up() {
        assert_eq!(
            shell_program_override("bash", from(&[("USAGE_SHELL_ZSH", "/bin/zsh")])),
            None
        );
        assert_eq!(
            shell_program_override("bash", from(&[("USAGECLI_SHELL_ZSH", "/bin/zsh")])),
            None
        );
    }

    // The tests above read the legacy name, which is the point: they were written before the
    // rename and still pass, so they are what says nothing that set it has broken.

    #[test]
    fn the_current_name_is_read() {
        assert_eq!(
            shell_program_override("bash", from(&[("USAGECLI_SHELL_BASH", "/usr/bin/bash")])),
            Some("/usr/bin/bash".to_string())
        );
    }

    #[test]
    fn the_current_name_wins_over_the_legacy_one() {
        assert_eq!(
            shell_program_override(
                "bash",
                from(&[
                    ("USAGECLI_SHELL_BASH", "/current"),
                    ("USAGE_SHELL_BASH", "/legacy"),
                ])
            ),
            Some("/current".to_string())
        );
    }

    #[test]
    fn a_blanked_current_name_falls_through_to_the_legacy_one() {
        // Blank reads as unset at each name rather than at the setting, so `USAGECLI_X= ` does
        // not switch off a legacy `USAGE_X` the way it switches off its own.
        assert_eq!(
            shell_program_override(
                "bash",
                from(&[
                    ("USAGECLI_SHELL_BASH", "  "),
                    ("USAGE_SHELL_BASH", "/legacy")
                ])
            ),
            Some("/legacy".to_string())
        );
    }

    #[test]
    fn the_log_filter_falls_back_through_both_spellings() {
        assert_eq!(log_filter(from(&[])), "info");
        assert_eq!(log_filter(from(&[("USAGE_LOG", "warn")])), "warn");
        assert_eq!(log_filter(from(&[("USAGECLI_LOG", "warn")])), "warn");
        // A blank current name is unset at that name, so the legacy one still answers. This is
        // what `Env::filter_or` could not do: its default applies only to an *unset* variable,
        // so a blank `USAGECLI_LOG` would have been taken as the filter itself.
        assert_eq!(
            log_filter(from(&[("USAGECLI_LOG", "  "), ("USAGE_LOG", "warn")])),
            "warn"
        );
    }

    #[test]
    fn the_log_filter_keeps_trace_over_debug_over_a_level() {
        assert_eq!(
            log_filter(from(&[("USAGE_DEBUG", "1"), ("USAGE_LOG", "warn")])),
            "debug"
        );
        assert_eq!(
            log_filter(from(&[("USAGECLI_TRACE", "true"), ("USAGE_DEBUG", "1")])),
            "trace"
        );
        // Only `1` and `true` switch it on; anything else is not a level request.
        assert_eq!(log_filter(from(&[("USAGECLI_DEBUG", "0")])), "info");
    }

    #[test]
    fn a_failed_lookup_reports_the_name_that_answered() {
        assert_eq!(
            setting_entry("SHELL_BASH", from(&[("USAGE_SHELL_BASH", "/legacy")])),
            Some(("USAGE_SHELL_BASH".to_string(), "/legacy".to_string()))
        );
        assert_eq!(
            setting_entry("SHELL_BASH", from(&[("USAGECLI_SHELL_BASH", "/current")])),
            Some(("USAGECLI_SHELL_BASH".to_string(), "/current".to_string()))
        );
    }

    #[test]
    fn settings_are_not_limited_to_shells() {
        // The same resolution serves USAGECLI_DEBUG, _TRACE and _LOG, which `main` reads.
        assert_eq!(
            setting("DEBUG", from(&[("USAGE_DEBUG", "1")])),
            Some("1".to_string())
        );
        assert_eq!(
            setting(
                "LOG",
                from(&[("USAGECLI_LOG", "trace"), ("USAGE_LOG", "debug")])
            ),
            Some("trace".to_string())
        );
        assert_eq!(setting("TRACE", from(&[])), None);
    }
}
