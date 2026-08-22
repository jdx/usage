//! A settings struct as its own declaration.
//!
//! The claim under test: `#[derive(usage::Config)]` on the struct a CLI already holds its
//! settings in generates the registry the resolver reads, a reader that fills the struct from
//! a resolution, and a `config` block the spec parser reads back — so the registry, the reader, and the documentation cannot drift from
//! the struct or from each other. That drift is the fleet's `settings.toml` + `build.rs`
//! pattern's whole failure mode: three descriptions of every setting, kept in step by hand.

use std::collections::BTreeMap;
use std::ffi::OsStr;
use std::path::PathBuf;

use usage_config::{
    resolve, resolve_with_context, Const, EnvLayer, Layers, ResolutionContext, Ty, Value,
};
use usage_derive::{Cli, Config};

fn default_cache_dir() -> PathBuf {
    PathBuf::from("/tmp/ex-cache")
}

/// The `task.*` settings, as their own group.
#[derive(Config, Debug, PartialEq)]
#[usage(prefix = "task")]
#[usage(file(path = "task.toml", findup))]
struct TaskSettings {
    /// How task output is interleaved
    #[usage(default = "prefix", choices("prefix", "interleave"), alias("task.out"))]
    output: String,

    /// Jobs for tasks alone
    #[usage(env = "EX_TASK_JOBS")]
    jobs: Option<u32>,
}

/// Every setting ex has — one of everything the derive carries.
#[derive(Config, Debug, PartialEq)]
#[usage(source(kind = "npmrc", name = ".npmrc"))]
#[usage(source(
    kind = "git",
    name = "git config",
    doc_hint = "git config `{key}`",
    set_hint = "git config {key} {value}"
))]
#[usage(file(path = "/etc/ex.toml", scope = "system", format = "toml"))]
#[usage(file(path = "ex.toml", findup))]
struct Settings {
    /// How many jobs to run at once
    #[usage(
        env("EX_JOBS", "EX_JOB"),
        deprecated_env = "EX_JOBS_OLD",
        default = 4,
        cli("--jobs", "-j"),
        source("git", "ex.jobs"),
        help_heading = "Performance",
        writes_to = "git",
        x("ex.rust_type", "u64"),
        x("ex.restart_required", true)
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
    assert_eq!(
        Settings::SETTINGS_SPEC.props[0].help_heading,
        Some("Performance")
    );
    assert_eq!(Settings::SETTINGS_SPEC.props[0].writes_to, Some("git"));
    assert_eq!(
        Settings::SETTINGS_SPEC.props[0].extensions,
        &[
            ("ex.rust_type", Const::Str("u64")),
            ("ex.restart_required", Const::Bool(true))
        ]
    );

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
    assert_eq!(jobs.help_heading.as_deref(), Some("Performance"));
    assert_eq!(jobs.writes_to.as_deref(), Some("git"));
    assert_eq!(
        jobs.extensions,
        vec![
            (
                "ex.rust_type".to_string(),
                usage::spec::config::SpecConfigValue::String("u64".to_string())
            ),
            (
                "ex.restart_required".to_string(),
                usage::spec::config::SpecConfigValue::Bool(true)
            ),
        ]
    );

    assert_eq!(
        spec.config.sources.keys().collect::<Vec<_>>(),
        vec![&"git".to_string(), &"npmrc".to_string()],
        "source declarations have deterministic key order"
    );
    let git = spec.config.sources.get("git").expect("declared");
    assert_eq!(git.name.as_deref(), Some("git config"));
    assert_eq!(git.doc_hint.as_deref(), Some("git config `{key}`"));
    assert_eq!(git.set_hint.as_deref(), Some("git config {key} {value}"));
    assert_eq!(
        spec.config
            .files
            .iter()
            .map(|file| file.path.as_str())
            .collect::<Vec<_>>(),
        vec!["/etc/ex.toml", "ex.toml"],
        "file declaration order is precedence"
    );
    assert!(spec.config.files[1].findup);
    assert!(
        !spec
            .config
            .files
            .iter()
            .any(|file| file.path == "task.toml"),
        "a flattened group's top-level files belong to its standalone spec, not its parent"
    );

