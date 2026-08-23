//! Do the generated scripts work in the shells they are written for?
//!
//! A completion script is the one artifact here that no amount of Rust testing reaches: it is text
//! handed to another program, and the way it fails is at a prompt, silently, for a user. So these
//! run the real shells — every one available on the machine — against a stand-in binary that
//! answers the way the derive's hidden command does.
//!
//! The stand-in matters as much as the script: it is what makes this a test of the *protocol* and
//! not of the parser. What the binary would have computed is already checked to the last byte
//! against usage-lib elsewhere.

#![cfg(feature = "complete")]

use std::fs;
use std::path::{Path, PathBuf};
use std::process::Command;
use usage_argv::complete::Shell;
use usage_argv::script::script;

/// A directory of this test's own, with the generated script and a stand-in `ex` binary in it.
struct Fixture {
    dir: PathBuf,
}

impl Fixture {
    /// `answer` is what the stand-in prints for a completion request, verbatim.
    fn new(name: &str, shell: Shell, answer: &str) -> Self {
        let dir =
            std::env::temp_dir().join(format!("usage-argv-scripts-{name}-{}", std::process::id()));
        let _ = fs::remove_dir_all(&dir);
        fs::create_dir_all(&dir).expect("a directory to work in");

        fs::write(dir.join("script"), script("ex", shell)).expect("writing the script");

        // The stand-in answers a completion request and nothing else, so a script that called it
        // for anything else would be caught rather than quietly working.
        let stand_in = format!(
            "#!/usr/bin/env bash\nif [[ ${{1:-}} != __complete_word__ ]]; then\n  \
             echo \"the script called the binary for something else: $*\" >&2\n  exit 1\nfi\n\
             printf '%s' {}\n",
            shell_quote(answer)
        );
        let bin = dir.join("ex");
        fs::write(&bin, stand_in).expect("writing the stand-in");
        make_executable(&bin);

        // Files for the path cases to find, and a directory among them so `dirs` can differ.
        fs::write(dir.join("alpha.txt"), "").expect("a file");
        fs::write(dir.join("beta.txt"), "").expect("another");
        fs::write(dir.join("manifest.toml"), "").expect("a filtered file");
        fs::write(dir.join("settings.yaml"), "").expect("another filtered file");
        fs::create_dir(dir.join("gamma")).expect("a directory");

        Self { dir }
    }

    /// A fixture whose stand-in reports the line it was handed, rather than answering.
    ///
    /// What the scripts do with the cursor is the part most likely to be silently wrong — every
    /// shell counts in its own units — so it is worth asserting on what the binary *receives*
    /// and not only on what comes back.
    fn echoing(name: &str, shell: Shell) -> Self {
        let fixture = Self::new(name, shell, "");
        let stand_in = "#!/usr/bin/env bash\n             line=\nwhile [[ $# -gt 0 ]]; do\n               if [[ $1 == --line ]]; then line=$2; shift; fi\n  shift\ndone\n             printf 'line=[%s]\\n' \"$line\"\n";
        let bin = fixture.dir.join("ex");
        fs::write(&bin, stand_in).expect("writing the stand-in");
        make_executable(&bin);
        fixture
    }

    /// Run `code` in `shell`, with the fixture's directory as the working directory and on `PATH`.
    fn run(&self, shell: &str, code: &str) -> String {
        let out = Command::new(shell)
            .arg("-c")
            .arg(code)
            .current_dir(&self.dir)
            .env(
                "PATH",
                format!(
                    "{}:{}",
                    self.dir.display(),
                    std::env::var("PATH").unwrap_or_default()
                ),
            )
            .output()
            .unwrap_or_else(|e| panic!("running {shell}: {e}"));
        assert!(
            out.status.success(),
            "{shell} failed: {}\n{}",
            String::from_utf8_lossy(&out.stderr),
            String::from_utf8_lossy(&out.stdout)
        );
        String::from_utf8_lossy(&out.stdout).into_owned()
    }
}

impl Drop for Fixture {
    fn drop(&mut self) {
        let _ = fs::remove_dir_all(&self.dir);
    }
}

