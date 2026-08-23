use std::ffi::{OsStr, OsString};

use futures::executor::block_on;
use usage_dynamic::{Catalog, Error, Outcome, Spec};
use usage_rs::{Cli, Subcommands};

#[derive(Cli)]
#[usage(bin = "host")]
struct Host {
    #[usage(subcommand)]
    command: HostCommand,
}

#[derive(Subcommands)]
enum HostCommand {
    /// Built into the host.
    Builtin,
    /// Manage plugins.
    #[usage(alias = "p")]
    Plugins {
        #[usage(subcommand)]
        command: PluginCommand,
    },
}

#[derive(Subcommands)]
enum PluginCommand {
    /// A static plugin operation.
    #[usage(alias = "ls")]
    List,
    #[usage(external_subcommand)]
    External(Vec<OsString>),
}

#[derive(Cli)]
#[usage(bin = "root-host")]
#[allow(dead_code)]
struct RootHost {
    #[usage(subcommand)]
    command: RootCommand,
}

#[derive(Subcommands)]
#[allow(dead_code)]
enum RootCommand {
    Known,
    #[usage(external_subcommand)]
    External(Vec<String>),
}

#[derive(Cli)]
#[usage(bin = "closed")]
#[allow(dead_code)]
struct Closed {
    #[usage(subcommand)]
    command: ClosedCommand,
}

#[derive(Subcommands)]
enum ClosedCommand {
    Child,
}

fn plugin(name: &str, extra: &str) -> Spec {
    format!("name \"{name}\"\nbin \"{name}\"\n{extra}")
        .parse()
        .unwrap()
}

fn catalog() -> usage_dynamic::Catalog<'static> {
    let mut formatter = plugin(
        "formatter",
        r#"
version "2.4"
unknown_flags "error"
flag "--color <WHEN>" env="USAGE_DYNAMIC_TEST_COLOR" default="always"
arg "[path]"
"#,
    );
    formatter.about = Some("Format a project".into());
    formatter.about_long = Some("Format a project using its configured style.".into());
    formatter.cmd.help_heading = Some("Installed plugins".into());
    formatter.cmd.display_order = Some(2);
    formatter.cmd.aliases = vec!["fmt".into(), "oldfmt".into()];
    formatter.cmd.hidden_aliases = vec!["oldfmt".into()];
    let mut audit = plugin("audit", "");
    audit.about = Some("Audit a project".into());
    audit.cmd.help_heading = Some("Installed plugins".into());
    audit.cmd.display_order = Some(1);
    Catalog::builder(Host::app())
        .under("p", formatter)
        .under("plugins", audit)
        .build()
        .unwrap()
}

#[test]
fn root_summaries_appear_in_help_and_completion() {
    let mut spec = plugin("doctor", "");
    spec.about = Some("Inspect plugin health".into());
    let catalog = Catalog::builder(RootHost::app())
        .root(spec)
        .build()
        .unwrap();
    assert_eq!(RootHost::app().to_kdl(), catalog.app().to_kdl());
    let help = catalog.app().help("", false).unwrap();
    assert!(help.contains("doctor  Inspect plugin health"), "{help}");

    let argv = [
        OsString::from("__complete_word__"),
        OsString::from("--words"),
        OsString::from("root-host"),
        OsString::new(),
    ];
    let answer = block_on(catalog.app().completion_app().completion_request(&argv)).unwrap();
    assert!(
        answer.lines().any(|line| line.starts_with("doctor")),
        "{answer}"
    );
}

#[test]
fn nested_help_merges_summaries_aliases_headings_and_ordering() {
    let app = catalog().app();
    for long in [false, true] {
        let help = app.help("plugins", long).unwrap();
        assert!(help.contains("Installed plugins:"), "{help}");
        assert!(help.contains("plugins formatter [aliases: fmt]"), "{help}");
        assert!(!help.contains("oldfmt"), "{help}");
        assert!(
            help.find("plugins audit").unwrap() < help.find("plugins formatter").unwrap(),
            "{help}"
        );
    }
}

#[test]
fn command_completion_offers_visible_names_and_delegates_after_selection() {
    let app = catalog().app().completion_app();
    let request = |words: &[&str]| {
        let mut argv = vec![
            OsString::from("__complete_word__"),
            OsString::from("--words"),
        ];
        argv.extend(words.iter().map(OsString::from));
        block_on(app.clone().completion_request(&argv)).unwrap()
    };
    let answer = request(&["host", "plugins", ""]);
    assert!(
        answer.lines().any(|line| line.starts_with("formatter")),
        "{answer}"
    );
    assert!(
        answer.lines().any(|line| line.starts_with("fmt")),
        "{answer}"
    );
    assert!(!answer.contains("oldfmt"), "{answer}");
    assert_eq!(request(&["host", "plugins", "formatter", ""]), "");
}

