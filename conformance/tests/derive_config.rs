//! A settings struct as its own declaration.
//!
//! The claim under test: `#[derive(usage::Config)]` on the struct a CLI already holds its
//! settings in generates the same registry `usage-config-build` would have generated from a
//! spec, a reader that fills the struct from a resolution, and a `config` block the spec
//! parser reads back — so the registry, the reader, and the documentation cannot drift from
//! the struct or from each other. That drift is the fleet's `settings.toml` + `build.rs`
//! pattern's whole failure mode: three descriptions of every setting, kept in step by hand.

use std::collections::BTreeMap;
use std::ffi::OsStr;
use std::path::PathBuf;

use usage_config::{resolve, Const, EnvLayer, Layers, Ty, Value};
use usage_derive::{Cli, Config};

fn default_cache_dir() -> PathBuf {
    PathBuf::from("/tmp/ex-cache")
}

/// The `task.*` settings, as their own group.
#[derive(Config, Debug, PartialEq)]
#[usage(prefix = "task")]
struct TaskSettings {
    /// How task output is interleaved
    #[usage(default = "prefix", choices("prefix", "interleave"))]
    output: String,

    /// Jobs for tasks alone
    #[usage(env = "EX_TASK_JOBS")]
    jobs: Option<u32>,
}

/// Every setting ex has — one of everything the derive carries.
#[derive(Config, Debug, PartialEq)]
struct Settings {
    /// How many jobs to run at once
    #[usage(
        env("EX_JOBS", "EX_JOB"),
        deprecated_env = "EX_JOBS_OLD",
        default = 4,
        cli("--jobs", "-j"),
        source("git", "ex.jobs")
    )]
    jobs: u64,

    /// Paths to leave alone
    #[usage(env = "EX_EXCLUDE", merge = "union", parse = "list_by_comma")]
    exclude: Option<Vec<String>>,

    /// Where the cache lives
    #[usage(
        env = "EX_CACHE_DIR",
        default_fn = default_cache_dir,
        default_note = "under the user cache directory"
    )]
    cache_dir: PathBuf,

    /// Which files to match
    // A keyword as a field, which is ordinary for a setting key and needs `r#` in Rust.
    #[usage(default = "all")]
    r#match: String,

    /// How long to wait, if at all
    // The registry declares a duration; the field holds its text, the same way a generated
    // struct does — the crate that owns the duration type owns its spelling.
    #[usage(env = "EX_TIMEOUT", ty = "duration")]
    timeout: Option<String>,

    /// Whether this checkout may run its own hooks
    #[usage(scope = "global", default = false)]
    trusted: bool,

    /// Ports to listen on
    #[usage(default(80, 443))]
    ports: Vec<u64>,

    /// Only from the environment
    #[usage(env = "CI", scope = "env", hide)]
    ci: Option<bool>,

    /// Rewrite these URL prefixes
    #[usage(merge = "deep")]
    url_replacements: Option<BTreeMap<String, String>>,

    #[usage(flatten)]
    task: TaskSettings,
}

#[test]
fn the_registry_is_the_struct_in_declaration_order() {
    let keys: Vec<&str> = Settings::SETTINGS_PROPS
        .iter()
        .map(|meta| meta.key)
        .collect();
    assert_eq!(
        keys,
        vec![
            "jobs",
            "exclude",
            "cache_dir",
            "match",
            "timeout",
            "trusted",
            "ports",
            "ci",
            "url_replacements",
            "task.output",
            "task.jobs",
        ],
        "a flattened group's props follow its field's position, under its prefix"
    );

    let jobs = &Settings::SETTINGS_PROPS[0];
    assert_eq!(jobs.ty, Ty::Uint);
    assert_eq!(jobs.default, Some(Const::Int(4)));
    assert_eq!(jobs.envs, &["EX_JOBS", "EX_JOB"]);
    assert_eq!(jobs.deprecated_envs, &["EX_JOBS_OLD"]);
    assert_eq!(jobs.cli, &["--jobs", "-j"]);
    assert_eq!(jobs.bindings, &[("git", "ex.jobs")]);
    assert_eq!(jobs.help, Some("How many jobs to run at once"));

    let cache_dir = &Settings::SETTINGS_PROPS[2];
    assert_eq!(cache_dir.ty, Ty::Path);
    assert_eq!(cache_dir.default, None, "a computed default is not a const");
    assert_eq!(cache_dir.optional, Some(true));
    assert_eq!(
        cache_dir.default_note,
        Some("under the user cache directory")
    );

    let timeout = &Settings::SETTINGS_PROPS[4];
    assert_eq!(
        timeout.ty,
        Ty::Option(&Ty::Duration),
        "an `Option` field is an optional setting, and `ty` renames what the spec calls it"
    );

    let ports = &Settings::SETTINGS_PROPS[6];
    assert_eq!(
        ports.default,
        Some(Const::List(&[Const::Int(80), Const::Int(443)]))
    );

    // The joined registry resolves dotted keys like any other.
    let found = Settings::SETTINGS_REGISTRY
        .lookup("task.output")
        .expect("declared");
    assert_eq!(
        Settings::SETTINGS_REGISTRY.get(found.id).help,
        Some("How task output is interleaved")
    );
}

