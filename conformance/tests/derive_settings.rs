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

use usage_config::{resolve, Const, Layers, PropMeta, Registry, Ty, Value};
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
        resolved
            .origin(REGISTRY.lookup("jobs").expect("declared").id)
            .map(|o| o.describe()),
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
        resolved
            .origin(REGISTRY.lookup("colour").expect("declared").id)
            .map(|o| o.describe()),
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
        resolved
            .origin(REGISTRY.lookup("colour").expect("declared").id)
            .map(|o| o.describe()),
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
            ("--exclude", "exclude"),
            ("--jobs", "jobs"),
            ("--no-colour", "colour"),
            ("-j", "jobs"),
        ]
    );
}