    let output = spec.config.props.get("task.output").expect("declared");
    assert_eq!(output.choices.len(), 2);

    let cache_dir = spec.config.props.get("cache_dir").expect("declared");
    assert_eq!(
        cache_dir.default_note.as_deref(),
        Some("under the user cache directory")
    );
    assert_eq!(cache_dir.optional, Some(true));
}

#[test]
fn a_nested_config_keeps_its_own_top_level_declarations_when_emitted_alone() {
    let kdl = format!("name \"task\"\nbin \"task\"\n{}", TaskSettings::spec_kdl());
    let spec: usage::Spec = kdl.parse().expect("nested declaration parses alone");
    assert_eq!(spec.config.files[0].path, "task.toml");
}

#[test]
fn a_groups_prefix_reaches_its_keys_and_not_its_aliases() {
    // `prefix` is applied to `key` alone: an alias is written out in full, prefix and all.
    //
    // Deliberate, because an alias is usually a *legacy* name and a setting that moved into a
    // group often wants the spelling it had before the move — `task.jobs` keeping plain
    // `jobs`. Prefixing aliases would make that unsayable, while writing them in full costs
    // one repeated word and can say either. The collision checks in `Config::from_input` and
    // `concat_props` are what keep a full-form alias from quietly taking a name off another
    // group, which is the risk this reading carries.
    let output = Settings::SETTINGS_PROPS
        .iter()
        .find(|meta| meta.key == "task.output")
        .expect("declared under its group's prefix");
    assert_eq!(output.aliases, &["task.out"]);
}

/// Every metadata attribute that only reaches the registry, declared at once.
#[derive(Config, Debug, PartialEq)]
struct Documented {
    /// Stop at the first failure
    ///
    /// Whether a failing job stops the rest of them.
    #[usage(
        alias("fail-fast.legacy", "failfast"),
        example("true", "false"),
        since = "5.2.0",
        deprecated = "use `stop-on-error`",
        deprecated_warn_at = "6.0.0",
        deprecated_remove_at = "7.0.0"
    )]
    fail_fast: Option<bool>,
}

#[test]
fn the_metadata_only_docs_read_survives_the_round_trip() {
    // These fields never reach `read` — they exist for docs, the JSON schema and the
    // completers, all of which go through the *spec*. So rendering them is only half a claim:
    // the other half is that usage-lib reads back what the derive wrote. A field the renderer
    // emits and the parser ignores would be metadata that silently does not exist.
    let props = Documented::SETTINGS_PROPS;
    assert_eq!(props.len(), 1);
    let declared = &props[0];

    let kdl = format!("name \"ex\"\nbin \"ex\"\n{}", Documented::spec_kdl());
    let spec: usage::Spec = kdl.parse().expect("usage-lib reads the emitted block");
    let parsed = spec.config.props.get("fail_fast").expect("declared");

    assert_eq!(parsed.aliases, declared.aliases.to_vec());
    assert_eq!(parsed.examples, declared.examples.to_vec());
    assert_eq!(parsed.since.as_deref(), declared.since);
    assert_eq!(parsed.deprecated.as_deref(), declared.deprecated);
    assert_eq!(
        parsed.deprecated_warn_at.as_deref(),
        declared.deprecated_warn_at
    );
    assert_eq!(
        parsed.deprecated_remove_at.as_deref(),
        declared.deprecated_remove_at
    );
    assert_eq!(parsed.optional, declared.optional);

    // A doc comment becomes both: the first paragraph is the short help, and the *whole*
    // comment is the long one — the same `doc_comment` helper the `Cli` derive uses, so a
    // setting's prose and a flag's are split the same way.
    assert_eq!(parsed.help.as_deref(), Some("Stop at the first failure"));
    assert_eq!(
        parsed.long_help.as_deref(),
        Some("Stop at the first failure\n\nWhether a failing job stops the rest of them.")
    );
    assert_eq!(parsed.long_help.as_deref(), declared.long_help);
}

#[derive(Config, Debug, PartialEq)]
struct GatedSetting {
    #[usage(
        env = "EX_LEGACY",
        deprecated = "use modern",
        deprecated_warn_at = "6.0.0",
        deprecated_remove_at = "7.0.0"
    )]
    legacy: Option<bool>,
}

