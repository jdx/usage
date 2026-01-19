use criterion::{black_box, criterion_group, criterion_main, Criterion};
use usage::{parse, Spec, SpecArg, SpecCommand, SpecFlag};

fn build_small_spec() -> Spec {
    let install_cmd = SpecCommand::builder()
        .name("install")
        .arg(SpecArg::builder().name("plugin").build())
        .arg(SpecArg::builder().name("version").build())
        .flag(SpecFlag::builder().short('g').long("global").build())
        .flag(SpecFlag::builder().short('f').long("force").build())
        .build();

    let mut cmd = SpecCommand::builder()
        .name("test")
        .flag(SpecFlag::builder().short('v').long("verbose").build())
        .build();
    cmd.subcommands.insert("install".to_string(), install_cmd);

    Spec {
        name: "test".to_string(),
        bin: "test".to_string(),
        cmd,
        ..Default::default()
    }
}

fn build_large_spec() -> Spec {
    // Build a spec with many flags and subcommands to stress test parsing
    let mut subcommands = indexmap::IndexMap::new();

    for cmd_name in [
        "install",
        "uninstall",
        "update",
        "list",
        "search",
        "info",
        "run",
        "exec",
    ] {
        let mut cmd = SpecCommand::builder()
            .name(cmd_name)
            .arg(SpecArg::builder().name("target").build())
            .flag(SpecFlag::builder().short('f').long("force").build())
            .flag(SpecFlag::builder().short('q').long("quiet").build())
            .build();

        // Add nested subcommands
        for sub_name in ["start", "stop", "restart"] {
            let sub = SpecCommand::builder()
                .name(sub_name)
                .arg(SpecArg::builder().name("name").build())
                .build();
            cmd.subcommands.insert(sub_name.to_string(), sub);
        }

        subcommands.insert(cmd_name.to_string(), cmd);
    }

    let mut cmd = SpecCommand::builder()
        .name("bench")
        .flag(
            SpecFlag::builder()
                .short('v')
                .long("verbose")
                .global(true)
                .build(),
        )
        .flag(
            SpecFlag::builder()
                .short('q')
                .long("quiet")
                .global(true)
                .build(),
        )
        .flag(
            SpecFlag::builder()
                .short('y')
                .long("yes")
                .global(true)
                .build(),
        )
        .flag(SpecFlag::builder().long("debug").global(true).build())
        .flag(SpecFlag::builder().long("trace").global(true).build())
        .build();
    cmd.subcommands = subcommands;

    Spec {
        name: "bench".to_string(),
        bin: "bench".to_string(),
        cmd,
        ..Default::default()
    }
}

fn bench_parse_small_spec(c: &mut Criterion) {
    let spec = build_small_spec();

    c.bench_function("parse_small_spec_args", |b| {
        b.iter(|| {
            parse(black_box(&spec), black_box(&["test".to_string()])).unwrap();
        })
    });

    c.bench_function("parse_small_spec_with_subcommand", |b| {
        b.iter(|| {
            parse(
                black_box(&spec),
                black_box(&[
                    "test".to_string(),
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
    let spec = build_large_spec();

    c.bench_function("parse_large_spec_args", |b| {
        b.iter(|| {
            parse(black_box(&spec), black_box(&["bench".to_string()])).unwrap();
        })
    });

    c.bench_function("parse_large_spec_subcommand", |b| {
        b.iter(|| {
            parse(
                black_box(&spec),
                black_box(&[
                    "bench".to_string(),
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
                    "bench".to_string(),
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

    c.bench_function("parse_large_spec_nested", |b| {
        b.iter(|| {
            parse(
                black_box(&spec),
                black_box(&[
                    "bench".to_string(),
                    "-v".to_string(),
                    "run".to_string(),
                    "start".to_string(),
                    "myapp".to_string(),
                ]),
            )
            .unwrap();
        })
    });
}

criterion_group!(benches, bench_parse_small_spec, bench_parse_large_spec);
criterion_main!(benches);
