use std::env;
use std::fs;
use std::path::{Path, PathBuf};
use std::process::{Command, Output};
use std::sync::atomic::{AtomicUsize, Ordering};

/// The program to run for `shell`, honouring the same `USAGECLI_SHELL_<SHELL>` override the CLI
/// itself reads (`shell_program_override` in `cli/src/env.rs`), and the legacy
/// `USAGE_SHELL_<SHELL>` behind it.
///
/// It matters on Windows, where the executable search order puts the system directory ahead of
/// `PATH` and installing WSL puts `bash.exe` there — so a bare `bash` is the WSL launcher,
/// which cannot open the Windows paths these tests write. Pointing the variable at a real bash
/// is what makes them runnable. Unset, which is every other platform, means the bare name.
///
/// The current spelling is also the one that survives `mise run`, which clears `usage_*` from a
/// task's environment — so it is what lets these tests be driven by `mise r test` rather than by
/// a bare `cargo test`.
fn shell_program(shell: &str) -> String {
    let upper = shell.to_ascii_uppercase();
    [
        format!("USAGECLI_SHELL_{upper}"),
        format!("USAGE_SHELL_{upper}"),
    ]
    .iter()
    .find_map(|key| {
        let value = env::var(key).ok()?;
        let value = value.trim().to_string();
        (!value.is_empty()).then_some(value)
    })
    .unwrap_or_else(|| shell.to_string())
}

/// A path as a POSIX shell will read it.
///
/// These tests interpolate paths into script bodies — `source "{}"`, `export PATH="{}:$PATH"` —
/// where a Windows `\` is an escape character rather than a separator, so `C:\Users\x` arrives
/// as `C:Usersx`. Every shell that can run these scripts accepts `/` instead, and on Unix this
/// changes nothing.
fn sh_path(path: &Path) -> String {
    path.display().to_string().replace('\\', "/")
}

/// A path as it must appear inside the shell's `$PATH`.
///
/// `$PATH` is colon-separated, so a Windows `C:/Users/…` splits into `C` and `/Users/…` and
/// neither resolves — a test that puts a directory there would silently look somewhere else.
/// The POSIX-flavoured shells that can run these scripts on Windows (Git Bash, MSYS2, Cygwin)
/// all ship `cygpath`, and only they know whether the answer is `/c/…`, `/cygdrive/c/…` or a
/// mount point set in fstab. Asking through `shell` rather than spawning `cygpath` directly
/// finds the one that belongs to the shell actually in use, which need not be on Windows' own
/// `PATH`.
///
/// Off Windows, and if the conversion is unavailable, the path is already what `$PATH` wants.
fn path_var_entry(shell: &str, path: &Path) -> String {
    if !cfg!(windows) {
        return sh_path(path);
    }
    Command::new(shell_program(shell))
        .args(["-c", "cygpath -u \"$1\"", "--"])
        .arg(sh_path(path))
        .output()
        .ok()
        .filter(|out| out.status.success())
        .map(|out| String::from_utf8_lossy(&out.stdout).trim().to_string())
        .filter(|converted| !converted.is_empty())
        .unwrap_or_else(|| sh_path(path))
}

/// Run `script` under `shell`, giving up after `secs` seconds.
///
/// `timeout` is spelled the same wherever these shells live, but spawning it directly walks
/// into the trap this whole file is about: Windows keeps an unrelated `timeout.exe` in the
/// system directory — a sleep, not a watchdog — and the search order puts it first. Going
/// through the shell resolves the one on *its* `PATH`, and reuses the program the guard
/// probed rather than a second, possibly different one.
fn run_with_timeout(shell: &str, secs: u32, script: &Path) -> std::io::Result<Output> {
    let program = shell_program(shell);
    Command::new(&program)
        .arg("-c")
        .arg(format!("timeout {secs} \"$0\" \"$1\""))
        .arg(&program)
        .arg(sh_path(script))
        .output()
}

/// Returns `true` if the test should be skipped because the shell cannot run a script.
///
/// Not "is it installed": a WSL `bash` on Windows answers `--version` perfectly well and then
/// fails to open the very files these tests hand it, which is how five bash tests came to fail
/// there rather than skip. Running a script from a temp directory is the precondition every
/// test in this file actually needs, so that is what gets checked.
///
/// It probes `shell_program(shell)`, which is also what every test here spawns — the guard and
/// the thing it guards must be the same executable, or an override would be honoured by one and
/// not the other.
///
/// Panics under `CI=1` (or any non-empty `CI`) to prevent silent false-positives in CI: if a
/// shell is unusable in CI it's a configuration bug, not an excuse to skip the test.
fn skip_if_shell_missing(shell: &str) -> bool {
    if shell_can_run_a_script(shell) {
        return false;
    }
    if env::var("CI").is_ok_and(|v| !v.is_empty()) {
        panic!("shell `{shell}` cannot run a script but CI is set — refusing to skip");
    }
    eprintln!(
        "Skipping {shell} test - `{}` cannot run a script from {}",
        shell_program(shell),
        env::temp_dir().display()
    );
    true
}

/// The system bash-completion library, or `None` if it isn't installed.
///
/// usage no longer embeds a copy of bash-completion, so the generated bash completion needs
/// the real one loaded before it — `_init_completion` and `__ltrim_colon_completions` come from
/// there.
///
/// Candidates are the library itself, never the `profile.d/bash_completion.sh` snippet that
/// Homebrew and others also install: that one is guarded on `$PS1` and so does nothing at all
/// in a non-interactive shell, which is the only kind these tests run. Sourcing it would look
/// like success and then fail the completion assertions. `BASH_COMPLETION_USER_DIR` is not
/// consulted either — it holds per-command completions, not the library.
///
/// `USAGE_TEST_BASH_COMPLETION` names a library directly, for testing against a copy that is
/// not installed system-wide.
fn bash_completion_candidates() -> Vec<PathBuf> {
    let mut candidates: Vec<PathBuf> = Vec::new();
    if let Some(path) = env::var_os("USAGE_TEST_BASH_COMPLETION") {
        candidates.push(PathBuf::from(path));
    }
    // Whichever Homebrew is installed: `/opt/homebrew` on Apple Silicon, `/usr/local` on Intel,
    // and anywhere else if the user relocated it.
    if let Some(prefix) = env::var_os("HOMEBREW_PREFIX") {
        candidates.push(PathBuf::from(prefix).join("share/bash-completion/bash_completion"));
    }
    candidates.extend(
        [
            "/usr/share/bash-completion/bash_completion",
            "/usr/local/share/bash-completion/bash_completion",
            "/opt/homebrew/share/bash-completion/bash_completion",
        ]
        .into_iter()
        .map(PathBuf::from),
    );
    candidates
}

