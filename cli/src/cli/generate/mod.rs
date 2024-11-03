use std::path::PathBuf;
use usage::error::UsageErr;

use usage::Spec;

mod completion;
mod markdown;
mod fig;

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
    Fig(fig::Fig),
}

impl Generate {
    pub fn run(&self) -> miette::Result<()> {
        match &self.command {
            Command::Completion(cmd) => cmd.run(),
            Command::Markdown(cmd) => cmd.run(),
            Command::Fig(cmd) => cmd.run(),
        }
    }
}

pub fn file_or_spec(file: &Option<PathBuf>, spec: &Option<String>) -> Result<Spec, UsageErr> {
    if let Some(file) = file {
        let (spec, _) = Spec::parse_file(file)?;
        Ok(spec)
    } else {
        spec.as_ref().unwrap().parse()
    }
}
