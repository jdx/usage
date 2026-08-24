use assert_cmd::cargo;
use assert_cmd::prelude::*;
use predicates::prelude::PredicateBooleanExt;
use predicates::str::contains;
use std::process::Command;

/// Test that examples/test-empty-defaults.sh runs successfully and demonstrates
/// the correct behavior of default="" vs no default
#[test]
fn test_empty_defaults_example() {
    // Through `usage bash`, like every other test here, rather than spawning the script and
    // letting its `#!/usr/bin/env -S usage bash` shebang do the work. Windows cannot execute a
    // `.sh` at all — the spawn fails with os error 193 before the shebang is ever read — and
    // going through the binary also drops the PATH juggling that let the shebang find `usage`.
    let mut cmd = Command::new(cargo::cargo_bin!("usage"));
    cmd.args(["bash", "../examples/test-empty-defaults.sh"]);

    let assert = cmd.assert().success();

    // Verify key behaviors are demonstrated
    assert
        .stdout(contains("1. Flag with default=\"\":"))
        .stdout(contains("SET ✓"))
        .stdout(contains("7. Optional arg with no default:"))
        .stdout(contains("UNSET ✓"))
        // Verify parameter expansion tests
        .stdout(contains("8. Parameter expansion with default=\"\" var:"))
        .stdout(contains("${var:-fallback}  = 'fallback'"))
        .stdout(contains("${var-fallback}   = ''"))
        // Verify error tests pass
        .stdout(contains(
            "✓ Error thrown (expected, because value is empty string)",
        ))
        .stdout(contains(
            "✓ No error (expected, because var IS set even though empty)",
        ))
        // Verify summary is shown
        .stdout(contains("=== Summary ==="))
        .stdout(contains(
            "default=\"\"      → Variable IS SET to empty string",
        ))
        .stdout(contains("No default (opt) → Variable is UNSET"));
}

/// Test that the new # [USAGE] syntax works correctly
#[test]
fn test_new_usage_syntax_with_space() {
    let mut cmd = Command::new(cargo::cargo_bin!("usage"));
    cmd.args(["bash", "../examples/test-new-usage-syntax.sh", "--help"]);

    cmd.assert()
        .success()
        .stdout(contains("Usage: test-new-syntax"))
        .stdout(contains("--foo"))
        .stdout(contains("Flag value"))
        .stdout(contains("--bar <bar>"))
        .stdout(contains("Option value"))
        .stdout(contains("baz"))
        .stdout(contains("Positional value"));
}

#[test]
fn shell_help_honours_terminal_colour_environment() {
    let mut coloured = Command::new(cargo::cargo_bin!("usage"));
    coloured
        .env("NO_COLOR", "")
        .env("CLICOLOR_FORCE", "1")
        .args(["bash", "../examples/test-new-usage-syntax.sh", "--help"]);
    coloured
        .assert()
        .success()
        .stdout(contains("\u{1b}[1;33mUsage:\u{1b}[0m"))
        .stdout(contains("\u{1b}[1;32m--foo\u{1b}[0m"));

    let mut plain = Command::new(cargo::cargo_bin!("usage"));
    plain.env("NO_COLOR", "1").env("CLICOLOR_FORCE", "1").args([
        "bash",
        "../examples/test-new-usage-syntax.sh",
        "--help",
    ]);
    plain.assert().success().stdout(contains("\u{1b}[").not());
}

#[test]
fn usage_own_help_uses_the_process_colour_policy() {
    let mut cmd = Command::new(cargo::cargo_bin!("usage"));
    cmd.env("NO_COLOR", "")
        .env("CLICOLOR_FORCE", "1")
        .arg("--help");
    cmd.assert()
        .success()
        .stdout(contains("\u{1b}[1;33mUsage:\u{1b}[0m"));
}

/// Test that the #[USAGE] syntax (no space) works correctly
#[test]
fn test_new_usage_syntax_no_space() {
    let mut cmd = Command::new(cargo::cargo_bin!("usage"));
    cmd.args([
        "bash",
        "../examples/test-usage-bracket-no-space.sh",
        "--help",
    ]);

    cmd.assert()
        .success()
        .stdout(contains("Usage: test-bracket-no-space"))
        .stdout(contains("--verbose"))
        .stdout(contains("Verbose output"))
        .stdout(contains("--output <file>"))
        .stdout(contains("Output file"))
        .stdout(contains("input"))
        .stdout(contains("Input file"));
}

