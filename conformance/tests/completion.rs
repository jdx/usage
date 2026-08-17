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

#[derive(Cli)]
#[usage(bin = "hinted", completion)]
struct Hinted {
    /// A file to read
    #[usage(long, value_hint = usage_argv::ValueHint::FilePath)]
    file: Option<std::path::PathBuf>,
    /// A directory to write
    #[usage(long, value_hint = usage_argv::ValueHint::DirPath)]
    dir: Option<std::path::PathBuf>,
}

fn ask_hinted(line: &str) -> String {
    let argv: Vec<OsString> = ["__complete_word__", "--shell", "bash", "--line", line]
        .iter()
        .map(OsString::from)
        .collect();
    Hinted::completion_request(&argv).expect("this is a completion request")
}

#[test]
fn usage_path_value_hints_reach_native_and_emitted_completions() {
    assert_eq!(
        ask_hinted("hinted --file "),
        format!("{}\n", usage_argv::complete::FILES_MARKER)
    );
    assert_eq!(
        ask_hinted("hinted --dir "),
        format!("{}\n", usage_argv::complete::DIRS_MARKER)
    );

    let kdl = Hinted::to_kdl();
    assert!(kdl.contains("complete \"file\" type=\"path\""), "{kdl}");
    assert!(kdl.contains("complete \"dir\" type=\"dir\""), "{kdl}");

    let argv = [OsStr::new("--file"), OsStr::new("input.kdl")];
    let parsed = Hinted::parse_from(&argv).expect("the hinted flag still parses");
    assert_eq!(
        parsed.file.as_deref(),
        Some(std::path::Path::new("input.kdl"))
    );
    assert!(parsed.dir.is_none());
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

/// A CLI that says it does not want completions, in the spelling that would once have been read
/// as wanting them.
#[derive(Cli)]
#[usage(bin = "off", completion = false)]
struct Off {
    /// Say more
    #[usage(long = "verbose")]
    verbose: bool,
}

/// The same name the derive would have generated, which is how the absence is asserted.
///
/// Rust refuses two inherent methods with one name on one type, so if `completion = false` were
/// read as opting in, this impl would collide with the generated one and the crate would not
/// build. A test can assert what is *there*; making it assert what is *not* takes a collision.
impl Off {
    fn completion_request(_argv: &[std::ffi::OsString]) -> Option<String> {
        Some("written by hand".to_string())
    }
}

#[test]
fn saying_false_is_taken_as_false() {
    // The attribute went through a path match, so any form carrying the word opted the CLI in —
    // `completion = false` included, which is the one spelling whose whole point is not to.
    assert_eq!(
        Off::completion_request(&[]).as_deref(),
        Some("written by hand"),
        "the derive generated a completion request for a CLI that declined one"
    );

    let parsed = Off::parse_from(&[std::ffi::OsStr::new("--verbose")]).expect("an ordinary parse");
    assert!(parsed.verbose);

    // And the CLI that does ask for it still answers.
    assert!(Ex::completion_request(&[std::ffi::OsString::from("__complete_word__")]).is_some());
    assert!(!Ex::completion_script(usage_argv::complete::Shell::Bash).is_empty());
}

#[test]
fn the_script_calls_the_command_this_cli_answers() {
    use usage_argv::complete::Shell;

    // The pair has to agree, which is why both are emitted under one attribute: a script naming
    // a command the binary does not answer is a silence at the prompt, and the only way to find
    // it is for a user to press Tab.
    for shell in [
        Shell::Bash,
        Shell::Zsh,
        Shell::Fish,
        Shell::Nu,
        Shell::PowerShell,
    ] {
        let script = Ex::completion_script(shell);
        assert!(
            script.contains("ex __complete_word__"),
            "{shell:?} script does not call this CLI: {script}"
        );
        assert!(script.contains(shell.as_str()), "{shell:?}");
    }

    // And the request that script makes is one this CLI answers, rather than one that merely
    // looks like it — the two halves checked against each other rather than each alone.
    let argv: Vec<OsString> = [
        "__complete_word__",
        "--shell",
        "bash",
        "--line",
        "ex ins",
        "--cursor",
        "6",
    ]
    .iter()
    .map(OsString::from)
    .collect();
    assert_eq!(Ex::completion_request(&argv).as_deref(), Some("install\n"));
}

/// A CLI whose task list depends on a file named earlier on the same line.
///
/// This is the case a `run=` cannot answer: `mise tasks ls --complete` is a fixed command, so it
/// reads whatever config the shell happens to be standing in, not the one being typed about.
#[derive(Args)]
struct Tasks {
    /// Which config to read
    #[usage(long = "file")]
    file: Option<String>,
    /// Only tasks in this group
    #[usage(long = "group", value_name = "GROUP", complete = groups)]
    group: Option<String>,
    /// Which task
    #[usage(arg, name = "TASK", complete = tasks_in_file)]
    task: Option<String>,
}

/// What answers for `TASK`, given what the command has been told so far.
fn tasks_in_file(
    partial: &<Tasks as usage_argv::spec::CommandArgs>::Partial,
    _ctx: &usage_argv::complete::CompleteCtx<'_>,
) -> Vec<usage_argv::complete::Candidate<'static>> {
    // The parser already decided what `--file` means; this reads the decision rather than
    // scraping the line for it again. The partial holds the bytes as typed, since a word that is
    // not valid UTF-8 is still a word somebody wrote.
    let file = partial
        .file
        .as_deref()
        .map(|bytes| String::from_utf8_lossy(bytes).into_owned());
    match file.as_deref() {
        Some("other.toml") => vec![usage_argv::complete::Candidate::new("other-task")],
        Some(_) => vec![],
        None => vec![
            usage_argv::complete::Candidate::described("build", "Build it"),
            usage_argv::complete::Candidate::new("test"),
        ],
    }
}