#[test]
fn derived_config_lifecycle_metadata_controls_resolution() {
    let env = EnvLayer::new([("EX_LEGACY".to_string(), "true".to_string())]);

    let before = resolve_with_context(
        GatedSetting::SETTINGS_REGISTRY,
        Layers::new().then(&env),
        ResolutionContext::for_cli_version("5.9.9"),
    )
    .expect("resolves before warning");
    assert_eq!(GatedSetting::read(&before).unwrap().legacy, Some(true));
    assert!(before.warnings.is_empty());

    let warning = resolve_with_context(
        GatedSetting::SETTINGS_REGISTRY,
        Layers::new().then(&env),
        ResolutionContext::for_cli_version("6.0.0"),
    )
    .expect("resolves at warning");
    assert_eq!(GatedSetting::read(&warning).unwrap().legacy, Some(true));
    assert_eq!(
        warning.warnings[0].kind,
        usage_config::WarningKind::Deprecated
    );

    let removed = resolve_with_context(
        GatedSetting::SETTINGS_REGISTRY,
        Layers::new().then(&env),
        ResolutionContext::for_cli_version("7.0.0"),
    )
    .expect("resolves at removal");
    assert_eq!(GatedSetting::read(&removed).unwrap().legacy, None);
    assert_eq!(removed.warnings[0].kind, usage_config::WarningKind::Removed);
}

/// A CLI whose settings-bound flag is deprecated.
#[derive(Cli, Debug)]
#[usage(bin = "dep", version = "2.0.0", config = Settings)]
struct Deprecated {
    /// How many jobs to run at once
    #[usage(long, setting = "jobs", deprecated = "use --parallel")]
    jobs: Option<u64>,
}

#[test]
fn the_settings_entry_reports_the_deprecations_a_parse_used() {
    // `parse` collects deprecations and prints them; `parse_with_settings` is the same kind of
    // thing — an entry point that *is* the process — so it has to say the same. It read the
    // partial and threw the warnings away, so a CLI that adopted settings went quiet about
    // every deprecation it had, which is the sort of difference nobody notices until a
    // release removes the flag.
    let argv = ["dep", "--jobs", "4"].map(OsStr::new);
    let mut warnings = Vec::new();
    let (parsed, layer) =
        Deprecated::parse_from_argv_with_settings_and_warnings(&argv, &mut warnings)
            .expect("parses");
    assert_eq!(parsed.jobs, Some(4));
    assert_eq!(
        warnings.len(),
        1,
        "the deprecated flag this parse used should be reported: {warnings:?}"
    );

    // The layer is still the layer: collecting warnings does not change what argv bound.
    let resolved =
        resolve(Settings::SETTINGS_REGISTRY, Layers::new().then(&layer)).expect("resolves");
    assert_eq!(Settings::read(&resolved).expect("reads").jobs, 4);

    // And a caller that does not ask walks no tree and gets no warnings, which is why the
    // collecting form is a separate entry rather than the only one.
    Deprecated::parse_from_argv_with_settings(&argv).expect("parses");
}

/// A setting nothing declares a value for: not an `Option`, and no default.
#[derive(Config, Debug, PartialEq)]
struct Required {
    /// The token every request needs
    #[usage(env = "EX_TOKEN")]
    token: String,
}

