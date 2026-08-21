//! `usage g completion --install` as a command: where it writes, and what it will not touch.
//!
//! Every test here resolves a path from `$HOME` and the XDG variables, so the one thing that must
//! not go wrong is a run escaping into the developer's own home. Two things stand between them: the
//! environment is set on the child process only — never `std::env::set_var`, which two tests running
//! at once would race on — and the reported path is asserted to be inside the scratch directory
//! *before* anything is read off disk, so a leak fails the test rather than quietly succeeding.

use assert_cmd::prelude::*;
use predicates::str::contains;
use std::path::{Path, PathBuf};
use std::process::Command;
use std::sync::atomic::{AtomicU32, Ordering};

const SHELLS: [&str; 5] = ["bash", "zsh", "fish", "nu", "powershell"];

/// A scratch directory, and a `usage` pointed at it and nowhere else.
struct Scratch {
    dir: PathBuf,
}

impl Scratch {
    fn new(name: &str) -> Self {
        static NEXT: AtomicU32 = AtomicU32::new(0);
        let dir = std::env::temp_dir().join(format!(
            "usage_completion_install_{name}_{}_{}",
            std::process::id(),
            NEXT.fetch_add(1, Ordering::Relaxed)
        ));
        let _ = std::fs::remove_dir_all(&dir);
        std::fs::create_dir_all(&dir).unwrap();
        std::fs::write(
            dir.join("mycli.usage.kdl"),
            "name \"mycli\"\nbin \"mycli\"\nflag \"--force\"\ncmd \"build\" help=\"Build it\"\n",
        )
        .unwrap();
        Self { dir }
    }

    fn spec(&self) -> PathBuf {
        self.dir.join("mycli.usage.kdl")
    }

    /// `usage`, with every variable any shell's table could reach pointed into the scratch
    /// directory.
    ///
    /// All of them, including the ones a given shell does not read: a variable left unset is one the
    /// resolver can fall through to, and the next one after it is the real home. `ZDOTDIR` and
    /// `BASH_COMPLETION_USER_DIR` are removed rather than set, because a developer who has either
    /// would otherwise send this test wherever theirs points.
    fn usage(&self) -> Command {
        let mut cmd = Command::new(assert_cmd::cargo::cargo_bin!("usage"));
        cmd.env("HOME", self.dir.join("home"))
            .env("XDG_DATA_HOME", self.dir.join("data"))
            .env("XDG_CONFIG_HOME", self.dir.join("config"))
            .env("XDG_CACHE_HOME", self.dir.join("cache"))
            .env("USERPROFILE", self.dir.join("home"))
            .env("APPDATA", self.dir.join("roaming"))
            .env("LOCALAPPDATA", self.dir.join("local"))
            .env_remove("ZDOTDIR")
            .env_remove("BASH_COMPLETION_USER_DIR")
            .env_remove("NU_VENDOR_AUTOLOAD_DIR");
        cmd
    }

    fn install(&self, shell: &str) -> std::process::Output {
        self.usage()
            .args(["g", "completion", shell, "mycli", "-f"])
            .arg(self.spec())
            .arg("--install")
            .output()
            .unwrap()
    }

    /// The path an install reported, checked to be inside this directory before it is used.
    fn installed_path(&self, stderr: &str) -> PathBuf {
        let line = stderr
            .lines()
            .find(|l| l.starts_with("installing to "))
            .unwrap_or_else(|| panic!("nothing said where it went:\n{stderr}"));
        let path = PathBuf::from(line.trim_start_matches("installing to "));
        assert!(
            path.starts_with(&self.dir),
            "an install escaped the scratch directory: {path:?}"
        );
        path
    }
}

impl Drop for Scratch {
    fn drop(&mut self) {
        let _ = std::fs::remove_dir_all(&self.dir);
    }
}

#[test]
fn install_writes_the_script_where_the_shell_looks_for_it() {
    for shell in SHELLS {
        let scratch = Scratch::new(&format!("writes_{shell}"));
        let out = scratch.install(shell);
        assert!(
            out.status.success(),
            "{shell}: {}",
            String::from_utf8_lossy(&out.stderr)
        );
        let path = scratch.installed_path(&String::from_utf8_lossy(&out.stderr));
        assert!(path.is_file(), "{shell}: nothing at {path:?}");
        assert!(
            std::fs::read_to_string(&path).unwrap().contains("mycli"),
            "{shell}"
        );
    }
}

