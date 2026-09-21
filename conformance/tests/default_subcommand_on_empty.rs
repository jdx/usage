use std::ffi::OsStr;

use usage_derive::{Args, Cli, Subcommands};

#[derive(Cli)]
#[usage(
    bin = "em",
    default_subcommand = "run",
    default_subcommand_on_empty,
    unknown_flags = "error"
)]
struct Em {
    #[usage(long, global = true)]
    verbose: bool,
    #[usage(subcommand)]
    command: Option<Commands>,
}

#[derive(Subcommands)]
enum Commands {
    Run(Run),
    Query,
}

#[derive(Args)]
#[usage(arg_required_else_help)]
struct Run {
    #[usage(long)]
    dry: bool,
    path: Option<String>,
}

/// The non-required child covers the successful binding path, including root
/// flag ownership, rather than only the child's implicit-help policy.
#[derive(Cli)]
#[usage(
    bin = "happy",
    version = "1.0",
    default_subcommand = "run",
    default_subcommand_on_empty,
    arg_required_else_help
)]
struct Happy {
    #[usage(long, global = true)]
    verbose: bool,
    #[usage(long, default = "root")]
    mode: String,
    #[usage(subcommand)]
    command: Option<HappyCommands>,
}

#[derive(Subcommands)]
enum HappyCommands {
    Run(HappyRun),
}

#[derive(Args)]
struct HappyRun {
    #[usage(long, default = "child")]
    mode: String,
    #[usage(long)]
    dry: bool,
    path: Option<String>,
}

#[derive(Cli)]
#[usage(
    bin = "required",
    default_subcommand = "run",
    default_subcommand_on_empty
)]
struct RequiredRoot {
    #[usage(long)]
    needed: String,
    input: String,
    #[usage(subcommand)]
    command: Option<RequiredCommands>,
}

#[derive(Subcommands)]
enum RequiredCommands {
    Run,
}

#[derive(Cli)]
#[usage(
    bin = "negated",
    default_subcommand = "run",
    default_subcommand_on_empty,
    subcommand_negates_reqs
)]
struct NegatedRoot {
    #[usage(long)]
    needed: String,
    input: String,
    #[usage(subcommand)]
    command: Option<RequiredCommands>,
}

#[test]
fn empty_and_parent_flags_select_default_before_child_validation() {
    assert!(
        matches!(Em::parse_from(&[]), Err(usage_argv::Error::MissingArgsHelp { cmd, .. }) if cmd.name == "run")
    );
    assert!(
        matches!(Em::parse_from(&[OsStr::new("--verbose")]), Err(usage_argv::Error::MissingArgsHelp { cmd, .. }) if cmd.name == "run")
    );
}

#[test]
fn explicit_routes_and_separator_suppress_empty_fallback() {
    assert!(matches!(
        Em::parse_from(&[OsStr::new("query")]).unwrap().command,
        Some(Commands::Query)
    ));
    assert!(matches!(
        Em::parse_from(&[OsStr::new("--"), OsStr::new("--help")]),
        Err(usage_argv::Error::UnexpectedArg { .. })
    ));
}

#[test]
fn parent_flags_and_child_values_bind_after_implicit_selection() {
    let empty = Happy::parse_from(&[]).expect("empty argv selects the default child");
    assert!(matches!(
        empty.command,
        Some(HappyCommands::Run(HappyRun { mode, .. })) if mode == "child"
    ));
    let parsed =
        Happy::parse_from(&[OsStr::new("--verbose")]).expect("implicit child accepts parent flags");
    assert!(parsed.verbose);
    assert!(matches!(
        parsed.command,
        Some(HappyCommands::Run(HappyRun { mode, dry: false, path: None })) if mode == "child"
    ));
    let parsed = Happy::parse_from(&[OsStr::new("--mode"), OsStr::new("custom")])
        .expect("parent local mode binds before implicit selection");
    assert_eq!(parsed.mode, "custom");
    assert!(matches!(
        parsed.command,
        Some(HappyCommands::Run(HappyRun { mode, .. })) if mode == "child"
    ));
}

#[test]
fn required_root_parity_and_negation_are_explicit() {
    assert!(matches!(
        RequiredRoot::parse_from(&[]),
        Err(usage_argv::Error::MissingRequired { name: "needed" })
    ));
    assert!(matches!(
        RequiredRoot::parse_from(&[OsStr::new("run")]),
        Err(usage_argv::Error::MissingRequired { name: "needed" })
    ));
    assert!(matches!(
        RequiredRoot::parse_from(&[OsStr::new("--needed"), OsStr::new("yes")]),
        Err(usage_argv::Error::MissingRequired { name: "INPUT" })
    ));
    assert!(matches!(
        RequiredRoot::parse_from(&[OsStr::new("--needed"), OsStr::new("yes"), OsStr::new("run")]),
        Err(usage_argv::Error::MissingRequired { name: "INPUT" })
    ));
    for argv in [&[][..], &[OsStr::new("run")][..]] {
        let parsed = NegatedRoot::parse_from(argv).expect("negated requirements are waived");
        assert!(matches!(parsed.command, Some(RequiredCommands::Run)));
    }
}

#[test]
fn root_help_and_version_bypass_implicit_child() {
    let Err(usage_argv::Error::Help { cmd, .. }) = Happy::parse_from(&[OsStr::new("--help")])
    else {
        panic!("expected root help")
    };
    assert_eq!(cmd.name, "happy");
    let Err(usage_argv::Error::Version { .. }) = Happy::parse_from(&[OsStr::new("--version")])
    else {
        panic!("expected root version")
    };
}

#[test]
fn false_override_roundtrips() {
    let mut spec: usage::Spec = Happy::to_kdl().parse().unwrap();
    let disabled: usage::Spec = "default_subcommand_on_empty #false".parse().unwrap();
    spec.merge(disabled);
    assert!(!spec.default_subcommand_on_empty);
    let roundtrip: usage::Spec = spec.to_string().parse().unwrap();
    assert!(!roundtrip.default_subcommand_on_empty);
}

#[test]
fn generated_spec_roundtrips_the_opt_in() {
    let kdl = Em::to_kdl();
    assert!(kdl.contains("default_subcommand_on_empty #true"), "{kdl}");
    let spec: usage::Spec = kdl.parse().unwrap();
    assert!(spec.default_subcommand_on_empty);
    let json: serde_json::Value =
        serde_json::from_str(&serde_json::to_string(&spec).unwrap()).unwrap();
    assert_eq!(json["default_subcommand_on_empty"], true);
}
