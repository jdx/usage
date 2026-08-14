//! The hidden command a shell calls, and what it answers.
//!
//! A completion request is a line and a cursor, and the answer comes from the same tables the
//! parse uses. What is checked here is the part between: that a CLI which asks for completion
//! recognizes the request, reads the three arguments a shell passes, and writes the answer the
//! way that shell reads it — and that an ordinary invocation is untouched.

use std::ffi::{OsStr, OsString};
use usage_derive::{Args, Cli, Subcommands};

#[derive(Args)]
struct Install {
    /// Which tool
    #[usage(arg, name = "TOOL", choices("node", "python"))]
    tool: Option<String>,
}

#[derive(Args)]
struct Uninstall {
    /// Which tool
    #[usage(arg, name = "TOOL", choices("node", "python"))]
    tool: Option<String>,
}

#[derive(Args)]
struct Run {
    /// Anything at all
    #[usage(arg, name = "TASK")]
    task: Option<String>,
}

#[derive(Subcommands)]
enum Commands {
    /// Run a task
    Run(Box<Run>),
    /// Install a tool
    Install(Box<Install>),
    /// Remove a tool
    #[usage(alias = "rm")]
    Uninstall(Box<Uninstall>),
}

/// A CLI that answers completions.
#[derive(Cli)]
#[usage(bin = "ex", completion)]
struct Ex {
    /// Say more
    #[usage(long = "verbose", short = 'v')]
    verbose: bool,
    #[usage(subcommand)]
    command: Option<Commands>,
}

fn ask(shell: &str, line: &str) -> String {
    let argv: Vec<OsString> = ["__complete_word__", "--shell", shell, "--line", line]
        .iter()
        .map(OsString::from)
        .collect();
    Ex::completion_request(&argv).expect("this is a completion request")
}

#[test]
fn a_request_is_answered_from_the_same_tables_the_parse_uses() {
    assert_eq!(ask("bash", "ex "), "install\nrm\nrun\nuninstall\n");
    assert_eq!(ask("bash", "ex ins"), "install\n");
    assert_eq!(ask("bash", "ex install "), "node\npython\n");
    assert_eq!(ask("bash", "ex --"), "--verbose\n");
}

#[test]
fn each_shell_gets_the_shape_it_reads() {
    // fish takes a description after a tab; bash does not; zsh takes what to type as well.
    assert_eq!(ask("fish", "ex ins"), "install\tInstall a tool\n");
    assert_eq!(ask("zsh", "ex ins"), "install\tInstall a tool\tinstall\n");
    assert_eq!(ask("bash", "ex ins"), "install\n");
}

#[test]
fn a_cursor_short_of_the_end_completes_the_word_it_is_in() {
    let argv: Vec<OsString> = [
        "__complete_word__",
        "--shell",
        "bash",
        "--line",
        "ex ins --verbose",
        "--cursor",
        "6",
    ]
    .iter()
    .map(OsString::from)
    .collect();
    assert_eq!(
        Ex::completion_request(&argv).expect("a request"),
        "install\n"
    );
}

#[test]
fn an_ordinary_invocation_is_not_a_request() {
    let argv: Vec<OsString> = ["install", "node"].iter().map(OsString::from).collect();
    assert!(Ex::completion_request(&argv).is_none());
    assert!(Ex::completion_request(&[]).is_none());

    // And the CLI still parses what it always did — the hidden command is not in its grammar,
    // so a positional that happens to be that word is still a positional.
    let argv = [OsStr::new("run"), OsStr::new("__complete_word__")];
    let parsed = Ex::parse_from(&argv).expect("an ordinary parse");
    let Some(Commands::Run(run)) = parsed.command else {
        panic!("expected run")
    };
    assert_eq!(run.task.as_deref(), Some("__complete_word__"));

    // The choices still bind where they are declared, which is what makes the candidates above
    // a description of this CLI rather than a list beside it.
    let argv = [OsStr::new("install"), OsStr::new("node")];
    let parsed = Ex::parse_from(&argv).expect("a declared choice");
    let Some(Commands::Install(install)) = parsed.command else {
        panic!("expected install")
    };
    assert_eq!(install.tool.as_deref(), Some("node"));

    let argv = [OsStr::new("--verbose"), OsStr::new("run")];
    let parsed = Ex::parse_from(&argv).expect("a flag the candidates offered");
    assert!(parsed.verbose);

    let argv = [OsStr::new("rm"), OsStr::new("python")];
    let parsed = Ex::parse_from(&argv).expect("under its alias");
    let Some(Commands::Uninstall(uninstall)) = parsed.command else {
        panic!("expected uninstall")
    };
    assert_eq!(uninstall.tool.as_deref(), Some("python"));
}

#[test]
fn a_shell_that_says_nothing_useful_still_gets_an_answer() {
    // A completion that errors out is a shell that beeps at every keystroke, so an unknown
    // argument is ignored and a missing cursor means the end of the line.
    let argv: Vec<OsString> = [
        "__complete_word__",
        "--shell",
        "klingon",
        "--who-knows",
        "x",
        "--line",
        "ex ins",
    ]
    .iter()
    .map(OsString::from)
    .collect();
    assert_eq!(
        Ex::completion_request(&argv).expect("a request"),
        "install\n"
    );
}
