//! Asking for help, and getting the page the reference would print.
//!
//! `--help` and `-h` are supplied by the parser rather than declared by a CLI, because no spec
//! declares them and one that did would render a `--help` in its own help output. They are
//! recognised after the command's own flags, so a CLI that *does* declare one keeps it.
//!
//! A request comes back as `Error::Help` rather than being printed: a parse that stops to show
//! help has produced no value, which is the shape every caller already handles, and a library
//! that writes to stdout on its own is one an adopter cannot embed. `parse()` — the convenience
//! that reads the process's own arguments — is the one place that prints and exits.

use std::ffi::OsStr;

use usage_argv::Error;
use usage_derive::{Args, Cli, Subcommands};

/// A command with its own flags, to be asked about.
#[derive(Args)]
struct Ls {
    /// Do not print a header
    #[usage(long)]
    no_header: bool,
}

#[derive(Subcommands)]
enum Commands {
    /// List things
    Ls(Box<Ls>),
}

/// A tool whose help is worth asking for
#[derive(Cli)]
#[usage(bin = "ex")]
struct Ex {
    /// Be loud
    #[usage(long, short = 'v')]
    verbose: bool,
    #[usage(subcommand)]
    command: Option<Commands>,
}

fn ask(tokens: &[&str]) -> (bool, String) {
    let argv: Vec<&OsStr> = tokens.iter().map(OsStr::new).collect();
    match Ex::parse_from(&argv) {
        Err(Error::Help { cmd, long }) => (
            long,
            usage_argv::help::render(Ex::spec(), cmd, long).expect("the command is this CLI's"),
        ),
        Err(other) => panic!("expected a help request, got {other:?}"),
        Ok(_) => panic!("expected a help request, not a parse"),
    }
}

#[test]
fn the_long_and_short_forms_ask_for_different_pages() {
    let (long, page) = ask(&["--help"]);
    assert!(long, "`--help` asks for the long form, as clap has it");
    assert!(page.contains("\nUsage: ex"), "{page}");

    let (long, short_page) = ask(&["-h"]);
    assert!(!long, "`-h` asks for the short one");
    assert_ne!(
        page, short_page,
        "the two forms differ, or there was no reason to tell them apart"
    );
}

#[test]
fn an_explicit_root_synopsis_is_used_by_both_help_forms() {
    #[derive(Cli)]
    #[usage(
        bin = "forms",
        usage = "Usage: forms <COMMAND>\n       forms --print-spec"
    )]
    #[allow(dead_code)]
    struct Forms {
        #[usage(subcommand)]
        command: Commands,
    }

    for flag in ["-h", "--help"] {
        let argv = [OsStr::new(flag)];
        let Err(Error::Help { cmd, long }) = Forms::parse_from(&argv) else {
            panic!("{flag} should request help");
        };
        let page = usage_argv::help::render(Forms::spec(), cmd, long).expect("the root page");
        assert!(
            page.contains("Usage: forms <COMMAND>\n       forms --print-spec"),
            "{page}"
        );
    }
}

#[test]
fn help_is_asked_about_the_command_the_words_reached() {
    // `ex ls --help` is a question about `ls`, not about `ex` — which is why the request carries
    // the command in scope rather than the root.
    let (_, page) = ask(&["ls", "--help"]);
    assert!(page.contains("\nUsage: ex ls"), "{page}");
    assert!(page.contains("--no-header"), "{page}");

    // The root lists it too, inside `ls`'s own usage line in the commands section — so what
    // says the flag is not the *root's* is that it has no entry in the root's flags.
    let (_, root) = ask(&["--help"]);
    assert!(
        !root.contains("\n  --no-header"),
        "the root has no such flag of its own: {root}"
    );
}

#[test]
fn asking_for_help_does_not_stop_the_cli_working() {
    // The fixture parses as any CLI does when nobody asks for help, which is also what keeps its
    // fields read: a test CLI nobody parses is dead code, and CI denies warnings.
    let argv = [
        OsStr::new("-v"),
        OsStr::new("ls"),
        OsStr::new("--no-header"),
    ];
    let parsed = Ex::parse_from(&argv).expect("no help was asked for");
    assert!(parsed.verbose);
    let Some(Commands::Ls(ls)) = parsed.command else {
        panic!("expected ls")
    };
    assert!(ls.no_header);
}

#[test]
fn a_help_flag_is_listed_but_still_not_declared() {
    // The split that replaced "a page lists exactly what its spec declares". The page names
    // `-h, --help`, because help is written for people and someone looking for how to ask for
    // help should find it there. The *spec* still does not declare it: the parser supplies it,
    // and a spec claiming otherwise would have every reader inventing a flag its CLI never
    // declared.
    let (_, page) = ask(&["--help"]);
    assert!(page.contains("-h, --help"), "{page}");
    assert!(page.contains("Print help"), "{page}");

    let kdl = Ex::to_kdl();
    assert!(!kdl.contains("help\""), "{kdl}");
}

