//! Putting a completion script where its shell will look for it.
//!
//! [`crate::script`] renders the script; this decides where it goes and writes it there. What it
//! does *not* do is the shape of the whole module:
//!
//! - **No startup file is ever edited.** Not `.zshrc`, not `.bashrc`, not `$PROFILE`. Writing the
//!   script again is a no-op, so an upgrade can re-run an install as often as it likes; appending a
//!   line to `.zshrc` again is not, and a tool that owns a user's dotfiles has no undo to offer.
//!   Where a shell needs a line somewhere anyway — zsh's `fpath+=`, PowerShell's dot-source — the
//!   plan carries it as data for the caller to print.
//! - **No shell detection.** The shell is named by the caller. `$SHELL` is the login shell, which is
//!   not necessarily the one running, and a guess made here would be a guess owned here.
//! - **No dependencies**, like the rest of this crate, and nothing on the parse path.
//! - **No uninstall, no install-every-shell, no `$PROFILE` discovery.** All additive later; said
//!   here so they arrive as requests rather than as bug reports.
//!
//! The two layers are separate on purpose. [`plan`] resolves a target from a *described*
//! environment and touches no filesystem, so every row of the table it holds is testable —
//! including the Windows rows, on a machine that is not Windows. [`write`] is the thin half that
//! creates directories and puts bytes in a file.
//!
//! ```
//! use usage_argv::complete::Shell;
//! use usage_argv::install::{plan, Env, Loading, Platform};
//!
//! let env = Env::new(Platform::Linux, [("HOME".to_string(), "/home/u".into())]);
//! let target = plan("mise", Shell::Zsh, &env)?;
//!
//! assert!(target.path.ends_with("zsh/site-functions/_mise"));
//! // zsh finds nothing on its own, so the one line it needs comes back rather than being applied.
//! assert!(matches!(target.loading, Loading::Manual { .. }));
//! # Ok::<(), usage_argv::install::Error>(())
//! ```

use std::collections::BTreeMap;
use std::ffi::{OsStr, OsString};
use std::path::{Path, PathBuf};

use crate::complete::Shell;
use crate::script::{is_one_shell_word, GENERATED_MARKER};

/// Which operating system's conventions a plan follows.
///
/// An input rather than a `cfg!`, which is the one place this diverges from
/// `usage_config::EnvLayer`: a Windows path resolved behind `cfg!(windows)` is checked by nobody
/// until a Windows user reports it, because the machine running the tests is the other one.
/// [`Platform::current`] is where the host gets to decide, and only [`Env::from_process`] calls it.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[non_exhaustive]
pub enum Platform {
    /// Linux, and every other Unix that follows the XDG base directory spec.
    Linux,
    /// macOS, which follows XDG for most of this and `~/Library/Application Support` for nushell.
    MacOs,
    /// Windows.
    Windows,
}

impl Platform {
    /// The platform this binary was compiled for.
    pub const fn current() -> Self {
        #[cfg(windows)]
        {
            Platform::Windows
        }
        #[cfg(target_os = "macos")]
        {
            Platform::MacOs
        }
        #[cfg(not(any(windows, target_os = "macos")))]
        {
            Platform::Linux
        }
    }

    /// Whether variable names are compared without regard to case, as Windows compares them.
    pub const fn is_windows(self) -> bool {
        matches!(self, Platform::Windows)
    }
}

/// The environment a plan is resolved against: some variables, and a platform.
///
/// Described rather than read, for the reason `usage_config::EnvLayer` is: a test should not have to
/// move `$HOME` to ask a question, and two tests asking different questions should be able to run at
/// the same time. A real CLI passes [`Env::from_process`].
#[derive(Debug, Clone)]
pub struct Env {
    platform: Platform,
    /// Keyed by the comparable form of the name — upper-cased when the platform is Windows, where
    /// `%LocalAppData%` and `%LOCALAPPDATA%` are the same variable.
    vars: BTreeMap<String, OsString>,
}

impl Env {
    /// A named environment, for a test or for a CLI that has its own idea of one.
    pub fn new(platform: Platform, vars: impl IntoIterator<Item = (String, OsString)>) -> Self {
        Self {
            platform,
            vars: vars
                .into_iter()
                .map(|(name, value)| (normalize(&name, platform), value))
                .collect(),
        }
    }

    /// The variables this process was started with, on this machine's platform.
    ///
    /// The only place `std::env` is read. Values stay `OsString`, because they become paths and a
    /// home directory that is not UTF-8 is still a home directory. A *name* that is not UTF-8 is
    /// skipped: no variable this looks for can be spelled that way, and `std::env::vars` would
    /// panic on one.
    pub fn from_process() -> Self {
        Self::new(
            Platform::current(),
            std::env::vars_os()
                .filter_map(|(name, value)| Some((name.to_str()?.to_string(), value))),
        )
    }

    /// The platform whose conventions this environment describes.
    pub fn platform(&self) -> Platform {
        self.platform
    }

    /// What this environment says `name` is, if anything.
    pub fn get(&self, name: &str) -> Option<&OsStr> {
        self.vars
            .get(&normalize(name, self.platform))
            .map(OsString::as_os_str)
    }

    /// The same environment with one more variable, for a test or a caller overriding one.
    pub fn with(mut self, name: &str, value: impl Into<OsString>) -> Self {
        self.vars
            .insert(normalize(name, self.platform), value.into());
        self
    }
}

/// A variable's name in the form an environment compares.
///
/// Windows environment variable names are case-insensitive — `std::env::var("PATH")` finds `Path` —
/// so a case-sensitive lookup there would miss a variable the user has plainly set, and would do it
/// only on Windows, which is the worst place for a difference like this to live.
fn normalize(name: &str, platform: Platform) -> String {
    if platform.is_windows() {
        name.to_ascii_uppercase()
    } else {
        name.to_string()
    }
}