#[test]
fn canonical_alias_unknown_help_version_and_invalid_argv_are_typed() {
    let catalog = catalog();
    let parsed = catalog
        .parse_external("p", &[OsString::from("fmt"), OsString::from("project")])
        .unwrap()
        .unwrap();
    let Outcome::Parsed(parsed) = parsed else {
        panic!("expected parsed")
    };
    assert_eq!(parsed.parent, "plugins");
    assert_eq!(parsed.name, "formatter");
    assert_eq!(parsed.invoked_as, "fmt");
    assert!(!parsed.output.tokens.is_empty());
    assert!(!parsed.output.flags.is_empty(), "default should be applied");

    std::env::set_var("USAGE_DYNAMIC_TEST_COLOR", "never");
    let from_env = catalog
        .parse_external("plugins", &[OsString::from("formatter")])
        .unwrap()
        .unwrap();
    std::env::remove_var("USAGE_DYNAMIC_TEST_COLOR");
    let Outcome::Parsed(from_env) = from_env else {
        panic!("expected parsed")
    };
    assert!(from_env
        .output
        .flag_origins
        .values()
        .flatten()
        .any(|origin| format!("{origin:?}").contains("Env")));

    assert!(catalog
        .parse_external("plugins", &[OsString::from("missing")])
        .unwrap()
        .is_none());
    let help = catalog
        .parse_external(
            "plugins",
            &[OsString::from("formatter"), OsString::from("--help")],
        )
        .unwrap();
    assert!(matches!(help, Some(Outcome::Help(_))), "{help:?}");
    assert!(matches!(
        catalog
            .parse_external(
                "plugins",
                &[OsString::from("formatter"), OsString::from("--version")]
            )
            .unwrap(),
        Some(Outcome::Version(_))
    ));
    assert!(matches!(
        catalog.parse_external(
            "plugins",
            &[OsString::from("formatter"), OsString::from("--bogus")]
        ),
        Err(Error::Parse(_)) | Err(Error::InvalidArgv(_))
    ));
}

#[test]
fn derived_external_variant_receives_the_same_argv_the_catalog_parses() {
    let host =
        Host::parse_from(&[OsStr::new("plugins"), OsStr::new("fmt"), OsStr::new("src")]).unwrap();
    let HostCommand::Plugins {
        command: PluginCommand::External(argv),
    } = host.command
    else {
        panic!("expected nested external command")
    };
    assert!(matches!(
        catalog().parse_external("plugins", &argv).unwrap(),
        Some(Outcome::Parsed(_))
    ));
}

#[test]
fn validation_rejects_bad_parents_namespaces_and_mounts() {
    assert!(matches!(
        Catalog::builder(Host::app())
            .under("absent", plugin("x", ""))
            .build(),
        Err(Error::MissingParent(_))
    ));
    assert!(matches!(
        Catalog::builder(Closed::app())
            .under("child", plugin("x", ""))
            .build(),
        Err(Error::ParentNotExternal(_))
    ));
    assert!(matches!(
        Catalog::builder(Host::app())
            .under("plugins", plugin("", ""))
            .build(),
        Err(Error::EmptyName)
    ));
    for name in ["list", "ls", "help"] {
        assert!(matches!(
            Catalog::builder(Host::app())
                .under("plugins", plugin(name, ""))
                .build(),
            Err(Error::Collision { .. })
        ));
    }
    let mut alias_collision = plugin("one", "");
    alias_collision.cmd.aliases.push("same".into());
    assert!(matches!(
        Catalog::builder(Host::app())
            .under("plugins", alias_collision)
            .under("plugins", plugin("same", ""))
            .build(),
        Err(Error::Collision { .. })
    ));
    assert!(matches!(
        Catalog::builder(Host::app())
            .under("plugins", plugin("mounted", "mount run=\"discover\"\n"))
            .build(),
        Err(Error::UnresolvedMount(_))
    ));
}

#[cfg(unix)]
#[test]
fn non_utf8_reports_the_external_token_position() {
    use std::os::unix::ffi::OsStringExt;
    let error = catalog()
        .parse_external(
            "plugins",
            &[OsString::from("formatter"), OsString::from_vec(vec![0xff])],
        )
        .unwrap_err();
    assert!(matches!(error, Error::NonUtf8 { index: 1 }));
}