/// Returns the first candidate that a non-interactive bash can actually load `_init_completion`
/// from.
///
/// Existing on disk is not the property the test needs — being sourceable into the shell the
/// test spawns is. Probing for it means a wrapper, a version too old to define the function, or
/// a path guess that turns out wrong is rejected here rather than surfacing as a puzzling
/// completion assertion failure further down.
fn system_bash_completion() -> Option<PathBuf> {
    bash_completion_candidates().into_iter().find(|candidate| {
        candidate.is_file()
            && Command::new(shell_program("bash"))
                .args(["-c", "source \"$1\" && declare -F _init_completion", "--"])
                .arg(sh_path(candidate))
                .output()
                .is_ok_and(|out| out.status.success())
    })
}

/// Returns `Some(path)` to the system bash-completion, or `None` if the test should be skipped.
///
/// Panics under `CI` for the same reason [`skip_if_shell_missing`] does: bash-completion is
/// installed by the workflow, so a run that quietly skipped this test would be reporting a
/// green bash suite that never exercised a completion.
///
/// What CI finds is `ubuntu-latest`'s package, which is 2.11 — deliberately the oldest version
/// the generated script claims to support, so the claim is tested rather than asserted.
fn bash_completion_or_skip() -> Option<PathBuf> {
    if let Some(path) = system_bash_completion() {
        return Some(path);
    }
    if env::var("CI").is_ok_and(|v| !v.is_empty()) {
        panic!(
            "no usable bash-completion but CI is set — refusing to skip. Tried: {:?}",
            bash_completion_candidates()
        );
    }
    eprintln!("Skipping bash completion test - no usable bash-completion library found");
    None
}

fn shell_can_run_a_script(shell: &str) -> bool {
    static NEXT: AtomicUsize = AtomicUsize::new(0);
    let dir = env::temp_dir().join(format!(
        "usage_shell_probe_{}_{}",
        std::process::id(),
        NEXT.fetch_add(1, Ordering::Relaxed)
    ));
    if fs::create_dir_all(&dir).is_err() {
        return false;
    }
    // `echo ok` is the one thing bash, zsh, fish and pwsh all spell the same way. pwsh is the
    // only one that insists on the extension.
    let script = dir.join(if shell == "pwsh" {
        "probe.ps1"
    } else {
        "probe"
    });
    let usable = fs::write(&script, "echo ok\n").is_ok()
        && script_command(shell, &script).output().is_ok_and(|out| {
            out.status.success() && String::from_utf8_lossy(&out.stdout).trim() == "ok"
        });
    let _ = fs::remove_dir_all(&dir);
    usable
}

/// The command that runs `script` under `shell`.
///
/// One place, because the probe above and the tests it guards have to agree on more than the
/// program: a guard that invokes the shell differently can call a working shell unusable, or
/// miss one that is. pwsh is where they would have diverged — it needs `-File` to take a script
/// path at all, and `-NoProfile -NonInteractive` so a developer's profile cannot add output or
/// a prompt to a run whose stdout is being compared.
fn script_command(shell: &str, script: &Path) -> Command {
    let mut cmd = Command::new(shell_program(shell));
    if shell == "pwsh" {
        cmd.args(["-NoProfile", "-NonInteractive", "-File"]);
    }
    cmd.arg(sh_path(script));
    cmd
}

/// Path to a checked-in generated completion under `cli/assets/completions/`.
/// These are what users actually source, so the shell-function collision tests
/// assert against them rather than a freshly generated stand-in.
fn checked_in_completion(name: &str) -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("assets")
        .join("completions")
        .join(name)
}

/// Helper to run usage complete-word and return stdout
fn run_complete_word(usage_bin: &Path, shell: &str, spec_file: &Path, words: &[&str]) -> String {
    run_complete_word_in(usage_bin, shell, spec_file, words, None)
}

fn run_complete_word_in(
    usage_bin: &Path,
    shell: &str,
    spec_file: &Path,
    words: &[&str],
    cwd: Option<&Path>,
) -> String {
    let mut args = vec![
        "complete-word".to_string(),
        "--shell".to_string(),
        shell.to_string(),
        "-f".to_string(),
        spec_file.to_str().unwrap().to_string(),
        "--".to_string(),
    ];
    args.extend(words.iter().map(|w| w.to_string()));

    let mut command = Command::new(usage_bin);
    command.args(&args);
    if let Some(cwd) = cwd {
        command.current_dir(cwd);
    }
    let output = command.output().expect("Failed to run usage complete-word");

    String::from_utf8_lossy(&output.stdout).to_string()
}

/// Build the usage binary and return its path
fn build_usage_binary() -> PathBuf {
    if let Some(usage_path) = std::env::var("CARGO_BIN_EXE_usage")
        .ok()
        .filter(|s| !s.is_empty())
    {
        return PathBuf::from(usage_path);
    }

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
    if skip_if_shell_missing("fish") {
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
        sh_path(usage_bin.parent().unwrap()),
        sh_path(&comp_file),
        sh_path(&temp_dir)
    );

    let script_file = temp_dir.join("test.fish");
    fs::write(&script_file, &test_script).unwrap();

    // Execute the test in fish
    let result = script_command("fish", &script_file)
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
    if skip_if_shell_missing("bash") {
        return;
    }
    let Some(bash_completion) = bash_completion_or_skip() else {
        return;
    };

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

    let output = Command::new(&usage_bin)
        .args(["generate", "completion", "bash", "testcli"])
        .arg("-f")
        .arg(spec_kdl_file.to_str().unwrap())
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

# bash-completion first: the generated completion calls its functions.
source {}
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
        path_var_entry("bash", usage_bin.parent().unwrap()),
        sh_path(&bash_completion),
        sh_path(&comp_file),
        sh_path(&temp_dir)
    );

    let script_file = temp_dir.join("test.sh");
    fs::write(&script_file, &test_script).unwrap();

    // Execute the test
    let result = script_command("bash", &script_file)
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
    if skip_if_shell_missing("zsh") {
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
export XDG_CACHE_HOME="{}"

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
zpty -r comptest REPLY $'*\002' 2>/dev/null

# Read actual completions
zpty -r comptest REPLY $'*\003' 2>/dev/null

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
        path_var_entry("zsh", usage_bin.parent().unwrap()),
        sh_path(&temp_dir),
        sh_path(&comp_file)
    );

    let script_file = temp_dir.join("test.zsh");
    fs::write(&script_file, &test_script).unwrap();

    // Execute the test with a timeout: the script drives a pty, which can hang.
    let result = run_with_timeout("zsh", 5, &script_file).expect("Failed to run zsh test");

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

