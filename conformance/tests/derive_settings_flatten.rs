//! Settings declared where the flag is, including in a group or a subcommand.
//!
//! A CLI keeps its shared flags in one struct and flattens it into several commands — that is what
//! `#[usage(flatten)]` is for, and it is where hk and mise keep `--jobs`. So a setting has to be
//! declarable there, not only on the root: a binding written next to the flag and read nowhere is
//! the drift this whole thing exists to remove, and "put it on the root instead" would have
//! reintroduced it as a rule people follow by hand.
//!
//! The root is still the only place that names `usage-config`. A group hands over what it was
//! given in `usage-argv`'s own vocabulary, and the root turns that into the layer.

use std::ffi::OsStr;

use usage_config::{resolve, Layers, PropMeta, Registry, Ty, Value};
use usage_derive::{Args, Cli, Subcommands};

/// Flags every command has
#[derive(Args, Debug)]
struct Common {
    /// How many jobs to run at once
    #[usage(long, short = 'j', setting = "jobs")]
    jobs: Option<usize>,

    /// Whether to colour the output
    #[usage(long, negate = "no-colour", setting = "colour")]
    colour: bool,
}

/// What to check
#[derive(Args, Debug)]
struct Check {
    /// Stop at the first failure
    #[usage(long, setting = "check.fail_fast")]
    fail_fast: bool,
}

/// What to fix
#[derive(Args, Debug)]
struct Fix {
    /// Leave the index alone
    #[usage(long, setting = "fix.no_stash")]
    no_stash: bool,
}

#[derive(Subcommands, Debug)]
enum Cmd {
    Check(Check),
    Fix(Fix),
}

/// A tool whose settings are not all on the root
#[derive(Cli, Debug)]
#[usage(bin = "ex")]
struct Ex {
    /// Where to write
    #[usage(long, setting = "out")]
    out: Option<String>,

    #[usage(flatten)]
    common: Common,

    #[usage(subcommand)]
    cmd: Cmd,
}

static PROPS: &[PropMeta] = &[
    PropMeta {
        cli: &["--out"],
        ..PropMeta::new("out", Ty::String)
    },
    PropMeta {
        cli: &["--jobs", "-j"],
        ..PropMeta::new("jobs", Ty::Uint)
    },
    PropMeta {
        cli: &["--colour", "--no-colour"],
        ..PropMeta::new("colour", Ty::Bool)
    },
    PropMeta {
        cli: &["--fail-fast"],
        ..PropMeta::new("check.fail_fast", Ty::Bool)
    },
    PropMeta {
        cli: &["--no-stash"],
        ..PropMeta::new("fix.no_stash", Ty::Bool)
    },
];
const REGISTRY: Registry = Registry::new(PROPS);

fn parse(tokens: &[&str]) -> (Ex, usage_config::CliLayer) {
    let argv: Vec<&OsStr> = tokens.iter().map(OsStr::new).collect();
    Ex::parse_from_with_settings(&argv).expect("should parse")
}

#[test]
fn the_flags_this_cli_reads_are_the_flags_its_spec_declares() {
    // Including the ones it does not declare itself. Before this, a group's binding was invisible
    // here — the spec's `cli` node would have been reported as bound by nothing.
    assert_eq!(REGISTRY.drift(Ex::SETTINGS_BINDINGS), Vec::<String>::new());
}

#[test]
fn a_group_contributes_what_it_was_given() {
    let (cli, layer) = parse(&["--out", "log", "--jobs", "8", "--no-colour", "check"]);
    assert_eq!(cli.out.as_deref(), Some("log"));
    assert_eq!(cli.common.jobs, Some(8));
    assert!(!cli.common.colour, "the field the parser filled");

    let resolved = resolve(REGISTRY, Layers::new().then(&layer)).expect("resolves");
    assert_eq!(resolved.get_key("out"), Some(&Value::from("log")));
    assert_eq!(resolved.get_key("jobs"), Some(&Value::Int(8)));
    // The negation, which is the case a layer built from the struct cannot see at all.
    assert_eq!(resolved.get_key("colour"), Some(&Value::Bool(false)));
    // Named by the flag the group declares, not by the root.
    assert_eq!(
        resolved
            .origin(REGISTRY.lookup("jobs").expect("declared").id)
            .map(|o| o.describe()),
        Some("--jobs")
    );
}

