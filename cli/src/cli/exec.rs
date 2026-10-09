use std::fmt::Debug;
use std::path::PathBuf;
use std::process::Stdio;

use itertools::Itertools;
use usage::miette::IntoDiagnostic;
use usage_rs::Args;

use usage::Spec;

use crate::env;

/// Run a script through any interpreter, with its parsed arguments as environment variables
///
/// For scripts in a language `usage` has no dedicated command for. A shebang of
/// `#!/usr/bin/env -S usage exec node` parses the arguments against the `USAGE` comments at the
/// top of the file, then runs `node <script> <args>` with each flag and argument exported as
/// `usage_<name>`. When a file named `.<script>.usage.kdl` sits beside the script, the spec is
/// read from it instead of from the comments.
///
/// A command with whitespace in it is split like a shell would, so interpreter arguments can come
/// before the script: `#!/usr/bin/env -S usage exec "deno run --allow-env=usage_*"` runs
/// `deno run --allow-env=usage_* <script> <args>`. A name that is an existing file, or is found on `PATH`, is never split.
///
/// `-h` and `--help` belong to the script once one is named, so they print its help page
/// rather than this one. Asked with no script to describe, they print this page.
#[derive(Debug, Args)]
// The words after the script are the script's, so a flag `usage` does not know is a value to
// forward rather than a mistake to report — the root's `error` stops here.
#[usage(alias = "x", unknown_flags = "value")]
pub struct Exec {
    /// The interpreter to run the script with, such as `node` or `python3`, or a quoted command
    /// with its own arguments, such as `"deno run --allow-env"`
    command: String,
    /// The script to run
    bin: PathBuf,
    /// Arguments to pass to the script
    args: Vec<String>,

    /// Print the script's help page instead of running it
    #[usage(short)]
    h: bool,

    /// Print the script's help page instead of running it
    #[usage(long)]
    help: bool,
}

/// Whether `command` is a file as written or a file of that name on `PATH`. On Windows the
/// executable extensions in `PATHEXT` count too, as they do when the program is spawned.
fn names_a_program(command: &str) -> bool {
    let mut names = vec![command.to_string()];
    if cfg!(windows) {
        let exts = std::env::var("PATHEXT").unwrap_or_else(|_| ".COM;.EXE;.BAT;.CMD".into());
        names.extend(
            exts.split(';')
                .filter(|e| !e.is_empty())
                .map(|e| format!("{command}{e}")),
        );
    }
    let dirs = std::env::var_os("PATH")
        .map(|p| std::env::split_paths(&p).collect_vec())
        .unwrap_or_default();
    names.iter().any(|name| {
        std::path::Path::new(name).is_file() || dirs.iter().any(|dir| dir.join(name).is_file())
    })
}

/// Splits a command the way Windows does (`CommandLineToArgvW`): only double quotes group, and a
/// backslash is literal unless it precedes a quote, where `2n` backslashes yield `n` and close
/// the quote while `2n+1` yield `n` and a literal quote. `C:\Tools\node.exe --no-warnings` keeps
/// its separators, and `"C:\dir\\"` ends in one backslash.
fn split_windows(command: &str) -> Vec<String> {
    let mut words = vec![];
    let mut word = String::new();
    let mut started = false;
    let mut quoted = false;
    let mut chars = command.chars().peekable();
    while let Some(c) = chars.next() {
        match c {
            '\\' => {
                let mut run = 1;
                while chars.next_if_eq(&'\\').is_some() {
                    run += 1;
                }
                started = true;
                if chars.peek() == Some(&'"') {
                    word.extend(std::iter::repeat_n('\\', run / 2));
                    if run % 2 == 1 {
                        word.push('"');
                        chars.next();
                    }
                } else {
                    word.extend(std::iter::repeat_n('\\', run));
                }
            }
            '"' => {
                quoted = !quoted;
                started = true;
            }
            c if c.is_whitespace() && !quoted => {
                if started {
                    words.push(std::mem::take(&mut word));
                    started = false;
                }
            }
            c => {
                word.push(c);
                started = true;
            }
        }
    }
    if started {
        words.push(word);
    }
    words
}