/// Regression test for https://github.com/jdx/usage/issues/634
///
/// `usage complete-word --shell zsh` emits two tab-separated columns per
/// match — `<display>\t<insert>` — and the generated completion script feeds
/// those to `_describe ... -U -Q -S ''` so values with spaces are inserted
/// with consistent single-quote quoting (e.g. `'Alice Alice'`) instead of
/// zsh's default mix of backslash and single-quote styles.
#[test]
fn test_zsh_completion_quotes_choices_with_spaces() {
    if skip_if_shell_missing("zsh") {
        return;
    }

    let usage_bin = build_usage_binary();

    let temp_dir = env::temp_dir().join(format!(
        "usage_zsh_choices_quote_test_{}",
        std::process::id()
    ));
    fs::create_dir_all(&temp_dir).unwrap();

    let spec = r#"
arg "<course>" required {
  choices "A B & C"
}
arg "<recipient>" required {
  choices "Alice Alice" "Bob Bob" "Carol Carol"
}
"#;
    let spec_kdl_file = temp_dir.join("testcli.kdl");
    fs::write(&spec_kdl_file, spec).unwrap();

    // 1. The raw `complete-word --shell zsh` output for the recipient arg
    //    should be tab-separated, with the insert column pre-quoted.
    let raw = Command::new(&usage_bin)
        .args(["complete-word", "--shell", "zsh", "-f"])
        .arg(spec_kdl_file.to_str().unwrap())
        .args(["--", "testcli", "A B & C", ""])
        .output()
        .expect("Failed to run complete-word");
    let raw_stdout = String::from_utf8_lossy(&raw.stdout);
    let expected_lines = [
        "Alice Alice\t\t'Alice Alice'",
        "Bob Bob\t\t'Bob Bob'",
        "Carol Carol\t\t'Carol Carol'",
    ];
    for line in expected_lines {
        assert!(
            raw_stdout.lines().any(|l| l == line),
            "Expected `{line}` in complete-word output, got:\n{raw_stdout}"
        );
    }

    // 2. The simple unquoted case still emits the three tab-separated columns
    //    (value, empty description, raw insert with no surrounding quotes).
    let raw_simple = Command::new(&usage_bin)
        .args(["complete-word", "--shell", "zsh", "-s"])
        .arg(r#"arg "<env>" { choices "dev" "prod" }"#)
        .args(["--", "testcli", ""])
        .output()
        .expect("Failed to run complete-word");
    let simple_stdout = String::from_utf8_lossy(&raw_simple.stdout);
    assert!(
        simple_stdout.lines().any(|l| l == "dev\t\tdev"),
        "Expected `dev\\t\\tdev` in complete-word output, got:\n{simple_stdout}"
    );

    // 3. The generated zsh completion script wires the three columns into
    //    `compadd -U -Q -d ...` so zsh inserts the pre-quoted value verbatim
    //    without re-filtering.
    let gen = Command::new(&usage_bin)
        .args(["generate", "completion", "zsh", "testcli", "-f"])
        .arg(spec_kdl_file.to_str().unwrap())
        .output()
        .expect("Failed to generate zsh completion");
    let script = String::from_utf8_lossy(&gen.stdout);
    let expected_fragments = [
        "(Q)words",                                            // unquote user input
        r#"${(@ps:\t:)line}"#,                                 // tab-split preserves empty fields
        "compadd -l -d _usage_display -U -Q -S '' -a inserts", // -U -Q on compadd
    ];
    for fragment in expected_fragments {
        assert!(
            script.contains(fragment),
            "Expected `{fragment}` in generated script:\n{script}"
        );
    }

    let _ = fs::remove_dir_all(&temp_dir);
}

/// Regression test: subcommands whose names contain `:` (e.g. `release:create`,
/// `release:docs-sync`, `release:pr`) must all appear in the completion menu.
///
/// The previous implementation handed `\:`-escaped `value:description` strings
/// to `_describe`, which groups matches that share a `\:`-escaped prefix and
/// surfaces only one entry per group — so out of four `release:*` subcommands
/// only `release:create` would show. The fix switches to `compadd` driven by
/// three-column output; this test stubs `compadd` to capture the inserts array
/// and asserts every colon-named subcommand is passed individually.
#[test]
fn test_zsh_completion_includes_all_colon_subcommands() {
    if skip_if_shell_missing("zsh") {
        return;
    }

    let usage_bin = build_usage_binary();
    let temp_dir =
        env::temp_dir().join(format!("usage_zsh_colon_subs_test_{}", std::process::id()));
    fs::create_dir_all(&temp_dir).unwrap();

    let spec = r#"
bin "testcli"
cmd "release:create" help="Create release"
cmd "release:docs-sync" help="Refresh docs"
cmd "release:pr" help="Open release PR"
cmd "release:update" help="Update release state"
cmd "lint" help="Run lints"
cmd "lint:fix" help="Auto-fix lints"
"#;
    let spec_kdl_file = temp_dir.join("testcli.kdl");
    fs::write(&spec_kdl_file, spec).unwrap();

    let gen = Command::new(&usage_bin)
        .args(["generate", "completion", "zsh", "testcli", "-f"])
        .arg(spec_kdl_file.to_str().unwrap())
        .output()
        .expect("Failed to generate zsh completion");
    let comp_file = temp_dir.join("_testcli");
    fs::write(&comp_file, &gen.stdout).unwrap();

    // Drive the generated `_testcli` function directly with stubbed compadd
    // so we can inspect the inserts array without needing a real ZLE context.
    let test_script = format!(
        r#"#!/usr/bin/env zsh
set -e
export PATH="{usage_dir}:$PATH"
export XDG_CACHE_HOME="{tmp}"

autoload -U compinit
compinit -u
source {comp}

compadd() {{
    local i="${{inserts[*]}}"
    print -r -- "[compadd:inserts] $i"
}}

words=(testcli "")
CURRENT=2
_testcli
"#,
        usage_dir = path_var_entry("zsh", usage_bin.parent().unwrap()),
        tmp = sh_path(&temp_dir),
        comp = sh_path(&comp_file),
    );

    let script_file = temp_dir.join("test.zsh");
    fs::write(&script_file, &test_script).unwrap();

    let result = script_command("zsh", &script_file)
        .output()
        .expect("Failed to run zsh test");
    let stdout = String::from_utf8_lossy(&result.stdout);
    let stderr = String::from_utf8_lossy(&result.stderr);
    assert!(
        result.status.success(),
        "zsh completion script exited non-zero ({}).\nstdout:\n{stdout}\nstderr:\n{stderr}",
        result.status
    );

    let expected = [
        "release:create",
        "release:docs-sync",
        "release:pr",
        "release:update",
        "lint",
        "lint:fix",
    ];
    for sub in expected {
        assert!(
            stdout.contains(sub),
            "Expected subcommand `{sub}` in compadd inserts.\nstdout:\n{stdout}\nstderr:\n{stderr}"
        );
    }

    let _ = fs::remove_dir_all(&temp_dir);
}

/// Regression test for user zsh options leaking into generated completions.
///
/// With KSH_ARRAYS enabled, zsh arrays become zero-indexed. The generated
/// completion script indexes tab-split completion rows as zsh arrays, so it
/// must enter local zsh emulation before parsing `complete-word` output.
#[test]
fn test_zsh_completion_ignores_ksh_arrays_option() {
    if skip_if_shell_missing("zsh") {
        return;
    }

    let usage_bin = build_usage_binary();
    let temp_dir =
        env::temp_dir().join(format!("usage_zsh_ksh_arrays_test_{}", std::process::id()));
    fs::create_dir_all(&temp_dir).unwrap();

    let spec = r#"
bin "testcli"
cmd "doctor" help="Check installation"
"#;
    let spec_kdl_file = temp_dir.join("testcli.kdl");
    fs::write(&spec_kdl_file, spec).unwrap();

    let gen = Command::new(&usage_bin)
        .args(["generate", "completion", "zsh", "testcli", "-f"])
        .arg(spec_kdl_file.to_str().unwrap())
        .output()
        .expect("Failed to generate zsh completion");
    let comp_file = temp_dir.join("_testcli");
    fs::write(&comp_file, &gen.stdout).unwrap();

    let test_script = format!(
        r#"#!/usr/bin/env zsh
set -e
setopt KSH_ARRAYS
export PATH="{usage_dir}:$PATH"
export XDG_CACHE_HOME="{tmp}"

autoload -U compinit
compinit -u
source {comp}

compadd() {{
    print -r -- "[compadd:inserts] ${{inserts[*]}}"
}}

words=(testcli d)
CURRENT=2
_testcli
"#,
        usage_dir = path_var_entry("zsh", usage_bin.parent().unwrap()),
        tmp = sh_path(&temp_dir),
        comp = sh_path(&comp_file),
    );

    let script_file = temp_dir.join("test.zsh");
    fs::write(&script_file, &test_script).unwrap();

    let result = script_command("zsh", &script_file)
        .output()
        .expect("Failed to run zsh test");
    let stdout = String::from_utf8_lossy(&result.stdout);
    let stderr = String::from_utf8_lossy(&result.stderr);
    assert!(
        result.status.success(),
        "zsh completion script exited non-zero ({}).\nstdout:\n{stdout}\nstderr:\n{stderr}",
        result.status
    );
    assert!(
        stdout.contains("[compadd:inserts] doctor"),
        "Expected only the insert value in compadd inserts.\nstdout:\n{stdout}\nstderr:\n{stderr}"
    );

    let _ = fs::remove_dir_all(&temp_dir);
}

#[test]
fn test_powershell_completion_integration() {
    if skip_if_shell_missing("pwsh") {
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
        sh_path(usage_bin.parent().unwrap()),
        sh_path(&temp_dir),
        sh_path(&comp_file),
        sh_path(&spec_file)
    );

    let script_file = temp_dir.join("test.ps1");
    fs::write(&script_file, &test_script).unwrap();

    // Execute the test in PowerShell
    let result = script_command("pwsh", &script_file)
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

#[test]
fn test_zsh_complete_word_output_format() {
    let usage_bin = build_usage_binary();

    let temp_dir = env::temp_dir().join(format!("usage_zsh_fmt_test_{}", std::process::id()));
    fs::create_dir_all(&temp_dir).unwrap();

    // Spec with subcommands (which have descriptions)
    let usage_spec = r#"name testcli
bin testcli
flag "-v --verbose" help="Verbose output"
arg <file> help="Input file"
cmd sub help="A subcommand"
cmd other help="Another subcommand"
"#;
    let spec_file = temp_dir.join("test.spec");
    fs::write(&spec_file, usage_spec).unwrap();

    // Test zsh output format: each line is `<value>\t<description>\t<insert>`
    // where <value> and <description> are the raw strings (no escaping) and
    // <insert> is the shell-quoted form for `compadd -Q`. The generated
    // completion script builds the menu label from value + description
    // directly, so no `\:` / `\(` / `\[` escaping is needed.
    let output = run_complete_word(&usage_bin, "zsh", &spec_file, &["testcli", ""]);
    let lines: Vec<&str> = output.lines().collect();

    assert!(
        lines.contains(&"sub\tA subcommand\tsub"),
        "Expected 'sub\\tA subcommand\\tsub' in zsh output, got: {:?}",
        lines
    );
    assert!(
        lines.contains(&"other\tAnother subcommand\tother"),
        "Expected 'other\\tAnother subcommand\\tother' in zsh output, got: {:?}",
        lines
    );

    // Cleanup
    let _ = fs::remove_dir_all(&temp_dir);
}

/// Stage a `usage`-shebang test script onto a temp `bin/` directory and
/// generate the `g completion-init <shell>` output. Returns (temp_dir,
/// bin_dir, init_script_path).
fn stage_init_test_env(usage_bin: &Path, shell: &str, label: &str) -> (PathBuf, PathBuf, PathBuf) {
    let temp_dir = env::temp_dir().join(format!("usage_{label}_init_test_{}", std::process::id()));
    let bin_dir = temp_dir.join("bin");
    fs::create_dir_all(&bin_dir).unwrap();

    let script = "\
#!/usr/bin/env -S usage bash
#USAGE bin \"ex\"
#USAGE flag \"--foo\" help=\"Flag value\"
#USAGE arg \"baz\" help=\"Positional values\"
#USAGE complete \"baz\" run=\"echo val-1; echo val-2; echo val-3\"
echo baz: $usage_baz
";
    let script_path = bin_dir.join("ex");
    fs::write(&script_path, script).unwrap();
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        let mut perms = fs::metadata(&script_path).unwrap().permissions();
        perms.set_mode(0o755);
        fs::set_permissions(&script_path, perms).unwrap();
    }

    // Generate the init script
    let output = Command::new(usage_bin)
        .args(["generate", "completion-init", shell])
        .output()
        .expect("Failed to generate completion-init");
    assert!(
        output.status.success(),
        "completion-init failed: {}",
        String::from_utf8_lossy(&output.stderr)
    );
    let init_script = temp_dir.join(format!("init.{shell}"));
    fs::write(&init_script, &output.stdout).unwrap();

    (temp_dir, bin_dir, init_script)
}

#[test]
fn test_bash_completion_init_integration() {
    if skip_if_shell_missing("bash") {
        return;
    }

    let usage_bin = build_usage_binary();
    let (temp_dir, bin_dir, init_script) = stage_init_test_env(&usage_bin, "bash", "bash");

    // Drive `_usage_default_complete` directly with simulated COMP_WORDS/CWORD.
    // This mirrors what bash does at `<Tab>` time.
    let test_script = format!(
        r#"#!/usr/bin/env bash
set -e
export PATH="{bin_dir}:{usage_dir}:$PATH"
source "{init_script}"

run_case() {{
    local label="$1"; shift
    local cword="$1"; shift
    COMP_WORDS=("$@")
    COMP_CWORD="$cword"
    COMPREPLY=()
    _usage_default_complete "${{COMP_WORDS[0]}}" "${{COMP_WORDS[$cword]}}" "${{COMP_WORDS[$((cword-1))]:-}}"
    echo "[$label] ${{COMPREPLY[*]}}"
}}

run_case empty 1 ex ""
run_case dashes 1 ex "--"
run_case foo 1 ex "--f"
"#,
        bin_dir = path_var_entry("bash", &bin_dir),
        usage_dir = path_var_entry("bash", usage_bin.parent().unwrap()),
        init_script = sh_path(&init_script),
    );
    let script_file = temp_dir.join("test.sh");
    fs::write(&script_file, &test_script).unwrap();

    let result = script_command("bash", &script_file)
        .output()
        .expect("Failed to run bash init test");
    let stdout = String::from_utf8_lossy(&result.stdout);
    let stderr = String::from_utf8_lossy(&result.stderr);
    println!("bash init stdout:\n{stdout}\nstderr:\n{stderr}");

    assert!(
        result.status.success(),
        "bash init script exited non-zero. stderr: {stderr}"
    );
    assert!(
        stdout.contains("[empty] val-1 val-2 val-3"),
        "expected positional completion, got: {stdout}"
    );
    assert!(
        stdout.contains("[dashes] --foo"),
        "expected flag listing for `--`, got: {stdout}"
    );
    assert!(
        stdout.contains("[foo] --foo"),
        "expected `--foo` for `--f`, got: {stdout}"
    );

    let _ = fs::remove_dir_all(&temp_dir);
}

#[test]
fn test_zsh_completion_init_integration() {
    if skip_if_shell_missing("zsh") {
        return;
    }

    let usage_bin = build_usage_binary();
    let (temp_dir, bin_dir, init_script) = stage_init_test_env(&usage_bin, "zsh", "zsh");

    // Stub `compadd`/`_files` to capture what the handler offers without
    // needing an interactive ZLE context. Drive with $words/$CURRENT.
    // The fallback must preserve completion-friendly options for `_files`.
    // The init template calls `compadd -l -d <display-arr> -U -Q -S '' -a <inserts-arr>`.
    let test_script = format!(
        r#"#!/usr/bin/env zsh
set -e
export PATH="{bin_dir}:{usage_dir}:$PATH"
autoload -U compinit
compinit -u
source "{init_script}"

compadd() {{
    local d="${{_usage_display[*]}}"
    local i="${{inserts[*]}}"
    print -r -- "[compadd:display] $d"
    print -r -- "[compadd:inserts] $i"
}}
_files() {{
    print -r -- "[files-fallback] nomatch=$options[nomatch] extendedglob=$options[extendedglob]"
}}

words=(ex "")
CURRENT=2
_usage_default_complete

words=(ex "--f")
CURRENT=2
_usage_default_complete

words=(plain "")
CURRENT=2
_usage_default_complete
"#,
        bin_dir = path_var_entry("zsh", &bin_dir),
        usage_dir = path_var_entry("zsh", usage_bin.parent().unwrap()),
        init_script = sh_path(&init_script),
    );
    let script_file = temp_dir.join("test.zsh");
    fs::write(&script_file, &test_script).unwrap();

    let result = script_command("zsh", &script_file)
        .output()
        .expect("Failed to run zsh init test");
    let stdout = String::from_utf8_lossy(&result.stdout);
    let stderr = String::from_utf8_lossy(&result.stderr);
    println!("zsh init stdout:\n{stdout}\nstderr:\n{stderr}");

    assert!(
        result.status.success(),
        "zsh init script exited non-zero. stderr: {stderr}"
    );
    assert!(
        stdout.contains("[compadd:inserts] val-1 val-2 val-3"),
        "expected positional completions in compadd inserts, got: {stdout}"
    );
    assert!(
        stdout.contains("[compadd:display]") && stdout.contains("--foo"),
        "expected --foo flag in compadd display, got: {stdout}"
    );
    assert!(
        stdout.contains("[files-fallback] nomatch=off extendedglob=on"),
        "expected file fallback to disable nomatch and enable extendedglob, got: {stdout}"
    );

    let _ = fs::remove_dir_all(&temp_dir);
}

/// Regression: init script (`compdef -default-` handler) must surface every
/// colon-named subcommand, not collapse them via `_describe` grouping.
/// Mirrors `test_zsh_completion_includes_all_colon_subcommands` but exercises
/// the shebang/init path instead of a per-bin `compdef` registration.
#[test]
fn test_zsh_init_completion_includes_all_colon_subcommands() {
    if skip_if_shell_missing("zsh") {
        return;
    }

    let usage_bin = build_usage_binary();
    let temp_dir = env::temp_dir().join(format!(
        "usage_zsh_init_colon_subs_test_{}",
        std::process::id()
    ));
    let bin_dir = temp_dir.join("bin");
    fs::create_dir_all(&bin_dir).unwrap();

    let script = "\
#!/usr/bin/env -S usage bash
#USAGE bin \"testcli\"
#USAGE cmd \"release:create\" help=\"Create release\"
#USAGE cmd \"release:docs-sync\" help=\"Refresh docs\"
#USAGE cmd \"release:pr\" help=\"Open release PR\"
#USAGE cmd \"release:update\" help=\"Update release state\"
#USAGE cmd \"lint\" help=\"Run lints\"
#USAGE cmd \"lint:fix\" help=\"Auto-fix lints\"
";
    let script_path = bin_dir.join("testcli");
    fs::write(&script_path, script).unwrap();
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        let mut perms = fs::metadata(&script_path).unwrap().permissions();
        perms.set_mode(0o755);
        fs::set_permissions(&script_path, perms).unwrap();
    }

    let gen = Command::new(&usage_bin)
        .args(["generate", "completion-init", "zsh"])
        .output()
        .expect("Failed to generate completion-init");
    assert!(
        gen.status.success(),
        "completion-init failed: {}",
        String::from_utf8_lossy(&gen.stderr)
    );
    let init_script = temp_dir.join("init.zsh");
    fs::write(&init_script, &gen.stdout).unwrap();

    let test_script = format!(
        r#"#!/usr/bin/env zsh
set -e
export PATH="{bin_dir}:{usage_dir}:$PATH"
export XDG_CACHE_HOME="{tmp}"

autoload -U compinit
compinit -u
source "{init}"

compadd() {{
    local i="${{inserts[*]}}"
    print -r -- "[compadd:inserts] $i"
}}

words=(testcli "")
CURRENT=2
_usage_default_complete
"#,
        bin_dir = path_var_entry("zsh", &bin_dir),
        usage_dir = path_var_entry("zsh", usage_bin.parent().unwrap()),
        tmp = sh_path(&temp_dir),
        init = sh_path(&init_script),
    );

    let script_file = temp_dir.join("test.zsh");
    fs::write(&script_file, &test_script).unwrap();

    let result = script_command("zsh", &script_file)
        .output()
        .expect("Failed to run zsh init test");
    let stdout = String::from_utf8_lossy(&result.stdout);
    let stderr = String::from_utf8_lossy(&result.stderr);
    assert!(
        result.status.success(),
        "zsh init completion script exited non-zero ({}).\nstdout:\n{stdout}\nstderr:\n{stderr}",
        result.status
    );

    let expected = [
        "release:create",
        "release:docs-sync",
        "release:pr",
        "release:update",
        "lint",
        "lint:fix",
    ];
    for sub in expected {
        assert!(
            stdout.contains(sub),
            "Expected subcommand `{sub}` in init-path compadd inserts.\nstdout:\n{stdout}\nstderr:\n{stderr}"
        );
    }

    let _ = fs::remove_dir_all(&temp_dir);
}

