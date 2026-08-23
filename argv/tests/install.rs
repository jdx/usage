//! Does an installed script land where the shell it was written for will find it?
//!
//! The path table itself is unit-tested against described environments, where no filesystem is
//! involved and every platform can be asked about. What is left for here is the half that only a
//! real directory can answer: that parents get created, that an upgrade is not a theft, that a
//! refusal leaves the user's file exactly as it was — and, for the two shells that claim to load a
//! script with no configuration at all, that they really do.
//!
//! Every environment below is *described* and points at this test's own directory, so nothing here
//! reads or writes a real home, and nothing mutates the process environment — which is what lets
//! these run in parallel with each other and with everything else.

#![cfg(feature = "complete")]

use std::fs;
use std::path::{Path, PathBuf};
use std::process::Command;

use usage_argv::complete::Shell;
use usage_argv::install::{install, plan, Env, Error, OnForeign, Platform, Wrote};
use usage_argv::script::script;

/// A directory of this test's own, and an environment that sends every shell into it.
struct Fixture {
    dir: PathBuf,
}

impl Fixture {
    fn new(name: &str) -> Self {
        let dir =
            std::env::temp_dir().join(format!("usage-argv-install-{name}-{}", std::process::id()));
        let _ = fs::remove_dir_all(&dir);
        fs::create_dir_all(&dir).expect("a directory to work in");
        Self { dir }
    }

    /// An environment naming this directory for every variable any shell's table consults.
    ///
    /// All of them, including the ones a given shell does not read: a variable left out is a
    /// variable the resolver could fall through to, and the next one after it is the developer's
    /// real home.
    fn env(&self) -> Env {
        let dir = &self.dir;
        Env::new(
            Platform::current(),
            [
                ("HOME", dir.join("home")),
                ("XDG_DATA_HOME", dir.join("data")),
                ("XDG_CONFIG_HOME", dir.join("config")),
                ("USERPROFILE", dir.join("home")),
                ("APPDATA", dir.join("roaming")),
                ("LOCALAPPDATA", dir.join("local")),
            ]
            .into_iter()
            .map(|(name, value)| (name.to_string(), value.into_os_string())),
        )
    }

    fn path_for(&self, shell: Shell) -> PathBuf {
        let target = plan("ex", shell, &self.env()).expect("a plan");
        // The guard that keeps a bug in the table from turning a test into a write to a real home.
        assert!(
            target.path.starts_with(&self.dir),
            "a plan escaped the fixture: {:?}",
            target.path
        );
        target.path
    }
}

impl Drop for Fixture {
    fn drop(&mut self) {
        let _ = fs::remove_dir_all(&self.dir);
    }
}

const SHELLS: [Shell; 6] = [
    Shell::Bash,
    Shell::Elvish,
    Shell::Zsh,
    Shell::Fish,
    Shell::Nu,
    Shell::PowerShell,
];

/// Whether a program is on `PATH`, so a missing shell is skipped rather than failing the suite.
fn available(program: &str) -> bool {
    Command::new(program)
        .arg("--version")
        .output()
        .map(|o| o.status.success())
        .unwrap_or(false)
}

#[test]
fn an_install_creates_the_directories_it_needs() {
    let fixture = Fixture::new("creates");
    for shell in SHELLS {
        let expected = fixture.path_for(shell);
        assert!(
            !expected.parent().unwrap().exists(),
            "{shell:?}: the directory should not exist yet"
        );

        let done = install("ex", shell, &fixture.env(), OnForeign::Refuse).expect("an install");
        assert_eq!(done.wrote, Wrote::Created, "{shell:?}");
        assert_eq!(done.plan.path, expected, "{shell:?}");
        assert_eq!(
            fs::read_to_string(&expected).expect("the script"),
            script("ex", shell),
            "{shell:?}"
        );
    }
}

#[test]
fn installing_the_same_version_twice_changes_nothing() {
    let fixture = Fixture::new("idempotent");
    let first = install("ex", Shell::Fish, &fixture.env(), OnForeign::Refuse).unwrap();
    assert_eq!(first.wrote, Wrote::Created);
    let before = fs::metadata(&first.plan.path).unwrap().modified().unwrap();

    let again = install("ex", Shell::Fish, &fixture.env(), OnForeign::Refuse).unwrap();
    assert_eq!(again.wrote, Wrote::Unchanged);
    // Not written at all, rather than written with the same bytes: an upgrade that runs in a
    // provisioning script should leave no trace when there was nothing to do.
    assert_eq!(
        fs::metadata(&first.plan.path).unwrap().modified().unwrap(),
        before
    );
}

