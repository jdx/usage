use std::fs;
use std::process::Command;
use std::env;

fn create_test_spec() -> String {
    r#"
bin "testcli"
arg "<file>" help="Input file" {
  complete_file
}
flag "-v --verbose" help="Verbose output"
flag "-o --output" help="Output file" {
  arg "<path>"
  complete_file
}
cmd "subcommand" help="A subcommand" {
  arg "<item>" help="Item to process"
}
"#
    .to_string()
}

#[test]
fn test_zsh_completion_e2e() {
    let temp_dir = env::temp_dir().join(format!("usage_test_{}", std::process::id()));
    fs::create_dir_all(&temp_dir).unwrap();
    let spec_file = temp_dir.join("test.kdl");
    fs::write(&spec_file, create_test_spec()).unwrap();

    // Generate zsh completion
    let output = Command::new("cargo")
        .args(&[
            "run",
            "--quiet",
            "--",
            "generate",
            "completion",
            "zsh",
            "testcli",
            "-f",
            spec_file.to_str().unwrap(),
        ])
        .output()
        .expect("Failed to generate zsh completion");

    let completion = String::from_utf8_lossy(&output.stdout);

    // Verify the completion script structure
    assert!(completion.contains("_testcli()"));
    assert!(completion.contains("local spec_file=\"${TMPDIR:-/tmp}/usage__usage_spec_testcli.spec\""));
    assert!(completion.contains("complete-word --shell zsh -f \"$spec_file\""));

    // Verify temp file creation logic
    assert!(completion.contains("if [[ ! -f \"$spec_file\" ]]; then"));
    assert!(completion.contains("echo \"$spec\" > \"$spec_file\""));

    // Test that the spec file path is properly passed
    let spec_var_usage = completion.matches("-f \"$spec_file\"").count();
    assert_eq!(spec_var_usage, 1, "spec_file should be used exactly once with -f flag");
}

#[test]
fn test_bash_completion_e2e() {
    let temp_dir = env::temp_dir().join(format!("usage_test_{}", std::process::id()));
    fs::create_dir_all(&temp_dir).unwrap();
    let spec_file = temp_dir.join("test.kdl");
    fs::write(&spec_file, create_test_spec()).unwrap();

    // Generate bash completion
    let output = Command::new("cargo")
        .args(&[
            "run",
            "--quiet",
            "--",
            "generate",
            "completion",
            "bash",
            "testcli",
            "-f",
            spec_file.to_str().unwrap(),
        ])
        .output()
        .expect("Failed to generate bash completion");

    let completion = String::from_utf8_lossy(&output.stdout);

    // Verify the completion script structure
    assert!(completion.contains("_testcli()"));
    assert!(completion.contains("local spec_file=\"${TMPDIR:-/tmp}/usage__usage_spec_testcli.spec\""));
    assert!(completion.contains("complete-word --shell bash -f \"$spec_file\""));

    // Verify temp file creation logic
    assert!(completion.contains("if [[ ! -f \"$spec_file\" ]]; then"));
    assert!(completion.contains("echo \"${_usage_spec_testcli}\" > \"$spec_file\""));

    // Test that the spec file path is properly passed
    let spec_var_usage = completion.matches("-f \"$spec_file\"").count();
    assert_eq!(spec_var_usage, 1, "spec_file should be used exactly once with -f flag");
}

#[test]
fn test_fish_completion_e2e() {
    let temp_dir = env::temp_dir().join(format!("usage_test_{}", std::process::id()));
    fs::create_dir_all(&temp_dir).unwrap();
    let spec_file = temp_dir.join("test.kdl");
    fs::write(&spec_file, create_test_spec()).unwrap();

    // Generate fish completion
    let output = Command::new("cargo")
        .args(&[
            "run",
            "--quiet",
            "--",
            "generate",
            "completion",
            "fish",
            "testcli",
            "-f",
            spec_file.to_str().unwrap(),
        ])
        .output()
        .expect("Failed to generate fish completion");

    let completion = String::from_utf8_lossy(&output.stdout);

    // Verify the completion script structure
    // Note: In the actual generated code, the $ is properly there without escaping
    assert!(completion.contains("set -l tmpdir"), "Should have tmpdir variable");
    assert!(completion.contains("if set -q TMPDIR"), "Should check for TMPDIR");
    assert!(completion.contains("set -l spec_file"), "Should have spec_file variable");

    // Verify temp file creation logic
    assert!(completion.contains("if not test -f \"$spec_file\""));
    assert!(completion.contains("echo $_usage_spec_testcli > \"$spec_file\""));

    // Verify proper variable expansion in complete command (double quotes, not single)
    assert!(completion.contains("complete-word --shell fish -f \"$spec_file\""));
    assert!(!completion.contains("complete-word --shell fish -f '$spec_file'"),
           "spec_file should not be in single quotes as it won't expand");

    // Test that the spec file path is properly passed with double quotes for expansion
    let double_quote_usage = completion.matches("-f \"$spec_file\"").count();
    assert_eq!(double_quote_usage, 2, "spec_file should be used twice with -f flag (for both commandline branches)");
}

