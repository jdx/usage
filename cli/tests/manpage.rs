use assert_cmd::prelude::*;
use predicates::str::contains;
use std::process::Command;

fn usage_cmd() -> Command {
    Command::new(assert_cmd::cargo::cargo_bin!("usage"))
}

fn example_path(name: &str) -> String {
    std::path::PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .parent()
        .unwrap()
        .join("examples")
        .join(name)
        .to_str()
        .unwrap()
        .to_string()
}

#[test]
fn test_generate_basic_manpage() {
    let mut cmd = usage_cmd();
    cmd.args([
        "generate",
        "manpage",
        "-f",
        &example_path("basic.usage.kdl"),
    ]);

    let assert = cmd.assert().success();

    // Check for standard man page sections
    assert
        .stdout(contains(".TH"))
        .stdout(contains(".SH NAME"))
        .stdout(contains(".SH SYNOPSIS"))
        .stdout(contains(".SH DESCRIPTION"))
        .stdout(contains(".SH COMMANDS"));
}

#[test]
fn test_generate_manpage_with_section() {
    let mut cmd = usage_cmd();
    cmd.args([
        "generate",
        "manpage",
        "-f",
        &example_path("basic.usage.kdl"),
        "--section",
        "7",
    ]);

    let assert = cmd.assert().success();

    // Check that section 7 is specified (the name comes from filename since spec has no name)
    assert.stdout(contains(" 7"));
}

#[test]
fn test_generate_manpage_with_flags() {
    let mut cmd = usage_cmd();
    cmd.args([
        "generate",
        "manpage",
        "-f",
        &example_path("basic.usage.kdl"),
    ]);

    let assert = cmd.assert().success();

    // Check for commands section (basic.usage.kdl has subcommands, not top-level flags)
    assert
        .stdout(contains(".SH COMMANDS"))
        .stdout(contains("plugins"));
}

#[test]
fn test_generate_manpage_output_to_file() {
    let temp_dir = std::env::temp_dir();
    let out_file = temp_dir.join("test-manpage.1");

    // Clean up any existing file
    let _ = std::fs::remove_file(&out_file);

    let mut cmd = usage_cmd();
    cmd.args([
        "generate",
        "manpage",
        "-f",
        &example_path("basic.usage.kdl"),
        "-o",
        out_file.to_str().unwrap(),
    ]);

    cmd.assert().success();

    // Verify file was created
    assert!(out_file.exists());

    // Verify content
    let content = std::fs::read_to_string(&out_file).unwrap();
    assert!(content.contains(".TH"));
    assert!(content.contains(".SH NAME"));

    // Clean up
    std::fs::remove_file(&out_file).unwrap();
}

#[test]
fn test_manpage_with_complex_spec() {
    let mut cmd = usage_cmd();
    cmd.args(["generate", "manpage", "-f", &example_path("mise.usage.kdl")]);

    let assert = cmd.assert().success();

    // Check for comprehensive sections
    assert
        .stdout(contains(".SH NAME"))
        .stdout(contains(".SH SYNOPSIS"))
        .stdout(contains(".SH DESCRIPTION"))
        .stdout(contains(".SH OPTIONS"));
}

#[test]
fn test_manpage_alias() {
    // Test that 'man' works as an alias for 'manpage'
    let mut cmd = usage_cmd();
    cmd.args(["generate", "man", "-f", &example_path("basic.usage.kdl")]);

    cmd.assert().success();
}
