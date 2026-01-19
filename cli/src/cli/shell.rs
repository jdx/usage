use std::fmt::Debug;
use std::path::PathBuf;
use std::process::Stdio;

use clap::Args;
use itertools::Itertools;
use miette::IntoDiagnostic;

use usage::Spec;

/// Execute a shell script with the specified shell
///
/// Typically, this will be called by a script's shebang.
///
/// If using `var=#true` on args/flags, they will be joined with spaces using `shell_words::join()`
/// to properly escape and quote values with spaces in them.
#[derive(Debug, Args)]
#[clap(disable_help_flag = true, verbatim_doc_comment)]
pub struct Shell {
    script: PathBuf,
    /// Arguments to pass to script
    #[clap(allow_hyphen_values = true)]
    args: Vec<String>,

    /// Show help
    #[clap(short)]
    h: bool,

    /// Show help
    #[clap(long)]
    help: bool,
}

impl Shell {
    pub fn run(&mut self, shell: &str) -> miette::Result<()> {
        let (spec, _script) = Spec::parse_file(&self.script)?;
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

        let mut cmd = std::process::Command::new(shell);
        cmd.stdin(Stdio::inherit());
        cmd.stdout(Stdio::inherit());
        cmd.stderr(Stdio::inherit());
        let script_path = self
            .script
            .to_str()
            .ok_or_else(|| miette::miette!("Invalid file path: {}", self.script.display()))?;
        let args = std::iter::once(script_path.to_string())
            .chain(self.args.clone())
            .collect_vec();
        cmd.args(&args);

        for (key, val) in &parsed.as_env() {
            cmd.env(key, val);
        }

        let result = cmd.spawn().into_diagnostic()?.wait().into_diagnostic()?;

        if !result.success() {
            std::process::exit(result.code().unwrap_or(1));
        }

        Ok(())
    }

    pub fn help(&self, spec: &Spec, args: &[String], long: bool) -> miette::Result<()> {
        let parsed = usage::parse::parse_partial(spec, args)?;
        println!("{}", usage::docs::cli::render_help(spec, &parsed.cmd, long));
        Ok(())
    }
}
