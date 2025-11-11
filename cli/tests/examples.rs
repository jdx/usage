use assert_cmd::cargo;
use assert_cmd::prelude::*;
use predicates::str::contains;
use std::{env, process::Command};

/// Test that examples/test-empty-defaults.sh runs successfully and demonstrates
/// the correct behavior of default="" vs no default
#[test]
fn test_empty_defaults_example() {
    // Set up PATH to include the usage binary
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

    let mut cmd = Command::new("../examples/test-empty-defaults.sh");
    cmd.env("PATH", env::join_paths(path).unwrap());

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
