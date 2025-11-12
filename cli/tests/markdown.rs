use assert_cmd::prelude::*;
use std::fs;
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
fn test_generate_markdown_with_examples() {
    let temp_dir = std::env::temp_dir();
    let out_file = temp_dir.join("test-markdown-examples.md");

    // Clean up any existing file
    let _ = std::fs::remove_file(&out_file);

    let mut cmd = usage_cmd();
    cmd.args([
        "generate",
        "markdown",
        "-f",
        &example_path("with-examples.usage.kdl"),
        "--out-file",
        out_file.to_str().unwrap(),
    ]);

    cmd.assert().success();

    // Verify file was created
    assert!(out_file.exists());

    // Verify content includes examples
    let content = fs::read_to_string(&out_file).unwrap();

    // Check for Examples section headers
    assert!(content.contains("### Examples"), "Should contain Examples section");

    // Check for example headers
    assert!(content.contains("**Basic deployment**"), "Should contain example header");
    assert!(content.contains("**Force deployment**"), "Should contain example header");

    // Check for example help text
    assert!(content.contains("Deploy to production environment"), "Should contain example help");
    assert!(content.contains("Force deploy to staging, skipping checks"), "Should contain example help");

    // Check for example code blocks
    assert!(content.contains("```\ndemo deploy -e prod\n```"), "Should contain example code block");
    assert!(content.contains("```\ndemo deploy -e staging --force\n```"), "Should contain example code block");

    // Check for nested subcommand examples
    assert!(content.contains("demo config set timeout 30"), "Should contain nested subcommand example");
    assert!(content.contains("demo config set debug true"), "Should contain nested subcommand example");

    // Clean up
    std::fs::remove_file(&out_file).unwrap();
}

#[test]
fn test_generate_markdown_basic() {
    let temp_dir = std::env::temp_dir();
    let out_file = temp_dir.join("test-markdown-basic.md");

    // Clean up any existing file
    let _ = std::fs::remove_file(&out_file);

    let mut cmd = usage_cmd();
    cmd.args([
        "generate",
        "markdown",
        "-f",
        &example_path("basic.usage.kdl"),
        "--out-file",
        out_file.to_str().unwrap(),
    ]);

    cmd.assert().success();

    // Verify file was created
    assert!(out_file.exists());

    // Verify content
    let content = fs::read_to_string(&out_file).unwrap();
    assert!(content.contains("# `basic.usage.kdl`"));

    // Clean up
    std::fs::remove_file(&out_file).unwrap();
}

#[test]
fn test_markdown_snapshot_with_examples() {
    let mut cmd = usage_cmd();
    cmd.args([
        "generate",
        "markdown",
        "-f",
        &example_path("with-examples.usage.kdl"),
        "--out-file",
        "/dev/stdout",
    ]);

    let output = cmd.output().unwrap();
    assert!(output.status.success());

    let stdout = String::from_utf8(output.stdout).unwrap();
    insta::assert_snapshot!(stdout);
}
