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
fn markdown_template_files_override_named_renderer_templates() {
    let dir = std::env::temp_dir().join(format!("usage_md_templates_{}", std::process::id()));
    let _ = fs::remove_dir_all(&dir);
    fs::create_dir_all(&dir).unwrap();
    let spec = dir.join("custom.usage.kdl");
    let spec_template = dir.join("spec.md.tera");
    let flag_template = dir.join("flag.md.tera");
    fs::write(&spec, "bin \"custom\"\nflag \"--force\"\n").unwrap();
    fs::write(
        &spec_template,
        "CUSTOM {{ spec.bin }}\n{% set cmd = spec.cmd %}{% include \"cmd_template.md.tera\" %}",
    )
    .unwrap();
    fs::write(&flag_template, "CUSTOM FLAG {{ flag.usage }}").unwrap();

    let mut cmd = usage_cmd();
    cmd.args(["generate", "markdown", "-f"])
        .arg(&spec)
        .arg("--template")
        .arg(format!("spec={}", spec_template.display()))
        .arg("--template")
        .arg(format!("flag={}", flag_template.display()))
        .args(["--out-file", "-"]);

    let output = cmd.output().unwrap();
    assert!(
        output.status.success(),
        "stderr: {}",
        String::from_utf8_lossy(&output.stderr)
    );
    let stdout = String::from_utf8(output.stdout).unwrap();
    assert!(stdout.contains("CUSTOM custom"), "{stdout}");
    assert!(stdout.contains("CUSTOM FLAG --force"), "{stdout}");

    fs::remove_dir_all(&dir).unwrap();
}

#[test]
fn test_source_code_links_exist() {
    // A command's path is not its file's path: `complete-word` lives in `complete_word.rs`,
    // `generate` in `generate/mod.rs`, and `bash` in `shell.rs`, which it shares with the
    // three other shells. Each of those is a 404 on GitHub when the template gets it wrong,
    // and nothing but the file system can catch it.
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
        assert!(
            repo_root.join(path).is_file(),
            "`{cur_cmd}` links to {path}, which does not exist"
        );
        checked.push((cur_cmd.clone(), path.to_string()));
    }
    // Every command carries a link, so a scrape that suddenly matches almost nothing means
    // the document changed shape and the assertions above went blind.
    assert!(
        checked.len() >= 10,
        "expected a link for every command, checked {checked:?}"
    );
    // Existence alone can't tell a correct mapping from a coincidence, and each of the three
    // rules in the template is currently carried by only part of the spec. Name one command
    // per rule so that dropping a rule fails here rather than in a reader's browser.
    for (cmd, file) in [
        ("usage complete-word", "cli/src/cli/complete_word.rs"),
        ("usage generate", "cli/src/cli/generate/mod.rs"),
        ("usage bash", "cli/src/cli/shell.rs"),
    ] {
        assert!(
            checked.contains(&(cmd.to_string(), file.to_string())),
            "expected `{cmd}` to link to {file}, checked {checked:?}"
        );
    }

    fs::remove_file(&out_file).unwrap();
}

#[test]
fn a_page_collision_is_refused_before_anything_is_written() {
    // A CLI with a literal `configuration` command: its page and the settings page want the same
    // file. Writing anyway destroys the command's documentation, so the build stops — and stops
    // *before* writing, so a refused run does not leave a half-built directory behind.
    let dir = std::env::temp_dir().join(format!("usage_md_collision_{}", std::process::id()));
    let _ = fs::remove_dir_all(&dir);

    let specs = std::env::temp_dir().join(format!("usage_md_specs_{}", std::process::id()));
    fs::create_dir_all(&specs).unwrap();
    let clashing = specs.join("clash.usage.kdl");
    fs::write(
        &clashing,
        "name \"ex\"\nbin \"ex\"\nconfig {\n  prop \"jobs\" type=\"uint\"\n}\ncmd \"configuration\" help=\"Odd\"\n",
    )
    .unwrap();
    let ordinary = specs.join("ok.usage.kdl");
    fs::write(
        &ordinary,
        "name \"ex\"\nbin \"ex\"\nconfig {\n  prop \"jobs\" type=\"uint\"\n}\ncmd \"settings\" help=\"Manage\"\n",
    )
    .unwrap();

    usage_cmd()
        .args(["generate", "markdown", "-f"])
        .arg(&clashing)
        .args(["--multi", "--out-dir", dir.to_str().unwrap()])
        .assert()
        .failure()
        // A fragment miette does not wrap: it hard-wraps the message across lines.
        .stderr(predicates::str::contains("rename the command"));
    assert!(
        !dir.exists() || fs::read_dir(&dir).unwrap().count() == 0,
        "a refused run wrote files into {}",
        dir.display()
    );

    // And the ordinary case — including a `settings` command, which no longer collides — still
    // writes both pages.
    usage_cmd()
        .args(["generate", "markdown", "-f"])
        .arg(&ordinary)
        .args(["--multi", "--out-dir", dir.to_str().unwrap()])
        .assert()
        .success();
    assert!(dir.join("configuration.md").exists());
    assert!(dir.join("settings.md").exists(), "the command's own page");

    let _ = fs::remove_dir_all(&dir);
    let _ = fs::remove_dir_all(&specs);
}