#[test]
fn test_fish_completion_init_integration() {
    if skip_if_shell_missing("fish") {
        return;
    }

    let usage_bin = build_usage_binary();
    let (temp_dir, bin_dir, init_script) = stage_init_test_env(&usage_bin, "fish", "fish");

    // Fish: source the init (which scans $PATH), then verify `complete -C`
    // produces the expected output. We restrict $PATH to the test bin dir
    // plus coreutils so the scan stays bounded.
    let test_script = format!(
        r#"#!/usr/bin/env fish
set -gx PATH "{bin_dir}" "{usage_dir}" /usr/bin /bin
source "{init_script}"

if not complete -c ex | string match -q -- '*usage*'
    echo "FAIL: completion not registered for ex"
    exit 1
end
echo "[registered] ex"

echo "[empty]" (complete -C 'ex ')
echo "[foo]" (complete -C 'ex --f')
"#,
        bin_dir = sh_path(&bin_dir),
        usage_dir = sh_path(usage_bin.parent().unwrap()),
        init_script = sh_path(&init_script),
    );
    let script_file = temp_dir.join("test.fish");
    fs::write(&script_file, &test_script).unwrap();

    let result = script_command("fish", &script_file)
        .output()
        .expect("Failed to run fish init test");
    let stdout = String::from_utf8_lossy(&result.stdout);
    let stderr = String::from_utf8_lossy(&result.stderr);
    println!("fish init stdout:\n{stdout}\nstderr:\n{stderr}");

    assert!(
        result.status.success(),
        "fish init script exited non-zero. stderr: {stderr}"
    );
    assert!(
        stdout.contains("[registered] ex"),
        "expected `ex` to be registered, got: {stdout}"
    );
    assert!(
        stdout.contains("[empty] val-1 val-2 val-3"),
        "expected positional completion, got: {stdout}"
    );
    assert!(
        stdout.contains("--foo"),
        "expected --foo for `--f`, got: {stdout}"
    );

    let _ = fs::remove_dir_all(&temp_dir);
}

