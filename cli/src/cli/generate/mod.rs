use std::path::PathBuf;
use usage::error::UsageErr;

use usage::Spec;

mod completion;
mod fig;
mod json;
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
    Fig(fig::Fig),
    Json(json::Json),
    Markdown(markdown::Markdown),
}

impl Generate {
    pub fn run(&self) -> miette7::Result<()> {
        match &self.command {
            Command::Completion(cmd) => cmd.run(),
            Command::Fig(cmd) => cmd.run(),
            Command::Json(cmd) => cmd.run(),
            Command::Markdown(cmd) => cmd.run(),
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