fn shell_quote(s: &str) -> String {
    format!("'{}'", s.replace('\'', "'\\''"))
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

/// Whether a program is on `PATH`, so a missing shell is skipped rather than failing the suite.
fn available(program: &str) -> bool {
    Command::new(program)
        .arg("--version")
        .output()
        .map(|o| o.status.success())
        .unwrap_or(false)
}

/// Whether a program can check a script this test would hand it, so a shell that cannot is
/// skipped rather than failing the suite.
///
/// Not `--version`. On Windows the executable search order puts the system directory ahead of
/// `PATH`, and installing WSL puts `bash.exe` there — a launcher that answers `--version`
/// perfectly well and then cannot open the file it is given, which is how this test came to
/// fail rather than skip. The precondition is the whole invocation, so that is what gets tried:
/// the same program and flags, on a script known to be valid.
fn can_check_a_script(program: &str, args: &[&str]) -> bool {
    let dir = std::env::temp_dir().join(format!(
        "usage_argv_shell_probe_{}_{program}",
        std::process::id()
    ));
    if fs::create_dir_all(&dir).is_err() {
        return false;
    }
    let probe = dir.join("probe");
    let usable = fs::write(&probe, "echo ok\n").is_ok()
        && Command::new(program)
            .args(args)
            .arg(&probe)
            .output()
            .is_ok_and(|out| out.status.success());
    let _ = fs::remove_dir_all(&dir);
    usable
}

#[test]
fn every_script_is_valid_in_its_own_shell() {
    // A syntax error in generated text is the failure mode that reaches a user as a broken
    // prompt, so it is worth checking even where the shell cannot be driven any further.
    let checks: &[(Shell, &str, &[&str])] = &[
        (Shell::Bash, "bash", &["-n"]),
        (Shell::Elvish, "elvish", &["-compileonly"]),
        (Shell::Zsh, "zsh", &["-n"]),
        (Shell::Fish, "fish", &["--no-execute"]),
    ];

    let mut ran = 0;
    for (shell, program, args) in checks {
        if !can_check_a_script(program, args) {
            continue;
        }
        let fixture = Fixture::new(&format!("syntax-{program}"), *shell, "");
        let out = Command::new(program)
            .args(*args)
            .arg(fixture.dir.join("script"))
            .output()
            .expect("running the shell");
        assert!(
            out.status.success(),
            "the {program} script does not parse:\n{}\n--- script ---\n{}",
            String::from_utf8_lossy(&out.stderr),
            script("ex", *shell)
        );
        ran += 1;
    }
    // Named rather than silently skipped: a test that checks nothing should say so.
    //
    // Unix only. CI installs zsh and fish for the Linux job, so none of the three being usable
    // there is a configuration bug worth stopping for. Nothing installs them on Windows, and a
    // bare `bash` is the WSL launcher that cannot open the file it is handed — so having no
    // shell to check against is the expected state there rather than a fault.
    if cfg!(unix) {
        assert!(
            ran > 0,
            "no shell was available to check a script against — bash, zsh and fish were all missing"
        );
    } else if ran == 0 {
        println!("no shell here could check a script; this verified nothing");
    }
    if ran < checks.len() {
        println!(
            "checked {ran} of {} shells; the rest are not installed",
            checks.len()
        );
    }
}

#[test]
fn bash_offers_what_the_binary_answered() {
    if !available("bash") {
        println!("bash is not installed; skipping");
        return;
    }
    let fixture = Fixture::new("bash-candidates", Shell::Bash, "install\nuninstall\n");
    // Driven the way bash drives it: the line, the cursor, the split words, and the index — then
    // the function, then whatever it left in COMPREPLY.
    let out = fixture.run(
        "bash",
        r#"source ./script
COMP_LINE='ex ins'
COMP_POINT=6
COMP_WORDS=(ex ins)
COMP_CWORD=1
_usage_complete_ex
printf '%s\n' "${COMPREPLY[@]}"
"#,
    );
    assert_eq!(out, "install\nuninstall\n");
}

#[test]
fn bash_asks_the_shell_for_paths_when_the_marker_says_so() {
    if !available("bash") {
        println!("bash is not installed; skipping");
        return;
    }
    // A candidate *and* the marker, which is the interesting shape: the answer this CLI knows
    // about, plus the paths it does not.
    let fixture = Fixture::new(
        "bash-files",
        Shell::Bash,
        &format!("keep\n{}\n", usage_argv::complete::FILES_MARKER),
    );
    let out = fixture.run(
        "bash",
        r#"source ./script
COMP_LINE='ex '
COMP_POINT=3
COMP_WORDS=(ex '')
COMP_CWORD=1
_usage_complete_ex
printf '%s\n' "${COMPREPLY[@]}"
"#,
    );
    let offered: Vec<&str> = out.lines().collect();
    assert!(offered.contains(&"keep"), "{offered:?}");
    assert!(offered.contains(&"alpha.txt"), "{offered:?}");
    assert!(offered.contains(&"beta.txt"), "{offered:?}");
    // The marker itself is a message to the script, not a candidate.
    assert!(
        !offered.iter().any(|o| o.contains('\u{1}')),
        "the marker reached the user: {offered:?}"
    );

    // And `dirs` narrows it to directories.
    let fixture = Fixture::new(
        "bash-dirs",
        Shell::Bash,
        &format!("{}\n", usage_argv::complete::DIRS_MARKER),
    );
    let out = fixture.run(
        "bash",
        r#"source ./script
COMP_LINE='ex '
COMP_POINT=3
COMP_WORDS=(ex '')
COMP_CWORD=1
_usage_complete_ex
printf '%s\n' "${COMPREPLY[@]}"
"#,
    );
    let offered: Vec<&str> = out.lines().filter(|l| !l.is_empty()).collect();
    assert_eq!(offered, ["gamma"], "only the directory");
}

#[test]
fn bash_filters_files_by_the_extensions_the_binary_declared() {
    if !available("bash") {
        println!("bash is not installed; skipping");
        return;
    }
    let fixture = Fixture::new(
        "bash-extensions",
        Shell::Bash,
        "\u{1}extensions\ttoml\tyaml\n",
    );
    let out = fixture.run(
        "bash",
        r#"source ./script
COMP_LINE='ex '
COMP_POINT=3
COMP_WORDS=(ex '')
COMP_CWORD=1
_usage_complete_ex
printf '%s\n' "${COMPREPLY[@]}"
"#,
    );
    let offered: Vec<&str> = out.lines().filter(|line| !line.is_empty()).collect();
    assert!(offered.contains(&"manifest.toml"), "{offered:?}");
    assert!(offered.contains(&"settings.yaml"), "{offered:?}");
    assert!(
        offered.contains(&"gamma"),
        "directories remain traversable: {offered:?}"
    );
    assert!(!offered.contains(&"alpha.txt"), "{offered:?}");
}

#[test]
fn fish_prints_the_candidates_it_was_given() {
    if !available("fish") {
        println!("fish is not installed; skipping");
        return;
    }
    // fish's completion functions print `value\tdescription` lines, which is exactly what the
    // binary hands over — so the function can be called directly and its output read.
    let fixture = Fixture::new(
        "fish-candidates",
        Shell::Fish,
        "install\tInstall a tool\nuninstall\tRemove it\n",
    );
    let out = fixture.run("fish", "source ./script; __usage_complete_ex");
    assert_eq!(out, "install\tInstall a tool\nuninstall\tRemove it\n");
}

/// A candidate that looks like an option to fish's own printer is still offered.
///
/// `echo -n` prints nothing at all: the flag is read as fish's, not as the data it was handed. So a
/// CLI with a `-n` flag had that candidate vanish on the way to the prompt.
///
/// Undescribed candidates, which is the case that breaks and the common one — the renderer writes a
/// description column only when something in the answer has one, so a spec whose flags carry no
/// help sends the bare spellings. With a description attached the argument is `-n\tDry run`, which
/// is nobody's option and survives either way; a test written that way passes against the bug.
#[test]
fn fish_prints_a_candidate_that_looks_like_an_option() {
    if !available("fish") {
        println!("fish is not installed; skipping");
        return;
    }
    let fixture = Fixture::new("fish-optionlike", Shell::Fish, "-n\n-e\n-E\n--dry-run\n");
    let out = fixture.run("fish", "source ./script; __usage_complete_ex");
    assert_eq!(
        out, "-n\n-e\n-E\n--dry-run\n",
        "every option-like candidate should arrive as itself"
    );
}

#[test]
fn fish_adds_paths_and_never_the_marker() {
    if !available("fish") {
        println!("fish is not installed; skipping");
        return;
    }
    let fixture = Fixture::new(
        "fish-files",
        Shell::Fish,
        &format!("keep\n{}\n", usage_argv::complete::FILES_MARKER),
    );
    let out = fixture.run("fish", "source ./script; __usage_complete_ex");
    let offered: Vec<&str> = out.lines().map(|l| l.split('\t').next().unwrap()).collect();
    assert!(offered.contains(&"keep"), "{offered:?}");
    assert!(offered.contains(&"alpha.txt"), "{offered:?}");
    assert!(
        !out.contains('\u{1}'),
        "the marker reached the user: {out:?}"
    );
}

/// zsh's completion builtins, stubbed so the script can be driven outside a completion context.
///
/// What is under test is the part of the script this crate wrote: the parsing of the three
/// columns, the aligned display, the menu decision, and whether paths are asked for. zsh's own
/// `compadd` and `_files` are the parts that need no testing here — but they are also what makes
/// the function impossible to call directly, so they are replaced by stubs that report what they
/// were handed. The arrays arrive by name, and zsh's dynamic scoping lets a stub read them.
const ZSH_STUBS: &str = r#"typeset -A compstate
compdef() { }
compadd() {
    local entry
    for entry in "${display[@]}"; do print -r -- "display:$entry"; done
    for entry in "${inserts[@]}"; do print -r -- "insert:$entry"; done
    return 0
}
_files() { print -r -- "_files:$*"; return 0 }
"#;

#[test]
fn zsh_presents_the_three_columns_it_was_given() {
    if !available("zsh") {
        println!("zsh is not installed; skipping");
        return;
    }
    let fixture = Fixture::new(
        "zsh-columns",
        Shell::Zsh,
        "install\tInstall a tool\tinstall\nrm\tRemove it\trm\n",
    );
    let out = fixture.run(
        "zsh",
        &format!(
            "{ZSH_STUBS}\nsource ./script\nBUFFER='ex i'\nCURSOR=4\n_ex\n\
             print -r -- \"insert_mode=${{compstate[insert]:-none}}\"\n"
        ),
    );

    // Descriptions aligned to the longest value, which is what makes a list of candidates read
    // as a table rather than as ragged text.
    assert!(out.contains("display:install  -- Install a tool"), "{out}");
    assert!(out.contains("display:rm       -- Remove it"), "{out}");
    // And what would actually be typed, which is the third column's whole purpose.
    assert!(out.contains("insert:install"), "{out}");
    // Nothing needed quoting, so no menu is forced.
    assert!(out.contains("insert_mode=none"), "{out}");
}

#[test]
fn zsh_hands_paths_to_files_and_forces_a_menu_when_a_value_needs_quoting() {
    if !available("zsh") {
        println!("zsh is not installed; skipping");
        return;
    }
    // A value with a space in it: shown as it is, typed quoted — and zsh should offer a menu
    // rather than silently inserting one of several possibilities.
    let fixture = Fixture::new(
        "zsh-quoted",
        Shell::Zsh,
        &format!(
            "with space\tOdd\t'with space'\n{}\n",
            usage_argv::complete::FILES_MARKER
        ),
    );
    let out = fixture.run(
        "zsh",
        &format!(
            "{ZSH_STUBS}\nsource ./script\nBUFFER='ex '\nCURSOR=3\n_ex\n\
             print -r -- \"insert_mode=${{compstate[insert]:-none}}\"\n"
        ),
    );
    assert!(out.contains("display:with space  -- Odd"), "{out}");
    assert!(out.contains("insert:'with space'"), "{out}");
    assert!(out.contains("insert_mode=menu"), "{out}");
    // And the marker asked zsh's own file completion, rather than becoming a candidate.
    assert!(out.contains("_files:"), "{out}");
    assert!(!out.contains('\u{1}'), "the marker reached the user: {out}");

    // Directories only, when that is what the position takes.
    let fixture = Fixture::new(
        "zsh-dirs",
        Shell::Zsh,
        &format!("{}\n", usage_argv::complete::DIRS_MARKER),
    );
    let out = fixture.run(
        "zsh",
        &format!("{ZSH_STUBS}\nsource ./script\nBUFFER='ex '\nCURSOR=3\n_ex\n"),
    );
    assert!(out.contains("_files:-/"), "{out}");
}

#[test]
fn zsh_passes_extension_filters_to_its_native_file_completer() {
    if !available("zsh") {
        println!("zsh is not installed; skipping");
        return;
    }
    let fixture = Fixture::new(
        "zsh-extensions",
        Shell::Zsh,
        "\u{1}extensions\ttoml\tyaml\n",
    );
    let out = fixture.run(
        "zsh",
        &format!("{ZSH_STUBS}\nsource ./script\nBUFFER='ex '\nCURSOR=3\n_ex\n"),
    );
    assert!(out.contains("_files:-g *.(toml|yaml)"), "{out}");
}

#[test]
fn the_line_a_shell_hands_over_is_cut_at_the_cursor_in_its_own_units() {
    // The cursor is counted differently by every shell — characters in a UTF-8 locale for bash
    // and zsh, characters for fish, an offset into the whole input buffer for PowerShell — so
    // no script passes one. Each cuts the line with its own offset, where the units cancel out.
    // A non-ASCII character before the cursor is what makes the difference visible: passed as a
    // number, `6` would have landed mid-character and completed the wrong word.
    if available("bash") {
        let fixture = Fixture::echoing("bash-cursor", Shell::Bash);
        let out = fixture.run(
            "bash",
            r#"source ./script
COMP_LINE='ex ünicode here'
COMP_POINT=6
COMP_WORDS=(ex ünicode here)
COMP_CWORD=1
_usage_complete_ex
printf '%s\n' "${COMPREPLY[@]}"
"#,
        );
        assert_eq!(out.trim(), "line=[ex üni]", "bash");
    }

    if available("zsh") {
        let fixture = Fixture::echoing("zsh-cursor", Shell::Zsh);
        let out = fixture.run(
            "zsh",
            &format!("{ZSH_STUBS}\nsource ./script\nBUFFER='ex ünicode here'\nCURSOR=6\n_ex\n"),
        );
        // Reported as a candidate by the stand-in, so it comes back through `display`.
        assert!(out.contains("display:line=[ex üni]"), "zsh: {out}");
    }

    if available("fish") {
        // fish is handed a line already cut at the cursor, so what it passes on is that line —
        // which is why its script says nothing about a cursor at all.
        let fixture = Fixture::echoing("fish-cursor", Shell::Fish);
        let out = fixture.run("fish", "source ./script; __usage_complete_ex");
        assert!(out.starts_with("line=["), "fish: {out}");
    }
}

#[test]
fn the_zsh_script_works_however_it_was_installed() {
    if !available("zsh") {
        println!("zsh is not installed; skipping");
        return;
    }
    // Two installs, and a script has to survive both. Dropped in `$fpath` as `_ex`, compinit
    // autoloads the file and calls the function *named after it* — which is why the function is
    // `_ex` rather than something tidier, and what a mismatched name would silently break.
    // Sourced from a config instead, nothing has called it yet, so it has to register itself.
    let fixture = Fixture::new("zsh-install", Shell::Zsh, "install\tInstall\tinstall\n");

    // Sourced: the tail should register, not complete.
    let out = fixture.run(
        "zsh",
        &format!("{ZSH_STUBS}\ncompdef() {{ print -r -- \"compdef:$*\" }}\nsource ./script\n"),
    );
    assert!(
        out.contains("compdef:_ex ex"),
        "sourcing should register: {out}"
    );

    // Autoloaded: the file defines `_ex`, and calling it by that name completes.
    let out = fixture.run(
        "zsh",
        &format!("{ZSH_STUBS}\nsource ./script\nBUFFER='ex i'\nCURSOR=4\n_ex\n"),
    );
    assert!(out.contains("display:install"), "autoload path: {out}");

    // And the name in the `#compdef` line is the one compinit will look for.
    let script_text = script("ex", Shell::Zsh);
    assert!(script_text.contains("#compdef ex"), "{script_text}");
    assert!(script_text.contains("_ex() {"), "{script_text}");
}
