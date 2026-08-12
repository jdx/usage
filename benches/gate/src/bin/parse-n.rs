//! Parses the same command line N times, N coming from the environment.
//!
//! Differencing two runs of *this* binary separates the first parse from the ones after
//! it: N=1 minus N=0 is what a cold parse costs in a fresh process, and N=2 minus N=1 is
//! what each one costs with the allocator and the caches warm. A CLI only ever does the
//! first, which is why the cold number is the one that matters.
//!
//! Differencing two runs of the same binary rather than two different binaries, which is
//! the mistake worth avoiding: subtracting a separate no-op binary looked equivalent and
//! was not — two binaries do measurably different amounts of setup before `main`, and
//! that difference lands in whatever you attribute to parsing. Holding the binary fixed
//! and varying only how many parses it does leaves nothing else to explain.
//!
//! Merely *linking* the tables costs nothing measurable, which was worth checking, since
//! 211 commands' worth of statics contain thousands of pointers between them: a binary
//! that reads one static and never parses matches one with no tables at all.

use std::ffi::OsString;

use shadow_mise::Cli;

fn main() {
    let n: usize = std::env::var("PARSE_N")
        .ok()
        .and_then(|v| v.parse().ok())
        .unwrap_or(1);
    let args: Vec<OsString> = std::env::args_os().skip(1).collect();
    let refs: Vec<&std::ffi::OsStr> = args.iter().map(|a| a.as_os_str()).collect();
    let mut seen = 0usize;
    for _ in 0..n {
        if let Ok(cli) = Cli::parse_from(std::hint::black_box(&refs)) {
            seen += usize::from(cli.command.is_some());
        }
    }
    println!("{seen}");
}
