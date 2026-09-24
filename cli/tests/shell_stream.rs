//! `usage bash <(…)`: a script that arrives through a pipe rather than a file (#157).
//!
//! Reading the spec out of a pipe consumes it, so the shell used to be handed an empty stream
//! and exit 0 without running anything. `/dev/stdin` stands in for the process substitution:
//! it is the same kind of path, and needs no shell to set up.
//!
//! Every shell command shares the copy, but PowerShell runs a differently named one, so each
//! shell that is installed is exercised. A missing shell is skipped rather than failed.
#![cfg(unix)]

use assert_cmd::cargo;
use assert_cmd::Command;
use predicates::str::contains;

/// `usage <command>`, the program it runs, and a script for it that prints `--foo` and exits 3.
const SHELLS: &[(&str, &str, &str)] = &[
    (
        "bash",
        "bash",
        "#!/usr/bin/env -S usage bash\n#USAGE flag \"--foo\"\necho \"foo=$usage_foo\"\nexit 3\n",
    ),
    (
        "zsh",
        "zsh",
        "#!/usr/bin/env -S usage zsh\n#USAGE flag \"--foo\"\necho \"foo=$usage_foo\"\nexit 3\n",
    ),
    (
        "fish",
        "fish",
        "#!/usr/bin/env -S usage fish\n#USAGE flag \"--foo\"\necho \"foo=$usage_foo\"\nexit 3\n",
    ),
    (
        "powershell",
        "pwsh",
        "#!/usr/bin/env -S usage powershell\n#USAGE flag \"--foo\"\nWrite-Output \"foo=$env:usage_foo\"\nexit 3\n",
    ),
];

fn installed(program: &str) -> bool {
    std::process::Command::new(program)
        .arg("--version")
        .output()
        .is_ok_and(|out| out.status.success())
}

fn usage_shell(command: &str, args: &[&str]) -> Command {
    let mut cmd = Command::new(cargo::cargo_bin!("usage"));
    for key in ["BASH", "ZSH", "FISH", "PWSH"] {
        cmd.env_remove(format!("USAGECLI_SHELL_{key}"))
            .env_remove(format!("USAGE_SHELL_{key}"));
    }
    cmd.args([command, "/dev/stdin"]).args(args);
    cmd
}

#[test]
fn a_piped_script_runs_with_its_spec() {
    for (command, program, script) in SHELLS {
        if !installed(program) {
            eprintln!("skipping {command}: {program} is not installed");
            continue;
        }
        usage_shell(command, &["--foo"])
            .write_stdin(*script)
            .assert()
            .code(3)
            .stdout(contains("foo=true"));
    }
}

/// The name comes from the original path, not from the copy PowerShell needs to run.
#[test]
fn a_piped_script_prints_its_help() {
    for (command, _, script) in SHELLS {
        usage_shell(command, &["--help"])
            .write_stdin(*script)
            .assert()
            .success()
            .stdout(contains("Usage: stdin [--foo]"));
    }
}
