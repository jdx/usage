use assert_cmd::assert::Assert;
use std::process::Command;

use assert_cmd::prelude::*;
use predicates::prelude::*;

#[test]
fn complete_word_completer() {
    assert_cmd(&["plugins", "install", "pl"]).stdout("plugin-1\nplugin-2\nplugin-3\n");
}

#[test]
fn complete_word_subcommands() {
    assert_cmd(&["plugins", "install"]).stdout(predicate::str::contains("install"));
}

#[test]
fn complete_word_cword() {
    assert_cmd(&["--cword=2", "plugins", "install"]).stdout(predicate::str::contains("plugin-2"));
}

#[test]
fn complete_word_long_flag() {
    assert_cmd(&["--", "plugins", "install", "--global", "pl"])
        .stdout(predicate::str::contains("plugin-2"));
}

#[test]
fn complete_word_short_flag() {
    assert_cmd(&["--", "plugins", "install", "-g", "pl"])
        .stdout(predicate::str::contains("plugin-2"));
}

fn cmd() -> Command {
    let mut cmd = Command::cargo_bin("usage").unwrap();
    cmd.args(["cw", "-f", "../examples/basic.usage.kdl"]);
    cmd
}

fn assert_cmd(args: &[&str]) -> Assert {
    cmd().args(args).assert().success()
}
