use std::io::{Read, Write};
use std::path::{Path, PathBuf};
use usage::error::UsageErr;

use usage::Spec;

mod completion;
mod completion_init;
mod fig;
mod go;
mod json;
mod json_schema;
mod manpage;
mod markdown;
mod sdk;

/// Generate completions, documentation, and other artifacts from usage specs
// Cannot run alone, and every child starts at `read`, so the parent is `read` too.
#[derive(usage_rs::Args)]
#[usage(alias = "g", effect = "read", run)]
pub struct Generate {
    #[usage(subcommand)]
    pub command: Command,
}

/// The generators.
///
/// Each command's help is its struct's doc comment rather than a second one here, which the
/// derive would let win: one description, in the file that owns the command.
#[derive(usage_rs::Subcommands)]
#[usage(run)]
pub enum Command {
    Completion(completion::Completion),
    CompletionInit(completion_init::CompletionInit),
    Fig(fig::Fig),
    Go(go::Go),
    Json(json::Json),
    JsonSchema(json_schema::JsonSchema),
    Manpage(manpage::Manpage),
    Markdown(markdown::Markdown),
    Sdk(sdk::Sdk),
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

/// Select a spec-declared executable view for a generator, when requested.
pub fn select_view(spec: Spec, view: Option<&str>) -> Result<Spec, UsageErr> {
    match view {
        Some(view) => spec.for_view(view),
        None => Ok(spec),
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
            write_file(path, contents)?;
        }
        _ => write_stdout(contents)?,
    }
    Ok(())
}

/// Write a generated file, creating the directories leading to it.
///
/// Every caller's path is a join onto an `--out-dir` the user named, so the parents are
/// created rather than demanded. The path travels with the error: "No such file or
/// directory" alone does not say which one.
pub fn write_file(path: &Path, contents: impl AsRef<[u8]>) -> Result<(), UsageErr> {
    if let Some(parent) = path.parent().filter(|p| !p.as_os_str().is_empty()) {
        std::fs::create_dir_all(parent)
            .map_err(|err| UsageErr::FileError(err, parent.to_path_buf()))?;
    }
    std::fs::write(path, contents).map_err(|err| UsageErr::FileError(err, path.to_path_buf()))
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
pub(crate) fn write_stdout(contents: &str) -> Result<(), UsageErr> {
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
