use std::path::PathBuf;

use miette::IntoDiagnostic;

use usage::Spec;

mod completion;
mod markdown;

#[derive(clap::Args)]
#[clap(visible_alias = "g")]
pub struct Generate {
    #[clap(subcommand)]
    pub command: Command,
}

#[derive(clap::Subcommand)]
pub enum Command {
    Completion(completion::Completion),
    Markdown(markdown::Markdown),
}

impl Generate {
    pub fn run(&self) -> miette::Result<()> {
        match &self.command {
            Command::Completion(cmd) => cmd.run(),
            Command::Markdown(cmd) => cmd.run(),
        }
    }
}

pub fn file_or_spec(file: &Option<PathBuf>, spec: &Option<String>) -> miette::Result<Spec> {
    if let Some(file) = file {
        let (spec, _) = Spec::parse_file(file).into_diagnostic()?;
        Ok(spec)
    } else {
        spec.as_ref().unwrap().parse().into_diagnostic()
    }
}
