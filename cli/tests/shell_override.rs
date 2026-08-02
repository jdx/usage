//! `USAGE_SHELL_<SHELL>` replaces the program `usage <shell>` runs.
//!
//! Unix-only because they pin the substitute to a real path (`/bin/echo`, `/bin/sh`). The
//! variable itself is not Unix-specific — it exists for Windows, where `bash` resolves to the
//! WSL launcher ahead of `PATH` — but the CI that runs these is Linux.

#![cfg(unix)]

use assert_cmd::cargo;
use assert_cmd::prelude::*;
use predicates::prelude::*;
use predicates::str::contains;
use std::process::Command;

const SCRIPT: &str = "../examples/test-new-usage-syntax.sh";

/// `usage bash <script> --foo`, with every `USAGE_SHELL_*` this test cares about cleared so a
/// developer's own environment cannot change the result.
fn usage_bash() -> Command {
    let mut cmd = Command::new(cargo::cargo_bin!("usage"));
    cmd.env_remove("USAGE_SHELL_BASH")
        .env_remove("USAGE_SHELL_ZSH")
        .args(["bash", SCRIPT, "--foo"]);
    cmd
}

#[test]
fn the_override_replaces_the_program() {
    // `/bin/echo` prints the argv it was handed, which shows both that a different program ran
    // and that the script path and arguments reached it untouched.
    usage_bash()
        .env("USAGE_SHELL_BASH", "/bin/echo")
        .assert()
        .success()
        .stdout(contains(SCRIPT))
        .stdout(contains("foo: true").not());
}

#[test]
fn a_blank_override_runs_the_default_shell() {
    usage_bash()
        .env("USAGE_SHELL_BASH", "")
        .assert()
        .success()
        .stdout(contains("foo: true"));
}

#[test]
fn the_override_may_name_a_program_on_path() {
    // The value does not have to be an absolute path — a name resolved through PATH works
    // too, which is what the docs promise. `bash` names the same shell that would have run
    // anyway, so the script's output must be unchanged.
    //
    // Not a different shell: every fixture here uses `set -o pipefail`, so a stand-in has to
    // be one that understands it.
    let expected = usage_bash().output().unwrap().stdout;
    usage_bash()
        .env("USAGE_SHELL_BASH", "bash")
        .assert()
        .success()
        .stdout(String::from_utf8(expected).unwrap());
}

#[test]
fn a_program_that_cannot_be_started_names_itself_and_the_variable() {
    usage_bash()
        .env("USAGE_SHELL_BASH", "/nonexistent/bash-xyz")
        .assert()
        .failure()
        .stderr(contains("/nonexistent/bash-xyz"))
        .stderr(contains("USAGE_SHELL_BASH"));
}

#[test]
fn another_shells_variable_is_ignored() {
    usage_bash()
        .env("USAGE_SHELL_ZSH", "/bin/echo")
        .assert()
        .success()
        .stdout(contains("foo: true"));
}