// ---------------------------------------------------------------------------
// Shell-function collision tests.
//
// Two distinct ways a shell function can defeat the "is the usage CLI
// installed?" logic:
//
//   1. Function only, no executable. `type -p` (bash) and `type -q` (fish)
//      both succeed on a function, so the guard passed and the failure
//      surfaced later as something unrelated. `type -P` searches $PATH only.
//      oh-my-bash defines a `usage` function, which is how this was found.
//
//   2. Function shadowing a real executable. The guard is satisfied honestly,
//      but a bare `usage --usage-spec` still resolves to the function, so the
//      spec file gets filled with the function's output. The shipped
//      completions call `command usage` to bypass function lookup.
// ---------------------------------------------------------------------------

#[test]
fn test_bash_guard_rejects_shell_function_masquerading_as_cli() {
    if skip_if_shell_missing("bash") {
        return;
    }

    let usage_bin = build_usage_binary();
    let temp_dir = env::temp_dir().join(format!("usage_bash_guard_test_{}", std::process::id()));
    fs::create_dir_all(&temp_dir).unwrap();

    // `--usage-bin` names a probe that cannot be on $PATH, so the shell
    // function defined below is the only thing that could satisfy the guard.
    let output = Command::new(&usage_bin)
        .args([
            "generate",
            "completion",
            "bash",
            "testcli",
            "--usage-bin",
            "usage_guard_probe",
            "--usage-cmd",
            "command usage_guard_probe --usage-spec",
        ])
        .output()
        .expect("Failed to generate bash completion");
    let comp_file = temp_dir.join("testcli.bash");
    fs::write(&comp_file, &output.stdout).unwrap();

    let test_script = format!(
        r#"#!/usr/bin/env bash
usage_guard_probe() {{ echo "I am a function"; }}
source "{comp}"
_testcli testcli "" testcli 1
echo "GUARD_EXIT=$?"
"#,
        comp = sh_path(&comp_file),
    );
    let script_file = temp_dir.join("test.sh");
    fs::write(&script_file, &test_script).unwrap();

    let result = script_command("bash", &script_file)
        .output()
        .expect("Failed to run bash guard test");
    let stdout = String::from_utf8_lossy(&result.stdout);
    let stderr = String::from_utf8_lossy(&result.stderr);

    assert!(
        stdout.contains("GUARD_EXIT=1"),
        "guard should return 1 when only a shell function shadows the CLI.\nstdout:\n{stdout}\nstderr:\n{stderr}"
    );
    assert!(
        stderr.contains("usage_guard_probe CLI not found"),
        "guard should explain the CLI is missing.\nstdout:\n{stdout}\nstderr:\n{stderr}"
    );

    let _ = fs::remove_dir_all(&temp_dir);
}

