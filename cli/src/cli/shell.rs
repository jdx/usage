use std::fmt::Debug;
use std::path::PathBuf;
use std::process::Stdio;

use itertools::Itertools;
use usage::miette::IntoDiagnostic;
use usage_rs::Args;

use usage::Spec;

use crate::env;

/// What the four shell commands accept, declared once.
///
/// Not a command itself: each of `bash`, `fish`, `powershell` and `zsh` flattens it, so the
/// emitted spec lists these on all four exactly as a hand-written spec would. They cannot
/// share one struct instead — a command collects into the struct that declares it, so the
/// derive refuses two variants wrapping one — and the shell to run is the command that ran.
#[derive(Debug, Args)]
pub struct Shell {
    /// The script to run; its spec is read from the `#USAGE` comments at the top of the file
    script: PathBuf,

    /// Arguments to pass to the script
    ///
    /// Anything `usage` does not recognise is a value rather than a mistake, which is what
    /// lets a shebang script take flags of its own.
    args: Vec<String>,

    /// Print the script's help page instead of running it
    #[usage(short)]
    h: bool,

    /// Print the script's help page instead of running it
    #[usage(long)]
    help: bool,
}

/// One of the four commands that run a script, which differ only in the shell they name.
///
/// Everything true of all four is written here once. What is not — the shebang to write, the
/// program that ends up running, and the variable that overrides it — is passed in as
/// `$shell_specific`, because a page that told a `usage zsh` reader to write a `usage bash`
/// shebang would be wrong three times out of four.
///
/// One literal per shell rather than interpolation: a `concat!` in a doc attribute is still a
/// macro call by the time the derive reads it, and the derive wants a literal.
macro_rules! shell_command {
    ($ty:ident, $program:literal, $about:tt, $shell_specific:literal) => {
        #[doc = "Run a script whose usage spec is written in its own comments"]
        #[doc = ""]
        #[doc = "Usually reached through the script's shebang: the kernel hands the"]
        #[doc = "script and its arguments to `usage`, which parses them against the"]
        #[doc = "`#USAGE` lines at the top of the file, then runs the script with this"]
        #[doc = "shell. Each flag and argument reaches the script as an environment"]
        #[doc = "variable named `usage_<name>`, so `--force` becomes `usage_force` and"]
        #[doc = "`<file>` becomes `usage_file`. A value declared `var=#true` arrives as"]
        #[doc = "one string, joined with `shell_words::join()` so that an element"]
        #[doc = "containing a space stays quoted."]
        #[doc = ""]
        #[doc = "`-h` and `--help` print the script's help page rather than this one."]
        #[doc = ""]
        #[doc = $shell_specific]
        #[derive(Debug, Args)]
        // The words after the script are the script's, so a flag `usage` does not know is a
        // value to forward rather than a mistake to report — the root's `error` stops here.
        #[usage(about = $about, unknown_flags = "value", verbatim_doc_comment)]
        pub struct $ty {
            #[usage(flatten)]
            pub shell: Shell,
        }

        impl usage_rs::Run for $ty {
            type Output = usage::miette::Result<()>;

            fn run(mut self) -> Self::Output {
                self.shell.run($program)
            }
        }
    };
}

