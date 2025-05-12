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

    /// Outputs a `usage.kdl` spec for this CLI itself
    #[clap(long)]
    usage_spec: bool,

    /// Outputs completions for the specified shell for completing the `usage` CLI itself
    completions: Option<String>,
}

#[derive(Subcommand)]
enum Command {
    CompleteWord(complete_word::CompleteWord),
    Exec(exec::Exec),
    Generate(generate::Generate),
    #[clap(about = "Use bash to execute the script")]
    Bash(shell::Shell),
    #[clap(about = "use fish to execute the script")]
    Fish(shell::Shell),
    #[clap(about = "use zsh to execute the script")]
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
