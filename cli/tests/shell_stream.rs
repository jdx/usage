//! `usage bash <(…)`: a script that arrives through a pipe rather than a file (#157).
//!
//! Reading the spec out of a pipe consumes it, so the shell used to be handed an empty stream
//! and exit 0 without running anything. `/dev/stdin` stands in for the process substitution:
//! it is the same kind of path, and needs no shell to set up.
#![cfg(unix)]

use assert_cmd::cargo;
use assert_cmd::Command;
use predicates::str::contains;

const SCRIPT: &str = "#!/usr/bin/env bash
#USAGE flag \"--foo\"
echo \"foo=$usage_foo\"
exit 3
";

fn usage_bash(args: &[&str]) -> Command {
    let mut cmd = Command::new(cargo::cargo_bin!("usage"));
    cmd.env_remove("USAGECLI_SHELL_BASH")
        .env_remove("USAGE_SHELL_BASH")
        .args(["bash", "/dev/stdin"])
        .args(args);
    cmd
}

#[test]
fn a_piped_script_runs_with_its_spec() {
    usage_bash(&["--foo"])
        .write_stdin(SCRIPT)
        .assert()
        .code(3)
        .stdout(contains("foo=true"));
}

#[test]
fn a_piped_script_prints_its_help() {
    usage_bash(&["--help"])
        .write_stdin(SCRIPT)
        .assert()
        .success()
        .stdout(contains("--foo"));
}