/// What answers for `--group`, to show a flag's value is completed the same way.
fn groups(
    _partial: &<Tasks as usage_argv::spec::CommandArgs>::Partial,
    _ctx: &usage_argv::complete::CompleteCtx<'_>,
) -> Vec<usage_argv::complete::Candidate<'static>> {
    vec![
        usage_argv::complete::Candidate::new("ci"),
        usage_argv::complete::Candidate::new("local"),
    ]
}

#[derive(Subcommands)]
enum WithTasks {
    /// List tasks
    Tasks(Box<Tasks>),
}

#[derive(Cli)]
#[usage(bin = "tk", completion)]
struct Tk {
    #[usage(subcommand)]
    command: Option<WithTasks>,
}

fn tk(line: &str) -> String {
    let argv: Vec<OsString> = ["__complete_word__", "--shell", "bash", "--line", line]
        .iter()
        .map(OsString::from)
        .collect();
    Tk::completion_request(&argv).expect("a completion request")
}

#[test]
fn a_completer_reads_its_own_commands_half_parsed_struct() {
    // Without a flag, the defaults.
    assert_eq!(tk("tk tasks "), "build\ntest\n");
    assert_eq!(tk("tk tasks bu"), "build\n");

    // A flag's value is answered the same way as an argument's.
    assert_eq!(tk("tk tasks --group "), "ci\nlocal\n");
    assert_eq!(tk("tk tasks --group c"), "ci\n");

    // And with one given earlier on the same line, the answer changes — which is the thing a
    // fixed `run=` command cannot do, because it never sees the line it is being asked about.
    assert_eq!(tk("tk tasks --file other.toml "), "other-task\n");
    // A file it knows nothing about: no candidates, and the position stays open — so what comes
    // back is the marker asking the shell for paths, not an empty answer claiming there is
    // nothing here.
    assert_eq!(
        tk("tk tasks --file nothing.toml "),
        format!("{}\n", usage_argv::complete::FILES_MARKER)
    );
}

#[test]
fn the_cli_that_declares_completers_still_parses() {
    // The fields the completers describe are the fields a parse fills, which is the whole claim:
    // one declaration, read by both.
    let argv = [
        OsStr::new("tasks"),
        OsStr::new("--file"),
        OsStr::new("other.toml"),
        OsStr::new("--group"),
        OsStr::new("ci"),
        OsStr::new("build"),
    ];
    let parsed = Tk::parse_from(&argv).expect("an ordinary parse");
    let Some(WithTasks::Tasks(tasks)) = parsed.command else {
        panic!("expected tasks")
    };
    assert_eq!(tasks.file.as_deref(), Some("other.toml"));
    assert_eq!(tasks.group.as_deref(), Some("ci"));
    assert_eq!(tasks.task.as_deref(), Some("build"));
}

