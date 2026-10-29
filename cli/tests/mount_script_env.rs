//! A `mount run=` that fetches the spec of another usage script (#1590).
//!
//! The runners parse the script's command line before its body starts, and that parse checks
//! the value of an env-bound flag. The body is what prints the spec, so an environment holding
//! a value the flag rejects used to stop the spec from being read at all, though the flag has
//! nothing to do with it. Only a run `usage` itself started gets the lenient retry.
#![cfg(unix)]

use std::path::{Path, PathBuf};

use assert_cmd::Command;
use assert_cmd::cargo;
use predicates::str::contains;

/// A fresh directory for one test's scripts, removed when it goes out of scope.
struct Scratch(PathBuf);

impl Scratch {
    fn new(name: &str) -> Self {
        let dir =
            std::env::temp_dir().join(format!("usage-mount-env-{name}-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&dir);
        std::fs::create_dir_all(&dir).unwrap();
        Self(dir)
    }

    fn write(&self, name: &str, content: &str) -> PathBuf {
        let path = self.0.join(name);
        std::fs::write(&path, content).unwrap();
        path
    }
}

impl Drop for Scratch {
    fn drop(&mut self) {
        let _ = std::fs::remove_dir_all(&self.0);
    }
}

fn usage_bin() -> &'static Path {
    cargo::cargo_bin!("usage")
}

fn usage() -> Command {
    let mut cmd = Command::new(usage_bin());
    cmd.env_remove("X_MODE").env_remove("__USAGE");
    cmd
}

/// The child prints its spec when asked with `--mount`, and says what mode it got otherwise.
const CHILD_BODY: &str = r#"#!/bin/sh
#USAGE flag "--mount" hide=#true
#USAGE flag "--mode <m>" env="X_MODE" { choices "a" "b" }
if [ "${usage_mount:-}" = true ]; then echo 'flag "--mode <m>"'; exit 0; fi
echo "mode=${usage_mode:-}"
"#;

fn parent(scratch: &Scratch, runner: &str, child: &Path) -> PathBuf {
    scratch.write(
        "parent",
        &format!(
            "#!/bin/sh\n#USAGE cmd \"sub\" {{\n#USAGE   mount run=\"{} {runner} {} --mount\"\n#USAGE }}\necho \"parent ok\"\n",
            usage_bin().display(),
            child.display()
        ),
    )
}

#[test]
fn a_mount_reads_the_spec_of_a_script_whatever_its_env_holds() {
    for (command, args) in [("bash", vec![]), ("exec", vec!["sh"])] {
        let scratch = Scratch::new(command);
        let child = scratch.write("child", CHILD_BODY);
        let runner = if command == "exec" {
            "exec sh"
        } else {
            command
        };
        let parent = parent(&scratch, runner, &child);

        usage()
            .arg(command)
            .args(&args)
            .arg(&parent)
            .arg("sub")
            .env("X_MODE", "zzz")
            .assert()
            .success()
            .stdout(contains("parent ok"));
    }
}

#[test]
fn completing_a_command_with_a_mount_reads_the_spec_too() {
    let scratch = Scratch::new("complete");
    let child = scratch.write("child", CHILD_BODY);
    let parent = parent(&scratch, "bash", &child);

    usage()
        .args(["complete-word", "--shell", "bash", "-f"])
        .arg(&parent)
        .args(["--", "parent", "sub", "--"])
        .env("X_MODE", "zzz")
        .assert()
        .success()
        .stdout(contains("--mode"));
}

/// Nothing is excused for a person running the script: a bad value is still theirs to hear about.
#[test]
fn running_the_script_itself_still_rejects_a_bad_env_value() {
    let scratch = Scratch::new("direct");
    let child = scratch.write("child", CHILD_BODY);

    usage()
        .arg("bash")
        .arg(&child)
        .env("X_MODE", "zzz")
        .assert()
        .failure()
        .stderr(contains("Invalid choice for option mode: zzz"));
}

#[test]
fn a_valid_env_value_still_reaches_a_script_started_by_usage() {
    let scratch = Scratch::new("valid");
    let child = scratch.write("child", CHILD_BODY);

    usage()
        .arg("bash")
        .arg(&child)
        .env("X_MODE", "a")
        .env("__USAGE", "test")
        .assert()
        .success()
        .stdout(contains("mode=a"));
}

/// What is left of the environment must still do its job: `PROJECT` is the only thing that
/// satisfies the required argument, so setting every binding aside would lose the spec again.
#[test]
fn a_valid_env_binding_beside_a_rejected_one_is_kept() {
    let scratch = Scratch::new("required");
    let child = scratch.write(
        "child",
        &CHILD_BODY.replace(
            "#USAGE flag \"--mode",
            "#USAGE arg \"<project>\" env=\"X_PROJECT\"\n#USAGE flag \"--mode",
        ),
    );
    let parent = parent(&scratch, "bash", &child);

    usage()
        .arg("bash")
        .arg(&parent)
        .arg("sub")
        .env("X_MODE", "zzz")
        .env("X_PROJECT", "demo")
        .assert()
        .success()
        .stdout(contains("parent ok"));

    // Without the value the argument needs there is nothing to keep, and the error says so.
    usage()
        .arg("bash")
        .arg(&parent)
        .arg("sub")
        .env("X_MODE", "zzz")
        .env_remove("X_PROJECT")
        .assert()
        .failure();
}

/// A bad value typed on the command line has no environment binding behind it to set aside.
#[test]
fn a_rejected_command_line_value_is_still_an_error_for_a_script_started_by_usage() {
    let scratch = Scratch::new("typed");
    let child = scratch.write("child", CHILD_BODY);

    usage()
        .arg("bash")
        .arg(&child)
        .args(["--mode", "zzz"])
        .env("X_MODE", "a")
        .env("__USAGE", "test")
        .assert()
        .failure()
        .stderr(contains("Invalid choice for option mode: zzz"));
}

/// Restoring `FORMAT` alone swaps its own missing-value error for the missing `--out` it
/// requires, which looks no better until `OUT` is back; both are valid and both must be kept.
#[test]
fn a_binding_that_needs_a_later_one_is_kept_once_it_is_restored() {
    let scratch = Scratch::new("requires");
    let child = scratch.write(
        "child",
        &CHILD_BODY.replace(
            "#USAGE flag \"--mode",
            "#USAGE flag \"--format <f>\" env=\"X_FORMAT\" required=#true {\n#USAGE   requires \"--out\"\n#USAGE }\n#USAGE flag \"--out <o>\" env=\"X_OUT\"\n#USAGE flag \"--mode",
        ),
    );
    let parent = parent(&scratch, "bash", &child);

    usage()
        .arg("bash")
        .arg(&parent)
        .arg("sub")
        .env("X_MODE", "zzz")
        .env("X_FORMAT", "json")
        .env("X_OUT", "file")
        .assert()
        .success()
        .stdout(contains("parent ok"));
}

/// The failure names the command whose mounted commands could not be read, not only the internal
/// `sh -c` call, and `usage` exits 181 for it while a direct run of the same script exits 1.
#[test]
fn a_failing_mount_names_the_command_it_was_resolving() {
    let scratch = Scratch::new("failing");
    let child = scratch.write("child", "#!/bin/sh\n#USAGE arg \"<project>\"\necho hi\n");
    let parent = scratch.write(
        "parent",
        &format!(
            "#!/bin/sh\n#USAGE cmd \"sub\" {{\n#USAGE   mount run=\"{} bash {}\"\n#USAGE }}\n",
            usage_bin().display(),
            child.display()
        ),
    );

    usage()
        .arg("bash")
        .arg(&parent)
        .arg("sub")
        .assert()
        .code(181)
        .stderr(contains("could not read the commands mounted under `sub`"))
        .stderr(contains("Missing required arg: <project>"));

    usage()
        .arg("bash")
        .arg(&child)
        .assert()
        .code(1)
        .stderr(contains("Missing required arg: <project>"));
}