#[test]
fn an_upgrade_rewrites_the_file_this_wrote() {
    let fixture = Fixture::new("upgrade");
    let target = fixture.path_for(Shell::Zsh);
    fs::create_dir_all(target.parent().unwrap()).unwrap();
    // An older script: different text, but carrying the marker that says whose it is.
    fs::write(
        &target,
        "#compdef ex\n# @generated by usage-argv for `ex __complete_word__ --shell zsh`\n_ex() { }\n",
    )
    .unwrap();

    let done = install("ex", Shell::Zsh, &fixture.env(), OnForeign::Refuse).unwrap();
    assert_eq!(done.wrote, Wrote::Updated);
    assert_eq!(
        fs::read_to_string(&target).unwrap(),
        script("ex", Shell::Zsh)
    );
}

#[test]
fn a_file_this_did_not_write_is_refused_and_left_alone() {
    let fixture = Fixture::new("foreign");
    let target = fixture.path_for(Shell::Zsh);
    fs::create_dir_all(target.parent().unwrap()).unwrap();
    let theirs = "#compdef ex\n# an evening's work\n_ex() { compadd mine }\n";
    fs::write(&target, theirs).unwrap();

    let err = install("ex", Shell::Zsh, &fixture.env(), OnForeign::Refuse).unwrap_err();
    match &err {
        Error::Foreign { path } => assert_eq!(path, &target),
        other => panic!("{other:?}"),
    }
    // The refusal is only worth anything if the file is still theirs afterwards.
    assert_eq!(fs::read_to_string(&target).unwrap(), theirs);
    // And the message names the file, since acting on it means finding it.
    assert!(err.to_string().contains(&target.display().to_string()));
}

#[test]
fn a_foreign_file_is_replaced_only_when_the_caller_says_so() {
    let fixture = Fixture::new("forced");
    let target = fixture.path_for(Shell::Zsh);
    fs::create_dir_all(target.parent().unwrap()).unwrap();
    fs::write(&target, "# an evening's work\n").unwrap();

    let done = install("ex", Shell::Zsh, &fixture.env(), OnForeign::Overwrite).unwrap();
    assert_eq!(done.wrote, Wrote::Replaced);
    assert_eq!(
        fs::read_to_string(&target).unwrap(),
        script("ex", Shell::Zsh)
    );
}

#[test]
fn a_directory_where_the_file_goes_is_reported_rather_than_panicked() {
    let fixture = Fixture::new("dir-in-the-way");
    let target = fixture.path_for(Shell::Fish);
    fs::create_dir_all(&target).unwrap();

    let err = install("ex", Shell::Fish, &fixture.env(), OnForeign::Refuse).unwrap_err();
    assert!(matches!(err, Error::Io { .. }), "{err:?}");
}

#[test]
fn a_parent_that_is_a_file_is_reported() {
    // Chosen over an unwritable directory because it fails the same way for root, which is who a
    // container runs tests as.
    let fixture = Fixture::new("file-parent");
    let target = fixture.path_for(Shell::Fish);
    let parent = target.parent().unwrap();
    fs::create_dir_all(parent.parent().unwrap()).unwrap();
    fs::write(parent, "not a directory\n").unwrap();

    let err = install("ex", Shell::Fish, &fixture.env(), OnForeign::Refuse).unwrap_err();
    let Error::Io { doing, .. } = &err else {
        panic!("{err:?}");
    };
    assert_eq!(doing.as_str(), "creating", "{err:?}");
}

#[test]
fn a_finished_install_leaves_no_temporary_files_behind() {
    let fixture = Fixture::new("no-litter");
    let done = install("ex", Shell::Fish, &fixture.env(), OnForeign::Refuse).unwrap();
    let dir = done.plan.path.parent().unwrap();
    let entries: Vec<String> = fs::read_dir(dir)
        .unwrap()
        .map(|e| e.unwrap().file_name().to_string_lossy().into_owned())
        .collect();
    assert_eq!(entries, vec!["ex.fish".to_string()], "{entries:?}");
}

#[test]
fn an_installed_zsh_script_is_named_what_compinit_will_look_for() {
    // The path table and the script's own first line are two halves of one contract, and each is
    // useless if the other moves: `compinit` autoloads `_ex` and calls the function named after
    // the file it found.
    let fixture = Fixture::new("zsh-name");
    let done = install("ex", Shell::Zsh, &fixture.env(), OnForeign::Refuse).unwrap();
    assert_eq!(done.plan.path.file_name().unwrap(), "_ex");
    let text = fs::read_to_string(&done.plan.path).unwrap();
    assert!(text.starts_with("#compdef ex\n"), "{text}");
    assert!(text.contains("_ex() {"), "{text}");
}

