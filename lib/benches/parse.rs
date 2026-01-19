use criterion::{black_box, criterion_group, criterion_main, Criterion};
use std::path::Path;
use usage::{parse, Spec};

fn bench_parse_small_spec(c: &mut Criterion) {
    let path = Path::new(env!("CARGO_MANIFEST_DIR"))
        .parent()
        .unwrap()
        .join("examples/kitchen-sink.usage.kdl");
    let (spec, _) = Spec::parse_file(&path).unwrap();

    c.bench_function("parse_small_spec_args", |b| {
        b.iter(|| {
            parse(black_box(&spec), black_box(&["mycli".to_string()])).unwrap();
        })
    });

    c.bench_function("parse_small_spec_with_flags", |b| {
        b.iter(|| {
            parse(
                black_box(&spec),
                black_box(&[
                    "mycli".to_string(),
                    "--shell".to_string(),
                    "bash".to_string(),
                    "install".to_string(),
                    "-g".to_string(),
                    "-f".to_string(),
                    "plugin-1".to_string(),
                    "1.0.0".to_string(),
                ]),
            )
            .unwrap();
        })
    });
}

fn bench_parse_large_spec(c: &mut Criterion) {
    let path = Path::new(env!("CARGO_MANIFEST_DIR"))
        .parent()
        .unwrap()
        .join("examples/mise.usage.kdl");
    let (spec, _) = Spec::parse_file(&path).unwrap();

    c.bench_function("parse_large_spec_args", |b| {
        b.iter(|| {
            parse(black_box(&spec), black_box(&["mise".to_string()])).unwrap();
        })
    });

    c.bench_function("parse_large_spec_subcommand", |b| {
        b.iter(|| {
            parse(
                black_box(&spec),
                black_box(&[
                    "mise".to_string(),
                    "install".to_string(),
                    "node@20".to_string(),
                ]),
            )
            .unwrap();
        })
    });

    c.bench_function("parse_large_spec_with_flags", |b| {
        b.iter(|| {
            parse(
                black_box(&spec),
                black_box(&[
                    "mise".to_string(),
                    "--verbose".to_string(),
                    "--yes".to_string(),
                    "install".to_string(),
                    "-f".to_string(),
                    "node@20".to_string(),
                ]),
            )
            .unwrap();
        })
    });
}

fn bench_spec_loading(c: &mut Criterion) {
    let path = Path::new(env!("CARGO_MANIFEST_DIR"))
        .parent()
        .unwrap()
        .join("examples/mise.usage.kdl");

    c.bench_function("load_large_spec_file", |b| {
        b.iter(|| {
            Spec::parse_file(black_box(&path)).unwrap();
        })
    });
}

criterion_group!(
    benches,
    bench_parse_small_spec,
    bench_parse_large_spec,
    bench_spec_loading
);
criterion_main!(benches);
