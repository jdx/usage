//! Wall clock for the four parsers, measured so it survives a loaded machine.
//!
//! `time-parse` takes the best of five rounds of 20,000 iterations. On a busy box that is
//! the wrong shape twice over: a round long enough to be interrupted almost certainly is,
//! and averaging 20,000 iterations folds every interruption into the number. Noise from
//! other tenants is *additive* — nothing another process does can make this one faster — so
//! the minimum over many short rounds is the estimator to want, and short rounds are the
//! ones an interruption can only spoil individually.
//!
//! Each framework gets rounds sized to about the same wall time rather than the same
//! iteration count, so a parser 8,000x slower than another is not asked for 8,000x the work.
//!
//! What this buys: on a 32-core box under load, `time-parse` swung 50% for an unchanged
//! binary where the minimum here moves a few percent — 3% for clap and 14% for usage-rs
//! across fourteen invocations, and the CI runner read the same four within 2 to 4% of a
//! developer machine. That is enough for two significant figures and not a third, which is
//! how the landing page quotes them. Anything that has to be firmer reads the instruction
//! counts beside these: those are deterministic per binary and agreed across the same two
//! machines to within 0.4%.

use std::ffi::{OsStr, OsString};
use std::hint::black_box;
use std::time::Instant;

use argh::FromArgs as _;
use clap::Parser as _;

/// Per-round iteration counts, chosen so a round is ~0.5–3ms of work.
const ROUNDS: usize = 4_000;

struct Stats {
    min: f64,
    p01: f64,
    p10: f64,
    median: f64,
}

/// Time `iters` calls of `f`, `rounds` times, and describe the distribution per call.
fn sweep(rounds: usize, iters: usize, mut f: impl FnMut()) -> Stats {
    // Warm the allocator, the caches and the branch predictors. Whatever the first call
    // pays for is not what a parse costs on the millionth.
    for _ in 0..iters.max(200) {
        f();
    }
    let mut per_call: Vec<f64> = Vec::with_capacity(rounds);
    for _ in 0..rounds {
        let start = Instant::now();
        for _ in 0..iters {
            f();
        }
        per_call.push(start.elapsed().as_secs_f64() * 1e9 / iters as f64);
    }
    per_call.sort_by(f64::total_cmp);
    let at = |q: f64| per_call[((per_call.len() - 1) as f64 * q) as usize];
    Stats {
        min: per_call[0],
        p01: at(0.01),
        p10: at(0.10),
        median: at(0.50),
    }
}

fn report(label: &str, s: Stats) {
    println!(
        "{label:<40}{:>9.0} {:>9.0} {:>9.0} {:>9.0}  ns",
        s.min, s.p01, s.p10, s.median
    );
}

fn main() {
    let words = ["use", "-g", "node@20"];
    let usage_argv: Vec<&OsStr> = words.iter().map(OsStr::new).collect();
    let clap_argv: Vec<OsString> = std::iter::once("mise")
        .chain(words)
        .map(OsString::from)
        .collect();
    let str_argv: Vec<&str> = words.to_vec();

    println!(
        "{:<40}{:>9} {:>9} {:>9} {:>9}",
        "", "min", "p01", "p10", "median"
    );

    report(
        "usage-rs: argv -> struct",
        sweep(ROUNDS, 2_000, || {
            black_box(shadow_mise::Cli::parse_from(black_box(&usage_argv))).ok();
        }),
    );
    report(
        "argh: argv -> struct",
        sweep(ROUNDS, 2_000, || {
            black_box(shadow_mise_argh::Cli::from_args(
                &["mise"],
                black_box(&str_argv),
            ))
            .ok();
        }),
    );
    report(
        "clap: build tree + parse -> struct",
        sweep(ROUNDS / 8, 4, || {
            black_box(shadow_mise_clap::Cli::try_parse_from(black_box(&clap_argv))).ok();
        }),
    );
    report(
        "bpaf: build parser + parse -> struct",
        sweep(ROUNDS / 40, 2, || {
            let parsed =
                shadow_mise_bpaf::cli_p().run_inner(bpaf::Args::from(black_box(&str_argv[..])));
            black_box(parsed).ok();
        }),
    );
}