/// Test that the new syntax actually parses and executes correctly
#[test]
fn test_new_usage_syntax_execution() {
    let mut cmd = Command::new(cargo::cargo_bin!("usage"));
    cmd.args([
        "bash",
        "../examples/test-new-usage-syntax.sh",
        "--foo",
        "--bar",
        "test123",
        "myvalue",
    ]);

    cmd.assert()
        .success()
        .stdout(contains("foo: true"))
        .stdout(contains("bar: test123"))
        .stdout(contains("baz: myvalue"));
}

/// Test that the new // [USAGE] syntax works correctly for non-shell script
#[test]
fn test_usage_double_slash_execution() {
    let mut cmd = Command::new(cargo::cargo_bin!("usage"));
    cmd.args([
        "exec",
        "node",
        "../examples/test-usage-double-slash.js",
        "--debug",
        "mycmd",
    ]);

    cmd.assert()
        .success()
        .stdout(contains("debug: true"))
        .stdout(contains("port: 3000"))
        .stdout(contains("command: mycmd"));
}

/// Test that the old //USAGE syntax (no space, no brackets) works correctly for non-shell script
#[test]
fn test_usage_double_slash_execution_old() {
    let mut cmd = Command::new(cargo::cargo_bin!("usage"));
    cmd.args([
        "exec",
        "node",
        "../examples/test-usage-double-slash-old.js",
        "--debug",
        "mycmd",
    ]);

    cmd.assert()
        .success()
        .stdout(contains("debug: true"))
        .stdout(contains("port: 3000"))
        .stdout(contains("command: mycmd"));
}

/// Test that blank comment lines in USAGE blocks don't stop parsing
#[test]
fn test_blank_comment_lines_in_usage() {
    let mut cmd = Command::new(cargo::cargo_bin!("usage"));
    cmd.args(["bash", "../examples/test-blank-comment-lines.sh", "--help"]);

    cmd.assert()
        .success()
        .stdout(contains("Usage: test-blank-lines"))
        .stdout(contains("workspace"))
        .stdout(contains("Workspace name"))
        .stdout(contains("-r --region <region>"))
        .stdout(contains("AWS region"))
        .stdout(contains("-t --tail"))
        .stdout(contains("Follow logs in real-time"));
}

/// Test that blank comment lines don't prevent flag parsing
#[test]
fn test_blank_comment_lines_execution() {
    let mut cmd = Command::new(cargo::cargo_bin!("usage"));
    cmd.args([
        "bash",
        "../examples/test-blank-comment-lines.sh",
        "my-workspace",
        "--tail",
    ]);

    cmd.assert()
        .success()
        .stdout(contains("workspace: my-workspace"))
        .stdout(contains("region: us-west-2"))
        .stdout(contains("tail: true"));
}

/// Test defaults work with blank comment lines
#[test]
fn test_blank_comment_lines_defaults() {
    let mut cmd = Command::new(cargo::cargo_bin!("usage"));
    cmd.args(["bash", "../examples/test-blank-comment-lines.sh"]);

    cmd.assert()
        .success()
        .stdout(contains("workspace: default-ws"))
        .stdout(contains("region: us-west-2"))
        .stdout(contains("tail: \n"));
}

/// Test that exec command properly handles --help flag for non-shell scripts
#[test]
fn test_exec_help() {
    let mut cmd = Command::new(cargo::cargo_bin!("usage"));
    cmd.args(["exec", "python3", "../examples/test-exec-help.py", "--help"]);

    cmd.assert()
        .success()
        .stdout(contains("Usage: test-exec-help"))
        .stdout(contains("-f --force"))
        .stdout(contains("Force the operation"))
        .stdout(contains("-v --verbose"))
        .stdout(contains("Enable verbose output"))
        .stdout(contains("<file>"))
        .stdout(contains("File to process"));
}
