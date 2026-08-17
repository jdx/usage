//! `parse-n`, in argh's vocabulary, so the four can be differenced the same way.

use argh::FromArgs as _;
use shadow_mise_argh::Cli;

fn main() {
    let n: usize = std::env::var("PARSE_N")
        .ok()
        .and_then(|v| v.parse().ok())
        .unwrap_or(1);
    // argh parses `&[&str]` rather than `&[OsStr]`: it cannot take a non-UTF-8 argument at
    // all, so unlike the other three this binary has nothing to convert from — which is
    // work argh does not do rather than work it does faster.
    let args: Vec<String> = std::env::args().skip(1).collect();
    let refs: Vec<&str> = args.iter().map(String::as_str).collect();
    let mut seen = 0usize;
    for _ in 0..n {
        if let Ok(cli) = Cli::from_args(&["mise"], std::hint::black_box(&refs)) {
            seen += usize::from(cli.command.is_some());
        }
    }
    println!("{seen}");
}
