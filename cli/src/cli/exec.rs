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

/// Whether `command` is a file as written or a file of that name on `PATH`.
fn names_a_program(command: &str) -> bool {
    let path = std::path::Path::new(command);
    path.is_file()
        || std::env::var_os("PATH")
            .is_some_and(|dirs| std::env::split_paths(&dirs).any(|dir| dir.join(command).is_file()))
}

impl Exec {
    /// The program followed by any arguments that belong before the script.
    fn interpreter(&self) -> usage::miette::Result<Vec<String>> {
        let command = self.command.as_str();
        if command.contains(char::is_whitespace) && !names_a_program(command) {
            let words = shell_words::split(command).into_diagnostic()?;
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
        let mut interpreter = self.interpreter()?.into_iter();
        let program = interpreter.next().unwrap();
        let interpreter_args = interpreter.collect_vec();
        let mut args = self.args.clone();
        args.insert(0, program.clone());

        if self.h {
            return self.help(&spec, &args, false);
        }
        if self.help {
            return self.help(&spec, &args, true);
        }

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
