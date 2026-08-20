//! Stateful settings are tested together, not only one feature at a time.
//!
//! Each case pairs settings whose order or ownership changes the answer. These
//! are deliberately small: when one fails, the interaction is visible without
//! having to reduce a fleet-sized command tree first.

use std::ffi::OsStr;

use usage_argv::Error;
use usage_derive::{Cli, Subcommands};

fn argv<const N: usize>(words: [&str; N]) -> [&OsStr; N] {
    words.map(OsStr::new)
}

#[derive(Cli)]
#[usage(bin = "layered-list")]
struct LayeredList {
    #[usage(
        long,
        env = "USAGE_COMBINATION_VALUES",
        default = "fallback-a,fallback-b",
        delimiter = ','
    )]
    values: Vec<String>,
}

#[test]
fn argv_env_defaults_and_delimiters_keep_their_precedence() {
    unsafe { std::env::remove_var("USAGE_COMBINATION_VALUES") };
    assert_eq!(
        LayeredList::parse_from(&[]).unwrap().values,
        ["fallback-a", "fallback-b"]
    );

    unsafe { std::env::set_var("USAGE_COMBINATION_VALUES", "env-a,env-b") };
    assert_eq!(
        LayeredList::parse_from(&[]).unwrap().values,
        ["env-a", "env-b"]
    );
    assert_eq!(
        LayeredList::parse_from(&argv(["--values", "argv-a,argv-b"]))
            .unwrap()
            .values,
        ["argv-a", "argv-b"]
    );
    unsafe { std::env::remove_var("USAGE_COMBINATION_VALUES") };
}

#[derive(Cli)]
#[usage(bin = "optional-equals")]
struct OptionalEquals {
    #[usage(long, require_equals)]
    color: Option<Option<String>>,
    #[usage(arg)]
    rest: Option<String>,
}

#[test]
fn optional_values_and_require_equals_leave_detached_words_positional() {
    let attached = OptionalEquals::parse_from(&argv(["--color=always"])).unwrap();
    assert_eq!(attached.color, Some(Some("always".into())));

    let detached = OptionalEquals::parse_from(&argv(["--color", "input"])).unwrap();
    assert_eq!(detached.color, Some(None));
    assert_eq!(detached.rest.as_deref(), Some("input"));
}

#[derive(Cli)]
#[usage(bin = "global-override")]
struct GlobalOverride {
    #[usage(long, global)]
    quiet: bool,
    #[usage(long, global, overrides = "--quiet")]
    verbose: bool,
    #[usage(subcommand)]
    command: Option<OneCommand>,
}

#[derive(Subcommands)]
enum OneCommand {
    Run,
}

#[test]
fn a_global_override_applies_across_a_subcommand_boundary() {
    let parsed = GlobalOverride::parse_from(&argv(["--quiet", "run", "--verbose"])).unwrap();
    assert!(!parsed.quiet);
    assert!(parsed.verbose);
    assert!(matches!(parsed.command, Some(OneCommand::Run)));
}

#[derive(Cli)]
#[usage(bin = "strict-parent")]
#[allow(dead_code)]
struct StrictParent {
    #[usage(arg)]
    profile: String,
    #[usage(subcommand)]
    command: Option<OneCommand>,
}

#[derive(Cli)]
#[usage(bin = "negated-parent", subcommand_negates_reqs)]
struct NegatedParent {
    #[usage(arg)]
    profile: String,
    #[usage(subcommand)]
    command: Option<OneCommand>,
}

#[test]
fn subcommands_only_suppress_required_positionals_when_declared() {
    assert!(matches!(
        StrictParent::parse_from(&argv(["run"])),
        Err(Error::MissingRequired { .. })
    ));
    let parsed = NegatedParent::parse_from(&argv(["run"])).unwrap();
    assert!(parsed.profile.is_empty());
    assert!(matches!(parsed.command, Some(OneCommand::Run)));
}

#[derive(Cli)]
#[usage(bin = "default-group", group("input", required))]
struct DefaultGroup {
    #[usage(long, group = "input", default = "stdin")]
    input: String,
    #[usage(long, group = "input")]
    url: Option<String>,
}

#[test]
fn a_default_satisfies_a_required_group_without_creating_a_conflict() {
    let defaulted = DefaultGroup::parse_from(&[]).unwrap();
    assert_eq!(defaulted.input, "stdin");

    let explicit = DefaultGroup::parse_from(&argv(["--url", "https://example.test"])).unwrap();
    assert_eq!(explicit.input, "stdin");
    assert_eq!(explicit.url.as_deref(), Some("https://example.test"));
}

#[derive(Cli)]
#[usage(bin = "collisions", version = "1.2.3")]
struct BuiltinCollisions {
    #[usage(long = "help")]
    own_long_help: bool,
    #[usage(short = 'V')]
    own_short_version: bool,
}

#[test]
fn help_and_version_collisions_only_replace_the_claimed_spellings() {
    assert!(
        BuiltinCollisions::parse_from(&argv(["--help"]))
            .unwrap()
            .own_long_help
    );
    assert!(
        BuiltinCollisions::parse_from(&argv(["-V"]))
            .unwrap()
            .own_short_version
    );
    assert!(matches!(
        BuiltinCollisions::parse_from(&argv(["-h"])),
        Err(Error::Help { .. })
    ));
    assert!(matches!(
        BuiltinCollisions::parse_from(&argv(["--version"])),
        Err(Error::Version { .. })
    ));
}
