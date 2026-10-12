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
