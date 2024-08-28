use assert_cmd::assert::Assert;
use std::process::Command;

use assert_cmd::prelude::*;
use predicates::prelude::*;
use predicates::str::contains;

#[test]
fn complete_word_completer() {
    assert_cmd("basic.usage.kdl", &["plugins", "install", "pl"])
        .stdout("plugin-1\nplugin-2\nplugin-3\n");
}

#[test]
fn complete_word_subcommands() {
    assert_cmd("basic.usage.kdl", &["plugins", "install"]).stdout(contains("install"));
}

#[test]
fn complete_word_cword() {
    assert_cmd("basic.usage.kdl", &["--cword=3", "plugins", "install"])
        .stdout(contains("plugin-2"));
}

#[test]
fn complete_word_long_flag() {
    assert_cmd("basic.usage.kdl", &["--", "plugins", "install", "--"]).stdout("--dir\n--global\n");
    assert_cmd("basic.usage.kdl", &["--", "plugins", "install", "--g"]).stdout("--global\n");
    assert_cmd(
        "basic.usage.kdl",
        &["--", "plugins", "install", "--global", "pl"],
    )
    .stdout(contains("plugin-2"));
}

#[test]
fn complete_word_long_flag_val() {
    assert_cmd(
        "basic.usage.kdl",
        &["--", "plugins", "install", "--dir", ""],
    )
    .stdout(contains("src").and(contains("tests")));
}

#[test]
fn complete_word_short_flag() {
    assert_cmd("basic.usage.kdl", &["--", "plugins", "install", "-"])
        .stdout("-d\n-g\n--dir\n--global\n");
    assert_cmd("basic.usage.kdl", &["--", "plugins", "install", "-g"]).stdout("-g\n");
    assert_cmd("basic.usage.kdl", &["--", "plugins", "install", "-g", "pl"])
        .stdout(contains("plugin-2"));
}

#[test]
fn complete_word_shebang() {
    assert_cmd("example.sh", &["--", "-"]).stdout("--bar\n--foo\n");
}

fn cmd(example: &str) -> Command {
    let mut cmd = Command::cargo_bin("usage").unwrap();
    cmd.args(["cw", "-f", &format!("../examples/{example}"), "mycli"]);
    cmd
}

fn assert_cmd(example: &str, args: &[&str]) -> Assert {
    cmd(example).args(args).assert().success()
}
