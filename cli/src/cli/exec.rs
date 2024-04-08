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
#[clap(visible_alias = "x")]
pub struct Exec {
    /// command to execute after parsing usage spec
    command: String,
    /// path to script to execute
    bin: PathBuf,
    /// arguments to pass to script
    #[clap(allow_hyphen_values = true)]
    args: Vec<String>,
}

impl Exec {
    pub fn run(&mut self) -> miette::Result<()> {
        let parent = self
            .bin
            .parent()
            .map(|p| p.to_path_buf())
            .unwrap_or_default();
        let bin_name = self.bin.file_name().unwrap().to_str().unwrap();
        let dotted_spec_path = parent.join(format!(".{}.usage.kdl", bin_name));
        let spec = if dotted_spec_path.exists() {
            let (spec, _) = Spec::parse_file(&dotted_spec_path)?;
            spec
        } else {
            let (spec, _script) = Spec::parse_file(&self.bin)?;
            // TODO: handle _script properly
            spec
        };
        let mut args = self.args.clone();
        args.insert(0, self.command.clone());
        let parsed = usage::cli::parse(&spec, &args)?;

        let mut cmd = std::process::Command::new(&self.command);
        cmd.stdin(Stdio::inherit());
        cmd.stdout(Stdio::inherit());
        cmd.stderr(Stdio::inherit());
        // TODO: set positional args

        let args = vec![self.bin.to_str().unwrap().to_string()];
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