/// Where a completion script goes, and what the user still has to do about it.
///
/// Returned by [`plan`], which touches no filesystem — so this is also what a preview prints. There
/// is deliberately no `dry_run` flag on [`write`]: planning *is* the dry run.
#[derive(Debug, Clone, PartialEq, Eq)]
#[non_exhaustive]
pub struct Plan {
    /// The shell this script is for.
    pub shell: Shell,
    /// The name the script registers — the binary's own, or an alias's.
    pub name: String,
    /// The absolute file to write.
    ///
    /// For zsh this ends in `_<name>`: the leading underscore is what `compinit` autoloads, and the
    /// generated script's `#compdef` first line is written for exactly that.
    pub path: PathBuf,
    /// Whether the shell finds this file on its own.
    pub loading: Loading,
    /// The variable that chose the directory, so a preview can explain itself.
    pub resolved_from: &'static str,
    /// A caveat that holds even when loading is automatic, or `None` when there is nothing to say.
    pub note: Option<&'static str>,
}

/// Whether the shell picks a script up by itself.
///
/// A type rather than a `bool` beside an `Option<String>`, because that pair can be read in the
/// wrong order and this distinction is the whole reason the outcome exists.
#[derive(Debug, Clone, PartialEq, Eq)]
#[non_exhaustive]
pub enum Loading {
    /// Nothing else to do: the shell loads this path with no configuration.
    Automatic,
    /// One line, once, in a file this never edits.
    Manual {
        /// The line to add, quoted for that shell.
        line: String,
        /// Where it goes, spelled the way a user would say it: `$PROFILE`, `$nu.config-path`.
        file: String,
        /// Why it is needed, so a caller can print a reason rather than an order.
        why: &'static str,
    },
}

impl Loading {
    /// The line a user still has to add, if there is one.
    pub fn instruction(&self) -> Option<&str> {
        match self {
            Loading::Automatic => None,
            Loading::Manual { line, .. } => Some(line),
        }
    }
}

/// Where `bin`'s completion script for `shell` goes. Touches no filesystem.
pub fn plan(bin: &str, shell: Shell, env: &Env) -> Result<Plan, Error> {
    plan_for(bin, bin, shell, env)
}

/// Where a script registered for `name`, answered by `bin`, goes.
///
/// The alias form, mirroring [`crate::script::script_for`]. Only `name` decides the path today —
/// `bin` is taken so the two pairs stay the same shape, and so a shell whose location depended on
/// the real binary would not be a signature change.
pub fn plan_for(bin: &str, name: &str, shell: Shell, env: &Env) -> Result<Plan, Error> {
    // Both are checked here, before a directory could have been created: an unregisterable name must
    // not leave a half-built tree behind, and validating `bin` too is what keeps `script_for`'s
    // assertions — correct for a name out of a spec — from being reachable as an abort from a name
    // off a command line.
    for candidate in [name, bin] {
        if !is_one_shell_word(candidate) {
            return Err(Error::InvalidName {
                name: candidate.to_string(),
            });
        }
    }

    let (dir, resolved_from) = base_dir(shell, env)?;
    let path = dir.join(file_name(shell, name));
    Ok(Plan {
        shell,
        name: name.to_string(),
        loading: loading(shell, resolved_from, &path),
        note: note(shell, resolved_from),
        path,
        resolved_from,
    })
}

/// The file name a shell looks for a script under.
fn file_name(shell: Shell, name: &str) -> String {
    match shell {
        // bash-completion loads `<cmd>` on demand, by the command's exact name.
        Shell::Bash => name.to_string(),
        // `compinit` autoloads `_<cmd>` and calls the function the file is named after, which is
        // what the generated script's tail is written for.
        Shell::Zsh => format!("_{name}"),
        Shell::Fish => format!("{name}.fish"),
        Shell::Nu => format!("{name}.nu"),
        Shell::PowerShell => format!("{name}.ps1"),
    }
}

/// One place a base directory might come from.
#[derive(Debug, Clone, Copy)]
struct Source {
    /// The variable naming the directory.
    var: &'static str,
    /// Components appended to it.
    under: &'static [&'static str],
    /// Whether the value is a `:`-separated list whose first entry is the one to write to.
    list: bool,
}

