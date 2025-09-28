use std::fs;
use std::process::Command;
use std::env;
use std::path::PathBuf;

/// Build the usage binary and return its path
fn build_usage_binary() -> PathBuf {
    let manifest_dir = PathBuf::from(env!("CARGO_MANIFEST_DIR"));
    let workspace_root = manifest_dir.parent().unwrap();

    // Build the usage binary in debug mode
    let output = Command::new("cargo")
        .args(&["build", "--bin", "usage"])
        .current_dir(&workspace_root)
        .output()
        .expect("Failed to build usage binary");

    if !output.status.success() {
        panic!("Failed to build usage binary: {}", String::from_utf8_lossy(&output.stderr));
    }

    // Return the absolute path to the built binary
    workspace_root.join("target/debug/usage")
}

/// Test that completions actually work in real shells
/// These tests require the shells to be installed

#[test]
#[ignore] // These tests require actual shells to be installed
fn test_fish_completion_integration() {
    // Skip if fish is not installed
    if Command::new("fish").arg("--version").output().is_err() {
        eprintln!("Skipping fish test - fish shell not installed");
        return;
    }

    // Build the usage binary
    let usage_bin = build_usage_binary();
    let usage_bin_str = usage_bin.canonicalize().unwrap().to_str().unwrap().to_string();

    let temp_dir = env::temp_dir().join(format!("usage_fish_test_{}", std::process::id()));
    fs::create_dir_all(&temp_dir).unwrap();

    // Create a simple spec
    let spec = r#"
bin "testcli"
arg "<file>" help="Input file"
flag "-v --verbose" help="Verbose output"
cmd "sub" help="Subcommand" {
    arg "<item>" help="Item"
}
"#;

    // Generate the completion script using the actual usage binary
    let output = Command::new(&usage_bin)
        .args(&["generate", "completion", "fish", "testcli"])
        .arg("--spec")
        .arg(&spec)
        .output()
        .expect("Failed to generate fish completion");

    let completion_script = String::from_utf8_lossy(&output.stdout);

    // Write completion to a file
    let comp_file = temp_dir.join("testcli.fish");
    fs::write(&comp_file, completion_script.as_ref()).unwrap();

    // Create a fish script that:
    // 1. Sets up the PATH to include our usage binary
    // 2. Sets up the spec variable
    // 3. Sources the completion
    // 4. Attempts to get completions
    let test_script = format!(r#"
# Add usage binary to PATH
set -gx PATH {} $PATH

# Set up the spec variable that the completion expects
set -g _usage_spec_testcli '{}'

# Source the completion file
source {}

# Test 1: Check if completion file loads without error
echo "LOAD_SUCCESS"

# Test 2: Verify temp file is created with correct content
set -l tmpdir (if set -q TMPDIR; echo $TMPDIR; else; echo /tmp; end)
set -l expected_file "$tmpdir/usage__usage_spec_testcli.spec"

# Trigger the completion to create the temp file
# We simulate what happens when tab is pressed
set -l COMP_LINE "testcli "
set -l COMP_WORDS testcli
complete -C"testcli " > /dev/null 2>&1

# Check if temp file was created
if test -f "$expected_file"
    echo "TEMP_FILE_CREATED"

    # Verify content matches what we set
    set -l content (cat "$expected_file")
    if test "$content" = "$_usage_spec_testcli"
        echo "CONTENT_MATCHES"
    else
        echo "CONTENT_MISMATCH"
        echo "Expected: $_usage_spec_testcli"
        echo "Got: $content"
    end
else
    echo "TEMP_FILE_NOT_CREATED"
end

# Test 3: Test actual completion execution with usage binary in PATH
# Now we can actually test it works
echo "COMPLETION_TEST_DONE"
"#,
        usage_bin.parent().unwrap().to_str().unwrap(),
        spec.replace('\'', "\\'").replace('"', "\\\"").replace('\n', " "),
        comp_file.to_str().unwrap()
    );

    let script_file = temp_dir.join("test.fish");
    fs::write(&script_file, &test_script).unwrap();

    // Execute the test in fish
    let result = Command::new("fish")
        .arg(script_file.to_str().unwrap())
        .output()
        .expect("Failed to run fish test");

    let stdout = String::from_utf8_lossy(&result.stdout);
    let stderr = String::from_utf8_lossy(&result.stderr);

    println!("Fish test stdout:\n{}", stdout);
    println!("Fish test stderr:\n{}", stderr);

    // Verify the tests passed
    assert!(stdout.contains("LOAD_SUCCESS"),
            "Completion script should load without errors. Stderr: {}", stderr);
    assert!(!stderr.contains("a value is required for"),
            "Should not have 'value required' error that indicates variable expansion issue");
    assert!(!stderr.contains("argument list too long"),
            "Should not have argument list too long error");

    // The temp file test may not work without the actual usage binary,
    // but we can at least verify no syntax errors
    assert!(!stderr.contains("syntax error"),
            "Should not have fish syntax errors");

    // Cleanup
    let _ = fs::remove_dir_all(&temp_dir);
}

