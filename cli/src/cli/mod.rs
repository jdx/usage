use clap::{Parser, Subcommand};
use miette::Result;

mod complete_word;
mod generate;

#[derive(Parser)]
#[clap(author, version, about)]
pub struct Cli {
    #[clap(subcommand)]
    command: Command,
}

#[derive(Subcommand)]
enum Command {
    CompleteWord(complete_word::CompleteWord),
    Generate(generate::Generate),
}

impl Cli {
    pub fn run(argv: &[String]) -> Result<()> {
        let cli = Self::parse_from(argv);
        match &cli.command {
            Command::Generate(cmd) => cmd.run(),
            Command::CompleteWord(cmd) => cmd.run(),
        }
    }
}