shell_command!(
    Bash,
    "bash",
    "Execute a shell script using bash",
    // Hand-wrapped for an 80-column terminal, like every other `verbatim_doc_comment` in
    // this crate: verbatim keeps these line breaks, so a longer line is re-wrapped by the
    // help renderer and comes out ragged.
    "This command's shebang is `#!/usr/bin/env -S usage bash`, and the\nprogram it runs is `bash` from PATH. `USAGECLI_SHELL_BASH` names a\ndifferent one, which is how it is pointed at Git Bash on a Windows\nmachine where `bash` is the WSL launcher."
);
shell_command!(
    Fish,
    "fish",
    "Execute a shell script using fish",
    "This command's shebang is `#!/usr/bin/env -S usage fish`, and the\nprogram it runs is `fish` from PATH. `USAGECLI_SHELL_FISH` names a\ndifferent one."
);
shell_command!(
    PowerShell,
    "pwsh",
    "Execute a shell script using PowerShell",
    "This command's shebang is `#!/usr/bin/env -S usage powershell`, and\nthe program it runs is `pwsh` from PATH. `USAGECLI_SHELL_PWSH` names a\ndifferent one, which is also how it is pointed at `powershell.exe` on a\nmachine that has only Windows PowerShell."
);
shell_command!(
    Zsh,
    "zsh",
    "Execute a shell script using zsh",
    "This command's shebang is `#!/usr/bin/env -S usage zsh`, and the\nprogram it runs is `zsh` from PATH. `USAGECLI_SHELL_ZSH` names a\ndifferent one."
);

impl Shell {
    pub fn run(&mut self, shell: &str) -> usage::miette::Result<()> {
        let spec = Spec::parse_file(&self.script)?;
        let mut args = self.args.clone();
        args.insert(0, spec.bin.clone());

        if self.h {
            return self.help(&spec, &args, false);
        }
        if self.help {
            return self.help(&spec, &args, true);
        }

        let parsed = usage::parse::parse(&spec, &args)?;
        debug!("{parsed:?}");

        let overridden = env::shell_program_override_entry(shell, |key| env::var(key).ok());
        let program = overridden
            .as_ref()
            .map_or_else(|| shell.to_string(), |(_, value)| value.clone());
        debug!("running {program}");

        let mut cmd = std::process::Command::new(&program);
        cmd.stdin(Stdio::inherit());
        cmd.stdout(Stdio::inherit());
        cmd.stderr(Stdio::inherit());
        let script_path = self.script.to_str().ok_or_else(|| {
            usage::miette::miette!("Invalid file path: {}", self.script.display())
        })?;
        let args = std::iter::once(script_path.to_string())
            .chain(self.args.clone())
            .collect_vec();
        cmd.args(&args);

        env::apply_parsed_env(&mut cmd, &parsed.as_env());

        // Name the program: the bare io error says only "No such file or directory", which for
        // an overridden shell gives no clue that the variable is what pointed here. And name the
        // variable the user actually set — the legacy `USAGE_SHELL_*` is still read, so assuming
        // the current spelling would send them to one they never touched.
        let mut child = cmd.spawn().map_err(|err| match &overridden {
            Some((key, _)) => {
                usage::miette::miette!("failed to run `{program}` (from ${key}): {err}")
            }
            None => usage::miette::miette!("failed to run `{program}`: {err}"),
        })?;
        let result = child.wait().into_diagnostic()?;

        if !result.success() {
            let code = result.code().unwrap_or(1);
            if cfg!(windows) && overridden.is_none() {
                if let Some(hint) = wsl_path_hint(shell, code, script_path) {
                    eprintln!("{hint}");
                }
            }
            std::process::exit(code);
        }

        Ok(())
    }

    pub fn help(&self, spec: &Spec, args: &[String], long: bool) -> usage::miette::Result<()> {
        let parsed = usage::parse::parse_partial(spec, args)?;
        print!(
            "{}",
            usage::docs::cli::render_help_styled(
                spec,
                &parsed.cmd,
                long,
                usage::docs::cli::Style::auto(),
            )
        );
        Ok(())
    }
}

