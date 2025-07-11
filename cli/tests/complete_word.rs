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
    assert_cmd("basic.usage.kdl", &["plugins", "install_desc", "pl"]).stdout(
        r#"'plugin-1'\:'desc'
'plugin-2'\:'desc'
'plugin-3'\:'desc'
"#,
    );
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
        .stdout("bash\nelvish\nfish\nnu\nxonsh\nzsh\npwsh\n");
}

#[test]
fn complete_word_shebang() {
    assert_cmd("example.sh", &["--", "-"]).stdout(
        r#"'--bar'\:'Option value'
'--defaulted'\:'Defaulted value'
'--foo'\:'Flag value'
"#,
    );
}

#[test]
fn complete_word_arg_completer() {
    assert_cmd("example.sh", &["--", "v"]).stdout("val-1\nval-2\nval-3\n");
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
    assert_cmd("mounted.sh", &["--", "-"])
        .stdout("\'--mount\'\\:\'Display kdl spec for mounted tasks\'\n");
    assert_cmd("mounted.sh", &["--", ""]).stdout("exec-task\n");
    assert_cmd("mounted.sh", &["--", "exec-task", ""]).stdout("task-a\ntask-b\n");
}

#[test]
fn complete_word_fallback_to_files() {
    // Use a minimal spec with no args or subcommands, so any argument is unknown
    let mut cmd = Command::cargo_bin("usage").unwrap();
    cmd.args([
        "cw",
        "--shell",
        "zsh",
        "-f",
        "../examples/basic.usage.kdl",
        "mycli",
        "plugins",
        "install",
        "foo",
        "",
    ]);
    // Assert for files always present in the project root
    cmd.assert()
        .success()
        .stdout(contains("Cargo.toml").and(contains("src")));
}

fn cmd(example: &str, shell: &str) -> Command {
    let mut cmd = Command::cargo_bin("usage").unwrap();
    cmd.args([
        "cw",
        "--shell",
        shell,
        "-f",
        &format!("../examples/{example}"),
        "mycli",
    ]);
    cmd
}

fn assert_cmd(example: &str, args: &[&str]) -> Assert {
    cmd(example, "zsh").args(args).assert().success()
}
