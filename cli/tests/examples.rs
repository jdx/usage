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
