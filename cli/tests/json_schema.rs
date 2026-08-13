//! `usage g json-schema` as a command: where it writes, and when it refuses.

use assert_cmd::prelude::*;
use predicates::str::contains;
use std::process::Command;

fn usage_cmd() -> Command {
    Command::new(assert_cmd::cargo::cargo_bin!("usage"))
}

fn schema_of(spec: &str) -> assert_cmd::assert::Assert {
    usage_cmd()
        .args(["generate", "json-schema", "--spec", spec])
        .assert()
}

#[test]
fn a_dash_means_stdout_rather_than_a_file_called_dash() {
    // The convention every sibling generator follows, via the shared writer. Writing a file
    // named `-` into whatever directory the user happened to be in is the kind of thing
    // nobody notices until they find it in `git status`.
    let dir = std::env::temp_dir().join(format!("usage_schema_dash_{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&dir);
    std::fs::create_dir_all(&dir).unwrap();

    usage_cmd()
        .current_dir(&dir)
        .args([
            "generate",
            "json-schema",
            "--spec",
            "name \"x\"\nbin \"x\"\nconfig {\n  prop \"jobs\" type=\"uint\"\n}\n",
            "--out-file",
            "-",
        ])
        .assert()
        .success()
        .stdout(contains("\"jobs\""));
    assert!(
        !dir.join("-").exists(),
        "a file named `-` was written instead of using stdout"
    );

    std::fs::remove_dir_all(&dir).unwrap();
}

#[test]
fn a_spec_with_nothing_a_file_can_hold_says_so() {
    // An empty `properties` next to `unevaluatedProperties: false` is a schema that rejects
    // every config file in existence. Refusing to write one is the only honest answer.
    schema_of("name \"x\"\nbin \"x\"\n")
        .failure()
        .stderr(contains("nothing a config file can hold"));

    // The case the props-only check missed: settings are declared, and every one of them is
    // `scope="env"`, so none may appear in a file.
    schema_of(
        "name \"x\"\nbin \"x\"\nconfig {\n  prop \"config_file\" type=\"path\" scope=\"env\"\n}\n",
    )
    .failure()
    .stderr(contains("nothing a config file can hold"));

    // And one settable key is enough to have something to describe.
    schema_of("name \"x\"\nbin \"x\"\nconfig {\n  prop \"jobs\" type=\"uint\"\n}\n")
        .success()
        .stdout(contains("\"jobs\""));
}
