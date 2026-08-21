//! Running the shell scripts a spec embeds in `run=`.

use std::io;
use std::process::{Command, ExitStatus};
use std::string::FromUtf8Error;

use crate::error::{Result, UsageErr};

/// The interpreter a `run=` script is handed to.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum ShellKind {
    /// `sh -c`, what `run=` is written for everywhere in the spec format.
    Posix,
    /// `cmd /c`. Windows only, and only when there is no POSIX shell to be had:
    /// it cannot run a shebang script, a pipeline, or `a; b`.
    Cmd,
}

fn shell_argv(kind: ShellKind) -> (&'static str, &'static str) {
    match kind {
        ShellKind::Posix => ("sh", "-c"),
        ShellKind::Cmd => ("cmd", "/c"),
    }
}

/// Which interpreter to try next after `kind` failed to start, if any.
///
/// Only a missing executable falls back. Anything else — a `sh` that exists but cannot be
/// executed, say — is reported as-is: quietly demoting a broken shell to a different one is
/// the same class of silent wrong behavior this fallback exists to get rid of.
fn fallback_for(kind: ShellKind, err: io::ErrorKind) -> Option<ShellKind> {
    match (kind, err) {
        (ShellKind::Posix, io::ErrorKind::NotFound) if cfg!(windows) => Some(ShellKind::Cmd),
        _ => None,
    }
}

/// The first line of a script, for putting in an error message.
///
/// A `run=` can be a whole multi-line `case … esac`, and these messages surface in a shell
/// completion, where a wall of text buries the prompt.
fn script_excerpt(script: &str) -> String {
    let first_line = script.lines().next().unwrap_or_default();
    match script.lines().nth(1) {
        Some(_) => format!("{first_line} …"),
        None => first_line.to_string(),
    }
}

fn no_shell_message(script: &str) -> String {
    format!(
        "failed to run `run=` script: neither `sh` nor `cmd` could be started\n  \
         script: {}\n  \
         `run=` is executed with `sh -c`, falling back to `cmd /c` on Windows. \
         Install a POSIX shell (Git for Windows ships sh.exe) and make sure it is on PATH.",
        script_excerpt(script)
    )
}

fn non_utf8_message(shell: &str, flag: &str, script: &str, err: &FromUtf8Error) -> String {
    format!(
        "`run=` script produced output that is not valid UTF-8: {err}\n  \
         script: {}\n  \
         shell: {shell} {flag}",
        script_excerpt(script)
    )
}

/// Run a `run=` script and return its stdout.
///
/// Executed with `sh -c`, which is the language the spec format's `run=` is written in — the
/// reference examples use pipelines, `;` sequences and shebang scripts. On Windows, where a
/// POSIX shell is not guaranteed, a missing `sh` falls back to `cmd /c`; that runs a plain
/// command invocation but none of the above, so a spec meant to work there should keep `run=`
/// to a single command.
///
/// stdin is closed and stderr is inherited, so a script cannot stall a completion waiting for
/// input but can still say why it failed. `__USAGE` is set to the usage version, letting a
/// script tell that it was invoked by usage.
///
/// Output that is not valid UTF-8 is an error, not a panic and not a lossy conversion. The
/// `cmd /c` fallback in particular emits the console code page, which is not UTF-8 outside
/// English locales, and a mount's output is parsed as a spec — replacement characters there
/// would resurface as a baffling KDL syntax error instead of an encoding one.
pub fn sh(script: &str) -> Result<String> {
    let mut kind = ShellKind::Posix;
    let output = loop {
        let (shell, flag) = shell_argv(kind);
        let err = match run(shell, flag, script) {
            Ok(output) => break output,
            Err(err) => err,
        };
        match fallback_for(kind, err.kind()) {
            Some(next) => kind = next,
            None if err.kind() == io::ErrorKind::NotFound && cfg!(windows) => {
                return Err(UsageErr::ShellError(no_shell_message(script)));
            }
            None => {
                return Err(UsageErr::ShellError(format!(
                    "{err}\n{shell} {flag} {script}"
                )));
            }
        }
    };

    let (shell, flag) = shell_argv(kind);
    if let Some(failure) = status_failure(output.status) {
        return Err(UsageErr::ShellError(format!(
            "{failure}\n{shell} {flag} {script}"
        )));
    }
    String::from_utf8(output.stdout)
        .map_err(|err| UsageErr::ShellError(non_utf8_message(shell, flag, script, &err)))
}

