use std::io::{Read, Write};
use std::path::{Path, PathBuf};
use usage::error::UsageErr;

use usage::Spec;

mod completion;
mod completion_init;
mod fig;
mod json;
mod json_schema;
mod manpage;
mod markdown;
mod sdk;

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
    CompletionInit(completion_init::CompletionInit),
    Fig(fig::Fig),
    Json(json::Json),
    JsonSchema(json_schema::JsonSchema),
    Manpage(manpage::Manpage),
    Markdown(markdown::Markdown),
    Sdk(sdk::Sdk),
}

impl Generate {
    pub fn run(&self) -> miette::Result<()> {
        match &self.command {
            Command::Completion(cmd) => cmd.run(),
            Command::CompletionInit(cmd) => cmd.run(),
            Command::Fig(cmd) => cmd.run(),
            Command::Json(cmd) => cmd.run(),
            Command::JsonSchema(cmd) => cmd.run(),
            Command::Manpage(cmd) => cmd.run(),
            Command::Markdown(cmd) => cmd.run(),
            Command::Sdk(cmd) => cmd.run(),
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

/// The mirror of [`parse_file_or_stdin`]: `-` means stdout, and so does no path at all.
///
/// Both spellings are collapsed here so a caller never has to ask which of the two it is
/// looking at. To write to a file actually named `-`, spell it `./-`.
///
/// The progress line goes to stderr. It used to go to stdout, where it ended up inside the
/// document whenever the document itself was going to stdout.
pub fn write_or_stdout(out_file: Option<&Path>, contents: &str) -> Result<(), UsageErr> {
    match out_file {
        Some(path) if path.as_os_str() != "-" => {
            eprintln!("writing to {}", path.display());
            xx::file::write(path, contents)?;
        }
        _ => write_stdout(contents)?,
    }
    Ok(())
}

/// Write a generated document to stdout, reporting a failed write rather than panicking.
///
/// `print!` panics if the write fails, and these documents are big enough to outlast a pipe
/// buffer: `usage g markdown -f mise.usage.kdl --out-file - | head -1` produces 100 KB and
/// used to end in `failed printing to stdout … (os error 109)` and exit 101.
///
/// A reader that closed early is not a failure to report, though. Rust ignores `SIGPIPE`, so
/// what would end the process silently in C arrives here as an ordinary write error; treating
/// it as success is what makes `| head` behave the way it does everywhere else.
fn write_stdout(contents: &str) -> Result<(), UsageErr> {
    let mut stdout = std::io::stdout().lock();
    let wrote = stdout.write_all(contents.as_bytes()).and_then(|_| {
        // Explicitly, because the flush at process exit discards whatever it hits.
        stdout.flush()
    });
    match wrote {
        Err(err) if err.kind() == std::io::ErrorKind::BrokenPipe => Ok(()),
        other => Ok(other?),
    }
}

fn read_spec_from_stdin() -> Result<Spec, UsageErr> {
    let mut input = String::new();
    std::io::stdin().read_to_string(&mut input)?;
    input.parse()
}
