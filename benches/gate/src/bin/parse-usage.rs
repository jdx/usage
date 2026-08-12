//! Parse a mise command line with the generated shadow, once, and exit.
//!
//! One invocation is the unit under test: a CLI parses its arguments once per run, so
//! what matters is the cost of a process reaching its first useful instruction — not a
//! throughput loop with everything warm.

use std::ffi::OsString;

use shadow_mise::Cli;

fn main() {
    let args: Vec<OsString> = std::env::args_os().skip(1).collect();
    let refs: Vec<&std::ffi::OsStr> = args.iter().map(|a| a.as_os_str()).collect();
    match Cli::parse_from(&refs) {
        // Printed, so the optimizer cannot decide the parse was unobservable.
        Ok(cli) => println!("{}", cli.command.is_some()),
        Err(e) => {
            eprintln!("{e:?}");
            std::process::exit(1)
        }
    }
}