#[test]
fn the_installed_bytes_are_the_bytes_it_would_have_printed() {
    // The invariant that keeps the two output paths from drifting apart, which is the way a feature
    // like this rots: the printed script gets a fix and the installed one does not.
    let scratch = Scratch::new("same_bytes");
    let printed = scratch
        .usage()
        .args(["g", "completion", "zsh", "mycli", "-f"])
        .arg(scratch.spec())
        .output()
        .unwrap();
    assert!(printed.status.success());

    let out = scratch.install("zsh");
    let path = scratch.installed_path(&String::from_utf8_lossy(&out.stderr));
    assert_eq!(
        std::fs::read(&path).unwrap(),
        printed.stdout,
        "installed and printed disagree"
    );
}

#[test]
fn installing_leaves_stdout_empty() {
    // The document is on disk, so anything on stdout is a leak — and
    // `usage g completion zsh mycli --install > file` should not produce a file full of prose.
    let scratch = Scratch::new("quiet");
    let out = scratch.install("zsh");
    assert!(
        out.stdout.is_empty(),
        "stdout: {}",
        String::from_utf8_lossy(&out.stdout)
    );
    assert!(!out.stderr.is_empty(), "it should still say where it went");
}

#[test]
fn installing_for_zsh_reports_the_fpath_line() {
    let scratch = Scratch::new("zsh_line");
    let out = scratch.install("zsh");
    let stderr = String::from_utf8_lossy(&out.stderr);
    let path = scratch.installed_path(&stderr);
    assert!(stderr.contains("fpath+="), "{stderr}");
    // The directory, never the file: `$fpath` holds directories, and a line naming the file would
    // have compinit looking for completions inside a completion.
    assert!(
        stderr.contains(&path.parent().unwrap().display().to_string()),
        "{stderr}"
    );
    assert!(stderr.contains(".zshrc"), "{stderr}");
}

#[test]
fn installing_for_powershell_reports_the_profile_line() {
    let scratch = Scratch::new("pwsh_line");
    let out = scratch.install("powershell");
    let stderr = String::from_utf8_lossy(&out.stderr);
    let path = scratch.installed_path(&stderr);
    assert!(stderr.contains("$PROFILE"), "{stderr}");
    assert!(
        stderr.contains(&format!(". '{}'", path.display())),
        "{stderr}"
    );
}

#[test]
fn installing_never_edits_a_shell_rc() {
    // The policy, asserted rather than documented. A later "helpful" change that appends a source
    // line to `.zshrc` fails here, which is the point: writing the script again is a no-op, and
    // appending a line again is not.
    let scratch = Scratch::new("rc_untouched");
    let home = scratch.dir.join("home");
    let rcs = [
        home.join(".zshrc"),
        home.join(".bashrc"),
        home.join(".config/fish/config.fish"),
        home.join(".config/powershell/Microsoft.PowerShell_profile.ps1"),
    ];
    for rc in &rcs {
        std::fs::create_dir_all(rc.parent().unwrap()).unwrap();
        std::fs::write(rc, "# mine\n").unwrap();
    }

    for shell in SHELLS {
        let out = scratch.install(shell);
        assert!(out.status.success(), "{shell}");
    }
    for rc in &rcs {
        assert_eq!(
            std::fs::read_to_string(rc).unwrap(),
            "# mine\n",
            "{rc:?} was edited"
        );
    }
}

#[test]
fn a_second_install_says_there_was_nothing_to_do() {
    let scratch = Scratch::new("idempotent");
    let first = scratch.install("fish");
    assert!(first.status.success());
    let again = scratch.install("fish");
    assert!(again.status.success());
    assert!(
        String::from_utf8_lossy(&again.stderr).contains("already up to date"),
        "{}",
        String::from_utf8_lossy(&again.stderr)
    );
}

#[test]
fn an_upgrade_that_changes_the_script_is_not_refused_as_a_stranger() {
    // The case a marker naming one crate gets wrong. These scripts are rendered by usage-lib and
    // stamped `@generated by usage-cli`, not `usage-argv` — so an ownership test naming the crate
    // rather than the family refuses every re-install where the bytes changed, and the
    // upgrade-needs-no-flag promise holds only while nothing changes, which is no promise at all.
    //
    // Identical bytes are not enough to catch it: `Unchanged` short-circuits before ownership is
    // ever consulted. The script's header carries the spec it was generated from, so generating
    // from a second path is a real content change of exactly the shape an upgrade has.
    let scratch = Scratch::new("upgrade");
    let out = scratch.install("zsh");
    let path = scratch.installed_path(&String::from_utf8_lossy(&out.stderr));
    let first = std::fs::read_to_string(&path).unwrap();

    let moved = scratch.dir.join("moved.usage.kdl");
    std::fs::copy(scratch.spec(), &moved).unwrap();
    let again = scratch
        .usage()
        .args(["g", "completion", "zsh", "mycli", "-f"])
        .arg(&moved)
        .arg("--install")
        .output()
        .unwrap();
    assert!(
        again.status.success(),
        "an upgrade should not need --force: {}",
        String::from_utf8_lossy(&again.stderr)
    );
    let second = std::fs::read_to_string(&path).unwrap();
    assert_ne!(first, second, "the two scripts should differ");
    assert!(
        !String::from_utf8_lossy(&again.stderr).contains("already up to date"),
        "the bytes changed, so this was an update rather than a no-op"
    );
}

