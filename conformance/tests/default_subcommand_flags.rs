use std::ffi::OsStr;
use usage_derive::{Args, Cli, Subcommands};

#[derive(Cli)]
#[usage(
    bin = "em",
    default_subcommand = "install",
    default_subcommand_flags,
    unknown_flags = "error"
)]
struct Em {
    #[usage(short = 'p')]
    pretend: bool,
    #[usage(subcommand)]
    command: Option<Commands>,
}

#[derive(Subcommands)]
enum Commands {
    Install(Install),
    Query,
}

#[derive(Args)]
struct Install {
    #[usage(short = 'u')]
    update: bool,
    #[usage(short = 'a')]
    ask: bool,
    package: Option<String>,
}

#[test]
fn default_flags_bind_to_the_derived_child_and_parent_prefix_keeps_its_owner() {
    let parsed = Em::parse_from(&["-p", "-ua", "@world"].map(OsStr::new)).unwrap();
    assert!(parsed.pretend);
    let Some(Commands::Install(install)) = parsed.command else {
        panic!("expected install")
    };
    assert!(install.update && install.ask);
    assert_eq!(install.package.as_deref(), Some("@world"));
}

#[test]
fn parent_help_and_explicit_siblings_keep_their_meaning() {
    let parsed = Em::parse_from(&[OsStr::new("-p")]).unwrap();
    assert!(parsed.pretend);
    assert!(parsed.command.is_none());
    let parsed = Em::parse_from(&[OsStr::new("query")]).unwrap();
    assert!(matches!(parsed.command, Some(Commands::Query)));
    assert!(Em::parse_from(&["-u", "query"].map(OsStr::new)).is_err());
    let Err(usage_argv::Error::Help { cmd, .. }) = Em::parse_from(&[OsStr::new("--help")]) else {
        panic!("expected parent help")
    };
    assert_eq!(cmd.name, "em");
    let Err(usage_argv::Error::Help { cmd, .. }) =
        Em::parse_from(&["-u", "--help"].map(OsStr::new))
    else {
        panic!("expected child help")
    };
    assert_eq!(cmd.name, "install");
}

#[test]
fn the_opt_in_survives_derive_kdl_and_json_roundtrips() {
    let kdl = Em::to_kdl();
    assert!(kdl.contains("default_subcommand_flags #true"), "{kdl}");
    let spec: usage::Spec = kdl.parse().unwrap();
    assert!(spec.default_subcommand_flags);
    let json = serde_json::to_string(&spec).unwrap();
    let json: serde_json::Value = serde_json::from_str(&json).unwrap();
    assert_eq!(json["default_subcommand_flags"], true);
    let again: usage::Spec = spec.to_string().parse().unwrap();
    assert!(again.default_subcommand_flags);
}

#[test]
fn an_explicit_false_overrides_an_enabled_spec() {
    let mut spec: usage::Spec = Em::to_kdl().parse().unwrap();
    let disabled: usage::Spec = "default_subcommand_flags #false".parse().unwrap();
    spec.merge(disabled);
    assert!(!spec.default_subcommand_flags);
    let roundtrip: usage::Spec = spec.to_string().parse().unwrap();
    assert!(!roundtrip.default_subcommand_flags);
}

/// Mixed clusters route each field to its declaring derived struct.
#[test]
fn mixed_short_bundles_keep_parent_fields_in_both_orders() {
    for bundle in ["-pua", "-upa", "-uap"] {
        let parsed = Em::parse_from(&[bundle, "@world"].map(OsStr::new)).unwrap();
        assert!(parsed.pretend);
        let Some(Commands::Install(install)) = parsed.command else {
            panic!("expected install")
        };
        assert!(install.update && install.ask);
        assert_eq!(install.package.as_deref(), Some("@world"));
    }
}

/// Conflict diagnostics name the selected command, not the token that implied it.
#[test]
fn implicit_conflicts_name_the_default_command() {
    let kdl = Em::to_kdl();
    let mut spec: usage::Spec = kdl.parse().unwrap();
    spec.cmd.args_conflicts_with_subcommands = true;
    spec.version = Some("1.2.3".into());
    for words in [
        vec!["em", "-p", "-u"],
        vec!["em", "-pu"],
        vec!["em", "-up"],
        vec!["em", "-uhp"],
        vec!["em", "-uVp"],
    ] {
        let error = usage::Parser::new(&spec)
            .parse(&words.into_iter().map(String::from).collect::<Vec<_>>())
            .unwrap_err();
        assert!(
            error.to_string().contains("subcommand 'install'"),
            "{error}"
        );
        assert!(!error.to_string().contains("subcommand '-u'"), "{error}");
    }
}

#[test]
fn mixed_bundles_honor_parent_conflicts_in_the_runtime() {
    #[derive(Cli)]
    #[usage(
        default_subcommand = "install",
        default_subcommand_flags,
        args_conflicts_with_subcommands,
        version = "1.2.3"
    )]
    struct Conflicts {
        #[usage(short = 'p')]
        pretend: bool,
        #[usage(subcommand)]
        command: Option<Commands>,
    }
    for bundle in ["-pu", "-up", "-uhp", "-uVp"] {
        assert!(matches!(
            Conflicts::parse_from(&[OsStr::new(bundle)]),
            Err(usage_argv::Error::SubcommandConflict { .. })
        ));
    }
}
