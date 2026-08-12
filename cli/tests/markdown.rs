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
    assert!(
        content.contains("### Examples"),
        "Should contain Examples section"
    );

    // Check for example headers
    assert!(
        content.contains("**Basic deployment**"),
        "Should contain example header"
    );
    assert!(
        content.contains("**Force deployment**"),
        "Should contain example header"
    );

    // Check for example help text
    assert!(
        content.contains("Deploy to production environment"),
        "Should contain example help"
    );
    assert!(
        content.contains("Force deploy to staging, skipping checks"),
        "Should contain example help"
    );

    // Check for example code blocks
    assert!(
        content.contains("```\ndemo deploy -e prod\n```"),
        "Should contain example code block"
    );
    assert!(
        content.contains("```\ndemo deploy -e staging --force\n```"),
        "Should contain example code block"
    );

    // Check for nested subcommand examples
    assert!(
        content.contains("demo config set timeout 30"),
        "Should contain nested subcommand example"
    );
    assert!(
        content.contains("demo config set debug true"),
        "Should contain nested subcommand example"
    );

    // Clean up
    std::fs::remove_file(&out_file).unwrap();
}

#[test]
fn test_generate_markdown_with_spec_examples() {
    let temp_dir = std::env::temp_dir();
    let out_file = temp_dir.join("test-markdown-spec-examples.md");

    // Clean up any existing file
    let _ = std::fs::remove_file(&out_file);

    let mut cmd = usage_cmd();
    cmd.args([
        "generate",
        "markdown",
        "-f",
        &example_path("spec-with-examples.usage.kdl"),
        "--out-file",
        out_file.to_str().unwrap(),
    ]);

    cmd.assert().success();

    // Verify file was created
    assert!(out_file.exists());

    // Verify content includes spec-level examples
    let content = fs::read_to_string(&out_file).unwrap();

    // Check for spec-level Examples section
    assert!(
        content.contains("## Examples"),
        "Should contain spec-level Examples section"
    );

    // Check for spec-level example headers
    assert!(
        content.contains("**Getting help**"),
        "Should contain spec-level example header"
    );
    assert!(
        content.contains("**Check version**"),
        "Should contain spec-level example header"
    );

    // Check for spec-level example help text
    assert!(
        content.contains("Display help information for the demo command"),
        "Should contain spec-level example help"
    );
    assert!(
        content.contains("Show the installed version of demo"),
        "Should contain spec-level example help"
    );

    // Check for spec-level example code blocks
    assert!(
        content.contains("```\ndemo --help\n```"),
        "Should contain spec-level example code block"
    );
    assert!(
        content.contains("```\ndemo --version\n```"),
        "Should contain spec-level example code block"
    );

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

    let output = cmd.output().unwrap();
    assert!(output.status.success());

    // Verify file was created
    assert!(out_file.exists());

    // Verify content
    let content = fs::read_to_string(&out_file).unwrap();
    assert!(content.contains("# `basic.usage.kdl`"));

    // The progress line belongs on stderr; on stdout it lands inside the document whenever
    // the document is going to stdout.
    let stdout = String::from_utf8(output.stdout).unwrap();
    let stderr = String::from_utf8(output.stderr).unwrap();
    assert!(!stdout.contains("writing to"), "stdout was: {stdout:?}");
    assert!(stderr.contains("writing to"), "stderr was: {stderr:?}");

    // Clean up
    std::fs::remove_file(&out_file).unwrap();
}

fn markdown_stdout(args: &[&str]) -> String {
    let mut cmd = usage_cmd();
    cmd.args(["generate", "markdown", "-f"]);
    cmd.arg(example_path("with-examples.usage.kdl"));
    cmd.args(args);

    let output = cmd.output().unwrap();
    assert!(output.status.success());
    String::from_utf8(output.stdout).unwrap()
}

#[test]
fn test_markdown_snapshot_with_examples() {
    insta::assert_snapshot!(markdown_stdout(&["--out-file", "-"]));
}

#[test]
fn test_markdown_stdout_when_out_file_omitted() {
    // The two spellings of "stdout" have to agree; `--out-file -` exists for callers that
    // build the path in a variable.
    assert_eq!(markdown_stdout(&[]), markdown_stdout(&["--out-file", "-"]));
}

#[test]
fn test_markdown_out_file_dash_writes_no_file() {
    // The bug this guards: `--out-file` was resolved as a path unconditionally, and
    // `xx::file::write` creates the parent directory before writing. `/dev/stdout` therefore
    // produced a real `C:\dev\stdout` on Windows; `-` would likewise leave a file named `-`.
    let dir = std::env::temp_dir().join(format!("usage_md_dash_{}", std::process::id()));
    let _ = fs::remove_dir_all(&dir);
    fs::create_dir_all(&dir).unwrap();

    let mut cmd = usage_cmd();
    cmd.current_dir(&dir).args([
        "generate",
        "markdown",
        "-f",
        &example_path("with-examples.usage.kdl"),
        "--out-file",
        "-",
    ]);
    cmd.assert().success();

    let leftovers: Vec<_> = fs::read_dir(&dir)
        .unwrap()
        .map(|e| e.unwrap().path())
        .collect();
    assert!(leftovers.is_empty(), "wrote files: {leftovers:?}");

    fs::remove_dir_all(&dir).unwrap();
}

#[test]
fn test_source_code_links_for_multi_word_commands_exist() {
    // A hyphenated command like `complete-word` lives in `complete_word.rs`; linking it as
    // `complete-word.rs` is a 404 on GitHub, and nothing but the file system can catch it.
    let repo_root = std::path::PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .parent()
        .unwrap()
        .to_path_buf();
    let spec = repo_root.join("cli").join("usage.usage.kdl");
    let out_file =
        std::env::temp_dir().join(format!("usage_md_src_links_{}.md", std::process::id()));
    let _ = fs::remove_file(&out_file);

    let mut cmd = usage_cmd();
    cmd.args([
        "generate",
        "markdown",
        "-f",
        spec.to_str().unwrap(),
        "--out-file",
        out_file.to_str().unwrap(),
    ]);
    cmd.assert().success();

    let content = fs::read_to_string(&out_file).unwrap();
    let mut cur_cmd = String::new();
    let mut checked = vec![];
    for line in content.lines() {
        if line.starts_with('#') {
            cur_cmd = line
                .trim_start_matches('#')
                .trim()
                .trim_matches('`')
                .to_string();
            continue;
        }
        let Some(rest) = line.trim().strip_prefix("- **Source code**: [`") else {
            continue;
        };
        let path = rest.split_once("`]").expect("malformed source code link").0;
        // Single-word commands are linked from the same template, but several of them
        // legitimately live elsewhere (`usage bash` is served by `shell.rs`), so only the
        // hyphen-to-underscore mapping is asserted here.
        let is_multi_word = cur_cmd
            .split_whitespace()
            .next_back()
            .unwrap_or("")
            .contains('-');
        if !is_multi_word {
            continue;
        }
        assert!(
            repo_root.join(path).is_file(),
            "`{cur_cmd}` links to {path}, which does not exist"
        );
        checked.push(cur_cmd.clone());
    }
    assert!(
        checked.len() >= 2,
        "expected to check several multi-word commands, checked {checked:?}"
    );

    fs::remove_file(&out_file).unwrap();
}