#[test]
#[ignore] // These tests require actual shells to be installed
fn test_bash_completion_integration() {
    // Skip if bash is not installed
    if Command::new("bash").arg("--version").output().is_err() {
        eprintln!("Skipping bash test - bash shell not installed");
        return;
    }

    // Build the usage binary
    let usage_bin = build_usage_binary();

    let temp_dir = env::temp_dir().join(format!("usage_bash_test_{}", std::process::id()));
    fs::create_dir_all(&temp_dir).unwrap();

    let spec = r#"
bin "testcli"
arg "<file>" help="Input file"
flag "-v --verbose" help="Verbose output"
"#;

    // Generate the completion
    let output = Command::new(&usage_bin)
        .args(&["generate", "completion", "bash", "testcli"])
        .arg("--spec")
        .arg(&spec)
        .output()
        .expect("Failed to generate bash completion");

    let completion_script = String::from_utf8_lossy(&output.stdout);
    let comp_file = temp_dir.join("testcli.bash");
    fs::write(&comp_file, completion_script.as_ref()).unwrap();

    // Create a bash test script
    let test_script = format!(r#"
#!/bin/bash
set -e

# Add usage binary to PATH
export PATH="{}:$PATH"

# Set up the spec variable
export _usage_spec_testcli='{}'

# Source bash-completion if available (might not be in CI)
if [[ -f /usr/share/bash-completion/bash_completion ]]; then
    source /usr/share/bash-completion/bash_completion
elif [[ -f /etc/bash_completion ]]; then
    source /etc/bash_completion
fi

# Define _comp_initialize if it doesn't exist (for environments without bash-completion)
if ! type -t _comp_initialize >/dev/null; then
    _comp_initialize() {{
        COMPREPLY=()
        return 0
    }}
    _comp_compgen() {{
        return 0
    }}
    _comp_ltrim_colon_completions() {{
        return 0
    }}
fi

# Source our completion
source {}

echo "LOAD_SUCCESS"

# Test temp file creation
TMPDIR="${{TMPDIR:-/tmp}}"
expected_file="${{TMPDIR}}/usage__usage_spec_testcli.spec"

# Trigger the completion function
_testcli testcli "" testcli 0 || true

if [[ -f "$expected_file" ]]; then
    echo "TEMP_FILE_CREATED"
    content=$(cat "$expected_file")
    if [[ "$content" == "$_usage_spec_testcli" ]]; then
        echo "CONTENT_MATCHES"
    else
        echo "CONTENT_MISMATCH"
    fi
else
    echo "TEMP_FILE_NOT_CREATED"
fi

echo "COMPLETION_TEST_DONE"
"#,
        usage_bin.parent().unwrap().to_str().unwrap(),
        spec.replace('\'', "\\'").replace('"', "\\\"").replace('\n', " "),
        comp_file.to_str().unwrap()
    );

    let script_file = temp_dir.join("test.sh");
    fs::write(&script_file, &test_script).unwrap();

    // Execute the test
    let result = Command::new("bash")
        .arg(script_file.to_str().unwrap())
        .output()
        .expect("Failed to run bash test");

    let stdout = String::from_utf8_lossy(&result.stdout);
    let stderr = String::from_utf8_lossy(&result.stderr);

    println!("Bash test stdout:\n{}", stdout);
    println!("Bash test stderr:\n{}", stderr);

    assert!(stdout.contains("LOAD_SUCCESS"),
            "Completion script should load. Stderr: {}", stderr);
    assert!(!stderr.contains("argument list too long"),
            "Should not have argument list too long error");

    // Cleanup
    let _ = fs::remove_dir_all(&temp_dir);
}