impl Exec {
    /// The program followed by any arguments that belong before the script.
    fn interpreter(&self) -> usage::miette::Result<Vec<String>> {
        let command = self.command.as_str();
        if command.contains(char::is_whitespace) && !names_a_program(command) {
            let words = if cfg!(windows) {
                split_windows(command)
            } else {
                shell_words::split(command).into_diagnostic()?
            };
            if !words.is_empty() {
                return Ok(words);
            }
        }
        Ok(vec![self.command.clone()])
    }

    pub fn help(&self, spec: &Spec, args: &[String], long: bool) -> usage::miette::Result<()> {
        let parsed = usage::parse::parse_partial(spec, args)?;
        print!(
            "{}",
            // The script's own help, so `choices run=` is run and its values listed.
            usage::docs::cli::render_runtime_help(
                spec,
                &parsed.cmd,
                long,
                usage::docs::cli::Style::auto(),
                None,
            )
        );
        Ok(())
    }
}

impl usage_rs::Run for Exec {
    type Output = usage::miette::Result<()>;

    fn run(self) -> Self::Output {
        let parent = self
            .bin
            .parent()
            .map(|p| p.to_path_buf())
            .unwrap_or_default();
        let bin_name = self
            .bin
            .file_name()
            .and_then(|n| n.to_str())
            .ok_or_else(|| usage::miette::miette!("Invalid file path: {}", self.bin.display()))?;
        let dotted_spec_path = parent.join(format!(".{bin_name}.usage.kdl"));
        let spec = if dotted_spec_path.exists() {
            Spec::parse_file(&dotted_spec_path)?
        } else {
            Spec::parse_file(&self.bin)?
        };
        // Help never runs the interpreter, so a command that cannot be split still gets a page.
        let words = self.interpreter();
        let name = words
            .as_ref()
            .ok()
            .and_then(|w| w.first().cloned())
            .unwrap_or_else(|| self.command.clone());
        let mut args = self.args.clone();
        args.insert(0, name);

        if self.h {
            return self.help(&spec, &args, false);
        }
        if self.help {
            return self.help(&spec, &args, true);
        }

        let mut interpreter = words?.into_iter();
        let program = interpreter.next().unwrap();
        let interpreter_args = interpreter.collect_vec();

        let parsed = usage::parse::parse(&spec, &args)?;

        let mut cmd = std::process::Command::new(&program);
        cmd.args(&interpreter_args);
        cmd.stdin(Stdio::inherit());
        cmd.stdout(Stdio::inherit());
        cmd.stderr(Stdio::inherit());
        let bin_path = self
            .bin
            .to_str()
            .ok_or_else(|| usage::miette::miette!("Invalid file path: {}", self.bin.display()))?;
        let args = std::iter::once(bin_path.to_string())
            .chain(self.args.clone())
            .collect_vec();
        cmd.args(&args);

        env::apply_parsed_env(&mut cmd, &parsed.as_env());

        let result = cmd.spawn().into_diagnostic()?.wait().into_diagnostic()?;

        if !result.success() {
            std::process::exit(result.code().unwrap_or(1));
        }

        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::split_windows;

    #[test]
    fn windows_split_keeps_path_separators() {
        assert_eq!(
            split_windows(r"C:\Tools\node.exe --no-warnings"),
            ["C:\\Tools\\node.exe", "--no-warnings"]
        );
        assert_eq!(
            split_windows(r#""C:\Program Files\node.exe" --title='x'"#),
            ["C:\\Program Files\\node.exe", "--title='x'"]
        );
    }

    #[test]
    fn windows_split_handles_backslashes_before_quotes() {
        assert_eq!(
            split_windows(r#"node --title="a\"b""#),
            ["node", "--title=a\"b"]
        );
        assert_eq!(
            split_windows(r#"python -X pycache_prefix="C:\cache\\" -u"#),
            ["python", "-X", "pycache_prefix=C:\\cache\\", "-u"]
        );
    }
}
