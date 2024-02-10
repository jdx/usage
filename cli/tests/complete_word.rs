use std::process::Command;

use assert_cmd::prelude::*;
use predicates::prelude::*;

#[test]
fn complete() {
    cmd()
        .args(["plugins", "install", "pl"])
        .assert()
        .success()
        .stdout(predicate::str::contains("plugin-2"));
}

fn cmd() -> Command {
    let mut cmd = Command::cargo_bin("usage").unwrap();
    cmd.args(["cw", "-f", "../examples/basic.usage.kdl", "--"]);
    cmd
}