#[test]
fn a_cli_that_declares_its_own_help_keeps_it() {
    // Recognised *after* the command's own flags, so declaring one is still possible — and then
    // it binds as any other flag does rather than stopping the parse.
    #[derive(Cli)]
    #[usage(bin = "own")]
    struct Own {
        /// A help of its own
        #[usage(long = "help", short = 'h')]
        help: bool,
    }

    let argv = [OsStr::new("--help")];
    let parsed = Own::parse_from(&argv).expect("the CLI's own flag, not a help request");
    assert!(parsed.help);

    let argv = [OsStr::new("-h")];
    let parsed = Own::parse_from(&argv).expect("the short form too");
    assert!(parsed.help);
}

#[test]
fn help_wins_over_what_would_otherwise_be_an_error() {
    // `ex --help` with a required argument missing still prints help: the request is answered
    // while parsing, before anything is judged. Anything else would make `--help` useless for
    // the person who needs it most — someone who does not yet know what to type.
    #[derive(Cli)]
    #[usage(bin = "strict")]
    struct Strict {
        /// Required
        #[usage(long)]
        file: String,
    }

    let argv = [OsStr::new("--help")];
    assert!(matches!(
        Strict::parse_from(&argv),
        Err(Error::Help { long: true, .. })
    ));

    // And with the argument given, the same CLI parses — which is what says the help request
    // was answered early rather than the requirement never having been there.
    let argv = [OsStr::new("--file"), OsStr::new("mise.toml")];
    let parsed = Strict::parse_from(&argv).expect("nothing missing");
    assert_eq!(parsed.file, "mise.toml");
}

/// A tree deep enough to ask about a nested command.
#[derive(Args)]
struct Set {
    /// Which key
    #[usage(arg, name = "KEY")]
    key: Option<String>,
}

#[derive(Subcommands)]
enum ConfigCommands {
    /// Set a value
    Set(Box<Set>),
}

#[derive(Args)]
struct Config {
    #[usage(subcommand)]
    command: Option<ConfigCommands>,
}

#[derive(Subcommands)]
enum DeepCommands {
    /// Manage config
    #[usage(alias = "cfg")]
    Config(Box<Config>),
}

#[derive(Cli)]
#[usage(bin = "deep")]
struct Deep {
    #[usage(subcommand)]
    command: Option<DeepCommands>,
}

fn ask_deep(tokens: &[&str]) -> String {
    let argv: Vec<&OsStr> = tokens.iter().map(OsStr::new).collect();
    match Deep::parse_from(&argv) {
        Err(Error::Help { cmd, long }) => {
            assert!(long, "the `help` command gives the fuller answer");
            usage_argv::help::render(Deep::spec(), cmd, long).expect("this CLI's own command")
        }
        Err(other) => panic!("expected a help request, got {other:?}"),
        Ok(_) => panic!("expected a help request, not a parse"),
    }
}

/// The `Usage:` line of a page — which command it is about, said exactly.
fn usage_line_of(page: &str) -> &str {
    page.lines()
        .find_map(|l| l.strip_prefix("Usage: "))
        .expect("every page has a usage line")
}

#[test]
fn the_help_command_answers_about_the_command_it_names() {
    // What the page itself advertises — "help  Print this message or the help of the given
    // subcommand(s)" — and what it did not do until now.
    assert_eq!(usage_line_of(&ask_deep(&["help"])), "deep <SUBCOMMAND>");
    assert_eq!(
        usage_line_of(&ask_deep(&["help", "config"])),
        "deep config <SUBCOMMAND>"
    );

    // Every name a command answers to, since `help` is asking about the command rather than
    // about the word: `ex help cfg` is a question about `config`.
    assert_eq!(
        usage_line_of(&ask_deep(&["help", "cfg"])),
        "deep config <SUBCOMMAND>"
    );

    // The whole path, not just the first word.
    let page = ask_deep(&["help", "config", "set"]);
    assert_eq!(usage_line_of(&page), "deep config set [KEY]");
    assert!(page.contains("Which key"), "{page}");
}

#[test]
fn help_stops_at_a_word_that_names_no_command() {
    // `deep help config nonsense` answers about `config` rather than failing: the words after
    // `help` are a question, and the most useful answer to a half-recognised one is the page
    // for as far as it got.
    let page = ask_deep(&["help", "config", "nonsense"]);
    assert_eq!(usage_line_of(&page), "deep config <SUBCOMMAND>");
}

#[test]
fn a_leaf_command_has_no_help_command() {
    // The page prints that line only where there are subcommands, so `help` is a word like any
    // other to a command that has none — here, `set`'s own `KEY`.
    let argv = [OsStr::new("config"), OsStr::new("set"), OsStr::new("help")];
    let parsed = Deep::parse_from(&argv).expect("a word, not a request");
    let Some(DeepCommands::Config(config)) = parsed.command else {
        panic!("expected config")
    };
    let Some(ConfigCommands::Set(set)) = config.command else {
        panic!("expected set")
    };
    assert_eq!(set.key.as_deref(), Some("help"));
}