#[test]
fn a_group_that_was_given_nothing_contributes_nothing() {
    let (_, layer) = parse(&["check"]);
    assert!(layer.is_empty(), "no flag, no entries");
}

#[test]
fn only_the_subcommand_that_ran_contributes() {
    // A flag that `fix` declares says nothing about an invocation that ran `check` — the same rule
    // that decides whose requirements are checked.
    let (cli, layer) = parse(&["check", "--fail-fast"]);
    match &cli.cmd {
        Cmd::Check(check) => assert!(check.fail_fast, "the field the parser filled"),
        Cmd::Fix(_) => panic!("ran check"),
    }
    let resolved = resolve(REGISTRY, Layers::new().then(&layer)).expect("resolves");
    assert_eq!(
        resolved.get_key("check.fail_fast"),
        Some(&Value::Bool(true))
    );
    assert_eq!(resolved.get_key("fix.no_stash"), None);

    let (cli, layer) = parse(&["fix", "--no-stash"]);
    match &cli.cmd {
        Cmd::Fix(fix) => assert!(fix.no_stash),
        Cmd::Check(_) => panic!("ran fix"),
    }
    let resolved = resolve(REGISTRY, Layers::new().then(&layer)).expect("resolves");
    assert_eq!(resolved.get_key("fix.no_stash"), Some(&Value::Bool(true)));
    assert_eq!(resolved.get_key("check.fail_fast"), None);
}

#[test]
fn the_bindings_are_every_command_s_and_not_only_the_one_that_ran() {
    // Bindings are what the CLI *can* do, so every subcommand's are in the table — a spec
    // documents `--no-stash` whether or not this invocation ran `fix`, and a drift check that only
    // saw the command that ran would report a different answer every run.
    let mut bound = Ex::SETTINGS_BINDINGS.to_vec();
    bound.sort_unstable();
    assert_eq!(
        bound,
        vec![
            ("--colour", "colour"),
            ("--fail-fast", "check.fail_fast"),
            ("--jobs", "jobs"),
            ("--no-colour", "colour"),
            ("--no-stash", "fix.no_stash"),
            ("--out", "out"),
            ("-j", "jobs"),
        ]
    );
}

/// A CLI whose only settings are somebody else's
///
/// `settings` is how it says so. A root cannot see another struct's fields, and generating the
/// entry points for every CLI would make a program with subcommands and no settings at all depend
/// on `usage-config` — so a root that binds nothing itself says once that it resolves settings.
/// Leaving it off is a compile error naming the attribute, not a CLI that quietly sets nothing.
#[derive(Cli, Debug)]
#[usage(bin = "only", settings)]
struct Only {
    #[usage(flatten)]
    common: Common,
}

#[test]
fn a_root_that_binds_nothing_itself_still_collects_its_group() {
    let argv = [OsStr::new("--jobs"), OsStr::new("2")];
    let (cli, layer) = Only::parse_from_with_settings(&argv).expect("should parse");
    assert_eq!(cli.common.jobs, Some(2));

    let resolved = resolve(REGISTRY, Layers::new().then(&layer)).expect("resolves");
    assert_eq!(resolved.get_key("jobs"), Some(&Value::Int(2)));

    let mut bound = Only::SETTINGS_BINDINGS.to_vec();
    bound.sort_unstable();
    assert_eq!(
        bound,
        vec![
            ("--colour", "colour"),
            ("--jobs", "jobs"),
            ("--no-colour", "colour"),
            ("-j", "jobs"),
        ]
    );
}
