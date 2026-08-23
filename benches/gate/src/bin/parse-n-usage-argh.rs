//! `parse-n` through usage, limited to exactly the vocabulary the argh shadow can express.

use shadow_mise_usage_argh::Cli;

fn main() {
    let n: usize = std::env::var("PARSE_N")
        .ok()
        .and_then(|v| v.parse().ok())
        .unwrap_or(1);
    let args: Vec<std::ffi::OsString> = std::env::args_os().skip(1).collect();
    let refs: Vec<&std::ffi::OsStr> = args.iter().map(|arg| arg.as_os_str()).collect();
    let mut seen = 0usize;
    for _ in 0..n {
        if let Ok(cli) = Cli::parse_from(std::hint::black_box(&refs)) {
            seen += usize::from(cli.command.is_some());
        }
    }
    println!("{seen}");
}