impl Source {
    const fn var(var: &'static str, under: &'static [&'static str]) -> Self {
        Self {
            var,
            under,
            list: false,
        }
    }

    const fn list(var: &'static str, under: &'static [&'static str]) -> Self {
        Self {
            var,
            under,
            list: true,
        }
    }
}

/// Where each shell keeps a user's own completion scripts, in the order to try.
///
/// The first source that yields an *absolute* path wins. A relative value is skipped rather than
/// obeyed — that is what the XDG base directory spec says to do with one, and it stops a stray
/// `XDG_DATA_HOME=share` from writing into whatever directory the CLI happened to be run from.
fn sources(shell: Shell, platform: Platform) -> &'static [Source] {
    use Platform::{Linux, MacOs, Windows};

    // bash-completion's own documented answer to "where do my own completions go": the `completions`
    // subdirectory of `$BASH_COMPLETION_USER_DIR`, else of `$XDG_DATA_HOME/bash-completion`. The
    // loader searches every entry of the list; a writer has to pick one, so it picks the first.
    const BASH: &[Source] = &[
        Source::list("BASH_COMPLETION_USER_DIR", &["completions"]),
        Source::var("XDG_DATA_HOME", &["bash-completion", "completions"]),
        Source::var(
            "HOME",
            &[".local", "share", "bash-completion", "completions"],
        ),
    ];
    // A bash on Windows is an msys or Cygwin bash, and it sets `HOME` itself — but a user who has
    // never started one has only `%USERPROFILE%`.
    const BASH_WINDOWS: &[Source] = &[
        Source::list("BASH_COMPLETION_USER_DIR", &["completions"]),
        Source::var("XDG_DATA_HOME", &["bash-completion", "completions"]),
        Source::var(
            "HOME",
            &[".local", "share", "bash-completion", "completions"],
        ),
        Source::var(
            "USERPROFILE",
            &[".local", "share", "bash-completion", "completions"],
        ),
    ];

    const ZSH: &[Source] = &[
        Source::var("XDG_DATA_HOME", &["zsh", "site-functions"]),
        Source::var("HOME", &[".local", "share", "zsh", "site-functions"]),
    ];
    const ZSH_WINDOWS: &[Source] = &[
        Source::var("XDG_DATA_HOME", &["zsh", "site-functions"]),
        Source::var("HOME", &[".local", "share", "zsh", "site-functions"]),
        Source::var("USERPROFILE", &[".local", "share", "zsh", "site-functions"]),
    ];

    const FISH: &[Source] = &[
        Source::var("XDG_CONFIG_HOME", &["fish", "completions"]),
        Source::var("HOME", &[".config", "fish", "completions"]),
    ];
    const FISH_WINDOWS: &[Source] = &[
        Source::var("XDG_CONFIG_HOME", &["fish", "completions"]),
        Source::var("HOME", &[".config", "fish", "completions"]),
        Source::var("USERPROFILE", &[".config", "fish", "completions"]),
    ];

    // A vendor autoload directory is the only place nushell reads a completion from without being
    // told to, and the only way to know where one is, is to be told. Everything after it is a plain
    // file plus a reported `source` line, which is true on every nushell that has ever shipped.
    const NU: &[Source] = &[
        Source::var("NU_VENDOR_AUTOLOAD_DIR", &[]),
        Source::var("XDG_CONFIG_HOME", &["nushell", "completions"]),
        Source::var("HOME", &[".config", "nushell", "completions"]),
    ];
    const NU_MACOS: &[Source] = &[
        Source::var("NU_VENDOR_AUTOLOAD_DIR", &[]),
        Source::var("XDG_CONFIG_HOME", &["nushell", "completions"]),
        Source::var(
            "HOME",
            &["Library", "Application Support", "nushell", "completions"],
        ),
    ];
    const NU_WINDOWS: &[Source] = &[
        Source::var("NU_VENDOR_AUTOLOAD_DIR", &[]),
        Source::var("APPDATA", &["nushell", "completions"]),
        Source::var(
            "USERPROFILE",
            &["AppData", "Roaming", "nushell", "completions"],
        ),
    ];

    // pwsh's own configuration directory on Unix. On Windows the profile lives under `Documents`,
    // which OneDrive redirects often enough that computing it would land the file in a real
    // directory nobody reads — and since PowerShell is handed the absolute path either way, the
    // location only has to be stable and the user's own.
    const POWERSHELL: &[Source] = &[
        Source::var("XDG_CONFIG_HOME", &["powershell", "completions"]),
        Source::var("HOME", &[".config", "powershell", "completions"]),
    ];
    const POWERSHELL_WINDOWS: &[Source] = &[
        Source::var("LOCALAPPDATA", &["PowerShell", "completions"]),
        Source::var(
            "USERPROFILE",
            &["AppData", "Local", "PowerShell", "completions"],
        ),
    ];

    match (shell, platform) {
        (Shell::Bash, Windows) => BASH_WINDOWS,
        (Shell::Bash, Linux | MacOs) => BASH,
        (Shell::Zsh, Windows) => ZSH_WINDOWS,
        (Shell::Zsh, Linux | MacOs) => ZSH,
        (Shell::Fish, Windows) => FISH_WINDOWS,
        (Shell::Fish, Linux | MacOs) => FISH,
        (Shell::Nu, Windows) => NU_WINDOWS,
        (Shell::Nu, MacOs) => NU_MACOS,
        (Shell::Nu, Linux) => NU,
        (Shell::PowerShell, Windows) => POWERSHELL_WINDOWS,
        (Shell::PowerShell, Linux | MacOs) => POWERSHELL,
    }
}

/// The directory a script goes in, and the variable that chose it.
fn base_dir(shell: Shell, env: &Env) -> Result<(PathBuf, &'static str), Error> {
    let sources = sources(shell, env.platform());
    for source in sources {
        let Some(value) = env.get(source.var) else {
            continue;
        };
        let Some(base) = first_entry(value, source.list, env.platform()) else {
            continue;
        };
        // Not `Path::is_absolute`, which decides by `cfg!` and would call every Windows path
        // relative on the machine these tests run on.
        if !is_absolute(base, env.platform()) {
            continue;
        }
        let mut dir = PathBuf::from(base);
        dir.extend(source.under.iter());
        return Ok((dir, source.var));
    }
    Err(Error::NoBaseDir {
        shell,
        platform: env.platform(),
        tried: sources.iter().map(|s| s.var).collect(),
    })
}

/// The entry of a variable to use: the whole value, or the first of a `:`-separated list.
///
/// A list is split as text, because splitting an `OsStr` on a byte and handing back the halves
/// needs an unsafe reconstruction this crate has nothing to spend. A value that is not text is
/// therefore taken whole — which is the right answer for the single-path case that a non-UTF-8 home
/// directory actually is.
///
/// On Windows nothing is split at all: a colon there is a drive letter's, so splitting
/// `C:\bash-completion` would leave `C`, which no rule calls absolute — and the directory a user
/// went out of their way to name would be skipped in favour of a default. A `:`-separated list is
/// a POSIX spelling, and giving up on it under Windows conventions costs an msys shell a second
/// entry it could have had; obeying the first path anyone actually sets is worth more.
fn first_entry(value: &OsStr, list: bool, platform: Platform) -> Option<&OsStr> {
    if !list || platform.is_windows() {
        return Some(value).filter(|v| !v.is_empty());
    }
    value
        .to_str()?
        .split(':')
        .find(|entry| !entry.is_empty())
        .map(OsStr::new)
}

/// Whether a value names a directory from the root, by the rules of the platform being planned for.
///
/// `Path::is_absolute` answers for the host, which is the wrong question twice over: a Windows path
/// is not absolute on Linux, and a `/etc`-style path is not absolute on Windows.
fn is_absolute(value: &OsStr, platform: Platform) -> bool {
    let bytes = value.as_encoded_bytes();
    if !platform.is_windows() {
        return bytes.first() == Some(&b'/');
    }
    // `\\server\share`, or a drive letter with a separator after the colon. A `C:relative` path is
    // relative to that drive's current directory, which is not somewhere to install anything.
    let unc = bytes.starts_with(br"\\") || bytes.starts_with(b"//");
    let rooted = matches!(bytes, [drive, b':', sep, ..]
        if drive.is_ascii_alphabetic() && matches!(sep, b'\\' | b'/'));
    unc || rooted
}

/// Whether a shell will find this file by itself, and the one line it needs if it will not.
fn loading(shell: Shell, resolved_from: &'static str, path: &Path) -> Loading {
    let dir = path.parent().unwrap_or(path);
    match shell {
        Shell::Bash | Shell::Fish => Loading::Automatic,
        Shell::Zsh => Loading::Manual {
            line: format!(
                "fpath+=({})\nautoload -Uz compinit && compinit",
                quote(shell, &dir.display().to_string())
            ),
            // Whether `$ZDOTDIR` is set is a fact about the shell that will read this line, not
            // about the process printing it, so the line names both rather than resolving one from
            // an environment that is not that shell's.
            file: "${ZDOTDIR:-$HOME}/.zshrc".to_string(),
            why: "no user directory is on zsh's default $fpath, and $fpath is not exported to a \
                  child process, so where to add this cannot be discovered — only reported. If \
                  completion still does not appear, compinit's cache predates the directory: \
                  `rm -f ~/.zcompdump*` and start a new shell.",
        },
        // Only a vendor autoload directory is read without being told to.
        Shell::Nu if resolved_from == NU_VENDOR_AUTOLOAD_DIR => Loading::Automatic,
        Shell::Nu => Loading::Manual {
            line: format!("source {}", quote(shell, &path.display().to_string())),
            file: "$nu.config-path".to_string(),
            why: "nushell autoloads only from a vendor autoload directory. Set \
                  NU_VENDOR_AUTOLOAD_DIR and install again to skip this step.",
        },
        Shell::PowerShell => Loading::Manual {
            line: format!(". {}", quote(shell, &path.display().to_string())),
            file: "$PROFILE".to_string(),
            why: "PowerShell has no completion autoload directory, so the profile has to \
                  dot-source the file. `New-Item -ItemType File -Force $PROFILE` first if there is \
                  no profile yet.",
        },
    }
}

/// The variable that makes nushell automatic, named once because two functions test for it.
const NU_VENDOR_AUTOLOAD_DIR: &str = "NU_VENDOR_AUTOLOAD_DIR";

/// What is true about a target even when the shell loads it by itself.
fn note(shell: Shell, resolved_from: &'static str) -> Option<&'static str> {
    match shell {
        Shell::Bash => Some(
            "bash loads this on demand through bash-completion, which has to be installed and \
             sourced by your shell for it to be read at all.",
        ),
        Shell::Nu if resolved_from == NU_VENDOR_AUTOLOAD_DIR => Some(
            "a config.nu that assigns $env.config wholesale after autoload replaces the completer \
             this script chained onto.",
        ),
        Shell::Zsh | Shell::Fish | Shell::Nu | Shell::PowerShell => None,
    }
}

/// A path as a single word the shell will read literally.
///
/// Single quotes rather than double: a single-quoted string has nothing left to expand, so a `$` or
/// a space in a home directory cannot change what the line means. The escape differs — POSIX shells
/// and nushell end the string, add an escaped quote and start again; PowerShell doubles its own —
/// and the two agree on every path containing no quote at all, which is almost all of them.
fn quote(shell: Shell, text: &str) -> String {
    match shell {
        Shell::PowerShell => format!("'{}'", text.replace('\'', "''")),
        _ => format!("'{}'", text.replace('\'', "'\\''")),
    }
}

/// What happened on disk.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[non_exhaustive]
pub enum Wrote {
    /// There was no file there.
    Created,
    /// The file was already byte for byte what would have been written; nothing was touched.
    Unchanged,
    /// A file this had written before held something else, and was replaced.
    Updated,
    /// A file this did not write was replaced, because the caller said to.
    Replaced,
}

/// What was written, and what the user still has to do about it.
#[derive(Debug, Clone, PartialEq, Eq)]
#[non_exhaustive]
pub struct Installed {
    /// Where it went, and what the shell still needs.
    pub plan: Plan,
    /// What happened to the file.
    pub wrote: Wrote,
}

/// What to do about a file at the target path that this did not write.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[non_exhaustive]
pub enum OnForeign {
    /// Report it and write nothing. What an ordinary install passes.
    Refuse,
    /// Overwrite it. What a `--force` passes, once the user has seen the refusal.
    Overwrite,
}

/// Write `script` to a planned path, creating the directories above it.
///
/// Takes the script rather than rendering one, so a caller can install a script that came from
/// somewhere else — a spec interpreted at run time, a view — through the same resolver instead of
/// reimplementing where files go.
///
/// A file already there is read first, which is what makes an upgrade distinguishable from a theft:
/// identical bytes are [`Wrote::Unchanged`] and nothing is written at all, a file carrying this
/// crate's generated marker is [`Wrote::Updated`], and anything else is [`Error::Foreign`] unless
/// `on_foreign` says otherwise.
pub fn write(plan: &Plan, script: &str, on_foreign: OnForeign) -> Result<Installed, Error> {
    if let Some(dir) = plan.path.parent() {
        io(dir, Doing::CreatingDir, std::fs::create_dir_all(dir))?;
    }

    let wrote = match std::fs::read(&plan.path) {
        Ok(existing) if existing == script.as_bytes() => {
            return Ok(Installed {
                plan: plan.clone(),
                wrote: Wrote::Unchanged,
            })
        }
        Ok(existing) if ours(&existing) => Wrote::Updated,
        Ok(_) => match on_foreign {
            OnForeign::Refuse => {
                return Err(Error::Foreign {
                    path: plan.path.clone(),
                })
            }
            OnForeign::Overwrite => Wrote::Replaced,
        },
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => Wrote::Created,
        Err(e) => {
            return Err(Error::Io {
                path: plan.path.clone(),
                doing: Doing::Reading,
                source: e,
            })
        }
    };

    replace(&plan.path, script)?;
    Ok(Installed {
        plan: plan.clone(),
        wrote,
    })
}

/// Put `script` at `path`, whole.
///
/// Written beside the target and renamed over it, because bash-completion loads a script on demand,
/// at a prompt, from the very file being written — and a shell that reads half of one gets a syntax
/// error instead of completion.
fn replace(path: &Path, script: &str) -> Result<(), Error> {
    let tmp = temp_beside(path);
    io(&tmp, Doing::Writing, std::fs::write(&tmp, script))?;
    match std::fs::rename(&tmp, path) {
        Ok(()) => Ok(()),
        Err(_) => {
            // Windows refuses to rename over a file another process holds open. A direct write is
            // the lesser evil there: a torn read is one bad prompt, while a failed rename is a CLI
            // whose completions can never be upgraded.
            let direct = std::fs::write(path, script);
            let _ = std::fs::remove_file(&tmp);
            // If the write fails too, its error is the one to report: it is the attempt that
            // actually ended this, and "permission denied" on the target is what a user can act on.
            // The rename's error is dropped rather than preferred — a caller told that *replacing*
            // failed, when what failed was writing, looks for the wrong problem.
            io(path, Doing::Writing, direct)
        }
    }
}

/// A scratch path in the target's own directory, so the rename cannot cross a filesystem.
fn temp_beside(path: &Path) -> PathBuf {
    use std::sync::atomic::{AtomicU64, Ordering};
    static NEXT: AtomicU64 = AtomicU64::new(0);

    let name = path
        .file_name()
        .map(|n| n.to_string_lossy().to_string())
        .unwrap_or_default();
    let unique = NEXT.fetch_add(1, Ordering::Relaxed);
    path.with_file_name(format!("{name}.usage-{}-{unique}.tmp", std::process::id()))
}

/// Whether a file that is already there is one of ours.
fn ours(existing: &[u8]) -> bool {
    let marker = GENERATED_MARKER.as_bytes();
    existing
        .windows(marker.len())
        .any(|window| window == marker)
}

/// Plan, render and write, in the described environment.
///
/// The one call a `completion install` command needs.
pub fn install(
    bin: &str,
    shell: Shell,
    env: &Env,
    on_foreign: OnForeign,
) -> Result<Installed, Error> {
    install_for(bin, bin, shell, env, on_foreign)
}

/// The alias form of [`install`]: the installed script registers `name` and asks `bin` for answers.
pub fn install_for(
    bin: &str,
    name: &str,
    shell: Shell,
    env: &Env,
    on_foreign: OnForeign,
) -> Result<Installed, Error> {
    let plan = plan_for(bin, name, shell, env)?;
    // `plan_for` has already refused everything `script_for` would assert on.
    let script = crate::script::script_for(bin, name, shell);
    write(&plan, &script, on_foreign)
}

/// Wrap a filesystem call with what it was doing and where.
fn io<T>(path: &Path, doing: Doing, result: std::io::Result<T>) -> Result<T, Error> {
    result.map_err(|source| Error::Io {
        path: path.to_path_buf(),
        doing,
        source,
    })
}

/// Why an install did not happen.
///
/// A type of its own rather than a variant of [`crate::Error`]: that one rides in the `Result` a
/// successful parse returns, is 40 bytes, and borrows everything it holds. This owns a `PathBuf` and
/// an `io::Error`, and nothing on the hot path should pay for either.
#[derive(Debug)]
#[non_exhaustive]
pub enum Error {
    /// A name no generated script could register, so no file was placed for it.
    InvalidName {
        /// The offending name.
        name: String,
    },
    /// Nothing in the described environment said where this shell keeps its completions.
    NoBaseDir {
        /// The shell that was asked about.
        shell: Shell,
        /// The platform whose conventions were followed.
        platform: Platform,
        /// The variables that were looked at, in the order they were tried.
        tried: Vec<&'static str>,
    },
    /// A file is already at the target path that this did not write.
    ///
    /// Carries the path and not the file's contents: whatever is there is the user's, and a caller
    /// that wants to show it can read it itself.
    Foreign {
        /// The path that was left alone.
        path: PathBuf,
    },
    /// Creating a directory, or reading or writing the file, failed.
    Io {
        /// What was being operated on.
        path: PathBuf,
        /// Which step failed, so a message can say more than "permission denied".
        doing: Doing,
        /// The underlying failure.
        source: std::io::Error,
    },
}

/// Which step of an install failed.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[non_exhaustive]
pub enum Doing {
    /// Creating the directories above the script.
    CreatingDir,
    /// Reading a file that is already there, to see whose it is.
    Reading,
    /// Writing the script, either beside the target or over it.
    Writing,
}

impl Doing {
    /// What to call this step in a message.
    pub fn as_str(self) -> &'static str {
        match self {
            Doing::CreatingDir => "creating",
            Doing::Reading => "reading",
            Doing::Writing => "writing",
        }
    }
}

impl std::fmt::Display for Error {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Error::InvalidName { name } => write!(
                f,
                "a completion script cannot be installed for {name:?}: a binary's name has to be \
                 one plain shell word"
            ),
            Error::NoBaseDir {
                shell,
                platform,
                tried,
            } => {
                write!(
                    f,
                    "nothing says where {} keeps its completions on {}: ",
                    shell.as_str(),
                    match platform {
                        Platform::Linux => "this system",
                        Platform::MacOs => "macOS",
                        Platform::Windows => "Windows",
                    }
                )?;
                if tried.is_empty() {
                    write!(f, "this shell has no known location")
                } else {
                    write!(f, "none of {} is set to an absolute path", tried.join(", "))
                }
            }
            Error::Foreign { path } => write!(
                f,
                "{} was not written by usage, so it was left alone",
                path.display()
            ),
            Error::Io { path, doing, .. } => {
                write!(f, "{} {} failed", doing.as_str(), path.display())
            }
        }
    }
}

