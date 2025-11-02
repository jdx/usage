use crate::usage_spec;
use clap::{Parser, Subcommand};
use miette::Result;

mod complete_word;
mod exec;
mod generate;
mod shell;

#[derive(Parser)]
#[clap(author, version, about)]
pub struct Cli {
    #[clap(subcommand)]
    command: Command,

    /// Outputs completions for the specified shell for completing the `usage` CLI itself
    completions: Option<String>,

    /// Outputs a `usage.kdl` spec for this CLI itself
    #[clap(long)]
    usage_spec: bool,
}

#[derive(Subcommand)]
enum Command {
    #[clap(about = "Execute a shell script using bash")]
    Bash(shell::Shell),
    CompleteWord(complete_word::CompleteWord),
    Exec(exec::Exec),
    #[clap(about = "Execute a shell script using fish")]
    Fish(shell::Shell),
    Generate(generate::Generate),
    #[clap(about = "Execute a shell script using zsh")]
    Zsh(shell::Shell),
}

impl Cli {
    pub fn run(argv: &[String]) -> Result<()> {
        let cli = Self::parse_from(argv);
        if cli.usage_spec {
            return usage_spec::generate();
        }
        match cli.command {
            Command::Bash(mut cmd) => cmd.run("bash"),
            Command::Fish(mut cmd) => cmd.run("fish"),
            Command::Zsh(mut cmd) => cmd.run("zsh"),
            Command::Generate(cmd) => cmd.run(),
            Command::Exec(mut cmd) => cmd.run(),
            Command::CompleteWord(cmd) => cmd.run(),
        }
    }
}
