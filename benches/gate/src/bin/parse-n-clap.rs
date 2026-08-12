//! `parse-n`, in clap's vocabulary, so the two can be differenced the same way.

use std::ffi::OsString;

use clap::Parser;
use shadow_mise_clap::Cli;

fn main() {
    let n: usize = std::env::var("PARSE_N")
        .ok()
        .and_then(|v| v.parse().ok())
        .unwrap_or(1);
    let args: Vec<OsString> = std::env::args_os().collect();
    let mut seen = 0usize;
    for _ in 0..n {
        // `try_parse_from` builds the command tree on every call, which is what a process
        // does once — the cost this project exists to avoid.
        if let Ok(cli) = Cli::try_parse_from(std::hint::black_box(&args)) {
            seen += usize::from(cli.command.is_some());
        }
    }
    println!("{seen}");
}
