use std::io::Read;
use std::path::{Path, PathBuf};
use usage::error::UsageErr;

use usage::Spec;

mod completion;
mod fig;
mod json;
mod manpage;
mod markdown;

/// Generate completions, documentation, and other artifacts from usage specs
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
    Manpage(manpage::Manpage),
    Markdown(markdown::Markdown),
}

impl Generate {
    pub fn run(&self) -> miette::Result<()> {
        match &self.command {
            Command::Completion(cmd) => cmd.run(),
            Command::Fig(cmd) => cmd.run(),
            Command::Json(cmd) => cmd.run(),
            Command::Manpage(cmd) => cmd.run(),
            Command::Markdown(cmd) => cmd.run(),
        }
    }
}

pub fn file_or_spec(file: &Option<PathBuf>, spec: &Option<String>) -> Result<Spec, UsageErr> {
    if let Some(file) = file {
        if file.as_os_str() == "-" {
            read_spec_from_stdin()
        } else {
            Spec::parse_file(file)
        }
    } else {
        spec.as_ref().unwrap().parse()
    }
}

pub fn parse_file_or_stdin(file: &Path) -> Result<Spec, UsageErr> {
    if file.as_os_str() == "-" {
        read_spec_from_stdin()
    } else {
        Spec::parse_file(file)
    }
}

fn read_spec_from_stdin() -> Result<Spec, UsageErr> {
    let mut input = String::new();
    std::io::stdin().read_to_string(&mut input)?;
    input.parse()
}
