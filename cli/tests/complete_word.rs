use assert_cmd::assert::Assert;
use std::env;
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
fn complete_word_kitchen_sink() {
    assert_cmd("kitchen-sink.usage.kdl", &["--", "install", "--"])
        .stdout("--dir\n--force\n--global\n--no-force\n");
    assert_cmd("kitchen-sink.usage.kdl", &["--", "--shell", ""]).stdout("bash\nzsh\nfish\n");
}

#[test]
fn complete_word_choices() {
    assert_cmd("mise.usage.kdl", &["--", "env", "--shell", ""])
        .stdout("bash\nfish\nnu\nxonsh\nzsh\n");
}

#[test]
fn complete_word_shebang() {
    assert_cmd("example.sh", &["--", "-"]).stdout("--bar\n--defaulted\n--foo\n");
}

#[test]
fn complete_word_mounted() {
    let mut path = env::split_paths(&env::var("PATH").unwrap()).collect::<Vec<_>>();
    path.insert(
        0,
        env::current_dir()
            .unwrap()
            .join("..")
            .join("target")
            .join("debug"),
    );
    path.insert(0, env::current_dir().unwrap().join("..").join("examples"));
    env::set_var("PATH", env::join_paths(path).unwrap());
    assert_cmd("mounted.sh", &["--", "-"]).stdout("--mount\n");
    assert_cmd("mounted.sh", &["--", ""]).stdout("exec-task\n");
    assert_cmd("mounted.sh", &["--", "exec-task", ""]).stdout("task-a\ntask-b\n");
}

fn cmd(example: &str) -> Command {
    let mut cmd = Command::cargo_bin("usage").unwrap();
    cmd.args(["cw", "-f", &format!("../examples/{example}"), "mycli"]);
    cmd
}

fn assert_cmd(example: &str, args: &[&str]) -> Assert {
    cmd(example).args(args).assert().success()
}
