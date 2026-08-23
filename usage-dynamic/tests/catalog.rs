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

/// A host with a value flag a runtime completer answers for.
#[derive(Cli)]
#[usage(bin = "themehost")]
#[allow(dead_code)]
struct ThemeHost {
    /// Which theme to use.
    #[usage(long)]
    theme: Option<String>,
    #[usage(subcommand)]
    command: ThemeCommand,
}

#[derive(Subcommands)]
#[allow(dead_code)]
enum ThemeCommand {
    Plugins {
        #[usage(subcommand)]
        command: ThemePluginCommand,
    },
}

#[derive(Subcommands)]
#[allow(dead_code)]
enum ThemePluginCommand {
    List,
    #[usage(external_subcommand)]
    External(Vec<OsString>),
}

/// A host flag, declared where runtime commands also appear.
#[derive(Cli)]
#[usage(bin = "flaghost")]
#[allow(dead_code)]
struct FlagHost {
    #[usage(subcommand)]
    command: FlagHostCommand,
}

#[derive(Subcommands)]
#[allow(dead_code)]
enum FlagHostCommand {
    Plugins {
        /// Say less.
        #[usage(long)]
        quiet: bool,
        #[usage(subcommand)]
        command: FlagPluginCommand,
    },
}

#[derive(Subcommands)]
#[allow(dead_code)]
enum FlagPluginCommand {
    /// A static plugin operation.
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
flag "--color <WHEN>" env="USAGE_DYNAMIC_TEST_COLOR" default="always" {
    choices "always" "never"
}
arg "[path]"
cmd "check" help="Check formatting" {
    flag "--fix" help="Apply fixes"
}
"#,
    );
    formatter.about = Some("Format a project".into());
    formatter.about_long = Some("Format a project using its configured style.".into());
    formatter.cmd.help_heading = Some("Installed plugins".into());
    formatter.cmd.display_order = Some(2);
    formatter.cmd.aliases = vec!["fmt".into()];
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
    let static_kdl = RootHost::app().to_kdl();
    let catalog = Catalog::builder(RootHost::app())
        .root(spec)
        .build()
        .unwrap();
    assert_eq!(static_kdl, RootHost::app().to_kdl());
    let help = catalog.app().unwrap().help("", false).unwrap();
    assert!(help.contains("doctor  Inspect plugin health"), "{help}");
    assert!(catalog
        .app()
        .unwrap()
        .help("doctor", false)
        .unwrap()
        .contains("Inspect plugin health"));

    let argv = [
        OsString::from("__complete_word__"),
        OsString::from("--words"),
        OsString::from("root-host"),
        OsString::new(),
    ];
    let answer = block_on(catalog.app().unwrap().completion_request(&argv)).unwrap();
    assert!(
        answer.lines().any(|line| line.starts_with("doctor")),
        "{answer}"
    );
    let RootCommand::External(argv) = RootHost::parse_from(&[OsStr::new("doctor")])
        .unwrap()
        .command
    else {
        panic!("expected top-level external command")
    };
    let argv: Vec<OsString> = argv.into_iter().map(OsString::from).collect();
    assert!(matches!(
        catalog.parse_external("", &argv).unwrap(),
        Some(Outcome::Parsed(_))
    ));
}

#[test]
fn nested_help_merges_summaries_aliases_headings_and_ordering() {
    let catalog = catalog();
    let app = catalog.app().unwrap();
    for long in [false, true] {
        let help = app.help("plugins", long).unwrap();
        assert!(help.contains("Installed plugins:"), "{help}");
        assert!(help.contains("plugins formatter"), "{help}");
        assert!(help.contains("[aliases: fmt]"), "{help}");
        assert!(!help.contains("oldfmt"), "{help}");
        assert!(
            help.find("plugins audit").unwrap() < help.find("plugins formatter").unwrap(),
            "{help}"
        );
    }
    let plugin_help = app.help("plugins formatter", false).unwrap();
    assert!(plugin_help.contains("--color"), "{plugin_help}");
    assert!(plugin_help.contains("formatter check"), "{plugin_help}");
    let nested_help = app.help("plugins fmt check", false).unwrap();
    assert!(nested_help.contains("--fix"), "{nested_help}");
}

