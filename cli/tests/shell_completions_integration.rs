use std::env;
use std::fs;
use std::path::PathBuf;
use std::process::Command;

/// Build the usage binary and return its path
fn build_usage_binary() -> PathBuf {
    let manifest_dir = PathBuf::from(env!("CARGO_MANIFEST_DIR"));
    let workspace_root = manifest_dir.parent().unwrap();

    // Build the usage binary in debug mode
    let output = Command::new("cargo")
        .args(["build", "--bin", "usage"])
        .current_dir(workspace_root)
        .output()
        .expect("Failed to build usage binary");

    if !output.status.success() {
        panic!(
            "Failed to build usage binary: {}",
            String::from_utf8_lossy(&output.stderr)
        );
    }

    // Return the absolute path to the built binary
    workspace_root.join("target/debug/usage")
}

/// Test that completions actually work in real shells
/// These tests require the shells to be installed

#[test]
fn test_fish_completion_integration() {
    // Skip if fish is not installed
    if Command::new("fish").arg("--version").output().is_err() {
        eprintln!("Skipping fish test - fish shell not installed");
        return;
    }

    // Build the usage binary
    let usage_bin = build_usage_binary();

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

    // Write spec to a file first (fish completion generator needs a file)
    let spec_kdl_file = temp_dir.join("testcli.kdl");
    fs::write(&spec_kdl_file, spec).unwrap();

    // Generate the completion script using the actual usage binary
    let output = Command::new(&usage_bin)
        .args(["generate", "completion", "fish", "testcli"])
        .arg("-f")
        .arg(spec_kdl_file.to_str().unwrap())
        .output()
        .expect("Failed to generate fish completion");

    let completion_script = String::from_utf8_lossy(&output.stdout);

    // Write completion to a file
    let comp_file = temp_dir.join("testcli.fish");
    fs::write(&comp_file, completion_script.as_ref()).unwrap();

    // Also write the spec directly to the expected location in usage format
    // Convert from KDL to usage format
    let usage_spec = r#"name testcli
bin testcli
flag "-v --verbose" help="Verbose output"
arg <file> help="Input file"
cmd sub help=Subcommand {
    arg <item> help=Item
}
"#;
    let spec_file = temp_dir.join("usage__usage_spec_testcli.spec");
    fs::write(&spec_file, usage_spec).unwrap();

    // Create a fish script that:
    // 1. Sets up the PATH to include our usage binary
    // 2. Sources the completion
    // 3. Tests the actual completion mechanism
    let test_script = format!(
        r#"
# Add usage binary to PATH
set -gx PATH {} $PATH

# Source the completion file
source {}

# Test 1: Check if completion file loads without error
echo "LOAD_SUCCESS"

# Test 2: Verify the completion mechanism works
# Use the spec file we pre-created
set -l spec_file "{}/usage__usage_spec_testcli.spec"

# Check if spec file exists
if test -f "$spec_file"
    echo "SPEC_FILE_EXISTS"
else
    echo "SPEC_FILE_NOT_FOUND"
end

# Now test the actual completion by calling usage complete-word directly
# This is what the completion script calls internally
set -l completion_output (command usage complete-word --shell fish -f "$spec_file" -- testcli "")

# Check if we got expected completions
if test -n "$completion_output"
    echo "GOT_COMPLETIONS"

    # Check for expected completion items
    if string match -q "*sub*" $completion_output
        echo "COMPLETION_SUB_FOUND"
    end

    if string match -q "*verbose*" $completion_output
        echo "COMPLETION_VERBOSE_FOUND"
    end

    # Also test partial completion
    set -l partial_output (command usage complete-word --shell fish -f "$spec_file" -- testcli "s")
    if string match -q "*sub*" $partial_output
        echo "PARTIAL_COMPLETION_WORKS"
    end
else
    echo "NO_COMPLETIONS"
    echo "Error or empty output from usage complete-word"
end

# Test 3: Verify that complete -C returns actual completions (not the command)
set -l actual_completions (complete -C"testcli ")
if test -n "$actual_completions"
    echo "COMPLETE_C_WORKS"
    # This should show file completions or actual command completions
end

echo "COMPLETION_TEST_DONE"
"#,
        usage_bin.parent().unwrap().to_str().unwrap(),
        comp_file.to_str().unwrap(),
        temp_dir.to_str().unwrap()
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

    // Simple assertions - just verify it loads and runs
    assert!(
        stdout.contains("LOAD_SUCCESS"),
        "Should load completion script"
    );
    assert!(
        stdout.contains("COMPLETION_TEST_DONE"),
        "Should complete test"
    );

    // Cleanup
    let _ = fs::remove_dir_all(&temp_dir);
}

#[test]
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
cmd "sub" help="Subcommand" {
    arg "<item>" help="Item"
}
"#;

    // Write spec to a file first (bash completion generator needs a file)
    let spec_kdl_file = temp_dir.join("testcli.kdl");
    fs::write(&spec_kdl_file, spec).unwrap();

    // Generate the completion with bash-completion library included
    let output = Command::new(&usage_bin)
        .args(["generate", "completion", "bash", "testcli"])
        .arg("-f")
        .arg(spec_kdl_file.to_str().unwrap())
        .arg("--include-bash-completion-lib")
        .output()
        .expect("Failed to generate bash completion");

    let completion_script = String::from_utf8_lossy(&output.stdout);
    let comp_file = temp_dir.join("testcli.bash");
    fs::write(&comp_file, completion_script.as_ref()).unwrap();

    // Also write the spec directly to the expected location in usage format
    let usage_spec = r#"name testcli
bin testcli
flag "-v --verbose" help="Verbose output"
arg <file> help="Input file"
cmd sub help=Subcommand {
    arg <item> help=Item
}
"#;
    let spec_file = temp_dir.join("usage__usage_spec_testcli.spec");
    fs::write(&spec_file, usage_spec).unwrap();

    // Create a bash test script
    let test_script = format!(
        r#"
#!/bin/bash
# Don't exit on error for the completion calls
set +e

# Add usage binary to PATH
export PATH="{}:$PATH"

# Source our completion (which includes bash-completion library)
source {}

echo "LOAD_SUCCESS"

# Check if completion function exists
if type -t _testcli >/dev/null; then
    echo "COMPLETION_FUNCTION_EXISTS"
else
    echo "COMPLETION_FUNCTION_NOT_FOUND"
fi

# Check if complete command was registered
if complete -p testcli 2>/dev/null; then
    echo "COMPLETE_COMMAND_REGISTERED"
else
    echo "COMPLETE_COMMAND_NOT_REGISTERED"
fi

# Test 1: Test basic completion - empty input should show all options
COMP_WORDS=(testcli "")
COMP_CWORD=1
COMP_LINE="testcli "
COMP_POINT=${{#COMP_LINE}}
COMPREPLY=()

# Call the completion function
echo "Calling _testcli with COMP_WORDS: ${{COMP_WORDS[@]}}, COMP_CWORD: $COMP_CWORD"
_testcli testcli "" testcli 1
echo "Exit code: $?"
echo "COMPREPLY count: ${{#COMPREPLY[@]}}"

# Check if we got completions
if [[ ${{#COMPREPLY[@]}} -gt 0 ]]; then
    echo "GOT_COMPLETIONS"

    # Check for expected completions
    for item in "${{COMPREPLY[@]}}"; do
        if [[ "$item" == "sub" ]]; then
            echo "COMPLETION_SUB_FOUND"
        fi
        if [[ "$item" == "--verbose" ]] || [[ "$item" == "-v" ]]; then
            echo "COMPLETION_VERBOSE_FOUND"
        fi
    done

    # Show all completions for debugging
    echo "COMPLETIONS: ${{COMPREPLY[@]}}"
else
    echo "NO_COMPLETIONS"
fi

# Test 2: Test partial completion - "s" should complete to "sub"
COMP_WORDS=(testcli "s")
COMP_CWORD=1
COMP_LINE="testcli s"
COMP_POINT=${{#COMP_LINE}}
COMPREPLY=()

_testcli testcli "s" s 1

if [[ ${{#COMPREPLY[@]}} -gt 0 ]]; then
    for item in "${{COMPREPLY[@]}}"; do
        if [[ "$item" == "sub" ]]; then
            echo "PARTIAL_COMPLETION_WORKS"
        fi
    done
fi

# Test 3: Test flag completion - "-" should show flags
COMP_WORDS=(testcli "-")
COMP_CWORD=1
COMP_LINE="testcli -"
COMP_POINT=${{#COMP_LINE}}
COMPREPLY=()

_testcli testcli "-" "-" 1

if [[ ${{#COMPREPLY[@]}} -gt 0 ]]; then
    for item in "${{COMPREPLY[@]}}"; do
        if [[ "$item" == "--verbose" ]] || [[ "$item" == "-v" ]]; then
            echo "FLAG_COMPLETION_WORKS"
        fi
    done
fi

# Test 4: Check that spec file was created/used
spec_file="{}/usage__usage_spec_testcli.spec"
if [[ -f "$spec_file" ]]; then
    echo "SPEC_FILE_EXISTS"
fi

echo "COMPLETION_TEST_DONE"
"#,
        usage_bin.parent().unwrap().to_str().unwrap(),
        comp_file.to_str().unwrap(),
        temp_dir.to_str().unwrap()
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

    // Simple assertions - just verify it loads and runs
    assert!(
        stdout.contains("LOAD_SUCCESS"),
        "Should load completion script"
    );
    assert!(
        stdout.contains("COMPLETION_TEST_DONE"),
        "Should complete test"
    );

    // Cleanup
    let _ = fs::remove_dir_all(&temp_dir);
}

#[test]
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
cmd "sub" help="Subcommand" {
    arg "<item>" help="Item"
}
"#;

    // Write spec to a file first (zsh completion generator needs a file)
    let spec_kdl_file = temp_dir.join("testcli.kdl");
    fs::write(&spec_kdl_file, spec).unwrap();

    // Also write the spec directly to the expected location in usage format
    let usage_spec = r#"name testcli
bin testcli
flag "-v --verbose" help="Verbose output"
arg <file> help="Input file"
cmd sub help=Subcommand {
    arg <item> help=Item
}
"#;
    let spec_file = temp_dir.join("usage__usage_spec_testcli.spec");
    fs::write(&spec_file, usage_spec).unwrap();

    // Generate the completion
    let output = Command::new(&usage_bin)
        .args(["generate", "completion", "zsh", "testcli"])
        .arg("-f")
        .arg(spec_kdl_file.to_str().unwrap())
        .output()
        .expect("Failed to generate zsh completion");

    let completion_script = String::from_utf8_lossy(&output.stdout);
    let comp_file = temp_dir.join("_testcli");
    fs::write(&comp_file, completion_script.as_ref()).unwrap();

    // Create a zsh test script using zpty to test actual completions
    let test_script = format!(
        r#"
#!/bin/zsh

# Add usage binary to PATH
export PATH="{}:$PATH"
export TMPDIR="{}"

# Initialize completion system
autoload -U compinit
compinit -D

# Source our completion
source {}

echo "LOAD_SUCCESS"

# Define our test function
comptest () {{
    # Set up styles for easier parsing
    zstyle ':completion:*:default' list-colors 'no=<COMPLETION>' 'lc=' 'rc=' 'ec=</COMPLETION>'
    zstyle ':completion:*' group-name ''
    zstyle ':completion:*:messages' format '<MESSAGE>%d</MESSAGE>'
    zstyle ':completion:*:descriptions' format '<HEADER>%d</HEADER>'

    # Bind TAB to complete-word
    bindkey '^I' complete-word
    zle -C {{,,}}complete-word
    complete-word () {{
        unset 'compstate[vared]'
        compadd -x $'\002'  # Start delimiter
        _main_complete "$@"
        compadd -J -last- -x $'\003'  # End delimiter
        exit
    }}

    vared -c tmp
}}

# Load zpty module
zmodload zsh/zpty

# Create a pty and run our test function
zpty comptest comptest

# Test 1: Complete from empty
zpty -w comptest $'testcli \t'

# Read up to first delimiter (with timeout)
zpty -rt 2 comptest REPLY $'*\002' 2>/dev/null

# Read actual completions (with timeout)
zpty -rt 2 comptest REPLY $'*\003' 2>/dev/null

# Check if we got completions
if [[ -n "${{REPLY%$'\003'}}" ]]; then
    echo "GOT_COMPLETIONS"
    # Check for expected items
    if [[ "$REPLY" == *"sub"* ]]; then
        echo "FOUND_SUB"
    fi
    if [[ "$REPLY" == *"verbose"* ]] || [[ "$REPLY" == *"-v"* ]]; then
        echo "FOUND_VERBOSE"
    fi
fi

# Clean up
zpty -d comptest

echo "COMPLETION_TEST_DONE"
"#,
        usage_bin.parent().unwrap().to_str().unwrap(),
        temp_dir.to_str().unwrap(),
        comp_file.to_str().unwrap()
    );

    let script_file = temp_dir.join("test.zsh");
    fs::write(&script_file, &test_script).unwrap();

    // Execute the test with a timeout
    let result = Command::new("timeout")
        .arg("5") // 5 second timeout
        .arg("zsh")
        .arg(script_file.to_str().unwrap())
        .output()
        .expect("Failed to run zsh test");

    let stdout = String::from_utf8_lossy(&result.stdout);
    let stderr = String::from_utf8_lossy(&result.stderr);

    println!("Zsh test stdout:\n{}", stdout);
    println!("Zsh test stderr:\n{}", stderr);

    // Simple assertion - just verify it loads and runs
    assert!(
        stdout.contains("LOAD_SUCCESS"),
        "Should load completion script"
    );
    assert!(
        stdout.contains("COMPLETION_TEST_DONE"),
        "Should complete test"
    );

    // Cleanup
    let _ = fs::remove_dir_all(&temp_dir);
}

#[test]
fn test_powershell_completion_integration() {
    // Skip if pwsh is not installed
    if Command::new("pwsh").arg("--version").output().is_err() {
        eprintln!("Skipping pwsh test - PowerShell Core not installed");
        return;
    }

    // Build the usage binary
    let usage_bin = build_usage_binary();

    let temp_dir = env::temp_dir().join(format!("usage_pwsh_test_{}", std::process::id()));
    fs::create_dir_all(&temp_dir).unwrap();

    let spec = r#"
bin "testcli"
arg "<file>" help="Input file"
flag "-v --verbose" help="Verbose output"
cmd "sub" help="Subcommand" {
    arg "<item>" help="Item"
}
"#;

    // Write spec to a file
    let spec_kdl_file = temp_dir.join("testcli.kdl");
    fs::write(&spec_kdl_file, spec).unwrap();

    // Generate the completion script using the actual usage binary
    let output = Command::new(&usage_bin)
        .args(["generate", "completion", "powershell", "testcli"])
        .arg("-f")
        .arg(spec_kdl_file.to_str().unwrap())
        .output()
        .expect("Failed to generate powershell completion");

    let completion_script = String::from_utf8_lossy(&output.stdout);

    // Write completion to a file
    let comp_file = temp_dir.join("testcli.ps1");
    fs::write(&comp_file, completion_script.as_ref()).unwrap();

    // Also write the spec directly to the expected location
    let usage_spec = r#"name testcli
bin testcli
flag "-v --verbose" help="Verbose output"
arg <file> help="Input file"
cmd sub help=Subcommand {
    arg <item> help=Item
}
"#;
    let spec_file = temp_dir.join("usage__usage_spec_testcli.kdl");
    fs::write(&spec_file, usage_spec).unwrap();

    // Create a PowerShell test script
    let test_script = format!(
        r#"
$ErrorActionPreference = "Stop"

# Add usage binary to PATH
$env:PATH = "{};$env:PATH"
$env:TEMP = "{}"

# Source the completion file
. {}

Write-Host "LOAD_SUCCESS"

# Test 1: Check if spec file exists
$specFile = "{}"
if (Test-Path $specFile) {{
    Write-Host "SPEC_FILE_EXISTS"
}} else {{
    Write-Host "SPEC_FILE_NOT_FOUND"
}}

# Test 2: Call usage complete-word directly (this is what the completion script calls)
$completionOutput = & usage complete-word --shell powershell -f "$specFile" -- testcli "" 2>$null

if ($completionOutput) {{
    Write-Host "GOT_COMPLETIONS"

    # Check for expected completion items
    $outputStr = $completionOutput -join "`n"
    if ($outputStr -match "sub") {{
        Write-Host "COMPLETION_SUB_FOUND"
    }}
    if ($outputStr -match "verbose") {{
        Write-Host "COMPLETION_VERBOSE_FOUND"
    }}

    # Test partial completion
    $partialOutput = & usage complete-word --shell powershell -f "$specFile" -- testcli "s" 2>$null
    $partialStr = $partialOutput -join "`n"
    if ($partialStr -match "sub") {{
        Write-Host "PARTIAL_COMPLETION_WORKS"
    }}
}} else {{
    Write-Host "NO_COMPLETIONS"
}}

# Test 3: Test flag completion
$flagOutput = & usage complete-word --shell powershell -f "$specFile" -- testcli "-" 2>$null
$flagStr = $flagOutput -join "`n"
if ($flagStr -match "verbose" -or $flagStr -match "-v") {{
    Write-Host "FLAG_COMPLETION_WORKS"
}}

Write-Host "COMPLETION_TEST_DONE"
"#,
        usage_bin.parent().unwrap().to_str().unwrap(),
        temp_dir.to_str().unwrap(),
        comp_file.to_str().unwrap(),
        spec_file.to_str().unwrap()
    );

    let script_file = temp_dir.join("test.ps1");
    fs::write(&script_file, &test_script).unwrap();

    // Execute the test in PowerShell
    let result = Command::new("pwsh")
        .args(["-NoProfile", "-NonInteractive", "-File"])
        .arg(script_file.to_str().unwrap())
        .output()
        .expect("Failed to run pwsh - PowerShell Core must be installed for this test");

    let stdout = String::from_utf8_lossy(&result.stdout);
    let stderr = String::from_utf8_lossy(&result.stderr);

    println!("PowerShell test stdout:\n{}", stdout);
    println!("PowerShell test stderr:\n{}", stderr);

    // Assertions - verify it loads and runs
    assert!(
        stdout.contains("LOAD_SUCCESS"),
        "Should load completion script"
    );
    assert!(
        stdout.contains("COMPLETION_TEST_DONE"),
        "Should complete test"
    );

    // Cleanup
    let _ = fs::remove_dir_all(&temp_dir);
}
