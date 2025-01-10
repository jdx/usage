use crate::usage_spec;
use clap::{Parser, Subcommand};
use miette::Result;

mod bash;
mod complete_word;
mod exec;
mod generate;

#[derive(Parser)]
#[clap(author, version, about)]
pub struct Cli {
    #[clap(subcommand)]
    command: Command,

    /// Outputs a `usage.kdl` spec for this CLI itself
    #[clap(long)]
    usage_spec: bool,

    /// Outputs completions for the specified shell for completing the `usage` CLI itself
    completions: Option<String>,
}

#[derive(Subcommand)]
enum Command {
    Bash(bash::Bash),
    CompleteWord(complete_word::CompleteWord),
    Exec(exec::Exec),
    Generate(generate::Generate),
}

impl Cli {
    pub fn run(argv: &[String]) -> Result<()> {
        let cli = Self::parse_from(argv);
        if cli.usage_spec {
            return usage_spec::generate();
        }
        match cli.command {
            Command::Bash(mut cmd) => cmd.run(),
            Command::Generate(cmd) => cmd.run(),
            Command::Exec(mut cmd) => cmd.run(),
            Command::CompleteWord(cmd) => cmd.run(),
        }
    }
}
