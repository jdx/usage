use std::ffi::OsStr;

use miette::Result;
use usage_derive::{Cli as DeriveCli, Subcommands};

pub mod complete_word;
mod exec;
pub(crate) mod generate;
mod lint;
mod mcp;
mod shell;
mod sponsors;

/// CLI for working with usage-based CLIs
// `usage` parses its own command line with the parser it ships: the tables below are the same
// ones an adopter's CLI compiles into, and `--usage-spec` prints the spec they emit rather
// than a transcription of a clap command. Said here rather than in the doc comment, which is
// the help page a user reads.
//
// 3.6 added `effect=` and 4.0 added it on flags and args; older `usage` CLIs reject the spec
// outright with "unsupported cmd prop effect", so this moves in lockstep with the fields the
// spec actually carries.
#[derive(DeriveCli)]
#[usage(
    bin = "usage",
    version,
    min_usage_version = "4.0",
    // Every flag the root accepts is one it declares, so an unrecognised one is a mistake and
    // saying so beats binding it to `[COMPLETIONS]`.
    //
    // Only the root, which is as far as the derive carries this today: the attribute is
    // accepted on an `Args` and then ignored, and a subcommand does not inherit the root's.
    // So `usage lint --nope f.kdl` still makes `--nope` the file and calls the real file
    // unexpected, where clap named the flag. The five commands that hand a command line to
    // somebody else's script want the lenient reading and get it by default, which is why
    // this is worth fixing in the derive rather than working around here.
    unknown_flags = "error"
)]
pub struct Cli {
    #[usage(subcommand)]
    command: Command,

    /// Outputs completions for the specified shell for completing the `usage` CLI itself
    // Declaration only, as it was under clap: `--completions <shell>` is answered in
    // `crate::run` before a parse happens, because a shell init script asks for it on every
    // new shell and it does not need a command line understood to answer. It is declared so
    // that help, the spec and the completions know it exists.
    #[allow(dead_code)]
    completions: Option<String>,

    /// Outputs a `usage.kdl` spec for this CLI itself
    #[usage(long)]
    usage_spec: bool,
}

/// What `usage` can be asked to do.
///
/// Each command's description is its struct's doc comment, in the file that owns it, except
/// where a variant holds nothing and there is no struct to carry one.
#[derive(Subcommands)]
enum Command {
    Bash(Box<shell::Bash>),
    #[usage(alias = "cw")]
    CompleteWord(Box<complete_word::CompleteWord>),
    #[usage(alias = "x")]
    Exec(Box<exec::Exec>),
    Fish(Box<shell::Fish>),
    #[usage(alias = "g")]
    Generate(Box<generate::Generate>),
    Lint(Box<lint::Lint>),
    #[usage(alias = "mcp-server")]
    Mcp(Box<mcp::Mcp>),
    #[usage(name = "powershell")]
    PowerShell(Box<shell::PowerShell>),
    /// Show the companies sponsoring usage and the jdx.dev open source tools
    #[usage(effect = "read")]
    Sponsors,
    Zsh(Box<shell::Zsh>),
}

impl Cli {
    pub fn run(argv: &[String]) -> Result<()> {
        // `parse_from` takes the command line without the program name, and hands back what
        // went wrong instead of ending the process — which is what lets the error come out
        // through the same path as every other failure here.
        let words: Vec<&OsStr> = argv.iter().skip(1).map(OsStr::new).collect();
        let cli = match Self::parse_from(&words) {
            Ok(cli) => cli,
            // Not failures: someone asked a question, and the answer goes to stdout.
            Err(usage_argv::Error::Help { cmd, long }) => {
                if let Some(page) = usage_argv::help::render(Self::spec(), cmd, long) {
                    print!("{page}");
                }
                return Ok(());
            }
            Err(usage_argv::Error::Version) => {
                println!("{} {}", env!("CARGO_PKG_NAME"), env!("CARGO_PKG_VERSION"));
                return Ok(());
            }
            Err(err) => {
                eprint!("{}", usage_argv::render_failure(Self::spec(), &words, &err));
                // clap's status for a command line it could not parse, which is what the
                // scripts that call this have been checking for.
                std::process::exit(2);
            }
        };
        if cli.usage_spec {
            return crate::usage_spec::generate();
        }
        match cli.command {
            Command::Bash(mut cmd) => cmd.run(),
            Command::Fish(mut cmd) => cmd.run(),
            Command::PowerShell(mut cmd) => cmd.run(),
            Command::Zsh(mut cmd) => cmd.run(),
            Command::Generate(cmd) => cmd.run(),
            Command::Exec(mut cmd) => cmd.run(),
            Command::CompleteWord(cmd) => cmd.run(),
            Command::Lint(cmd) => cmd.run(),
            Command::Mcp(cmd) => cmd.run(),
            Command::Sponsors => sponsors::run(),
        }
    }
}
