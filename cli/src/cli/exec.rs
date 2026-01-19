use std::fmt::Debug;
use std::path::PathBuf;
use std::process::Stdio;

use clap::Args;
use itertools::Itertools;
use miette::IntoDiagnostic;

use usage::Spec;

#[derive(Debug, Args)]
#[clap(
    disable_help_flag = true,
    visible_alias = "x",
    about = "Execute a script, parsing args and exposing them as environment variables"
)]
pub struct Exec {
    /// command to execute after parsing usage spec
    command: String,
    /// path to script to execute
    bin: PathBuf,
    /// arguments to pass to script
    #[clap(allow_hyphen_values = true)]
    args: Vec<String>,

    /// Show help
    #[clap(short)]
    h: bool,

    /// Show help
    #[clap(long)]
    help: bool,
}

impl Exec {
    pub fn run(&mut self) -> miette::Result<()> {
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