#[test]
fn test_bash_guard_reports_missing_bash_completion() {
    if skip_if_shell_missing("bash") {
        return;
    }

    // usage no longer ships a copy of bash-completion, so "it isn't installed" is a
    // reachable state for anyone sourcing a generated completion. It has to say so
    // rather than fail as `_init_completion: command not found`.
    let usage_bin = build_usage_binary();
    let temp_dir = env::temp_dir().join(format!("usage_bash_nobc_test_{}", std::process::id()));
    fs::create_dir_all(&temp_dir).unwrap();

    let output = Command::new(&usage_bin)
        .args(["generate", "completion", "bash", "testcli"])
        .args(["--usage-cmd", "command usage --usage-spec"])
        .output()
        .expect("Failed to generate bash completion");
    let comp_file = temp_dir.join("testcli.bash");
    fs::write(&comp_file, &output.stdout).unwrap();

    // A non-interactive bash loads no bash-completion of its own, so sourcing only the
    // generated script is exactly the state being tested.
    let test_script = format!(
        r#"#!/usr/bin/env bash
export PATH="{path}:$PATH"
source "{comp}"
_testcli testcli "" testcli 1
echo "GUARD_EXIT=$?"
"#,
        path = path_var_entry("bash", usage_bin.parent().unwrap()),
        comp = sh_path(&comp_file),
    );
    let script_file = temp_dir.join("test.sh");
    fs::write(&script_file, &test_script).unwrap();

    let result = script_command("bash", &script_file)
        .output()
        .expect("Failed to run bash test");
    let stdout = String::from_utf8_lossy(&result.stdout);
    let stderr = String::from_utf8_lossy(&result.stderr);

    assert!(
        stdout.contains("GUARD_EXIT=1"),
        "guard should return 1 when bash-completion is not loaded.\nstdout:\n{stdout}\nstderr:\n{stderr}"
    );
    assert!(
        stderr.contains("bash-completion is required"),
        "guard should name bash-completion.\nstdout:\n{stdout}\nstderr:\n{stderr}"
    );
    assert!(
        !stderr.contains("_init_completion: command not found"),
        "the guard should fire before bash reports a missing function.\nstderr:\n{stderr}"
    );

    let _ = fs::remove_dir_all(&temp_dir);
}

