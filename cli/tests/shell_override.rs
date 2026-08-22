//! `USAGECLI_SHELL_<SHELL>` replaces the program `usage <shell>` runs, and the legacy
//! `USAGE_SHELL_<SHELL>` still does.
//!
//! Runs on every platform, Windows above all: that is where `bash` resolves to the WSL
//! launcher ahead of `PATH`, which is the reason the variable exists. Nothing here assumes the
//! default shell works, because on the platform this feature is for, it may well not.

use assert_cmd::cargo;
use assert_cmd::prelude::*;
use predicates::str::contains;
use std::process::{Command, Output};

const SCRIPT: &str = "../examples/test-new-usage-syntax.sh";

/// A program that exists wherever these tests run and is not a shell.
///
/// Cargo sets `$CARGO` for the processes it spawns, so this is the one executable a test can
/// name without knowing the platform — `/bin/echo` has no Windows counterpart. Handed a script
/// path it does not recognize, cargo repeats it back (`no such subcommand \`…\``), which is
/// what makes the substitution observable.
///
/// Not the `usage` binary itself: its top level has a positional argument, so a path lands
/// there and never reaches the error message.
fn substitute_program() -> String {
    std::env::var("CARGO").expect("cargo sets $CARGO for the processes it spawns")
}

/// `usage bash <script> --foo`, with every shell override cleared under both spellings so a
/// developer's own environment cannot change the result.
fn usage_bash() -> Command {
    let mut cmd = Command::new(cargo::cargo_bin!("usage"));
    cmd.env_remove("USAGECLI_SHELL_BASH")
        .env_remove("USAGECLI_SHELL_ZSH")
        .env_remove("USAGECLI_SHELL_FISH")
        .env_remove("USAGECLI_SHELL_PWSH")
        .env_remove("USAGE_SHELL_BASH")
        .env_remove("USAGE_SHELL_ZSH")
        .env_remove("USAGE_SHELL_FISH")
        .env_remove("USAGE_SHELL_PWSH")
        .args(["bash", SCRIPT, "--foo"]);
    cmd
}

/// What the same invocation does with nothing overridden.
///
/// The fallback tests compare against this rather than asserting the script's output, because
/// whether the default `bash` can run the script is a property of the machine — on Windows it
/// depends on whether one is installed ahead of the WSL launcher. "Same as unset" is the
/// property those tests are actually about, and it holds either way.
fn baseline() -> Output {
    usage_bash().output().unwrap()
}

#[test]
fn the_override_replaces_the_program() {
    let out = usage_bash()
        .env("USAGE_SHELL_BASH", substitute_program())
        .output()
        .unwrap();

    // The substitute names the script back, so argv reached it untouched...
    let stderr = String::from_utf8_lossy(&out.stderr);
    assert!(stderr.contains(SCRIPT), "{stderr}");
    // ...and the script itself never ran.
    let stdout = String::from_utf8_lossy(&out.stdout);
    assert!(!stdout.contains("foo: true"), "{stdout}");
}

#[test]
fn a_blank_override_runs_the_default_shell() {
    let base = baseline();
    for blank in ["", "   ", "\t"] {
        let out = usage_bash()
            .env("USAGE_SHELL_BASH", blank)
            .output()
            .unwrap();
        assert_eq!(out.status.code(), base.status.code(), "blank {blank:?}");
        assert_eq!(out.stdout, base.stdout, "blank {blank:?}");
        assert_eq!(out.stderr, base.stderr, "blank {blank:?}");
    }
}

#[test]
fn the_override_may_name_a_program_on_path() {
    // The value does not have to be an absolute path — a name resolved through PATH works too,
    // which is what the docs promise. `bash` names the same shell that would have run anyway,
    // so the outcome must be unchanged.
    //
    // Not a different shell: every fixture here uses `set -o pipefail`, so a stand-in has to be
    // one that understands it.
    //
    // stderr is left out of the comparison on purpose. Naming a program makes the run an
    // overridden one, and `wsl_path_hint` only fires when nothing was overridden — so on a
    // Windows machine where the default `bash` fails, the two runs differ by that hint alone.
    let base = baseline();
    let out = usage_bash()
        .env("USAGE_SHELL_BASH", "bash")
        .output()
        .unwrap();

    assert_eq!(out.status.code(), base.status.code());
    assert_eq!(out.stdout, base.stdout);
}

#[test]
fn a_program_that_cannot_be_started_names_itself_and_the_variable() {
    // A bare name rather than a path, so it fails the same way on every platform: nothing
    // resolves it on PATH, and no filesystem layout can accidentally make it exist.
    //
    // The variable named back is the one that was set, not whichever spelling is current — a
    // message pointing at USAGECLI_SHELL_BASH would send whoever set the legacy name looking
    // at a variable they never touched.
    usage_bash()
        .env("USAGE_SHELL_BASH", "usage-no-such-shell-xyz")
        .assert()
        .failure()
        .stderr(contains("usage-no-such-shell-xyz"))
        .stderr(contains("USAGE_SHELL_BASH"));
}

// The tests above set the legacy name, which is the point: they were written before the rename
// and still pass, so they are what says nothing that set it has broken. Below is the same
// ground under the current one.

#[test]
fn the_current_name_replaces_the_program() {
    let out = usage_bash()
        .env("USAGECLI_SHELL_BASH", substitute_program())
        .output()
        .unwrap();

    let stderr = String::from_utf8_lossy(&out.stderr);
    assert!(stderr.contains(SCRIPT), "{stderr}");
    let stdout = String::from_utf8_lossy(&out.stdout);
    assert!(!stdout.contains("foo: true"), "{stdout}");
}

#[test]
fn the_current_name_wins_over_the_legacy_one() {
    // Both set, and only the current one is read — so the run looks exactly like the one where
    // the legacy name was not there at all.
    let expected = usage_bash()
        .env("USAGECLI_SHELL_BASH", substitute_program())
        .output()
        .unwrap();
    let out = usage_bash()
        .env("USAGECLI_SHELL_BASH", substitute_program())
        .env("USAGE_SHELL_BASH", "usage-no-such-shell-xyz")
        .output()
        .unwrap();

    assert_eq!(out.status.code(), expected.status.code());
    assert_eq!(out.stdout, expected.stdout);
}

#[test]
fn a_failure_under_the_current_name_names_it() {
    usage_bash()
        .env("USAGECLI_SHELL_BASH", "usage-no-such-shell-xyz")
        .assert()
        .failure()
        .stderr(contains("USAGECLI_SHELL_BASH"));
}

#[test]
fn another_shells_variable_is_ignored() {
    let base = baseline();
    let out = usage_bash()
        .env("USAGE_SHELL_ZSH", substitute_program())
        .output()
        .unwrap();

    assert_eq!(out.status.code(), base.status.code());
    assert_eq!(out.stdout, base.stdout);
    assert_eq!(out.stderr, base.stderr);
}
