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
fn a_help_flag_is_not_in_the_spec_it_renders() {
    // The two are supplied by the parser and belong to no command's metadata, so they cannot
    // appear in help output or in the emitted spec. A page advertising a `--help` that the spec
    // it came from does not declare is a page that disagrees with its own CLI.
    let (_, page) = ask(&["--help"]);
    assert!(!page.contains("--help"), "{page}");
    assert!(!page.contains("-h "), "{page}");

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