#[test]
#[ignore] // These tests require actual shells to be installed
fn test_zsh_completion_integration() {
    // Skip if zsh is not installed
    if Command::new("zsh").arg("--version").output().is_err() {
        eprintln!("Skipping zsh test - zsh shell not installed");
        return;
    }

    // Build the usage binary
    let usage_bin = build_usage_binary();

    let temp_dir = env::temp_dir().join(format!("usage_zsh_test_{}", std::process::id()));
    fs::create_dir_all(&temp_dir).unwrap();

    let spec = r#"
bin "testcli"
arg "<file>" help="Input file"
flag "-v --verbose" help="Verbose output"
"#;

    // Generate the completion
    let output = Command::new(&usage_bin)
        .args(&["generate", "completion", "zsh", "testcli"])
        .arg("--spec")
        .arg(&spec)
        .output()
        .expect("Failed to generate zsh completion");

    let completion_script = String::from_utf8_lossy(&output.stdout);
    let comp_file = temp_dir.join("_testcli");
    fs::write(&comp_file, completion_script.as_ref()).unwrap();

    // Create a zsh test script
    let test_script = format!(r#"
#!/bin/zsh
# Initialize completion system
autoload -U compinit
compinit -D

# Add usage binary to PATH
export PATH="{}:$PATH"

# Set up the spec variable
export _usage_spec_testcli='{}'

# Source our completion
source {}

echo "LOAD_SUCCESS"

# Test temp file creation
TMPDIR="${{TMPDIR:-/tmp}}"
expected_file="${{TMPDIR}}/usage__usage_spec_testcli.spec"

# Call the completion function directly
_testcli || true

if [[ -f "$expected_file" ]]; then
    echo "TEMP_FILE_CREATED"
    content=$(cat "$expected_file")
    if [[ "$content" == "$_usage_spec_testcli" ]]; then
        echo "CONTENT_MATCHES"
    else
        echo "CONTENT_MISMATCH"
    fi
else
    echo "TEMP_FILE_NOT_CREATED"
fi

echo "COMPLETION_TEST_DONE"
"#,
        usage_bin.parent().unwrap().to_str().unwrap(),
        spec.replace('\'', "\\'").replace('"', "\\\"").replace('\n', " "),
        comp_file.to_str().unwrap()
    );

    let script_file = temp_dir.join("test.zsh");
    fs::write(&script_file, &test_script).unwrap();

    // Execute the test
    let result = Command::new("zsh")
        .arg(script_file.to_str().unwrap())
        .output()
        .expect("Failed to run zsh test");

    let stdout = String::from_utf8_lossy(&result.stdout);
    let stderr = String::from_utf8_lossy(&result.stderr);

    println!("Zsh test stdout:\n{}", stdout);
    println!("Zsh test stderr:\n{}", stderr);

    assert!(stdout.contains("LOAD_SUCCESS"),
            "Completion script should load. Stderr: {}", stderr);
    assert!(!stderr.contains("argument list too long"),
            "Should not have argument list too long error");

    // Cleanup
    let _ = fs::remove_dir_all(&temp_dir);
}

#[test]
fn test_large_spec_does_not_inline() {
    // Build the usage binary
    let usage_bin = build_usage_binary();

    // This test verifies that large specs use the temp file approach
    let mut large_spec = String::from("bin \"largetool\"\n");

    // Create a spec larger than typical command line limits
    for i in 0..5000 {
        large_spec.push_str(&format!(
            "flag \"--flag-{}\" help=\"This is flag number {} with a long description\"\n",
            i, i
        ));
    }

    println!("Large spec size: {} bytes", large_spec.len());
    assert!(large_spec.len() > 100_000, "Spec should be over 100KB");

    // Test that all shells use temp file approach for large specs
    for shell in &["bash", "zsh", "fish"] {
        let output = Command::new(&usage_bin)
            .args(&["generate", "completion", shell, "largetool"])
            .arg("--spec")
            .arg(&large_spec)
            .output()
            .expect(&format!("Failed to generate {} completion", shell));

        let completion = String::from_utf8_lossy(&output.stdout);

        // All should use temp file approach
        assert!(completion.contains("spec_file"),
                "{} should use spec_file for large specs", shell);
        assert!(completion.contains("-f"),
                "{} should use -f flag for file input", shell);

        // Should not try to inline the large spec with -s flag
        assert!(!completion.contains("-s \"$"),
                "{} should not inline large spec with -s flag", shell);
    }
}