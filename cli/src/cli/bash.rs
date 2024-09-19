use std::fmt::Debug;
use std::path::PathBuf;
use std::process::Stdio;

use clap::Args;
use heck::ToSnakeCase;
use itertools::Itertools;
use miette::IntoDiagnostic;

use usage::cli::ParseValue;
use usage::Spec;

#[derive(Debug, Args)]
#[clap()]
pub struct Bash {
    script: PathBuf,
    /// arguments to pass to script
    #[clap(allow_hyphen_values = true)]
    args: Vec<String>,
}

impl Bash {
    pub fn run(&mut self) -> miette::Result<()> {
        let (spec, _script) = Spec::parse_file(&self.script)?;
        let mut args = self.args.clone();
        args.insert(0, spec.bin.clone());
        let parsed = usage::cli::parse(&spec, &args)?;

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

        for (flag, val) in &parsed.flags {
            let key = format!("usage_{}", flag.name.to_snake_case());
            let val = match val {
                ParseValue::Bool(b) => if *b { "1" } else { "0" }.to_string(),
                ParseValue::String(s) => s.clone(),
                ParseValue::MultiBool(b) => b.iter().map(|b| if *b { "1" } else { "0" }).join(","),
                ParseValue::MultiString(_s) => unimplemented!("multi string"),
            };
            cmd.env(key, val);
        }

        cmd.spawn().into_diagnostic()?.wait().into_diagnostic()?;

        Ok(())
    }
}