/// Whether `path` is a path only Windows understands — a drive letter or a UNC share.
///
/// Hand-written rather than `Path::is_absolute`, whose answer depends on the platform it was
/// compiled for and so cannot be tested from the Linux CI. `/c/foo` (msys) and plain relative
/// paths are not included: both work through the WSL launcher, which translates the working
/// directory for them.
///
/// A drive letter counts even without a following separator. `C:x.sh` is drive-*relative*
/// rather than absolute, but it is still a spelling only Windows resolves — WSL looks for a
/// file named `C:x.sh` and reports 127 exactly as it does for `C:\x.sh`. Requiring the
/// separator would drop the hint for a case that needs it.
fn looks_like_windows_path(path: &str) -> bool {
    let bytes = path.as_bytes();
    let drive_letter = bytes.len() >= 2 && bytes[0].is_ascii_alphabetic() && bytes[1] == b':';
    drive_letter || path.starts_with(r"\\")
}

/// The guess to print when `bash` on Windows could not find a script it was handed.
///
/// Narrow on purpose. `bash` is the only one of the four shells that ships in the system
/// directory, which Windows searches ahead of `PATH`, so on a machine with WSL installed it is
/// the only one that silently resolves to something that cannot read a `C:\…` path. 127 is the
/// shell's own "command not found", which is what the WSL launcher passes back up.
fn wsl_path_hint(shell: &str, code: i32, script: &str) -> Option<String> {
    if shell != "bash" || code != 127 || !looks_like_windows_path(script) {
        return None;
    }
    Some(format!(
        "usage: `bash` exited 127 (command not found) and the script was given as a Windows path.\n\
         usage: On Windows the system directory is searched before $PATH, so `bash` resolves to\n\
         usage: C:\\Windows\\System32\\bash.exe — the WSL launcher — which cannot open `{script}`.\n\
         usage: If that is what happened, pass the script by relative path, or point usage at the\n\
         usage: bash you meant:\n\
         usage:     set {}=C:\\Program Files\\Git\\bin\\bash.exe\n\
         usage: If the script really did exit 127 on its own, ignore this.",
        env::shell_var_name(shell)
    ))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn windows_paths_are_recognized() {
        for path in [r"C:\x", "C:/x", "c:/x", r"\\srv\share\x"] {
            assert!(looks_like_windows_path(path), "{path}");
        }
    }

    #[test]
    fn a_drive_relative_path_counts_too() {
        // Not absolute — `C:x.sh` means "x.sh on drive C's current directory" — but still a
        // spelling only Windows resolves. Measured: `usage bash C:x.sh` reaches WSL, which
        // looks for a file called `C:x.sh` and exits 127, so the hint belongs there.
        for path in ["C:x.sh", "c:x.sh"] {
            assert!(looks_like_windows_path(path), "{path}");
        }
    }

    #[test]
    fn paths_a_posix_shell_can_open_are_not_windows_paths() {
        for path in ["/c/x", "./x.sh", "x.sh", "/usr/local/bin/x", "", "C"] {
            assert!(!looks_like_windows_path(path), "{path}");
        }
    }

    #[test]
    fn the_hint_names_the_script_and_the_override() {
        let hint = wsl_path_hint("bash", 127, r"C:\Users\me\script.sh").unwrap();
        assert!(hint.contains(r"C:\Users\me\script.sh"), "{hint}");
        // The current spelling, since the hint is telling someone what to set. The legacy
        // `USAGE_SHELL_BASH` is still read, but pointing them at it would send them to the one
        // a mise task never receives.
        assert!(hint.contains("USAGECLI_SHELL_BASH"), "{hint}");
        // Hedged, because a script really can exit 127 on its own.
        assert!(hint.contains("ignore this"), "{hint}");
    }

    #[test]
    fn the_hint_stays_quiet_when_it_would_be_guessing() {
        // A script that failed for its own reasons.
        assert!(wsl_path_hint("bash", 1, r"C:\x").is_none());
        // Shells that do not exist in the system directory.
        assert!(wsl_path_hint("zsh", 127, r"C:\x").is_none());
        assert!(wsl_path_hint("pwsh", 127, r"C:\x").is_none());
        // A path the WSL launcher can resolve anyway.
        assert!(wsl_path_hint("bash", 127, "./x.sh").is_none());
    }
}