#[test]
fn a_setting_nothing_defaults_says_it_is_required() {
    // Leaving `optional` unset invited the reader's own inference — "no default means
    // optional" — while `read` reports the key as missing. Docs, the JSON schema and the
    // `config_keys` completer all read the registry, so the registry has to state what
    // `read` will do rather than let each consumer guess.
    assert_eq!(Required::SETTINGS_PROPS[0].optional, Some(false));
    assert_eq!(
        Settings::SETTINGS_PROPS[0].optional,
        None,
        "a setting with a declared default always has one, and inference agrees"
    );

    // Through the spec too, because that is the copy a docs page reads.
    let kdl = format!("name \"ex\"\nbin \"ex\"\n{}", Required::spec_kdl());
    let spec: usage::Spec = kdl.parse().expect("usage-lib reads the emitted block");
    assert_eq!(
        spec.config.props.get("token").expect("declared").optional,
        Some(false)
    );

    // And the reader says the same thing: nothing supplied it, so it is missing.
    let resolved = resolve(Required::SETTINGS_REGISTRY, Layers::new()).expect("resolves");
    Required::read(&resolved).expect_err("a required setting nothing supplied");

    let env = EnvLayer::new([("EX_TOKEN".to_string(), "abc".to_string())]);
    let resolved =
        resolve(Required::SETTINGS_REGISTRY, Layers::new().then(&env)).expect("resolves");
    assert_eq!(
        Required::read(&resolved).expect("supplied"),
        Required {
            token: "abc".to_string()
        }
    );
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
    assert!(
        spec.config.sources.contains_key("git"),
        "struct-level source declarations ride with the CLI spec: {kdl}"
    );
    assert_eq!(
        spec.config.files.last().map(|file| file.path.as_str()),
        Some("ex.toml")
    );
}