#[test]
fn test_bash_self_completion_uses_executable_not_shadowing_function() {
    if skip_if_shell_missing("bash") {
        return;
    }

    let usage_bin = build_usage_binary();
    let temp_dir = env::temp_dir().join(format!("usage_bash_shadow_test_{}", std::process::id()));
    fs::create_dir_all(&temp_dir).unwrap();

    // The real usage binary is on $PATH *and* a `usage` function is defined,
    // so the guard passes legitimately — only the spec call can go wrong.
    // bash-completion isn't loaded here; stub the helpers it provides so the spec
    // write is the only thing under test.
    let test_script = format!(
        r#"#!/usr/bin/env bash
export PATH="{usage_dir}:$PATH"
export XDG_CACHE_HOME="{cache}"
usage() {{ echo "FUNCTION_MARKER"; }}
_init_completion() {{ return 0; }}
__ltrim_colon_completions() {{ return 0; }}
source "{asset}"
_usage usage "" usage 1
echo "SPEC_BEGIN"
cat "$XDG_CACHE_HOME/usage/usage__usage_spec_usage.spec"
"#,
        usage_dir = path_var_entry("bash", usage_bin.parent().unwrap()),
        cache = sh_path(&temp_dir),
        asset = sh_path(&checked_in_completion("usage.bash")),
    );
    let script_file = temp_dir.join("test.sh");
    fs::write(&script_file, &test_script).unwrap();

    let result = script_command("bash", &script_file)
        .output()
        .expect("Failed to run bash shadow test");
    let stdout = String::from_utf8_lossy(&result.stdout);
    let stderr = String::from_utf8_lossy(&result.stderr);

    assert!(
        !stdout.contains("FUNCTION_MARKER"),
        "spec file was written by the shell function instead of the CLI.\nstdout:\n{stdout}\nstderr:\n{stderr}"
    );
    assert!(
        // Canonical KDL leaves a safe identifier bare. The spec still comes from the
        // CLI's own derive rather than from the shell function or clap bridge.
        stdout.contains("bin usage"),
        "spec file should hold the real usage spec.\nstdout:\n{stdout}\nstderr:\n{stderr}"
    );

    let _ = fs::remove_dir_all(&temp_dir);
}

#[test]
fn test_bash_init_guard_rejects_shell_function() {
    if skip_if_shell_missing("bash") {
        return;
    }

    let usage_bin = build_usage_binary();
    let (temp_dir, bin_dir, init_script) =
        stage_init_test_env(&usage_bin, "bash", "bash_guard_init");

    // $PATH deliberately omits the usage binary. A pre-registered `complete -D`
    // handler gives us a positive signal: the init handler must chain to it
    // rather than dispatch to a `usage` that is only a shell function.
    let test_script = format!(
        r#"#!/usr/bin/env bash
export PATH="{bin_dir}:/usr/bin:/bin"
usage() {{ echo "I am a function"; }}
_test_chained_handler() {{ echo "FELL_THROUGH"; }}
complete -D -F _test_chained_handler
source "{init_script}"

COMP_WORDS=(ex "")
COMP_CWORD=1
COMPREPLY=()
_usage_default_complete ex "" ex
echo "INIT_EXIT=$?"
"#,
        bin_dir = path_var_entry("bash", &bin_dir),
        init_script = sh_path(&init_script),
    );
    let script_file = temp_dir.join("test.sh");
    fs::write(&script_file, &test_script).unwrap();

    let result = script_command("bash", &script_file)
        .output()
        .expect("Failed to run bash init guard test");
    let stdout = String::from_utf8_lossy(&result.stdout);
    let stderr = String::from_utf8_lossy(&result.stderr);

    assert!(
        stdout.contains("FELL_THROUGH"),
        "init handler should chain to the previous default handler instead of dispatching to a shell function.\nstdout:\n{stdout}\nstderr:\n{stderr}"
    );
    assert!(
        !stderr.contains("command not found"),
        "init handler dispatched to a missing CLI.\nstdout:\n{stdout}\nstderr:\n{stderr}"
    );

    let _ = fs::remove_dir_all(&temp_dir);
}

#[test]
fn test_fish_guard_rejects_shell_function_masquerading_as_cli() {
    if skip_if_shell_missing("fish") {
        return;
    }

    let usage_bin = build_usage_binary();
    let temp_dir = env::temp_dir().join(format!("usage_fish_guard_test_{}", std::process::id()));
    fs::create_dir_all(&temp_dir).unwrap();

    let output = Command::new(&usage_bin)
        .args([
            "generate",
            "completion",
            "fish",
            "testcli",
            "--usage-bin",
            "usage_guard_probe",
            "--usage-cmd",
            "command usage_guard_probe --usage-spec",
        ])
        .output()
        .expect("Failed to generate fish completion");
    let comp_file = temp_dir.join("testcli.fish");
    fs::write(&comp_file, &output.stdout).unwrap();

    let test_script = format!(
        r#"#!/usr/bin/env fish
function usage_guard_probe
    echo "I am a function"
end
source "{comp}"
echo "[completions]" (complete -c testcli | string collect)
"#,
        comp = sh_path(&comp_file),
    );
    let script_file = temp_dir.join("test.fish");
    fs::write(&script_file, &test_script).unwrap();

    let result = script_command("fish", &script_file)
        .output()
        .expect("Failed to run fish guard test");
    let stdout = String::from_utf8_lossy(&result.stdout);
    let stderr = String::from_utf8_lossy(&result.stderr);

    assert!(
        stderr.contains("usage_guard_probe CLI not found"),
        "guard should explain the CLI is missing.\nstdout:\n{stdout}\nstderr:\n{stderr}"
    );
    assert!(
        stdout.contains("[completions]") && !stdout.contains("complete-word"),
        "no completion should be registered when the CLI is absent.\nstdout:\n{stdout}\nstderr:\n{stderr}"
    );

    let _ = fs::remove_dir_all(&temp_dir);
}