#[test]
fn the_spec_names_a_completer_the_binary_answers() {
    // What the KDL promises and what the binary does, checked against each other.
    let kdl = Tk::to_kdl();
    // Inside the command that declares it, and carrying the line, so that whoever runs it asks
    // about the same thing a native request would.
    assert!(
        kdl.contains(r#"complete "task" run="tk __complete_word__ --candidates task --line"#),
        "{kdl}"
    );
    assert!(kdl.contains(r#"complete "group""#), "{kdl}");

    let argv: Vec<OsString> = [
        "__complete_word__",
        "--shell",
        "bash",
        "--candidates",
        "task",
        "--line",
        "tk tasks --file other.toml ",
    ]
    .iter()
    .map(OsString::from)
    .collect();
    assert_eq!(
        Tk::completion_request(&argv).as_deref(),
        Some("other-task\n"),
        "the command the spec names has to answer, and to see the same line"
    );
}

/// Completers reached by every spelling a user might write one with.
mod completers {
    use usage_argv::complete::{Candidate, CompleteCtx};

    pub fn absolute(
        _partial: &<super::Qualified as usage_argv::spec::CommandArgs>::Partial,
        _ctx: &CompleteCtx<'_>,
    ) -> Vec<Candidate<'static>> {
        vec![Candidate::new("from-crate")]
    }

    pub fn relative(
        _partial: &<super::Qualified as usage_argv::spec::CommandArgs>::Partial,
        _ctx: &CompleteCtx<'_>,
    ) -> Vec<Candidate<'static>> {
        vec![Candidate::new("from-self")]
    }
}

#[derive(Args)]
struct Qualified {
    /// Named by a path rooted at the crate
    #[usage(arg, name = "ONE", complete = crate::completers::absolute)]
    one: Option<String>,
    /// Named by a path relative to this module
    #[usage(long = "two", value_name = "TWO", complete = self::completers::relative)]
    two: Option<String>,
}

#[derive(Subcommands)]
enum QualifiedCommands {
    /// Take both
    Both(Box<Qualified>),
}

#[derive(Cli)]
#[usage(bin = "q", completion)]
struct Q {
    #[usage(subcommand)]
    command: Option<QualifiedCommands>,
}

#[test]
fn a_completer_can_be_named_however_a_path_is_written() {
    // A path is rewritten the way a field's *type* is, because the generated module sits one
    // level below where the user wrote it — and `crate::…` or `self::…` mean something already,
    // so prefixing them produced a path that did not resolve. This test is mostly the fact that
    // it compiles; the assertions are what makes it worth reading.
    let argv: Vec<OsString> = ["__complete_word__", "--shell", "bash", "--line", "q both "]
        .iter()
        .map(OsString::from)
        .collect();
    assert_eq!(
        Q::completion_request(&argv).as_deref(),
        Some("from-crate\n")
    );

    let argv: Vec<OsString> = [
        "__complete_word__",
        "--shell",
        "bash",
        "--line",
        "q both --two ",
    ]
    .iter()
    .map(OsString::from)
    .collect();
    assert_eq!(Q::completion_request(&argv).as_deref(), Some("from-self\n"));

    // The fields the completers describe are the fields a parse fills.
    let argv = [
        OsStr::new("both"),
        OsStr::new("--two"),
        OsStr::new("b"),
        OsStr::new("a"),
    ];
    let parsed = Q::parse_from(&argv).expect("an ordinary parse");
    let Some(QualifiedCommands::Both(both)) = parsed.command else {
        panic!("expected both")
    };
    assert_eq!(both.one.as_deref(), Some("a"));
    assert_eq!(both.two.as_deref(), Some("b"));
}

/// A command that declares a global flag with a completer, completed inside its *child*.
///
/// The case a wrapper gets wrong if it reparses the deepest command's words: a global belongs to
/// an ancestor, and the words that reached the child are not the words the ancestor was given.
#[derive(Args)]
struct Leaf {
    /// Something of the leaf's own
    #[usage(arg, name = "WHAT")]
    what: Option<String>,
}

#[derive(Subcommands)]
enum GlobalCommands {
    /// A subcommand
    Leaf(Box<Leaf>),
}

#[derive(Args)]
struct Outer {
    /// Where to work
    #[usage(long = "cd", value_name = "DIR", global)]
    cd: Option<String>,
    /// Which profile, answered against `--cd`
    #[usage(long = "profile", value_name = "PROFILE", global, complete = profiles)]
    profile: Option<String>,
    #[usage(subcommand)]
    command: Option<GlobalCommands>,
}

#[derive(Subcommands)]
enum OuterCommands {
    /// The command that declares the globals
    Outer(Box<Outer>),
}

#[derive(Cli)]
#[usage(bin = "g", completion)]
struct G {
    #[usage(subcommand)]
    command: Option<OuterCommands>,
}

/// What answers for `--profile`: `Outer`'s own partial, which is where `--cd` landed.
fn profiles(
    partial: &<Outer as usage_argv::spec::CommandArgs>::Partial,
    _ctx: &usage_argv::complete::CompleteCtx<'_>,
) -> Vec<usage_argv::complete::Candidate<'static>> {
    let cd = partial
        .cd
        .as_deref()
        .map(|bytes| String::from_utf8_lossy(bytes).into_owned());
    match cd.as_deref() {
        Some(dir) => vec![usage_argv::complete::Candidate::new(format!("in-{dir}"))],
        None => vec![usage_argv::complete::Candidate::new("default")],
    }
}