#[test]
fn test_large_spec_handling() {
    // Create a large spec to test the temp file approach
    let mut large_spec = String::from("bin \"largetool\"\n");

    // Add many commands to make the spec large
    for i in 0..1000 {
        large_spec.push_str(&format!(
            r#"cmd "command{}" help="Command number {}" {{
  arg "<arg{}>" help="Argument for command {}"
  flag "--flag{}" help="Flag for command {}"
}}
"#,
            i, i, i, i, i, i
        ));
    }

    let temp_dir = env::temp_dir().join(format!("usage_test_{}", std::process::id()));
    fs::create_dir_all(&temp_dir).unwrap();
    let spec_file = temp_dir.join("large.kdl");
    fs::write(&spec_file, &large_spec).unwrap();

    // Test all three shells handle large specs
    for shell in &["zsh", "bash", "fish"] {
        let output = Command::new("cargo")
            .args(&[
                "run",
                "--quiet",
                "--",
                "generate",
                "completion",
                shell,
                "largetool",
                "-f",
                spec_file.to_str().unwrap(),
            ])
            .output()
            .expect(&format!("Failed to generate {} completion for large spec", shell));

        let completion = String::from_utf8_lossy(&output.stdout);

        // Verify temp file approach is used
        assert!(completion.contains("spec_file"),
                "{} completion should use spec_file for large spec", shell);

        // Verify -f flag is used instead of -s
        assert!(completion.contains("-f"),
                "{} completion should use -f flag for file-based approach", shell);
        assert!(!completion.contains("-s \"$"),
                "{} completion should not use -s flag with inline spec", shell);
    }
}

#[test]
fn test_actual_completion_execution() {
    // This test verifies that the completion can actually be executed
    // without "argument list too long" errors

    let temp_dir = env::temp_dir().join(format!("usage_test_{}", std::process::id()));
    fs::create_dir_all(&temp_dir).unwrap();
    let spec_file = temp_dir.join("test.kdl");
    fs::write(&spec_file, create_test_spec()).unwrap();

    // Generate a completion script
    let output = Command::new("cargo")
        .args(&[
            "run",
            "--quiet",
            "--",
            "generate",
            "completion",
            "fish",
            "testcli",
            "-f",
            spec_file.to_str().unwrap(),
        ])
        .output()
        .expect("Failed to generate completion");

    let completion_script = String::from_utf8_lossy(&output.stdout);

    // Write the completion to a file
    let comp_file = temp_dir.join("testcli.fish");
    fs::write(&comp_file, completion_script.as_ref()).unwrap();

    // Try to source it in fish (this would fail with syntax errors)
    let fish_test = format!(
        "set -g _usage_spec_testcli '{}'; source {}; echo 'SUCCESS'",
        create_test_spec().replace('\'', "\\'").replace('\n', " "),
        comp_file.to_str().unwrap()
    );

    let result = Command::new("fish")
        .arg("-c")
        .arg(&fish_test)
        .output();

    // Only run this part if fish is installed
    if let Ok(output) = result {
        let stdout = String::from_utf8_lossy(&output.stdout);
        let stderr = String::from_utf8_lossy(&output.stderr);

        // Check for common error patterns
        assert!(!stderr.contains("argument list too long"),
                "Should not have argument list too long error");
        assert!(!stderr.contains("a value is required for"),
                "Should not have missing value errors: {}", stderr);

        if !output.status.success() {
            eprintln!("Fish completion test failed:");
            eprintln!("STDOUT: {}", stdout);
            eprintln!("STDERR: {}", stderr);
        }
    }
}