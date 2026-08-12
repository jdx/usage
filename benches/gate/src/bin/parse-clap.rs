//! Parse a mise command line with the clap shadow, once, and exit.
//!
//! The baseline. Deliberately a whole process rather than a loop with the command tree
//! already built: clap constructs its tree at runtime, on the way to parsing, and a
//! benchmark that hoisted that out of the loop would measure the half of clap that was
//! never the problem.

use std::ffi::OsString;

use clap::Parser;
use shadow_mise_clap::Cli;

fn main() {
    let args: Vec<OsString> = std::env::args_os().collect();
    match Cli::try_parse_from(&args) {
        // Printed, so the optimizer cannot decide the parse was unobservable.
        Ok(cli) => println!("{}", cli.command.is_some()),
        Err(e) => {
            eprintln!("{e}");
            std::process::exit(1)
        }
    }
}
