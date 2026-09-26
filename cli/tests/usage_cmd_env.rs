//! `usage_cmd`: the subcommand path a script is run with.
#![cfg(unix)]

use assert_cmd::cargo;
use assert_cmd::Command;

const SCRIPT: &str = r#"#!/usr/bin/env -S usage bash
#USAGE cmd "db" {
#USAGE   cmd "migrate"
#USAGE }
echo "cmd=[${usage_cmd-unset}]"
"#;

fn run(args: &[&str]) -> Command {
    let mut cmd = Command::new(cargo::cargo_bin!("usage"));
    cmd.env_remove("USAGECLI_SHELL_BASH")
        .env_remove("USAGE_SHELL_BASH")
        .args(["bash", "/dev/stdin"])
        .args(args)
        .write_stdin(SCRIPT);
    cmd
}

#[test]
fn the_chosen_subcommand_path_reaches_the_script() {
    run(&["db", "migrate"])
        .assert()
        .success()
        .stdout("cmd=[db migrate]\n");
}

/// A script run from another usage script must not dispatch on its caller's subcommand.
#[test]
fn an_inherited_usage_cmd_is_cleared_when_none_is_chosen() {
    run(&[])
        .env("usage_cmd", "deploy")
        .assert()
        .success()
        .stdout("cmd=[unset]\n");
}
