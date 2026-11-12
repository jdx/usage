//! Spawning that reports a program that cannot be started.

use std::io;
use std::process::{Child, Command};

/// Spawns `command`, reporting a program that cannot be started as an `Err`.
///
/// The callers tell "could not start" (name the program and the variable that chose it, exit 1)
/// from "ran and exited non-zero" (forward the status, 127 included). `Command::spawn` uses
/// `posix_spawn` where it can, and POSIX lets that report a failed `execve` as a child that exits
/// 127 instead of an error. glibc reports it properly, but `qemu-user` does not, so there a missing
/// interpreter looked like a script that exited 127 and said nothing.
///
/// A `pre_exec` hook makes std fork and exec, which carries the `execve` error back to the parent
/// on every platform. The hook itself does nothing.
pub(crate) fn spawn(command: &mut Command) -> io::Result<Child> {
    #[cfg(unix)]
    {
        use std::os::unix::process::CommandExt as _;
        // SAFETY: the closure runs between fork and exec and does nothing at all.
        unsafe { command.pre_exec(|| Ok(())) };
    }
    command.spawn()
}

#[cfg(all(test, unix))]
mod tests {
    use super::spawn;
    use std::io::ErrorKind;
    use std::process::Command;

    #[test]
    fn a_missing_program_is_an_error() {
        let err = spawn(&mut Command::new("usage-no-such-program-xyz")).unwrap_err();
        assert_eq!(err.kind(), ErrorKind::NotFound);
    }

    #[test]
    fn a_program_that_exits_127_is_not_an_error() {
        let status = spawn(Command::new("sh").args(["-c", "exit 127"]))
            .unwrap()
            .wait()
            .unwrap();
        assert_eq!(status.code(), Some(127));
    }
}
