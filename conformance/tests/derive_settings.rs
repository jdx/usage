//! What a derived CLI contributes to a resolution.
//!
//! The claim under test is one sentence: the flags a CLI *documents* and the flags it *reads into
//! settings* are the same flags, and the values it contributes are the ones the user actually typed.
//!
//! Both halves are things the fleet gets wrong today. hk declares eighteen `sources.cli` bindings
//! and reads five; mise hand-copies thirteen flags in a forty-nine-line function. The second half —
//! "actually typed" — is the subtler one: a `bool` field is `false` whether the flag was left off or
//! explicitly negated, so a layer built by reading the parsed struct sets every switch on every run,
//! and the command line outranks every file on the machine.

use std::ffi::OsStr;
use std::path::PathBuf;

use usage_config::{resolve, Const, Layers, PropMeta, Registry, Ty, Value, WarningKind};
use usage_derive::Cli;

/// A tool with settings
#[derive(Cli, Debug)]
#[usage(bin = "ex")]
struct Ex {
    /// How many jobs to run at once
    // `long` as well as `short`: with only `short` the derive binds `-j` and nothing else, and the
    // drift test below is what said so — the spec declares `--jobs` and this would have read it
    // nowhere. That is the failure this whole PR exists to make loud, found on its own fixture.
    #[usage(long, short = 'j', setting = "jobs")]
    jobs: Option<usize>,

    /// Whether to colour the output
    #[usage(long, negate = "no-colour", setting = "colour")]
    colour: bool,

    /// Paths to leave alone
    #[usage(long, var, setting = "exclude")]
    exclude: Vec<String>,

    /// Where the config file is
    // A path rather than a `String`, which is the case that can hold what a setting cannot: parsing
    // into a `String` refuses non-UTF-8 outright, and a `PathBuf` on Unix takes it.
    #[usage(long, setting = "config.file")]
    config: Option<PathBuf>,

    /// Nothing to do with settings, and it should stay that way.
    #[usage(short = 'v')]
    verbose: bool,
}

/// The registry a build would generate from the spec beside it.
static PROPS: &[PropMeta] = &[
    PropMeta {
        default: Some(Const::Int(4)),
        cli: &["--jobs", "-j"],
        ..PropMeta::new("jobs", Ty::Uint)
    },
    PropMeta {
        default: Some(Const::Bool(true)),
        cli: &["--colour", "--no-colour"],
        ..PropMeta::new("colour", Ty::Bool)
    },
    PropMeta {
        cli: &["--exclude"],
        ..PropMeta::new("exclude", Ty::List(&Ty::String))
    },
    PropMeta {
        cli: &["--config"],
        ..PropMeta::new("config.file", Ty::Path)
    },
];
const REGISTRY: Registry = Registry::new(PROPS);

fn parse(argv: &[&str]) -> (Ex, usage_config::CliLayer) {
    let argv: Vec<&OsStr> = argv.iter().map(OsStr::new).collect();
    Ex::parse_from_with_settings(&argv).expect("should parse")
}

#[test]
fn the_flags_this_cli_reads_are_the_flags_its_spec_declares() {
    // The test hk needed and did not have. It is one line in an adopter's suite, and it fails when
    // a flag is documented and read by nothing, or read and documented by nothing.
    assert_eq!(REGISTRY.drift(Ex::SETTINGS_BINDINGS), Vec::<String>::new());
}

#[test]
fn a_flag_that_was_given_sets_its_setting() {
    let (cli, layer) = parse(&["--jobs", "8", "--exclude", "target", "--exclude", "dist"]);
    assert_eq!(cli.jobs, Some(8));
    // Read here as well as through the layer: the struct is what the CLI's own code uses, and the
    // two have to be the same command line.
    assert_eq!(cli.exclude, vec!["target".to_string(), "dist".to_string()]);

    let resolved = resolve(REGISTRY, Layers::new().then(&layer)).expect("resolves");
    assert_eq!(resolved.get_key("jobs"), Some(&Value::Int(8)));
    assert_eq!(
        resolved.get_key("exclude"),
        Some(&Value::List(vec![
            Value::from("target"),
            Value::from("dist")
        ]))
    );
    // Named as the flag, which is what makes an explanation actionable.
    assert_eq!(
        resolved.origin_key("jobs").map(|o| o.describe()),
        Some("--jobs")
    );
}

