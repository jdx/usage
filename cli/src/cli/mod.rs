use std::collections::HashMap;
use std::ffi::OsStr;

use miette::Result;
use usage_rs::{Cli as DeriveCli, Subcommands};

pub mod complete_word;
mod exec;
mod explain;
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
//
// Owed a bump: the four shell commands flatten `Shell`, which now emits a `flagset`, and no
// 4.0 can read that node. The floor is whichever release carries flagsets, so the number waits
// for that release rather than being guessed at here — and this crate warning about its own
// spec until then is worse than the stale claim.
#[derive(DeriveCli)]
#[usage(
    bin = "usage",
    version,
    min_usage_version = "4.0",
    repository = "https://github.com/jdx/usage",
    // The command path is not the file path: command names are hyphenated where the files
    // that implement them are snake_case, a command with subcommands lives in its directory's
    // `mod.rs`, and the four shell commands are all served by a single `shell.rs`.
    //
    // Unindented, because a raw string keeps every leading space it is given and only the
    // `{%-`/`-%}` markers take any back — so indenting to match the attribute would be
    // trusting each line to be surrounded by them.
    source_code_link_template = r#"{%- set path = path | replace(from='-', to='_') -%}
{%- if cmd.subcommands | length > 0 -%}
{%- set path = path ~ "/mod.rs" -%}
{%- elif path in ["bash", "fish", "powershell", "zsh"] -%}
{%- set path = "shell.rs" -%}
{%- else -%}
{%- set path = path ~ ".rs" -%}
{%- endif -%}
https://github.com/jdx/usage/blob/main/cli/src/cli/{{path}}"#,
    usage = "Usage: usage <COMMAND>\n       usage --completions <COMPLETIONS>\n       usage --usage-spec",
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
    // `--completions <shell>` is normally answered in `crate::run` before a parse happens,
    // because a shell init script asks for it on every new shell and it does not need a
    // subcommand to answer. It remains a real parsed field so direct callers of `Cli::run`
    // and the help, spec, and completions all read the same declaration.
    //
    // A flag, which is how it is typed. The clap declaration made it a *positional* — so the
    // spec, the docs and the generated completions all described a `[COMPLETIONS]` argument
    // that nothing accepts, while the flag that does work went undocumented. Carried over
    // faithfully at first, wrong included; the point of emitting the spec from the
    // declaration is that the two cannot disagree, so the declaration is what changes.
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
/// Each command's description is its struct's doc comment, in the file that owns it — and so
/// is the code that carries it out: `run` generates the match that hands the selected command
/// to its `usage_rs::Run` implementation, so this list is the only place a command is named.
#[derive(Subcommands)]
#[usage(run)]
enum Command {
    Bash(shell::Bash),
    CompleteWord(complete_word::CompleteWord),
    Exec(exec::Exec),
    Explain(explain::Explain),
    Fish(shell::Fish),
    Generate(generate::Generate),
    Lint(lint::Lint),
    Mcp(mcp::Mcp),
    #[usage(name = "powershell")]
    PowerShell(shell::PowerShell),
    Sponsors(sponsors::Sponsors),
    Zsh(shell::Zsh),
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
            Err(usage_rs::Error::Help { cmd, long }) => {
                if let Some(page) = usage_rs::help::render(Self::spec(), cmd, long) {
                    print!("{page}");
                }
                return Ok(());
            }
            Err(usage_rs::Error::HelpAll { cmd }) => {
                if let Some(page) = usage_rs::help::render_all(Self::spec(), cmd) {
                    print!("{page}");
                }
                return Ok(());
            }
            Err(usage_rs::Error::Version { .. }) => {
                println!("{}", version());
                return Ok(());
            }
            Err(err) => {
                eprint!("{}", usage_rs::render_failure(Self::spec(), &words, &err));
                // clap's status for a command line it could not parse, which is what the
                // scripts that call this have been checking for.
                std::process::exit(2);
            }
        };
        if let Some(shell) = cli.completions.as_deref() {
            return crate::usage_spec::complete(shell);
        }
        if cli.usage_spec {
            return crate::usage_spec::generate();
        }
        // The match that used to be here, one arm per command, is generated from the enum
        // above — so a command added there cannot be left unrouted, and no arm can route to
        // the wrong handler.
        usage_rs::Run::run(cli.command)
    }
}

/// A spec that declares nothing, as the answer to every mount in the tree.
///
/// Keyed by the exact `run` string, which is how injected answers are looked up. It is a
/// whole spec rather than an empty string because that is what a mount's stdout is.
///
/// Shared by `lint` and `explain`, which want it for the same reason: usage-lib resolves a
/// command's mounts on the way *into* it, so a spec that mounts anything cannot be parsed
/// without either spawning the mounted program or being handed its answer — and a command
/// that reads a file and prints a report should not spawn whatever that file names.
pub(crate) fn empty_mount_answers(cmd: &usage::SpecCommand) -> HashMap<String, String> {
    let mut answers = HashMap::new();
    collect_mount_answers(cmd, &mut answers);
    answers
}

fn collect_mount_answers(cmd: &usage::SpecCommand, answers: &mut HashMap<String, String>) {
    for mount in &cmd.mounts {
        answers.insert(
            mount.run.clone(),
            "name \"mounted\"\nbin \"mounted\"\n".to_string(),
        );
    }
    for sub in cmd.subcommands.values() {
        collect_mount_answers(sub, answers);
    }
}

/// How a command that can print either prose or JSON was asked to print.
///
/// Shared rather than declared twice: `lint` and `explain` both offer it, and a third
/// copy of the same four-line `FromStr` is how the two spellings drift apart.
#[derive(Debug, Clone, Copy, Default, usage_rs::ValueEnum)]
pub(crate) enum OutputFormat {
    #[default]
    Text,
    Json,
}

impl std::str::FromStr for OutputFormat {
    type Err = String;

    fn from_str(value: &str) -> Result<Self, Self::Err> {
        // Delegated rather than matched again: the derive already lists the words, and a
        // second list beside it is one more thing that can fall out of step with the type.
        use usage_rs::spec::ValueEnum;
        Self::from_choice(value).ok_or_else(|| {
            format!(
                "`{value}` is not one of: {}",
                Self::ACCEPTED_CHOICES.join(", ")
            )
        })
    }
}
