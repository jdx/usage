//! `parse-n`, in bpaf's vocabulary, so the four can be differenced the same way.

use shadow_mise_bpaf::cli_p;

fn main() {
    let n: usize = std::env::var("PARSE_N")
        .ok()
        .and_then(|v| v.parse().ok())
        .unwrap_or(1);
    let args: Vec<String> = std::env::args().skip(1).collect();
    let refs: Vec<&str> = args.iter().map(String::as_str).collect();
    let mut seen = 0usize;
    for _ in 0..n {
        // `cli_p()` assembles the combinator tree, which for bpaf is per-run work in the
        // same way clap builds its command tree — so it belongs inside the loop, which is
        // what a process does once.
        let parsed = cli_p().run_inner(bpaf::Args::from(std::hint::black_box(&refs[..])));
        if let Ok(cli) = parsed {
            seen += usize::from(cli.command.is_some());
        }
    }
    println!("{seen}");
}