#[test]
fn a_global_flags_completer_reads_the_command_it_was_declared_on() {
    // `--cd here` was given to `outer`, and the cursor is inside `outer leaf` — so the words that
    // reached the deepest command do not contain it. A wrapper reparsing those would have found
    // nothing and answered as though the flag had never been typed.
    assert_eq!(tk_g("g outer --cd here leaf --profile "), "in-here\n");

    // And where the two coincide, the same answer.
    assert_eq!(tk_g("g outer --cd there --profile "), "in-there\n");
    assert_eq!(tk_g("g outer leaf --profile "), "default\n");
}

/// A flattened group has fields of its own, but no command of its own on the active path.
#[derive(Args)]
struct FlattenedGlobals {
    /// Where to work
    #[usage(long = "workspace", value_name = "DIR", global)]
    workspace: Option<String>,
    /// Which profile, answered against `--workspace`
    #[usage(
        long = "flat-profile",
        value_name = "PROFILE",
        global,
        complete = flattened_profiles
    )]
    profile: Option<String>,
}

#[derive(Args)]
struct FlattenedOuter {
    #[usage(flatten)]
    globals: FlattenedGlobals,
    #[usage(subcommand)]
    command: Option<GlobalCommands>,
}

#[derive(Subcommands)]
enum FlattenedCommands {
    /// The command containing the flattened globals
    Outer(Box<FlattenedOuter>),
}

#[derive(Cli)]
#[usage(bin = "fg", completion)]
struct FG {
    #[usage(subcommand)]
    command: Option<FlattenedCommands>,
}

fn flattened_profiles(
    partial: &<FlattenedGlobals as usage_argv::spec::CommandArgs>::Partial,
    _ctx: &usage_argv::complete::CompleteCtx<'_>,
) -> Vec<usage_argv::complete::Candidate<'static>> {
    let workspace = partial
        .workspace
        .as_deref()
        .map(|bytes| String::from_utf8_lossy(bytes).into_owned());
    vec![usage_argv::complete::Candidate::new(
        workspace.unwrap_or_else(|| "default".to_string()),
    )]
}

#[test]
fn a_flattened_global_completer_reads_its_parent_commands_words() {
    // The flattened type's command key is not on the path. Its fields are in `outer`, so that
    // command and its complete word slice are what the wrapper must reparse.
    assert_eq!(
        tk_fg("fg outer --workspace before leaf --flat-profile "),
        "before\n"
    );

    // Reparse the actual parent command as well as its words: the flattened type does not know
    // `leaf`, so reparsing only its own table would stop there and miss this later global.
    assert_eq!(
        tk_fg("fg outer leaf --workspace after --flat-profile "),
        "after\n"
    );

    // The declarations used for completion are still the ones an ordinary parse fills.
    let argv = [
        OsStr::new("outer"),
        OsStr::new("leaf"),
        OsStr::new("--workspace"),
        OsStr::new("after"),
        OsStr::new("--flat-profile"),
        OsStr::new("selected"),
    ];
    let parsed = FG::parse_from(&argv).expect("an ordinary parse");
    let Some(FlattenedCommands::Outer(outer)) = parsed.command else {
        panic!("expected outer")
    };
    assert_eq!(outer.globals.workspace.as_deref(), Some("after"));
    assert_eq!(outer.globals.profile.as_deref(), Some("selected"));
    assert!(matches!(outer.command, Some(GlobalCommands::Leaf(_))));
}

fn tk_fg(line: &str) -> String {
    let argv: Vec<OsString> = ["__complete_word__", "--shell", "bash", "--line", line]
        .iter()
        .map(OsString::from)
        .collect();
    FG::completion_request(&argv).expect("a completion request")
}

fn tk_g(line: &str) -> String {
    let argv: Vec<OsString> = ["__complete_word__", "--shell", "bash", "--line", line]
        .iter()
        .map(OsString::from)
        .collect();
    G::completion_request(&argv).expect("a completion request")
}

#[test]
fn the_cli_with_globals_still_parses_them() {
    // The fields those completers read are the fields a parse fills — one declaration, read by
    // both, which is the whole point of the completer living on the field.
    let argv = [
        OsStr::new("outer"),
        OsStr::new("--cd"),
        OsStr::new("here"),
        OsStr::new("leaf"),
        OsStr::new("--profile"),
        OsStr::new("work"),
        OsStr::new("thing"),
    ];
    let parsed = G::parse_from(&argv).expect("an ordinary parse");
    let Some(OuterCommands::Outer(outer)) = parsed.command else {
        panic!("expected outer")
    };
    assert_eq!(outer.cd.as_deref(), Some("here"));
    assert_eq!(outer.profile.as_deref(), Some("work"));
    let Some(GlobalCommands::Leaf(leaf)) = outer.command else {
        panic!("expected leaf")
    };
    assert_eq!(leaf.what.as_deref(), Some("thing"));
}