#[test]
fn a_flag_that_was_not_given_leaves_the_lower_layers_alone() {
    let (_, layer) = parse(&[]);
    assert!(layer.is_empty(), "nothing was typed, so nothing was given");

    let resolved = resolve(REGISTRY, Layers::new().then(&layer)).expect("resolves");
    // The declared defaults, untouched: a CLI layer that contributed here would beat every file on
    // the machine with a value nobody asked for.
    assert_eq!(resolved.get_key("jobs"), Some(&Value::Int(4)));
    assert_eq!(resolved.get_key("colour"), Some(&Value::Bool(true)));
    assert_eq!(
        resolved.origin_key("colour").map(|o| o.describe()),
        Some("the default")
    );
}

#[test]
fn a_negated_flag_is_a_value_and_not_an_absence() {
    // The whole reason this is built from what the parser saw rather than from the struct. `colour`
    // is `false` either way; only the parser knows the user said so.
    let (cli, layer) = parse(&["--no-colour"]);
    assert!(!cli.colour);
    assert!(!layer.is_empty());

    let resolved = resolve(REGISTRY, Layers::new().then(&layer)).expect("resolves");
    assert_eq!(resolved.get_key("colour"), Some(&Value::Bool(false)));
    assert_eq!(
        resolved.origin_key("colour").map(|o| o.describe()),
        Some("--colour"),
        "reported as the flag the setting declares first"
    );
}

#[test]
fn a_flag_bound_to_nothing_contributes_nothing() {
    // `--verbose` names no setting, so it is a flag like any other: parsed, and not in the layer.
    let (cli, layer) = parse(&["-v"]);
    assert!(cli.verbose);
    assert!(layer.is_empty());
    assert!(
        !Ex::SETTINGS_BINDINGS.iter().any(|(flag, _)| *flag == "-v"),
        "a flag with no setting is not a binding"
    );
}

#[test]
fn the_bindings_are_every_spelling_of_every_bound_flag() {
    // Every spelling, because each one is a promise: a spec that declares `-j` and a CLI that reads
    // only `--jobs` has a short flag that silently does nothing to the setting.
    let mut bound = Ex::SETTINGS_BINDINGS.to_vec();
    bound.sort_unstable();
    assert_eq!(
        bound,
        vec![
            ("--colour", "colour"),
            ("--config", "config.file"),
            ("--exclude", "exclude"),
            ("--jobs", "jobs"),
            ("--no-colour", "colour"),
            ("-j", "jobs"),
        ]
    );
}

#[cfg(unix)]
#[test]
fn a_value_that_is_not_text_is_reported_rather_than_rendered() {
    // An argument is bytes and a setting is text: every layer below this one read a file or a
    // variable that had to be UTF-8 to exist. Rendered lossily, `--exclude $'\xff'` would set
    // `exclude` to a string nobody typed — naming a file that does not exist — while `cli.exclude`
    // still held the real bytes, and the command line's answer is the one that outranks every file
    // on the machine.
    use std::os::unix::ffi::OsStrExt;

    let bytes = OsStr::from_bytes(b"/etc/co\xffnfig");
    let argv = [OsStr::new("--config"), bytes];
    let (cli, layer) = Ex::parse_from_with_settings(&argv).expect("should parse");
    assert_eq!(
        cli.config.as_deref(),
        Some(std::path::Path::new(bytes)),
        "the field keeps the bytes, which is what the CLI's own code needs"
    );

    let resolved = resolve(REGISTRY, Layers::new().then(&layer)).expect("resolves");
    assert_eq!(
        resolved.get_key("config.file"),
        None,
        "and the setting keeps what it had"
    );
    let kinds: Vec<_> = resolved.warnings.iter().map(|w| w.kind).collect();
    assert_eq!(kinds, vec![WarningKind::WrongType]);
}