#[test]
fn command_completion_offers_visible_names_and_delegates_after_selection() {
    let catalog = catalog();
    let app = catalog.app().unwrap();
    let parsed = usage_parser::parse::parse_partial(
        app.spec(),
        &["host".into(), "plugins".into(), "formatter".into()],
    )
    .unwrap();
    assert_eq!(parsed.cmd.name, "formatter", "{:?}", parsed.cmds);
    let request = |words: &[&str]| {
        let mut argv = vec![
            OsString::from("__complete_word__"),
            OsString::from("--words"),
        ];
        argv.extend(words.iter().map(OsString::from));
        block_on(app.completion_request(&argv)).unwrap()
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
    let flags = request(&["host", "plugins", "formatter", "--"]);
    assert!(
        flags.lines().any(|line| line.starts_with("--color")),
        "{flags}"
    );
    let nested = request(&["host", "plugins", "formatter", "ch"]);
    assert!(
        nested.lines().any(|line| line.starts_with("check")),
        "{nested}"
    );
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

/// The answer to a completion request, as the lines a shell would read.
fn answer(catalog: &Catalog<'_>, words: &[&str]) -> Vec<String> {
    let mut argv = vec![
        OsString::from("__complete_word__"),
        OsString::from("--words"),
    ];
    argv.extend(words.iter().map(OsString::from));
    block_on(catalog.app().unwrap().completion_request(&argv))
        .unwrap()
        .lines()
        .map(|line| line.split('\t').next().unwrap_or_default().to_string())
        .collect()
}

#[test]
fn a_catalogued_name_is_offered_where_a_subcommand_belongs() {
    let catalog = catalog();
    let offered = answer(&catalog, &["host", "plugins", ""]);
    // Static commands and runtime ones, in one list: a user typing here cannot tell which is
    // which, and should not have to.
    for name in ["list", "ls", "audit", "formatter", "fmt"] {
        assert!(offered.contains(&name.to_string()), "{name}: {offered:?}");
    }
    // A hidden alias answers but is never advertised — the same rule static ones follow.
    assert!(!offered.contains(&"oldfmt".to_string()), "{offered:?}");
    // One list, sorted and deduplicated, rather than two concatenated.
    let mut sorted = offered.clone();
    sorted.sort();
    sorted.dedup();
    assert_eq!(offered, sorted, "{offered:?}");

    // A prefix narrows runtime names as it narrows static ones.
    assert_eq!(answer(&catalog, &["host", "plugins", "fo"]), ["formatter"]);
}

#[test]
fn a_flag_position_is_the_hosts_alone() {
    // Runtime commands are commands. Where a flag is what could be typed, the host's tables are
    // the whole answer and a plugin name would be a word that cannot go there.
    let catalog = Catalog::builder(FlagHost::app())
        .under("plugins", plugin("formatter", ""))
        .build()
        .unwrap();
    let offered = answer(&catalog, &["flaghost", "plugins", "--"]);
    assert!(offered.iter().any(|line| line == "--quiet"), "{offered:?}");
    assert!(!offered.contains(&"formatter".to_string()), "{offered:?}");
}

#[test]
fn completion_descends_into_a_plugins_own_spec() {
    let catalog = catalog();
    // Its subcommands, and not the sibling static ones it is being completed alongside.
    let offered = answer(&catalog, &["host", "plugins", "formatter", ""]);
    assert!(offered.contains(&"check".to_string()), "{offered:?}");
    assert!(!offered.contains(&"list".to_string()), "{offered:?}");

    // Its flags.
    let flags = answer(&catalog, &["host", "plugins", "formatter", "--"]);
    assert!(flags.contains(&"--color".to_string()), "{flags:?}");

    // Its nested command's flags, reached through an alias of its own.
    let nested = answer(&catalog, &["host", "plugins", "fmt", "check", "--f"]);
    assert!(nested.contains(&"--fix".to_string()), "{nested:?}");

    // And its declared choices, which are the whole answer: a mistyped one is no matches, not
    // an invitation to complete a path.
    let choices = answer(&catalog, &["host", "plugins", "formatter", "--color", ""]);
    assert_eq!(choices, ["always", "never"], "{choices:?}");
    let mistyped = answer(&catalog, &["host", "plugins", "formatter", "--color", "zz"]);
    assert!(mistyped.is_empty(), "{mistyped:?}");
}

#[test]
fn a_hidden_alias_descends_without_being_advertised() {
    let catalog = catalog();
    let offered = answer(&catalog, &["host", "plugins", "oldfmt", ""]);
    assert!(offered.contains(&"check".to_string()), "{offered:?}");
}

#[test]
fn an_uncatalogued_name_is_answered_with_nothing() {
    // The host has no idea what `unknownthing` is, and neither does this catalog. Offering the
    // parent's static subcommands there would complete a line that runs something else, and
    // offering the working directory would claim a path belongs to a program nobody can ask.
    let catalog = catalog();
    let offered = answer(&catalog, &["host", "plugins", "unknownthing", ""]);
    assert!(offered.is_empty(), "{offered:?}");
    let raw = {
        let argv = [
            OsString::from("__complete_word__"),
            OsString::from("--words"),
            OsString::from("host"),
            OsString::from("plugins"),
            OsString::from("unknownthing"),
            OsString::new(),
        ];
        block_on(catalog.app().unwrap().completion_request(&argv)).unwrap()
    };
    assert!(!raw.contains('\u{1}'), "no path fallback either: {raw:?}");
}

#[test]
fn a_host_flag_before_the_name_does_not_move_the_boundary() {
    let catalog = Catalog::builder(FlagHost::app())
        .under("plugins", plugin("formatter", "cmd \"check\" {}\n"))
        .build()
        .unwrap();
    let offered = answer(
        &catalog,
        &["flaghost", "plugins", "--quiet", "formatter", ""],
    );
    assert!(offered.contains(&"check".to_string()), "{offered:?}");
}

#[test]
fn a_line_and_a_word_list_are_the_same_question() {
    // Two ways in to the same protocol: elvish hands over pre-split words, the others a line
    // and a cursor. A host answering half the line itself must not make them disagree.
    let catalog = catalog();
    let by_words = answer(&catalog, &["host", "plugins", "formatter", ""]);
    let by_line = {
        let argv = [
            OsString::from("__complete_word__"),
            OsString::from("--line"),
            OsString::from("host plugins formatter "),
        ];
        block_on(catalog.app().unwrap().completion_request(&argv))
            .unwrap()
            .lines()
            .map(|line| line.split('\t').next().unwrap_or_default().to_string())
            .collect::<Vec<_>>()
    };
    assert_eq!(by_words, by_line);

    // A cursor before the end completes what is under it, and the words after it are not part
    // of the question.
    let mid = {
        let line = "host plugins formatter check";
        let argv = [
            OsString::from("__complete_word__"),
            OsString::from("--line"),
            OsString::from(line),
            OsString::from("--cursor"),
            OsString::from("21"),
        ];
        block_on(catalog.app().unwrap().completion_request(&argv))
            .unwrap()
            .lines()
            .map(|line| line.split('\t').next().unwrap_or_default().to_string())
            .collect::<Vec<_>>()
    };
    assert_eq!(mid, ["formatter"], "completing `form⌶` mid-line");
}

#[test]
fn an_open_position_still_defers_to_the_shells_paths() {
    // The tightening is about positions that state their own answers. One that has nothing to
    // say still hands over to the shell, or completing a plugin's file argument would offer
    // nothing at all. `check` declares a flag and no words of its own, so a bare word there is
    // the filesystem's to answer.
    let catalog = catalog();
    let argv = [
        OsString::from("__complete_word__"),
        OsString::from("--words"),
        OsString::from("host"),
        OsString::from("plugins"),
        OsString::from("formatter"),
        OsString::from("check"),
        OsString::new(),
    ];
    let raw = block_on(catalog.app().unwrap().completion_request(&argv)).unwrap();
    assert!(raw.contains("\u{1}files"), "{raw:?}");
}

#[test]
fn dispatch_never_builds_the_merged_tree() {
    // The point of the split: running a plugin command is the hot path, and the merged tree is
    // a KDL round trip of the whole host spec. A catalog that is only ever dispatched through
    // must never assemble one — which is observable, because `app()` is what assembles it.
    let catalog = catalog();
    assert!(matches!(
        catalog
            .parse_external("plugins", &[OsString::from("fmt")])
            .unwrap(),
        Some(Outcome::Parsed(_))
    ));
    assert!(!catalog.is_assembled());
    catalog.app().unwrap();
    assert!(catalog.is_assembled());
}

/// A runtime completer of the host's own, of the kind `completions` registers.
fn theme_values(
    _: &usage_rs::complete::CompleteCtx<'_>,
) -> Vec<usage_rs::complete::Candidate<'static>> {
    vec![usage_rs::complete::Candidate::new("solarized")]
}

fn theme_values_async(
    ctx: usage_rs::complete::CompleteCtx<'_>,
) -> usage_rs::complete::CompletionFuture<'_> {
    let _ = ctx;
    Box::pin(async { vec![usage_rs::complete::Candidate::new("midnight")] })
}

