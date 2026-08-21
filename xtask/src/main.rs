//! Maintainer tasks. One so far: turning a spec into a shadow CLI.
//!
//! `gen-shadow <spec.kdl> <out-dir>` reads a usage spec and writes a crate whose
//! `#[derive(usage::Cli)]` types declare the same commands, flags and arguments. The
//! point is the benchmark: to compare this parser against clap at a real CLI's scale
//! you need the same CLI expressed both ways, and mise's 210 commands are not
//! something to transcribe by hand.
//!
//! It is deliberately a *shadow* — it parses and does nothing else. What it proves is
//! that the derive can express a real spec, and how fast the result is.

use std::collections::{BTreeMap, HashSet};
use std::fmt::Write as _;
use std::path::Path;

use usage::{Spec, SpecArg, SpecChoices, SpecCommand, SpecFlag, SpecGroup};

mod cobra;
mod help_pages;
mod shadow;

fn main() {
    let args: Vec<String> = std::env::args().skip(1).collect();
    let (cmd, rest) = args
        .split_first()
        .map(|(c, r)| (c.as_str(), r))
        .unwrap_or(("help", &[] as &[String]));
    match cmd {
        "gen-shadow" => match rest {
            [spec, out] => {
                shadow::generate(Path::new(spec), Path::new(out), shadow::Dialect::Usage)
            }
            [spec, out, dialect] => match dialect.as_str() {
                "usage" => {
                    shadow::generate(Path::new(spec), Path::new(out), shadow::Dialect::Usage)
                }
                "clap" => shadow::generate(Path::new(spec), Path::new(out), shadow::Dialect::Clap),
                "argh" => shadow::generate(Path::new(spec), Path::new(out), shadow::Dialect::Argh),
                "bpaf" => shadow::generate(Path::new(spec), Path::new(out), shadow::Dialect::Bpaf),
                // Go rather than Rust, so it has an emitter of its own.
                "cobra" => cobra::generate(Path::new(spec), Path::new(out)),
                other => fail(&format!(
                    "unknown dialect `{other}`; the dialects are \
                     `usage`, `clap`, `argh`, `bpaf` and `cobra`"
                )),
            },
            _ => {
                fail("gen-shadow needs a spec file, an output directory, and optionally a dialect")
            }
        },
        // The pages usage-lib renders, as JSON, so an implementation in another
        // language can be measured against them rather than against itself.
        "help-pages" => match rest {
            [spec] => help_pages::dump(Path::new(spec)),
            _ => fail("help-pages needs a spec file"),
        },
        other => fail(&format!(
            "unknown task `{other}`; the tasks are: \
             gen-shadow <spec.kdl> <out-dir> [usage|clap|argh|bpaf|cobra], \
             help-pages <spec.kdl>"
        )),
    }
}

pub(crate) fn fail(message: &str) -> ! {
    eprintln!("xtask: {message}");
    std::process::exit(1)
}