#[test]
fn a_file_usage_did_not_write_is_refused_until_forced() {
    let scratch = Scratch::new("foreign");
    let out = scratch.install("zsh");
    let path = scratch.installed_path(&String::from_utf8_lossy(&out.stderr));
    let theirs = "#compdef mycli\n# an evening's work\n";
    std::fs::write(&path, theirs).unwrap();

    let refused = scratch.install("zsh");
    assert!(!refused.status.success(), "it should have refused");
    let stderr = String::from_utf8_lossy(&refused.stderr);
    assert!(stderr.contains("--force"), "{stderr}");
    // A refusal that had already overwritten the file would be worse than no refusal.
    assert_eq!(std::fs::read_to_string(&path).unwrap(), theirs);

    scratch
        .usage()
        .args(["g", "completion", "zsh", "mycli", "-f"])
        .arg(scratch.spec())
        .args(["--install", "--force"])
        .assert()
        .success();
    assert_ne!(std::fs::read_to_string(&path).unwrap(), theirs);
}

#[test]
fn force_without_install_is_refused_before_anything_runs() {
    // `--force` only means anything to an install, and a flag that silently does nothing is a flag
    // somebody will believe in.
    let scratch = Scratch::new("force_alone");
    scratch
        .usage()
        .args(["g", "completion", "zsh", "mycli", "-f"])
        .arg(scratch.spec())
        .arg("--force")
        .assert()
        .failure()
        .stderr(contains("--install"));
}

#[test]
fn a_bin_that_is_not_a_plain_word_is_refused_and_writes_nothing() {
    // `usage g completion zsh ../../../.zshrc --install` is a write outside the resolved
    // directory, and `<BIN>` is a word off a command line.
    let scratch = Scratch::new("traversal");
    for bin in ["../../../.zshrc", "my tool", "a/b"] {
        let out = scratch
            .usage()
            .args(["g", "completion", "zsh", bin, "-f"])
            .arg(scratch.spec())
            .arg("--install")
            .output()
            .unwrap();
        assert!(!out.status.success(), "{bin:?} was accepted");
        assert!(
            !files_under(&scratch.dir)
                .iter()
                .any(|p| p.extension().is_none_or(|e| e != "kdl")
                    && p.file_name().is_some_and(|n| n != "mycli.usage.kdl")),
            "{bin:?} left something behind: {:?}",
            files_under(&scratch.dir)
        );
    }
}

#[test]
fn an_environment_with_no_home_is_reported_rather_than_guessed() {
    let scratch = Scratch::new("homeless");
    let mut cmd = scratch.usage();
    for var in [
        "HOME",
        "XDG_DATA_HOME",
        "XDG_CONFIG_HOME",
        "USERPROFILE",
        "APPDATA",
        "LOCALAPPDATA",
    ] {
        cmd.env_remove(var);
    }
    let out = cmd
        .args(["g", "completion", "zsh", "mycli", "-f"])
        .arg(scratch.spec())
        .arg("--install")
        .output()
        .unwrap();
    assert!(!out.status.success());
    let stderr = String::from_utf8_lossy(&out.stderr);
    // The message names what it looked at, so setting one is a thing the user can do.
    assert!(stderr.contains("XDG_DATA_HOME"), "{stderr}");
    assert!(stderr.contains("HOME"), "{stderr}");
}

/// Every file under `dir`, so a test can say that nothing was written.
fn files_under(dir: &Path) -> Vec<PathBuf> {
    let mut out = vec![];
    let Ok(entries) = std::fs::read_dir(dir) else {
        return out;
    };
    for entry in entries.flatten() {
        let path = entry.path();
        if path.is_dir() {
            out.extend(files_under(&path));
        } else {
            out.push(path);
        }
    }
    out
}
