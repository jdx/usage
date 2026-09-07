//! In-process help timing for real mise pages. Run with `cargo run -p gate --release --bin time-help`.
use std::hint::black_box;
use std::time::Instant;

fn bench(label: &str, runs: u32, mut render: impl FnMut() -> String) {
    if std::env::args().any(|arg| arg == "--dump") {
        print!("{label}\n{}", render());
        return;
    }
    for _ in 0..10 {
        black_box(render());
    }
    let mut best = f64::MAX;
    for _ in 0..7 {
        let start = Instant::now();
        for _ in 0..runs {
            black_box(render());
        }
        best = best.min(start.elapsed().as_secs_f64() * 1e9 / f64::from(runs));
    }
    println!("{label:<32}{best:>10.0} ns");
}

fn main() {
    let spec = shadow_mise::Cli::spec();
    for path in [
        &[][..],
        &["version"][..],
        &["use"][..],
        &["tasks", "run"][..],
    ] {
        let mut meta = spec.root;
        for name in path {
            meta = meta
                .subcommands
                .iter()
                .find(|meta| meta.cmd.name == *name)
                .unwrap();
        }
        let label = format!("mise {} --help", path.join(" "));
        bench(&label, 1_000, || {
            usage_argv::help::render(black_box(spec), black_box(meta.cmd), true).unwrap()
        });
    }
    // The process renderer also recognizes structural spans for colored output.
    for (name, style) in [
        ("plain", usage_argv::help::Style::PLAIN),
        ("colored", usage_argv::help::Style::COLOURED),
    ] {
        bench(&format!("mise root process help ({name})"), 1_000, || {
            usage_argv::help::render_styled(
                black_box(spec),
                black_box(spec.root.cmd),
                true,
                black_box(style),
            )
            .unwrap()
        });
    }
    bench("mise KDL", 20, || black_box(spec).to_kdl());
    bench("mise recursive help", 5, || {
        usage_argv::help::render_all(black_box(spec), black_box(spec.root.cmd)).unwrap()
    });
}