#[test]
fn test_fish_self_completion_uses_executable_not_shadowing_function() {
    if skip_if_shell_missing("fish") {
        return;
    }

    let usage_bin = build_usage_binary();
    let temp_dir = env::temp_dir().join(format!("usage_fish_shadow_test_{}", std::process::id()));
    fs::create_dir_all(&temp_dir).unwrap();

    let test_script = format!(
        r#"#!/usr/bin/env fish
set -gx PATH "{usage_dir}" /usr/bin /bin
set -gx XDG_CACHE_HOME "{cache}"
function usage
    echo "FUNCTION_MARKER"
end
source "{asset}"
echo "SPEC_BEGIN"
cat "$XDG_CACHE_HOME/usage/usage__usage_spec_usage.spec"
"#,
        usage_dir = sh_path(usage_bin.parent().unwrap()),
        cache = sh_path(&temp_dir),
        asset = sh_path(&checked_in_completion("usage.fish")),
    );
    let script_file = temp_dir.join("test.fish");
    fs::write(&script_file, &test_script).unwrap();

    let result = script_command("fish", &script_file)
        .output()
        .expect("Failed to run fish shadow test");
    let stdout = String::from_utf8_lossy(&result.stdout);
    let stderr = String::from_utf8_lossy(&result.stderr);

    assert!(
        !stdout.contains("FUNCTION_MARKER"),
        "spec file was written by the shell function instead of the CLI.\nstdout:\n{stdout}\nstderr:\n{stderr}"
    );
    assert!(
        // Canonical KDL leaves a safe identifier bare. The spec still comes from the
        // CLI's own derive rather than from the shell function or clap bridge.
        stdout.contains("bin usage"),
        "spec file should hold the real usage spec.\nstdout:\n{stdout}\nstderr:\n{stderr}"
    );

    let _ = fs::remove_dir_all(&temp_dir);
}

#[test]
fn test_fish_init_guard_rejects_shell_function() {
    if skip_if_shell_missing("fish") {
        return;
    }

    let usage_bin = build_usage_binary();
    let (temp_dir, bin_dir, init_script) =
        stage_init_test_env(&usage_bin, "fish", "fish_guard_init");

    // $PATH omits the usage binary but still holds the `ex` shebang script, so
    // the scan would register `ex` if the guard let a shell function through.
    let test_script = format!(
        r#"#!/usr/bin/env fish
set -gx PATH "{bin_dir}" /usr/bin /bin
function usage
    echo "I am a function"
end
source "{init_script}"
echo "[registered]" (complete -c ex | string collect)
"#,
        bin_dir = sh_path(&bin_dir),
        init_script = sh_path(&init_script),
    );
    let script_file = temp_dir.join("test.fish");
    fs::write(&script_file, &test_script).unwrap();

    let result = script_command("fish", &script_file)
        .output()
        .expect("Failed to run fish init guard test");
    let stdout = String::from_utf8_lossy(&result.stdout);
    let stderr = String::from_utf8_lossy(&result.stderr);

    assert!(
        stdout.contains("[registered]") && !stdout.contains("complete-word"),
        "init scan should register nothing when only a shell function shadows the CLI.\nstdout:\n{stdout}\nstderr:\n{stderr}"
    );

    let _ = fs::remove_dir_all(&temp_dir);
}

#[test]
fn test_complete_path_adds_trailing_slash_for_directories() {
    let usage_bin = build_usage_binary();

    let temp_dir = env::temp_dir().join(format!("usage_path_test_{}", std::process::id()));
    fs::create_dir_all(&temp_dir).unwrap();

    // Create a test directory structure
    let test_dir = temp_dir.join("testdir");
    fs::create_dir_all(test_dir.join("subdir")).unwrap();
    fs::write(test_dir.join("file.txt"), "hello").unwrap();

    // Spec with a path-type arg
    let usage_spec = r#"name testcli
bin testcli
arg <path>
complete path type="path"
"#;
    let spec_file = temp_dir.join("test.spec");
    fs::write(&spec_file, usage_spec).unwrap();

    // Complete with a path prefix pointing to our test directory
    let test_dir_str = format!("{}/", test_dir.to_str().unwrap());
    let output = run_complete_word(&usage_bin, "bash", &spec_file, &["testcli", &test_dir_str]);
    let lines: Vec<&str> = output.lines().collect();

    // Directory should have trailing slash
    assert!(
        lines.iter().any(|l| l.ends_with("subdir/")),
        "Expected directory completion to end with '/', got: {:?}",
        lines
    );

    // File should NOT have trailing slash
    assert!(
        lines.iter().any(|l| l.ends_with("file.txt")),
        "Expected file completion without trailing '/', got: {:?}",
        lines
    );

    // Cleanup
    let _ = fs::remove_dir_all(&temp_dir);
}

#[test]
fn test_complete_path_preserves_partially_typed_segments() {
    static NEXT: AtomicUsize = AtomicUsize::new(0);
    let usage_bin = build_usage_binary();
    let temp_dir = env::temp_dir().join(format!(
        "usage_segment_path_test_{}_{}",
        std::process::id(),
        NEXT.fetch_add(1, Ordering::Relaxed)
    ));
    fs::create_dir_all(temp_dir.join("target/debug/incremental")).unwrap();

    let spec_file = temp_dir.join("test.spec");
    fs::write(
        &spec_file,
        "name testcli\nbin testcli\narg <path>\ncomplete path type=\"path\"\n",
    )
    .unwrap();

    let output = run_complete_word_in(
        &usage_bin,
        "bash",
        &spec_file,
        &["testcli", "target/de/inc"],
        Some(&temp_dir),
    );
    assert_eq!(output, "target/debug/incremental/\n");

    let output = run_complete_word_in(
        &usage_bin,
        "bash",
        &spec_file,
        &["testcli", "tar/"],
        Some(&temp_dir),
    );
    assert_eq!(output, "target/\n");

    let output = run_complete_word_in(
        &usage_bin,
        "bash",
        &spec_file,
        &["testcli", "target/de/"],
        Some(&temp_dir),
    );
    assert_eq!(output, "target/debug/\n");

    let _ = fs::remove_dir_all(&temp_dir);
}