static OVERLAYS: [usage_rs::complete::CompletionOverlay<'static>; 1] =
    [usage_rs::complete::CompletionOverlay::sync_any(
        "theme",
        theme_values,
    )];

static ASYNC_OVERLAYS: [usage_rs::complete::CompletionOverlay<'static>; 1] =
    [usage_rs::complete::CompletionOverlay::async_any(
        "theme",
        theme_values_async,
    )];

#[test]
fn the_hosts_own_completers_still_answer_for_the_hosts_words() {
    // The catalog answers part of the line, not all of it. Everything the host registered has
    // to keep working, or adopting runtime commands would quietly cost a CLI its completions.
    for (overlays, expected) in [
        (&OVERLAYS[..], "solarized"),
        (&ASYNC_OVERLAYS[..], "midnight"),
    ] {
        let catalog = Catalog::builder(ThemeHost::app())
            .completions(overlays)
            .under("plugins", plugin("formatter", ""))
            .build()
            .unwrap();
        let argv = [
            OsString::from("__complete_word__"),
            OsString::from("--line"),
            OsString::from("themehost --theme "),
        ];
        let rendered = block_on(catalog.app().unwrap().completion_request(&argv)).unwrap();
        assert!(rendered.contains(expected), "{rendered:?}");
    }
}

#[test]
fn a_named_completer_request_is_answered_by_the_host() {
    // What a spec's `run=` line asks for: one named completer's values rather than everything
    // the cursor could take. The request has its own flag, and a host that dropped it would
    // answer a different question than the KDL it emitted promised.
    let catalog = Catalog::builder(ThemeHost::app())
        .completions(&OVERLAYS)
        .under("plugins", plugin("formatter", ""))
        .build()
        .unwrap();
    let argv = [
        OsString::from("__complete_word__"),
        OsString::from("--candidates"),
        OsString::from("theme"),
        OsString::from("--line"),
        OsString::from("themehost "),
    ];
    let rendered = block_on(catalog.app().unwrap().completion_request(&argv)).unwrap();
    assert!(rendered.contains("solarized"), "{rendered:?}");

    // Past a runtime command there is nothing to run it with: a `run=` completer is a
    // subprocess, and this crate spawns none.
    let argv = [
        OsString::from("__complete_word__"),
        OsString::from("--candidates"),
        OsString::from("theme"),
        OsString::from("--line"),
        OsString::from("themehost plugins formatter "),
    ];
    let rendered = block_on(catalog.app().unwrap().completion_request(&argv)).unwrap();
    assert!(rendered.trim().is_empty(), "{rendered:?}");
}

#[test]
fn a_builtin_never_needs_a_catalog_and_a_fallback_needs_one_spec() {
    // The design's cost promise: parse first, against the static tables alone. A built-in
    // command dispatches before any catalog exists, so a plugin manager pays for discovery
    // only when the catch-all actually fired — and then only for the specs it chose to load.
    let host = Host::parse_from(&[OsStr::new("builtin")]).unwrap();
    assert!(matches!(host.command, HostCommand::Builtin));
    // No catalog was constructed on that path at all.

    let host = Host::parse_from(&[
        OsStr::new("plugins"),
        OsStr::new("formatter"),
        OsStr::new("src"),
    ])
    .unwrap();
    let HostCommand::Plugins {
        command: PluginCommand::External(captured),
    } = host.command
    else {
        panic!("expected the catch-all")
    };
    // Load exactly one plugin's spec, after the parse decided one is needed.
    let catalog = Catalog::builder(Host::app())
        .under("plugins", plugin("formatter", "arg \"[path]\"\n"))
        .build()
        .unwrap();
    assert!(matches!(
        catalog.parse_external("plugins", &captured).unwrap(),
        Some(Outcome::Parsed(_))
    ));
    assert!(!catalog.is_assembled(), "dispatch built no merged tree");

    // A name the loaded specs do not answer to falls through, same as always.
    let catalog = Catalog::builder(Host::app())
        .under("plugins", plugin("audit", ""))
        .build()
        .unwrap();
    assert!(catalog
        .parse_external("plugins", &captured)
        .unwrap()
        .is_none());
}