#[test]
fn an_installed_zsh_script_loads_from_where_it_was_installed() {
    // The only test that proves the *reported* line is the right one: it does what the instruction
    // says — put the directory on `$fpath`, run `compinit` — and then completes.
    if !available("zsh") {
        println!("zsh is not installed; skipping");
        return;
    }
    let fixture = Fixture::new("zsh-autoload");
    let done = install("ex", Shell::Zsh, &fixture.env(), OnForeign::Refuse).unwrap();
    // The instruction verbatim, which is the point: a line that does not work is worse than no line
    // at all, because the user has no way to tell which half is wrong.
    let instruction = done
        .plan
        .loading
        .instruction()
        .expect("zsh needs a line")
        .to_string();
    // Used as a prefix rather than copied, so a change to the reported line still flows through
    // here. What is appended is two flags the *instruction* should not carry: `compaudit` calls a
    // world-writable ancestor insecure, which is what `std::env::temp_dir()` is on a CI runner, and
    // plain `compinit` answers that by prompting — "not interactive and can't open terminal", then
    // aborting. `-u` takes the answer a human would give; `-d` keeps the dump out of `$HOME`. A
    // real user's own data directory is neither world-writable nor shared, which is why the line
    // they are given stays plain.
    assert!(
        instruction.ends_with("compinit"),
        "the flags below are appended to a `compinit` at the end: {instruction}"
    );
    let stubs = "typeset -A compstate\ncompadd() { local e; for e in \"${display[@]}\"; do print -r -- \"display:$e\"; done }\n_files() { }\n";
    let code = format!(
        "{stubs}{instruction} -u -d {}/zcompdump\nBUFFER='ex i'\nCURSOR=4\n_ex\n",
        fixture.dir.display()
    );
    let out = Command::new("zsh")
        .arg("-c")
        .arg(&code)
        .env("PATH", stand_in(&fixture, "install\tInstall\tinstall\n"))
        // So a compdump lands in the fixture rather than in whoever's home is running the tests.
        .env("HOME", fixture.dir.join("home"))
        .env("ZDOTDIR", fixture.dir.join("home"))
        .output()
        .expect("running zsh");
    assert!(
        out.status.success(),
        "zsh failed: {}",
        String::from_utf8_lossy(&out.stderr)
    );
    let text = String::from_utf8_lossy(&out.stdout);
    assert!(text.contains("display:install"), "{text}");
}

#[test]
fn an_installed_fish_script_completes_without_being_sourced() {
    // fish's plan says `Loading::Automatic`, and this is what makes that a fact rather than a
    // claim: the script is installed and nothing else is done to the shell at all.
    if !available("fish") {
        println!("fish is not installed; skipping");
        return;
    }
    let fixture = Fixture::new("fish-autoload");
    install("ex", Shell::Fish, &fixture.env(), OnForeign::Refuse).unwrap();

    let out = Command::new("fish")
        .arg("-c")
        .arg("complete -C 'ex i'")
        .env("PATH", stand_in(&fixture, "install\tInstall\n"))
        .env("XDG_CONFIG_HOME", fixture.dir.join("config"))
        .env("HOME", fixture.dir.join("home"))
        .output()
        .expect("running fish");
    assert!(
        out.status.success(),
        "fish failed: {}",
        String::from_utf8_lossy(&out.stderr)
    );
    let text = String::from_utf8_lossy(&out.stdout);
    assert!(text.contains("install"), "{text}");
}

/// A `PATH` with a stand-in `ex` on it that answers a completion request with `answer`.
fn stand_in(fixture: &Fixture, answer: &str) -> String {
    let bin_dir = fixture.dir.join("bin");
    fs::create_dir_all(&bin_dir).expect("a bin directory");
    let bin = bin_dir.join("ex");
    fs::write(
        &bin,
        format!(
            "#!/usr/bin/env bash\nprintf '%s' '{}'\n",
            answer.replace('\'', "'\\''")
        ),
    )
    .expect("writing the stand-in");
    make_executable(&bin);
    format!(
        "{}:{}",
        bin_dir.display(),
        std::env::var("PATH").unwrap_or_default()
    )
}

#[cfg(unix)]
fn make_executable(path: &Path) {
    use std::os::unix::fs::PermissionsExt;
    let mut perms = fs::metadata(path).expect("metadata").permissions();
    perms.set_mode(0o755);
    fs::set_permissions(path, perms).expect("making it executable");
}

#[cfg(not(unix))]
fn make_executable(_path: &Path) {}