impl std::error::Error for Error {
    fn source(&self) -> Option<&(dyn std::error::Error + 'static)> {
        match self {
            Error::Io { source, .. } => Some(source),
            _ => None,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// The environments below name their variables and nothing else, so a test never has to move
    /// `$HOME` and two tests can describe different machines at once.
    fn described(platform: Platform, vars: &[(&str, &str)]) -> Env {
        Env::new(
            platform,
            vars.iter()
                .map(|(name, value)| ((*name).to_string(), OsString::from(*value))),
        )
    }

    fn home(platform: Platform) -> Env {
        described(platform, &[("HOME", "/home/u")])
    }

    const SHELLS: [Shell; 5] = [
        Shell::Bash,
        Shell::Zsh,
        Shell::Fish,
        Shell::Nu,
        Shell::PowerShell,
    ];

    const PLATFORMS: [Platform; 3] = [Platform::Linux, Platform::MacOs, Platform::Windows];

    #[test]
    fn bash_prefers_the_directory_bash_completion_names() {
        let env = described(
            Platform::Linux,
            &[
                ("BASH_COMPLETION_USER_DIR", "/opt/bc"),
                ("XDG_DATA_HOME", "/home/u/data"),
                ("HOME", "/home/u"),
            ],
        );
        let target = plan("ex", Shell::Bash, &env).unwrap();
        assert_eq!(target.path, Path::new("/opt/bc/completions/ex"));
        assert_eq!(target.resolved_from, "BASH_COMPLETION_USER_DIR");
    }

    #[test]
    fn bash_falls_back_from_xdg_to_home() {
        let xdg = described(
            Platform::Linux,
            &[("XDG_DATA_HOME", "/home/u/data"), ("HOME", "/home/u")],
        );
        let target = plan("ex", Shell::Bash, &xdg).unwrap();
        assert_eq!(
            target.path,
            Path::new("/home/u/data/bash-completion/completions/ex")
        );
        assert_eq!(target.resolved_from, "XDG_DATA_HOME");

        let fallback = plan("ex", Shell::Bash, &home(Platform::Linux)).unwrap();
        assert_eq!(
            fallback.path,
            Path::new("/home/u/.local/share/bash-completion/completions/ex")
        );
        assert_eq!(fallback.resolved_from, "HOME");
    }

    #[test]
    fn bash_takes_the_first_of_a_colon_separated_list() {
        let listed = described(
            Platform::Linux,
            &[
                ("BASH_COMPLETION_USER_DIR", "/a:/b:/c"),
                ("HOME", "/home/u"),
            ],
        );
        assert_eq!(
            plan("ex", Shell::Bash, &listed).unwrap().path,
            Path::new("/a/completions/ex")
        );

        // An empty leading entry is a `:` somebody left in, not a request to write to a relative
        // path, so it is skipped rather than resolved.
        let leading = described(
            Platform::Linux,
            &[("BASH_COMPLETION_USER_DIR", ":/b"), ("HOME", "/home/u")],
        );
        assert_eq!(
            plan("ex", Shell::Bash, &leading).unwrap().path,
            Path::new("/b/completions/ex")
        );
    }

    #[test]
    fn bash_and_fish_load_without_being_told() {
        for shell in [Shell::Bash, Shell::Fish] {
            let target = plan("ex", shell, &home(Platform::Linux)).unwrap();
            assert_eq!(target.loading, Loading::Automatic, "{shell:?}");
            assert!(target.loading.instruction().is_none(), "{shell:?}");
        }
        // bash's automatic is conditional on bash-completion being there, which the note says and
        // fish has nothing to add to.
        assert!(plan("ex", Shell::Bash, &home(Platform::Linux))
            .unwrap()
            .note
            .is_some());
        assert!(plan("ex", Shell::Fish, &home(Platform::Linux))
            .unwrap()
            .note
            .is_none());
    }

    #[test]
    fn fish_completes_from_its_config_directory() {
        let target = plan("ex", Shell::Fish, &home(Platform::Linux)).unwrap();
        assert_eq!(
            target.path,
            Path::new("/home/u/.config/fish/completions/ex.fish")
        );
    }

    #[test]
    fn zsh_names_the_file_compinit_will_autoload() {
        let target = plan("ex", Shell::Zsh, &home(Platform::Linux)).unwrap();
        assert_eq!(target.path.file_name().unwrap(), "_ex");
        assert_eq!(
            target.path.parent().unwrap(),
            Path::new("/home/u/.local/share/zsh/site-functions")
        );

        // An alias is registered under its own name, so it is its own file.
        let alias = plan_for("mise", "m", Shell::Zsh, &home(Platform::Linux)).unwrap();
        assert_eq!(alias.path.file_name().unwrap(), "_m");
    }

    #[test]
    fn zsh_reports_the_line_that_makes_it_load() {
        let target = plan("ex", Shell::Zsh, &home(Platform::Linux)).unwrap();
        let Loading::Manual { line, file, why } = &target.loading else {
            panic!("zsh finds nothing on its own: {:?}", target.loading);
        };
        // The directory goes on `$fpath`, never the file — adding the file would have `compinit`
        // looking for completions inside a completion.
        assert!(
            line.contains("fpath+=('/home/u/.local/share/zsh/site-functions')"),
            "{line}"
        );
        assert!(!line.contains("_ex"), "{line}");
        assert!(line.contains("compinit"), "{line}");
        // Whether `$ZDOTDIR` is set is the reading shell's business, so the line names both.
        assert_eq!(file, "${ZDOTDIR:-$HOME}/.zshrc");
        // The trap that makes a correct instruction look like it did not work.
        assert!(why.contains("zcompdump"), "{why}");
    }

    #[test]
    fn nothing_here_pretends_to_have_found_fpath() {
        // `$fpath` is a shell array, not an exported variable: a child process cannot see it. A plan
        // that changed when one appeared would be a plan built on a coincidence.
        for shell in SHELLS {
            let plain = plan("ex", shell, &home(Platform::Linux)).unwrap();
            let with = plan(
                "ex",
                shell,
                &home(Platform::Linux)
                    .with("FPATH", "/usr/share/zsh/site-functions")
                    .with("fpath", "/usr/share/zsh/site-functions"),
            )
            .unwrap();
            assert_eq!(plain, with, "{shell:?}");
        }
    }

    #[test]
    fn nu_is_automatic_only_when_the_environment_names_the_vendor_directory() {
        let vendored = home(Platform::Linux).with("NU_VENDOR_AUTOLOAD_DIR", "/opt/nu/autoload");
        let target = plan("ex", Shell::Nu, &vendored).unwrap();
        assert_eq!(target.path, Path::new("/opt/nu/autoload/ex.nu"));
        assert_eq!(target.loading, Loading::Automatic);
        assert!(target.note.is_some(), "a wholesale $env.config still wins");

        let told = plan("ex", Shell::Nu, &home(Platform::Linux)).unwrap();
        assert_eq!(
            told.path,
            Path::new("/home/u/.config/nushell/completions/ex.nu")
        );
        let Loading::Manual { line, file, .. } = &told.loading else {
            panic!("without a vendor directory nushell has to be told: {told:?}");
        };
        assert!(line.starts_with("source "), "{line}");
        assert!(line.contains("ex.nu"), "{line}");
        assert_eq!(file, "$nu.config-path");
    }

    #[test]
    fn nu_follows_each_platform_to_its_own_config_directory() {
        assert_eq!(
            plan("ex", Shell::Nu, &home(Platform::MacOs)).unwrap().path,
            Path::new("/home/u/Library/Application Support/nushell/completions/ex.nu")
        );
        let windows = described(
            Platform::Windows,
            &[("APPDATA", r"C:\Users\u\AppData\Roaming")],
        );
        let target = plan("ex", Shell::Nu, &windows).unwrap();
        assert!(
            target.path.ends_with("nushell/completions/ex.nu"),
            "{target:?}"
        );
        assert_eq!(target.resolved_from, "APPDATA");
    }

    #[test]
    fn powershell_always_reports_a_dot_source_line() {
        for platform in PLATFORMS {
            let env = described(
                platform,
                &[
                    ("HOME", "/home/u"),
                    ("LOCALAPPDATA", r"C:\Users\u\AppData\Local"),
                ],
            );
            let target = plan("ex", Shell::PowerShell, &env).unwrap();
            let Loading::Manual { line, file, .. } = &target.loading else {
                panic!("PowerShell has no autoload directory anywhere: {platform:?}");
            };
            // The line carries the path that was actually chosen, which is what makes the choice of
            // directory a matter of stability rather than of discovery.
            assert!(
                line.contains(&target.path.display().to_string()),
                "{line} / {target:?}"
            );
            assert!(line.starts_with(". "), "{line}");
            assert_eq!(file, "$PROFILE");
        }
    }

    #[test]
    fn windows_plans_land_under_the_windows_variables() {
        let env = described(
            Platform::Windows,
            &[
                ("USERPROFILE", r"C:\Users\u"),
                ("APPDATA", r"C:\Users\u\AppData\Roaming"),
                ("LOCALAPPDATA", r"C:\Users\u\AppData\Local"),
            ],
        );
        // Asserted as a text prefix rather than with `Path::starts_with`, which compares whole
        // components: a Windows path is one component to a Linux `Path`, and the host running this
        // is not Windows. What is being checked is which variable answered, not how a `PathBuf`
        // renders a separator, which is the host's business either way.
        for (shell, expected) in [
            (Shell::Bash, ("USERPROFILE", r"C:\Users\u")),
            (Shell::Zsh, ("USERPROFILE", r"C:\Users\u")),
            (Shell::Fish, ("USERPROFILE", r"C:\Users\u")),
            (Shell::Nu, ("APPDATA", r"C:\Users\u\AppData\Roaming")),
            (
                Shell::PowerShell,
                ("LOCALAPPDATA", r"C:\Users\u\AppData\Local"),
            ),
        ] {
            let (var, base) = expected;
            let target = plan("ex", shell, &env).unwrap();
            assert_eq!(target.resolved_from, var, "{shell:?}");
            assert!(
                target.path.to_string_lossy().starts_with(base),
                "{shell:?}: {:?}",
                target.path
            );
        }
    }

    #[test]
    fn a_windows_path_is_absolute_where_a_unix_one_is_not_and_the_reverse() {
        // `Path::is_absolute` answers for the host. Both directions matter: a plan for Windows made
        // on Linux must accept `C:\`, and a plan for Linux must not accept it.
        assert!(is_absolute(OsStr::new(r"C:\Users\u"), Platform::Windows));
        assert!(is_absolute(OsStr::new("C:/Users/u"), Platform::Windows));
        assert!(is_absolute(OsStr::new(r"\\host\share"), Platform::Windows));
        assert!(!is_absolute(OsStr::new("C:relative"), Platform::Windows));
        assert!(!is_absolute(OsStr::new("/home/u"), Platform::Windows));
        assert!(is_absolute(OsStr::new("/home/u"), Platform::Linux));
        assert!(!is_absolute(OsStr::new(r"C:\Users\u"), Platform::Linux));
    }

    #[test]
    fn windows_variable_names_are_matched_without_case() {
        let windows = described(Platform::Windows, &[("LocalAppData", r"C:\Users\u\Local")]);
        assert_eq!(
            plan("ex", Shell::PowerShell, &windows)
                .unwrap()
                .resolved_from,
            "LOCALAPPDATA"
        );
        // And only on Windows: elsewhere the name is the name, and a lowercase `home` belongs to
        // something else.
        let linux = described(Platform::Linux, &[("home", "/home/u")]);
        assert!(matches!(
            plan("ex", Shell::Zsh, &linux),
            Err(Error::NoBaseDir { .. })
        ));
    }

    #[test]
    fn a_relative_value_is_ignored_rather_than_obeyed() {
        let relative = described(
            Platform::Linux,
            &[("XDG_DATA_HOME", "share"), ("HOME", "/home/u")],
        );
        let target = plan("ex", Shell::Zsh, &relative).unwrap();
        assert_eq!(target.resolved_from, "HOME");
        assert!(target.path.is_absolute(), "{:?}", target.path);
    }

    #[test]
    fn an_empty_environment_is_an_error_and_not_a_guess() {
        for platform in PLATFORMS {
            for shell in SHELLS {
                let err = plan("ex", shell, &described(platform, &[])).unwrap_err();
                let Error::NoBaseDir { tried, .. } = &err else {
                    panic!("{shell:?} on {platform:?}: {err:?}");
                };
                assert!(!tried.is_empty(), "{shell:?} on {platform:?}");
                // The message names them, so a user can set one rather than guess.
                let rendered = err.to_string();
                assert!(rendered.contains(tried[0]), "{rendered}");
            }
        }
    }

    #[test]
    fn every_plan_names_a_file_under_a_rooted_directory() {
        for platform in PLATFORMS {
            let env = described(
                platform,
                &[
                    ("HOME", "/home/u"),
                    ("USERPROFILE", r"C:\Users\u"),
                    ("APPDATA", r"C:\Users\u\Roaming"),
                    ("LOCALAPPDATA", r"C:\Users\u\Local"),
                ],
            );
            for shell in SHELLS {
                let target = plan("ex", shell, &env).unwrap();
                assert!(
                    !target
                        .path
                        .components()
                        .any(|c| matches!(c, std::path::Component::ParentDir)),
                    "{shell:?} on {platform:?}: {:?}",
                    target.path
                );
                assert!(target.path.file_name().is_some_and(|n| !n.is_empty()));
            }
        }
    }

    #[test]
    fn an_alias_plans_its_own_file_beside_the_binarys() {
        for shell in SHELLS {
            let own = plan("mise", shell, &home(Platform::Linux)).unwrap();
            let alias = plan_for("mise", "m", shell, &home(Platform::Linux)).unwrap();
            assert_ne!(own.path, alias.path, "{shell:?}");
            assert_eq!(alias.name, "m");
        }
    }

    #[test]
    fn a_name_no_script_could_register_is_refused_before_a_directory_exists() {
        // The traversal case is why this is an error rather than the assertion `script_for` makes: a
        // name can reach here from a command line.
        for bad in ["", "my tool", "../../../.zshrc", "a/b"] {
            let err = plan(bad, Shell::Zsh, &home(Platform::Linux)).unwrap_err();
            assert!(
                matches!(&err, Error::InvalidName { name } if name == bad),
                "{bad:?}: {err:?}"
            );
        }
        // The binary is checked too, so `script_for`'s assertions are unreachable from here.
        assert!(matches!(
            plan_for("my tool", "ex", Shell::Zsh, &home(Platform::Linux)),
            Err(Error::InvalidName { .. })
        ));
    }

    #[test]
    fn the_environment_a_process_has_is_read_as_this_machine() {
        let from_process = Env::from_process();
        assert_eq!(from_process.platform(), Platform::current());
        assert!(from_process.get("PATH").is_some(), "every process has one");
    }

    #[test]
    fn a_windows_bash_directory_is_not_split_at_its_drive_letter() {
        // `BASH_COMPLETION_USER_DIR` is the one list-valued source, and a colon on Windows belongs
        // to a drive rather than to a list. Splitting left `C`, which is not absolute, so the
        // directory the user named was skipped in favour of a default — obeying a setting is the
        // point of reading it.
        let windows = described(
            Platform::Windows,
            &[
                ("BASH_COMPLETION_USER_DIR", r"C:\bc"),
                ("USERPROFILE", r"C:\Users\u"),
            ],
        );
        let target = plan("ex", Shell::Bash, &windows).unwrap();
        assert_eq!(target.resolved_from, "BASH_COMPLETION_USER_DIR");
        assert!(
            target.path.to_string_lossy().starts_with(r"C:\bc"),
            "{:?}",
            target.path
        );

        // Elsewhere a list is still a list.
        let unix = described(
            Platform::Linux,
            &[("BASH_COMPLETION_USER_DIR", "/a:/b"), ("HOME", "/home/u")],
        );
        assert_eq!(
            plan("ex", Shell::Bash, &unix).unwrap().path,
            Path::new("/a/completions/ex")
        );
    }

    #[test]
    fn a_script_generated_by_the_cli_is_ours_too() {
        // The marker names the family, not the crate. usage-lib stamps `@generated by usage-cli`,
        // and `usage g completion --install` writes those scripts through this same writer — so an
        // ownership test naming only `usage-argv` would refuse to upgrade every script the CLI
        // installed, which is the promise exactly inverted.
        assert!(ours(
            b"# @generated by usage-cli from usage spec\n_usage() { }\n"
        ));
        assert!(ours(
            b"#compdef ex\n# @generated by usage-argv for `ex ...`\n"
        ));
        // And a file nobody generated is still a stranger's.
        assert!(!ours(b"#compdef ex\n# an evening's work\n"));
    }

    #[test]
    fn a_name_that_is_only_dots_is_refused() {
        // Both are spelled out of accepted characters, and for bash the file is called the name
        // itself — so `..` would plan a path leaving the directory the plan just chose.
        for bad in [".", ".."] {
            assert!(
                matches!(
                    plan(bad, Shell::Bash, &home(Platform::Linux)),
                    Err(Error::InvalidName { .. })
                ),
                "{bad:?} was accepted"
            );
        }
        // A dot inside a name is ordinary: `foo.sh` is a binary somebody has.
        assert!(plan("foo.sh", Shell::Bash, &home(Platform::Linux)).is_ok());
    }

    #[test]
    fn every_generated_script_carries_the_marker_the_installer_looks_for() {
        // Ownership detection reads this string back off disk. If the header stops carrying it,
        // every already-installed script becomes a stranger's file and every upgrade starts
        // refusing — a failure nobody would connect to an edit in `header`.
        for shell in SHELLS {
            let script = crate::script::script("ex", shell);
            assert!(
                ours(script.as_bytes()),
                "{shell:?} carries no {GENERATED_MARKER:?}"
            );
        }
    }
}
