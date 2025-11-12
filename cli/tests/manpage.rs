use assert_cmd::prelude::*;
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

    let output = cmd.output().unwrap();
    assert!(output.status.success());

    let stdout = String::from_utf8(output.stdout).unwrap();
    insta::assert_snapshot!(stdout);
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

    let output = cmd.output().unwrap();
    assert!(output.status.success());

    let stdout = String::from_utf8(output.stdout).unwrap();
    insta::assert_snapshot!(stdout);
}

#[test]
fn test_generate_manpage_with_flags() {
    // This test uses mise.usage.kdl which actually has flags
    let mut cmd = usage_cmd();
    cmd.args(["generate", "manpage", "-f", &example_path("mise.usage.kdl")]);

    let output = cmd.output().unwrap();
    assert!(output.status.success());

    let stdout = String::from_utf8(output.stdout).unwrap();
    insta::assert_snapshot!(stdout);
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
fn test_manpage_output_first_50_lines() {
    // Test first 50 lines of mise manpage to avoid huge snapshot
    let mut cmd = usage_cmd();
    cmd.args(["generate", "manpage", "-f", &example_path("mise.usage.kdl")]);

    let output = cmd.output().unwrap();
    assert!(output.status.success());

    let stdout = String::from_utf8(output.stdout).unwrap();
    let first_lines: Vec<&str> = stdout.lines().take(50).collect();
    insta::assert_snapshot!(first_lines.join("\n"));
}

#[test]
fn test_manpage_alias() {
    // Test that 'man' works as an alias for 'manpage'
    let mut cmd = usage_cmd();
    cmd.args(["generate", "man", "-f", &example_path("basic.usage.kdl")]);

    cmd.assert().success();
}

#[test]
fn test_generate_manpage_with_examples() {
    let mut cmd = usage_cmd();
    cmd.args([
        "generate",
        "manpage",
        "-f",
        &example_path("with-examples.usage.kdl"),
    ]);

    let output = cmd.output().unwrap();
    assert!(output.status.success());

    let stdout = String::from_utf8(output.stdout).unwrap();

    // Verify EXAMPLES sections are present for subcommands
    assert!(
        stdout.contains("\\fBExamples:\\fR"),
        "Should contain Examples section"
    );

    // Verify example headers (bold text)
    assert!(
        stdout.contains("\\fBBasic deployment\\fR"),
        "Should contain example header"
    );
    assert!(
        stdout.contains("\\fBForce deployment\\fR"),
        "Should contain example header"
    );

    // Verify example help text
    assert!(
        stdout.contains("Deploy to production environment"),
        "Should contain example help"
    );
    assert!(
        stdout.contains("Force deploy to staging, skipping checks"),
        "Should contain example help"
    );

    // Verify example code is indented (.RS 4)
    assert!(
        stdout.contains(".RS 4\ndemo deploy \\-e prod"),
        "Should contain indented example code"
    );
    assert!(
        stdout.contains(".RS 4\ndemo deploy \\-e staging \\-\\-force"),
        "Should contain indented example code"
    );

    // Verify nested subcommand examples
    assert!(
        stdout.contains("demo config set timeout 30"),
        "Should contain nested subcommand example"
    );
    assert!(
        stdout.contains("demo config set debug true"),
        "Should contain nested subcommand example"
    );

    // Use insta snapshot for full output
    insta::assert_snapshot!(stdout);
}
