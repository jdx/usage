//! `usage diff` as a command: what it prints, what it exits with, and what it reads.
//!
//! The classification rules are unit-tested beside the comparison itself. What is
//! only observable from outside is here: the exit status a release job gates on, the
//! two output formats, and reading one of the two specs from stdin.

use assert_cmd::Command;
use predicates::prelude::PredicateBooleanExt;
use predicates::str::contains;
use std::path::{Path, PathBuf};

const OLD: &str = r#"
name "ex"
bin "ex"
version "1.0.0"
flag "-j --jobs <n>" help="how many at once"
flag "-f --force" help="force it"
arg "<file>" help="the file"
cmd "run" help="run it"
"#;

const NEW: &str = r#"
name "ex"
bin "ex"
version "2.0.0"
flag "--jobs <n>" help="how many at once"
flag "-f --force" help="force it"
flag "--quiet" help="be quiet"
arg "<file>" help="the file"
cmd "run" help="run it"
"#;

fn usage_cmd() -> Command {
    // `assert_cmd::Command` rather than the standard one: two of these cases feed a
    // spec on stdin, which is what `write_stdin` is for.
    Command::new(assert_cmd::cargo::cargo_bin!("usage"))
}

/// A directory of this test's own, named for the case, so parallel tests cannot
/// read each other's fixtures.
fn fixtures(case: &str) -> PathBuf {
    let dir = std::env::temp_dir().join(format!("usage_diff_{case}_{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&dir);
    std::fs::create_dir_all(&dir).unwrap();
    std::fs::write(dir.join("old.usage.kdl"), OLD).unwrap();
    std::fs::write(dir.join("new.usage.kdl"), NEW).unwrap();
    dir
}

fn old(dir: &Path) -> PathBuf {
    dir.join("old.usage.kdl")
}

fn new(dir: &Path) -> PathBuf {
    dir.join("new.usage.kdl")
}

#[test]
fn a_breaking_change_exits_one_so_a_release_job_can_gate_on_it() {
    let dir = fixtures("gate");
    usage_cmd()
        .arg("diff")
        .arg(old(&dir))
        .arg(new(&dir))
        .assert()
        .code(1)
        .stdout(contains(
            "breaking [flag-spelling-removed] at ex: flag '--jobs' no longer answers to '-j'",
        ))
        .stdout(contains(
            "compatible [flag-added] at ex: flag '--quiet' was added",
        ))
        .stdout(contains(
            "Found 1 breaking, 1 compatible, 0 metadata change(s)",
        ));
    std::fs::remove_dir_all(&dir).unwrap();
}

#[test]
fn exit_zero_reports_without_failing() {
    let dir = fixtures("exit_zero");
    usage_cmd()
        .arg("diff")
        .arg(old(&dir))
        .arg(new(&dir))
        .arg("--exit-zero")
        .assert()
        .success()
        .stdout(contains("breaking [flag-spelling-removed]"));
    std::fs::remove_dir_all(&dir).unwrap();
}

#[test]
fn identical_specs_are_silent_and_succeed() {
    let dir = fixtures("identical");
    usage_cmd()
        .arg("diff")
        .arg(old(&dir))
        .arg(old(&dir))
        .assert()
        .success()
        .stdout(contains("No interface changes."));
    std::fs::remove_dir_all(&dir).unwrap();
}

#[test]
fn breaking_only_hides_the_rest() {
    let dir = fixtures("breaking_only");
    usage_cmd()
        .arg("diff")
        .arg(old(&dir))
        .arg(new(&dir))
        .arg("--breaking")
        .assert()
        .code(1)
        .stdout(contains("breaking [flag-spelling-removed]"))
        .stdout(contains("flag-added").not());
    std::fs::remove_dir_all(&dir).unwrap();
}

#[test]
fn json_is_a_list_a_program_can_read() {
    let dir = fixtures("json");
    let output = usage_cmd()
        .arg("diff")
        .arg(old(&dir))
        .arg(new(&dir))
        .args(["--format", "json"])
        .output()
        .unwrap();
    let changes: serde_json::Value = serde_json::from_slice(&output.stdout).unwrap();
    let changes = changes.as_array().unwrap();
    assert_eq!(changes.len(), 2);
    assert_eq!(changes[0]["category"], "breaking");
    assert_eq!(changes[0]["code"], "flag-spelling-removed");
    assert_eq!(changes[0]["location"], "ex");
    assert_eq!(changes[1]["category"], "compatible");
    std::fs::remove_dir_all(&dir).unwrap();
}

#[test]
fn the_new_spec_can_come_from_stdin() {
    // The shape a release job wants: the released spec on disk against what the
    // binary being built says about itself, with no temporary file in between.
    let dir = fixtures("stdin");
    usage_cmd()
        .arg("diff")
        .arg(old(&dir))
        .arg("-")
        .write_stdin(NEW)
        .assert()
        .code(1)
        .stdout(contains("breaking [flag-spelling-removed]"));
    std::fs::remove_dir_all(&dir).unwrap();
}

#[test]
fn both_specs_cannot_be_stdin() {
    usage_cmd()
        .args(["diff", "-", "-"])
        .write_stdin(NEW)
        .assert()
        .failure()
        .stderr(contains("only one of the two specs can be read from stdin"));
}