/// A CLI that means something else by `help` — the command counterpart of a declared
/// `--help` flag.
#[derive(Args)]
struct OwnHelp {
    /// What to look up
    #[usage(arg, name = "TOPIC")]
    topic: Option<String>,
}

#[derive(Subcommands)]
enum OwnCommands {
    /// Open the manual
    Help(Box<OwnHelp>),
}

#[derive(Cli)]
#[usage(bin = "own")]
struct OwnCli {
    #[usage(subcommand)]
    command: Option<OwnCommands>,
}

#[test]
fn a_cli_that_declares_its_own_help_command_keeps_it() {
    // Asked after the subcommand lookup, so a CLI that declares `help` still gets its own
    // command and its own arguments, as one declaring `--help` keeps that.
    let argv = [OsStr::new("help"), OsStr::new("topic")];
    let parsed = OwnCli::parse_from(&argv).expect("the CLI's own command, not a help request");
    let Some(OwnCommands::Help(help)) = parsed.command else {
        panic!("expected the declared help command")
    };
    assert_eq!(help.topic.as_deref(), Some("topic"));
}

#[test]
fn the_page_advertises_exactly_where_the_word_works() {
    // The page prints "help  Print this message…" at the end of its Commands section, so the
    // word has to work wherever that line appears and nowhere else — a page promising a
    // command that does nothing, or a command no page mentions, are the same defect twice.
    let spec = Deep::spec();
    let root_page = usage_argv::help::short_help(spec, &["deep"], &[spec.root]);
    assert!(
        root_page.contains("\n  help  Print this message"),
        "{root_page}"
    );

    let config = spec.root.subcommands[0];
    let set = config.subcommands[0];
    // The whole chain, `config` included. A gap in it is a gap in what the page can see: the
    // globals `config` declares would go unlisted, and an ancestry regression would pass.
    let leaf_page =
        usage_argv::help::short_help(spec, &["deep", "config", "set"], &[spec.root, config, set]);
    assert!(
        !leaf_page.contains("help  Print this message"),
        "a leaf promises nothing: {leaf_page}"
    );
}

/// A command whose page should be about *it*
#[derive(usage_derive::Args)]
struct Deploy {
    /// Which environment
    #[usage(long)]
    env: Option<String>,
}

#[derive(usage_derive::Subcommands)]
enum Which {
    /// Ship the current build somewhere
    Deploy(Box<Deploy>),
}

/// A tool that does several things
#[derive(usage_derive::Cli)]
#[usage(name = "tool", bin = "tool", version = "2.0")]
struct Described {
    #[usage(subcommand)]
    command: Option<Which>,
}

#[test]
fn a_commands_page_says_what_that_command_does() {
    // The question `tool deploy --help` asks is "what does deploy do", and the answer used to
    // be the program's own description — every page carried the root's banner and about, so a
    // subcommand page never once said what the subcommand was for. clap prints the command's
    // own description here.
    let root = Described::spec().root.cmd;
    let deploy = root
        .subcommands
        .iter()
        .find(|c| c.name == "deploy")
        .expect("deploy");

    for long in [false, true] {
        let page = usage_argv::help::render(Described::spec(), deploy, long).expect("a page");
        assert!(
            page.starts_with("Ship the current build somewhere\n"),
            "long={long}: {page}"
        );
        // And not the program's banner, which belongs to the program's page.
        assert!(!page.contains("tool 2.0"), "long={long}: {page}");
        assert!(
            !page.contains("A tool that does several things"),
            "long={long}: {page}"
        );
    }
}

#[test]
fn the_programs_own_page_still_introduces_the_program() {
    // The other half: the root has no command of its own to describe, so it keeps the banner
    // and the program's description.
    for long in [false, true] {
        let page = usage_argv::help::render(Described::spec(), Described::spec().root.cmd, long)
            .expect("a page");
        // `name`, not `bin` — usage-lib's rule, which is why the fixture declares one: a
        // struct called `Described` would otherwise banner itself as `described`.
        assert!(page.starts_with("tool 2.0\n"), "long={long}: {page}");
        assert!(
            page.contains("A tool that does several things"),
            "long={long}: {page}"
        );
    }
}

#[test]
fn the_described_fields_are_bound() {
    use std::ffi::OsStr;
    let argv = ["deploy", "--env", "prod"].map(OsStr::new);
    let parsed = Described::parse_from(&argv).expect("should parse");
    let Some(Which::Deploy(d)) = parsed.command else {
        panic!("expected deploy")
    };
    assert_eq!(d.env.as_deref(), Some("prod"));
}
