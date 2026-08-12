//! In-process timing, so process startup is not in the way.
use std::ffi::{OsStr, OsString};
use std::hint::black_box;
use std::time::Instant;

use clap::{CommandFactory, FromArgMatches, Parser};

const RUNS: u32 = 20_000;

fn bench(label: &str, mut f: impl FnMut()) {
    // Warm caches and branch predictors first, then keep the fastest of several rounds:
    // the minimum is the measurement least polluted by whatever else the machine is doing.
    for _ in 0..1_000 {
        f();
    }
    let mut best = f64::MAX;
    for _ in 0..5 {
        let start = Instant::now();
        for _ in 0..RUNS {
            f();
        }
        let per = start.elapsed().as_secs_f64() * 1e9 / RUNS as f64;
        best = best.min(per);
    }
    println!("{label:<44}{best:>9.0} ns  {:>8.2} µs", best / 1000.0);
}

fn main() {
    let words = ["use", "-g", "node@20"];
    let usage_argv: Vec<&OsStr> = words.iter().map(OsStr::new).collect();
    let clap_argv: Vec<OsString> = std::iter::once("mise")
        .chain(words)
        .map(OsString::from)
        .collect();

    bench("usage: argv -> struct", || {
        black_box(shadow_mise::Cli::parse_from(black_box(&usage_argv))).ok();
    });

    bench("clap: build tree + parse -> struct", || {
        black_box(shadow_mise_clap::Cli::try_parse_from(black_box(&clap_argv))).ok();
    });

    // The same parse with the tree already built, which is the half of clap that was
    // never the problem. `try_get_matches_from_mut` borrows rather than consuming, so
    // the tree is reused instead of cloned.
    let mut cmd = shadow_mise_clap::Cli::command();
    bench("clap: parse -> struct, tree reused", || {
        let m = cmd.try_get_matches_from_mut(black_box(&clap_argv)).unwrap();
        black_box(shadow_mise_clap::Cli::from_arg_matches(&m)).ok();
    });

    bench("clap: build tree only", || {
        black_box(shadow_mise_clap::Cli::command());
    });
}