#[test]
fn nothing_supplied_reads_as_the_declared_defaults() {
    let resolved = resolve(Settings::SETTINGS_REGISTRY, Layers::new()).expect("resolves");
    let settings = Settings::read(&resolved).expect("every field reads");
    assert_eq!(
        settings,
        Settings {
            jobs: 4,
            exclude: None,
            cache_dir: PathBuf::from("/tmp/ex-cache"),
            r#match: "all".to_string(),
            timeout: None,
            trusted: false,
            ports: vec![80, 443],
            ci: None,
            url_replacements: None,
            task: TaskSettings {
                output: "prefix".to_string(),
                jobs: None,
            },
        }
    );
}

#[test]
fn a_layer_fills_the_struct_through_the_declared_types() {
    let env = EnvLayer::new([
        ("EX_JOB".to_string(), "9".to_string()),
        ("EX_EXCLUDE".to_string(), "target, dist".to_string()),
        ("EX_CACHE_DIR".to_string(), "/var/cache/ex".to_string()),
        ("EX_TASK_JOBS".to_string(), "2".to_string()),
        ("CI".to_string(), "1".to_string()),
    ]);
    let resolved =
        resolve(Settings::SETTINGS_REGISTRY, Layers::new().then(&env)).expect("resolves");
    let settings = Settings::read(&resolved).expect("every value fits its field");
    assert_eq!(
        settings.jobs, 9,
        "the second declared variable still sets it"
    );
    assert_eq!(
        settings.exclude.as_deref(),
        Some(&["target".to_string(), "dist".to_string()][..]),
        "the named parser splits one variable into the list"
    );
    assert_eq!(
        settings.cache_dir,
        PathBuf::from("/var/cache/ex"),
        "a supplied value wins over `default_fn`"
    );
    assert_eq!(settings.ci, Some(true));
    assert_eq!(
        settings.task.jobs,
        Some(2),
        "a flattened field reads through its prefix"
    );
}

#[test]
fn the_emitted_config_block_is_the_spec_grammar() {
    // Parsed by the reference implementation, not merely inspected: the block the derive
    // renders has to be a declaration usage-lib reads, or docs, JSON schema and completions
    // see a different CLI than the one that runs.
    let kdl = format!("name \"ex\"\nbin \"ex\"\n{}", Settings::spec_kdl());
    let spec: usage::Spec = kdl.parse().expect("usage-lib reads the emitted block");

    let jobs = spec.config.props.get("jobs").expect("declared");
    assert_eq!(jobs.envs, vec!["EX_JOBS".to_string(), "EX_JOB".to_string()]);
    assert_eq!(jobs.deprecated_envs, vec!["EX_JOBS_OLD".to_string()]);
    assert_eq!(jobs.cli, vec!["--jobs".to_string(), "-j".to_string()]);
    assert!(
        matches!(
            jobs.default,
            Some(usage::spec::config::SpecConfigValue::Int(4))
        ),
        "the declared default survives the round-trip: {:?}",
        jobs.default
    );
    assert_eq!(jobs.help.as_deref(), Some("How many jobs to run at once"));

    let output = spec.config.props.get("task.output").expect("declared");
    assert_eq!(output.choices.len(), 2);

    let cache_dir = spec.config.props.get("cache_dir").expect("declared");
    assert_eq!(
        cache_dir.default_note.as_deref(),
        Some("under the user cache directory")
    );
    assert_eq!(cache_dir.optional, Some(true));
}

/// A tool with settings, declaring which type holds them.
#[derive(Cli, Debug)]
#[usage(bin = "ex", config = Settings)]
struct Ex {
    /// How many jobs to run at once
    #[usage(long, short = 'j', setting = "jobs")]
    jobs: Option<u64>,
}

#[test]
fn the_flags_this_cli_reads_are_the_flags_its_settings_declare() {
    // The one-line adopter test, now with both sides generated: the bindings from the `Cli`
    // derive, the registry from the `Config` derive.
    assert_eq!(
        Settings::SETTINGS_REGISTRY.drift(Ex::SETTINGS_BINDINGS),
        Vec::<String>::new()
    );
}

#[test]
fn the_clis_spec_carries_its_settings() {
    let kdl = Ex::to_kdl();
    let spec: usage::Spec = kdl.parse().expect("usage-lib reads the whole spec");
    assert_eq!(spec.bin, "ex");
    assert!(
        spec.config.props.contains_key("task.output"),
        "the config block rides in the emitted spec: {kdl}"
    );
}

#[test]
fn the_command_line_outranks_every_other_layer() {
    let argv = [OsStr::new("-j"), OsStr::new("12")];
    let (_, cli) = Ex::parse_from_with_settings(&argv).expect("parses");
    let env = EnvLayer::new([("EX_JOBS".to_string(), "9".to_string())]);
    let resolved = resolve(
        Settings::SETTINGS_REGISTRY,
        Layers::new().then(&cli).then(&env),
    )
    .expect("resolves");
    let settings = Settings::read(&resolved).expect("reads");
    assert_eq!(settings.jobs, 12);
    assert_eq!(
        resolved.get_key("jobs"),
        Some(&Value::Int(12)),
        "and the registry agrees with the struct"
    );
}
