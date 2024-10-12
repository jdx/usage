use std::fmt::Debug;
use std::path::PathBuf;
use std::process::Stdio;

use clap::Args;
use itertools::Itertools;
use miette::IntoDiagnostic;

use usage::Spec;

#[derive(Debug, Args)]
#[clap(disable_help_flag = true)]
pub struct Bash {
    script: PathBuf,
    /// arguments to pass to script
    #[clap(allow_hyphen_values = true)]
    args: Vec<String>,

    #[clap(short, long)]
    help: bool,
}

impl Bash {
    pub fn run(&mut self) -> miette::Result<()> {
        let (spec, _script) = Spec::parse_file(&self.script)?;
        let mut args = self.args.clone();
        args.insert(0, spec.bin.clone());

        if self.help {
            return self.help(&spec, &args);
        }

        let parsed = usage::parse::parse(&spec, &args)?;

        let mut cmd = std::process::Command::new("bash");
        cmd.stdin(Stdio::inherit());
        cmd.stdout(Stdio::inherit());
        cmd.stderr(Stdio::inherit());
        // TODO: set positional args

        let args = vec![self.script.to_str().unwrap().to_string()]
            .into_iter()
            .chain(self.args.clone())
            .collect_vec();
        cmd.args(&args);

        for (key, val) in &parsed.as_env() {
            cmd.env(key, val);
        }

        cmd.spawn().into_diagnostic()?.wait().into_diagnostic()?;

        Ok(())
    }

    pub fn help(&self, spec: &Spec, _args: &[String]) -> miette::Result<()> {
        println!("{}", usage::docs::cli::render_help(spec));
        Ok(())
    }
}
