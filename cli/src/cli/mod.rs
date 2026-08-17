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
    // Every flag `usage` accepts is one it declares, so an unrecognised one is a mistake and
    // saying so beats offering it to a positional — `usage lint --nope f.kdl` would otherwise
    // make `--nope` the file and call the real file unexpected.
    //
    // Declared once, on the root: every subcommand inherits it. The five that hand a command
    // line to somebody else's script say `value` for themselves, which is the whole reason
    // both halves are needed.
    unknown_flags = "error"
)]
pub struct Cli {
    #[usage(subcommand)]
    command: Command,

    /// Outputs completions for the specified shell for completing the `usage` CLI itself
    // Declaration only: `--completions <shell>` is answered in `crate::run` before a parse
    // happens, because a shell init script asks for it on every new shell and it does not
    // need a command line understood to answer. It is declared so that help, the spec and
    // the completions know it exists.
    //
    // A flag, which is how it is typed. The clap declaration made it a *positional* — so the
    // spec, the docs and the generated completions all described a `[COMPLETIONS]` argument
    // that nothing accepts, while the flag that does work went undocumented. Carried over
    // faithfully at first, wrong included; the point of emitting the spec from the
    // declaration is that the two cannot disagree, so the declaration is what changes.
    #[allow(dead_code)]
    #[usage(long)]
    completions: Option<String>,

    /// Outputs a `usage.kdl` spec for this CLI itself
    #[usage(long)]
    usage_spec: bool,
}

/// What `--version` and `-v` answer with.
///
/// The binary's name, not the crate's. They differ here — `usage-cli` ships `usage` — and
/// everything else this CLI says about itself now comes from the spec, where the name is
/// `usage`. Read from the spec rather than written out again, so a rename cannot leave the
/// version line saying something the help page above it contradicts.
pub(crate) fn version() -> String {
    let spec = Cli::spec();
    format!(
        "{} {}",
        spec.bin.unwrap_or(spec.name),
        spec.version.unwrap_or(env!("CARGO_PKG_VERSION"))
    )
}

/// What `usage` can be asked to do.
///
/// Each command's description is its struct's doc comment, in the file that owns it, except
/// where a variant holds nothing and there is no struct to carry one.
#[derive(Subcommands)]
enum Command {
    Bash(Box<shell::Bash>),
    CompleteWord(Box<complete_word::CompleteWord>),
    Exec(Box<exec::Exec>),
    Fish(Box<shell::Fish>),
    Generate(Box<generate::Generate>),
    Lint(Box<lint::Lint>),
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
                println!("{}", version());
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