/// How a non-successful exit reads, or `None` when the script succeeded.
///
/// A signal has no code, so it gets its own wording rather than a missing number.
fn status_failure(status: ExitStatus) -> Option<String> {
    if status.success() {
        return None;
    }
    Some(match status.code() {
        Some(code) => format!("exited with code {code}"),
        None => "terminated by signal".to_string(),
    })
}

fn run(shell: &str, flag: &str, script: &str) -> io::Result<std::process::Output> {
    Command::new(shell)
        .arg(flag)
        .arg(script)
        .stdin(std::process::Stdio::null())
        .stderr(std::process::Stdio::inherit())
        .env("__USAGE", env!("CARGO_PKG_VERSION"))
        .output()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn shell_argv_maps_each_kind() {
        assert_eq!(shell_argv(ShellKind::Posix), ("sh", "-c"));
        assert_eq!(shell_argv(ShellKind::Cmd), ("cmd", "/c"));
    }

    #[test]
    fn a_missing_posix_shell_falls_back_only_on_windows() {
        let fallback = fallback_for(ShellKind::Posix, io::ErrorKind::NotFound);
        if cfg!(windows) {
            assert_eq!(fallback, Some(ShellKind::Cmd));
        } else {
            assert_eq!(fallback, None);
        }
    }

    #[test]
    fn a_shell_that_exists_but_fails_is_not_demoted() {
        // Falling back here would hide a broken `sh` behind a shell that silently
        // mis-executes the script, which is the failure mode this is meant to remove.
        assert_eq!(
            fallback_for(ShellKind::Posix, io::ErrorKind::PermissionDenied),
            None
        );
    }

    #[test]
    fn cmd_is_the_last_resort() {
        assert_eq!(fallback_for(ShellKind::Cmd, io::ErrorKind::NotFound), None);
    }

    #[test]
    fn no_shell_message_names_both_shells_and_the_script() {
        let msg = no_shell_message("echo hello");
        assert!(msg.contains("`sh`"), "{msg}");
        assert!(msg.contains("`cmd`"), "{msg}");
        assert!(msg.contains("echo hello"), "{msg}");
    }

    #[test]
    fn no_shell_message_truncates_a_multi_line_script() {
        let msg = no_shell_message("case $cur in\n  a) echo a ;;\nesac");
        assert!(msg.contains("case $cur in …"), "{msg}");
        assert!(!msg.contains("esac"), "{msg}");
    }

    #[test]
    fn non_utf8_message_names_the_script_and_the_shell() {
        let err = String::from_utf8(vec![0xff]).unwrap_err();
        let msg = non_utf8_message("cmd", "/c", "chcp 932 && dir", &err);
        assert!(msg.contains("chcp 932 && dir"), "{msg}");
        assert!(msg.contains("cmd /c"), "{msg}");
        assert!(msg.contains("not valid UTF-8"), "{msg}");
    }

    #[cfg(unix)]
    #[test]
    fn sh_reports_non_utf8_output_instead_of_panicking() {
        // A `run=` that emits raw bytes used to take the whole process down. `cmd /c` on a
        // non-English Windows reaches this through its console code page.
        let err = sh(r"printf '\377'").unwrap_err();
        assert!(
            err.to_string().contains("not valid UTF-8"),
            "{}",
            err.to_string()
        );
    }

    #[test]
    fn sh_returns_stdout() {
        // `echo` behaves the same under `sh -c` and `cmd /c`, so this runs anywhere.
        assert!(sh("echo hello").unwrap().contains("hello"));
    }

    #[test]
    fn sh_fails_on_a_nonzero_exit() {
        assert!(sh("exit 1").is_err());
    }

    #[cfg(unix)]
    #[test]
    fn sh_exposes_the_usage_version() {
        assert_eq!(
            sh("echo $__USAGE").unwrap().trim(),
            env!("CARGO_PKG_VERSION")
        );
    }
}
