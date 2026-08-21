use std::fmt::Debug;
use std::path::PathBuf;
use std::process::Stdio;

use itertools::Itertools;
use miette::IntoDiagnostic;
use usage_rs::Args;

use usage::Spec;

use crate::env;

/// Execute a script, parsing args and exposing them as environment variables
#[derive(Debug, Args)]
// The words after the script are the script's, so a flag `usage` does not know is a value to
// forward rather than a mistake to report — the root's `error` stops here.
#[usage(alias = "x", unknown_flags = "value")]
pub struct Exec {
    /// command to execute after parsing usage spec
    command: String,
    /// path to script to execute
    bin: PathBuf,
    /// arguments to pass to script
    args: Vec<String>,

    /// Show help
    #[usage(short)]
    h: bool,

    /// Show help
    #[usage(long)]
    help: bool,
}

impl Exec {
    pub fn help(&self, spec: &Spec, args: &[String], long: bool) -> miette::Result<()> {
        let parsed = usage::parse::parse_partial(spec, args)?;
        println!("{}", usage::docs::cli::render_help(spec, &parsed.cmd, long));
        Ok(())
    }
}

impl usage_rs::Run for Exec {
    type Output = miette::Result<()>;

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
            .ok_or_else(|| miette::miette!("Invalid file path: {}", self.bin.display()))?;
        let dotted_spec_path = parent.join(format!(".{bin_name}.usage.kdl"));
        let spec = if dotted_spec_path.exists() {
            Spec::parse_file(&dotted_spec_path)?
        } else {
            Spec::parse_file(&self.bin)?
        };
        let mut args = self.args.clone();
        args.insert(0, self.command.clone());

        if self.h {
            return self.help(&spec, &args, false);
        }
        if self.help {
            return self.help(&spec, &args, true);
        }

        let parsed = usage::parse::parse(&spec, &args)?;

        let mut cmd = std::process::Command::new(&self.command);
        cmd.stdin(Stdio::inherit());
        cmd.stdout(Stdio::inherit());
        cmd.stderr(Stdio::inherit());
        let bin_path = self
            .bin
            .to_str()
            .ok_or_else(|| miette::miette!("Invalid file path: {}", self.bin.display()))?;
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
