//! `usage bash`: what follows a `--` belongs to the script, and `-h` never reads it as a request.
#![cfg(unix)]

use assert_cmd::Command;
use assert_cmd::cargo;

const SCRIPT: &str = r#"#!/usr/bin/env -S usage bash
#USAGE arg "[a]..."
echo "a=${usage_a}"
"#;

fn run(args: &[&str]) -> Command {
    let mut cmd = Command::new(cargo::cargo_bin!("usage"));
    cmd.env_remove("USAGECLI_SHELL_BASH")
        .env_remove("USAGE_SHELL_BASH")
        .args(["bash", "/dev/stdin"])
        .args(args)
        .write_stdin(SCRIPT);
    cmd
}

#[test]
fn help_words_after_the_separator_are_values() {
    for word in ["--help", "-h"] {
        run(&["positional", "--", word])
            .assert()
            .success()
            .stdout(format!("a=positional {word}\n"));
        run(&["--", word])
            .assert()
            .success()
            .stdout(format!("a={word}\n"));
    }
}

#[test]
fn help_words_before_the_separator_still_print_the_help_page() {
    for word in ["--help", "-h"] {
        run(&["positional", word])
            .assert()
            .success()
            .stdout(predicates::str::starts_with("Usage:"));
    }
}

const ARGV_SCRIPT: &str =
    "#!/usr/bin/env -S usage bash\n#USAGE arg \"[a]...\"\necho \"argv=[$*]\"\n";

fn run_argv(args: &[&str]) -> Command {
    let mut cmd = Command::new(cargo::cargo_bin!("usage"));
    cmd.env_remove("USAGECLI_SHELL_BASH")
        .env_remove("USAGE_SHELL_BASH")
        .args(args)
        .write_stdin(ARGV_SCRIPT);
    cmd
}

#[test]
fn the_script_keeps_a_separator_that_is_its_own() {
    run_argv(&["bash", "/dev/stdin", "x", "--", "--", "y"])
        .assert()
        .success()
        .stdout("argv=[x -- y]\n");
    run_argv(&["bash", "--", "/dev/stdin", "--", "value"])
        .assert()
        .success()
        .stdout("argv=[-- value]\n");
}

#[test]
fn a_separator_before_the_script_makes_every_later_word_a_value() {
    for word in ["--help", "-h"] {
        run_argv(&["bash", "--", "/dev/stdin", word])
            .assert()
            .success()
            .stdout(format!("argv=[{word}]\n"));
    }
}

#[test]
fn a_script_named_like_the_command_does_not_hide_the_separator() {
    let dir = std::path::Path::new(env!("CARGO_TARGET_TMPDIR")).join("double_dash_named_bash");
    std::fs::create_dir_all(&dir).unwrap();
    std::fs::write(dir.join("bash"), ARGV_SCRIPT).unwrap();
    Command::new(cargo::cargo_bin!("usage"))
        .current_dir(&dir)
        .env_remove("USAGECLI_SHELL_BASH")
        .env_remove("USAGE_SHELL_BASH")
        .args(["bash", "--", "bash", "--help"])
        .assert()
        .success()
        .stdout("argv=[--help]\n");
}