#[test]
fn the_spec_endpoint_answers_with_the_settings_too() {
    // `to_kdl` is where the config block is appended and also what the `__usage_spec__`
    // endpoint answers with, so the point of the endpoint — that a tool can ask *any* usage
    // binary what it is — has to include what it can be configured with. Worth its own test
    // because `to_kdl` now has two independent appenders, this one and `spec_extra`, and they
    // write to the same string.
    let request = [OsStr::new(usage_argv::SPEC_REQUEST)];
    let answered = Ex::spec_request(&request).expect("the endpoint answers");
    let spec: usage::Spec = answered.parse().expect("usage-lib reads what it answered");
    assert!(
        spec.config.props.contains_key("task.output"),
        "the endpoint's answer carries the settings: {answered}"
    );
    assert_eq!(
        answered,
        Ex::to_kdl(),
        "the endpoint answers the whole spec"
    );

    // And an ordinary command line is not a request, so the endpoint does not shadow a parse.
    assert!(Ex::spec_request(&[OsStr::new("--jobs"), OsStr::new("4")]).is_none());
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

#[test]
fn the_argv_entry_strips_argv0_and_returns_the_layer() {
    // What a fleet `main` calls: the process-shaped argv with the settings beside the
    // struct. `parse_with_settings` wraps this with the same help/version/exit behaviour
    // as `parse`, which a test cannot hold without leaving the process.
    let argv = [OsStr::new("ex"), OsStr::new("-j"), OsStr::new("7")];
    let (cli, layer) = Ex::parse_from_argv_with_settings(&argv).expect("parses");
    assert_eq!(cli.jobs, Some(7));
    let resolved =
        resolve(Settings::SETTINGS_REGISTRY, Layers::new().then(&layer)).expect("resolves");
    assert_eq!(resolved.get_key("jobs"), Some(&Value::Int(7)));
}

/// A tool whose only spelling for a bound flag is the negation.
///
/// tak's `--no-credit`: the setting defaults to true and the command line can only turn it
/// off. The regression here was in view-aware codegen — a global flag's selector list was
/// built from longs and shorts alone, so a negate-only flag produced an empty `matches!`
/// that did not parse.
#[derive(Cli, Debug)]
#[usage(bin = "negonly")]
struct NegOnly {
    /// Leave the credit line off
    // clap's SetFalse spelling: the single long becomes the negative spelling, so this
    // flag's *only* selector is the negation.
    #[arg(long = "no-credit", action = clap::ArgAction::SetFalse, global = true)]
    #[usage(default = "true", setting = "credit")]
    credit: bool,

    #[usage(subcommand)]
    command: Option<NegOnlyCommands>,
}

#[derive(usage_derive::Subcommands, Debug)]
enum NegOnlyCommands {
    /// Run it
    Run,
}

#[test]
fn a_negate_only_global_flag_still_binds_its_setting() {
    static PROPS: &[usage_config::PropMeta] = &[usage_config::PropMeta {
        default: Some(Const::Bool(true)),
        cli: &["--no-credit"],
        ..usage_config::PropMeta::new("credit", Ty::Bool)
    }];
    const REGISTRY: usage_config::Registry = usage_config::Registry::new(PROPS);
    assert_eq!(
        REGISTRY.drift(NegOnly::SETTINGS_BINDINGS),
        Vec::<String>::new()
    );

    let argv = [OsStr::new("--no-credit")];
    let (cli, layer) = NegOnly::parse_from_with_settings(&argv).expect("parses");
    assert!(!cli.credit);
    let resolved = resolve(REGISTRY, Layers::new().then(&layer)).expect("resolves");
    assert_eq!(resolved.get_key("credit"), Some(&Value::Bool(false)));

    // And left off, the default stands: absence is not a value.
    let (cli, layer) = NegOnly::parse_from_with_settings(&[]).expect("parses");
    assert!(cli.credit);
    assert!(layer.is_empty());
}

#[test]
fn a_negate_only_flag_is_not_named_twice_in_help() {
    // `no-credit: --no-credit` — the declared name of a `SetFalse` flag is its negation, so
    // the `name:` prefix help adds for a name the forms do not imply only repeats it.
    let page =
        usage_argv::help::render(NegOnly::spec(), NegOnly::spec().root.cmd, false).expect("page");
    assert!(page.contains("--no-credit"), "{page}");
    assert!(!page.contains("no-credit: --no-credit"), "{page}");
}

#[test]
fn a_lossy_read_keeps_every_setting_that_does_read() {
    let env = EnvLayer::new([("EX_TASK_JOBS".to_string(), "2".to_string())]);
    let mut resolved =
        resolve(Settings::SETTINGS_REGISTRY, Layers::new().then(&env)).expect("resolves");

    // The way a value gets past the merge holding something its field cannot take: a post-merge
    // hook, unchecked by design because it is where a CLI's own rules live.
    let jobs = Settings::SETTINGS_REGISTRY
        .lookup("jobs")
        .expect("declared")
        .id;
    resolved.coerced(jobs, Value::Int(-1), "a hook that got it wrong");

    // Strict: one bad field costs the caller the whole resolution, and a CLI left holding
    // `Settings::default()` has thrown away the environment over a value it never set.
    Settings::read(&resolved).expect_err("-1 does not fit a u64");

    // Lossy: the bad field falls back to what it declared, and nothing else is touched.
    let (settings, errors) = Settings::read_lossy(&resolved);
    let settings = settings.expect("every field had a value or a default");
    assert_eq!(settings.jobs, 4, "the declared default, not the hook's -1");
    assert_eq!(
        settings.task.jobs,
        Some(2),
        "EX_TASK_JOBS is not collateral damage"
    );
    // A `default_fn` field has no `Const` to fall back to and does not need one: nothing was
    // wrong with it, and the fold never reached for a fallback.
    assert_eq!(settings.cache_dir, default_cache_dir());

    // Reported, not repaired — a CLI that decides a bad value is fatal has lost nothing by
    // asking lossily.
    assert_eq!(errors.0.len(), 1, "{errors}");
    assert_eq!(errors.0[0].key, "jobs");
    assert!(
        errors.0[0].origin.is_some(),
        "the hook that wrote it is named: {errors}"
    );
}

#[test]
fn a_lossy_read_with_nothing_wrong_is_the_strict_one() {
    let env = EnvLayer::new([("EX_JOBS".to_string(), "9".to_string())]);
    let resolved =
        resolve(Settings::SETTINGS_REGISTRY, Layers::new().then(&env)).expect("resolves");

    let strict = Settings::read(&resolved).expect("reads");
    let (lossy, errors) = Settings::read_lossy(&resolved);
    assert_eq!(lossy.expect("reads"), strict);
    assert!(errors.0.is_empty(), "{errors}");
}

#[test]
fn a_lossy_read_does_not_invent_a_default_that_was_never_declared() {
    // The merge seeds every declared default into the values, so a setting with no value at all
    // is one with nothing to fall back to. Filling it anyway would hide a hole in the spec.
    let resolved = resolve(Required::SETTINGS_REGISTRY, Layers::new()).expect("resolves");
    let (settings, errors) = Required::read_lossy(&resolved);
    assert!(settings.is_none(), "no value and no default");
    assert_eq!(errors.0.len(), 1, "{errors}");
    assert_eq!(errors.0[0].key, "token");
}
