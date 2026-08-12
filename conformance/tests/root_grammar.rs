//! The command-level properties mise patches into its spec by hand.
//!
//! `src/cli/usage.rs` exists because clap cannot say any of this: it clears `run`'s
//! arguments, adds a mount of `mise tasks --usage`, and sets `:::` as the restart token,
//! after the spec has been generated. Declared here instead, they travel with the code that
//! defines the command — which is the point of the spec being emitted rather than patched.
//!
//! Two of the three are emission-only for a parser: a mount costs a subprocess and belongs
//! to completions, which is the cold path, and a restart token is read by whoever splits an
//! invocation into several.
//!
//! `default_subcommand` is not in that category: it decides where a word goes, so the parser
//! reads it. `mise build` means `mise run build`, and the word binds to *run's* argument even
//! where the root declares one of its own.

use usage::Spec as LibSpec;
use usage_derive::{Args, Cli, Subcommands};

/// Run task(s)
#[derive(Args)]
#[usage(restart_token = ":::", mount = "mise tasks --usage")]
struct Run {
    /// Do not actually run anything
    #[usage(long)]
    dry_run: bool,
    /// The task to run
    #[usage(arg, name = "TASK")]
    task: Option<String>,
}

/// List things
#[derive(Args)]
struct Ls {
    /// Show everything
    #[usage(long)]
    all: bool,
}

#[derive(Subcommands)]
enum Commands {
    /// Run task(s)
    Run(Box<Run>),
    /// List things
    Ls(Box<Ls>),
}

/// A tool whose bare invocation means one of its commands
#[derive(Cli)]
#[usage(bin = "mise", default_subcommand = "run")]
struct Cli_ {
    /// Task to run
    #[usage(arg, name = "TASK")]
    task: Option<String>,
    #[usage(subcommand)]
    command: Option<Commands>,
}

#[test]
fn the_root_says_what_a_bare_invocation_means() {
    let spec: LibSpec = Cli_::to_kdl().parse().expect("valid spec");
    assert_eq!(spec.default_subcommand.as_deref(), Some("run"));
}

#[test]
fn a_command_carries_its_mount_and_restart_token() {
    let spec: LibSpec = Cli_::to_kdl().parse().expect("valid spec");
    let run = spec.cmd.subcommands.get("run").expect("run");
    assert_eq!(run.restart_token.as_deref(), Some(":::"));
    assert_eq!(
        run.mounts
            .iter()
            .map(|m| m.run.as_str())
            .collect::<Vec<_>>(),
        ["mise tasks --usage"]
    );

    // Only where declared: a sibling gets neither.
    let ls = spec.cmd.subcommands.get("ls").expect("ls");
    assert!(ls.restart_token.is_none());
    assert!(ls.mounts.is_empty());
}

#[test]
fn a_mount_is_not_consulted_while_parsing() {
    use std::ffi::OsStr;

    // A mount is not consulted while parsing — it would cost a subprocess — so `run`'s own
    // flags still bind and an unknown word is still just a word.
    let argv = [OsStr::new("run"), OsStr::new("--dry-run")];
    let parsed = Cli_::parse_from(&argv).expect("should parse");
    let Some(Commands::Run(run)) = parsed.command else {
        panic!("expected run")
    };
    assert!(run.dry_run);

    // The sibling that declares none of the three binds exactly the same way, which is the
    // other half of "per-command": the properties are absent from `ls` and their absence
    // costs it nothing.
    let argv = [OsStr::new("ls"), OsStr::new("--all")];
    let parsed = Cli_::parse_from(&argv).expect("should parse");
    let Some(Commands::Ls(ls)) = parsed.command else {
        panic!("expected ls")
    };
    assert!(ls.all);
}

#[test]
fn a_default_subcommand_routes_a_word_that_names_nothing() {
    use std::ffi::OsStr;

    // What `default_subcommand` is for, and what mise routes by hand today: `mise build`
    // means `mise run build`. The word names no subcommand, so the parser descends into
    // `run` and the word is re-examined there — landing on **run's** argument, not the
    // root's, even though the root declares one. Matches usage-lib, which is where the
    // behaviour was read from rather than invented.
    let argv = [OsStr::new("build")];
    let parsed = Cli_::parse_from(&argv).expect("should parse");
    let Some(Commands::Run(run)) = parsed.command else {
        panic!("a word naming nothing should have reached `run`")
    };
    assert_eq!(run.task.as_deref(), Some("build"), "bound by `run`");
    assert!(
        parsed.task.is_none(),
        "the root's own argument must not have taken it"
    );

    // A word that does name a command still selects it: the default is only for the words
    // that name nothing.
    let argv = [OsStr::new("ls"), OsStr::new("--all")];
    let parsed = Cli_::parse_from(&argv).expect("should parse");
    let Some(Commands::Ls(ls)) = parsed.command else {
        panic!("expected ls")
    };
    assert!(ls.all);
}

#[test]
fn the_name_is_resolved_when_the_program_is_compiled() {
    // `default_subcommand` names a command whose declaration is in another macro expansion,
    // so the derive cannot look it up — but a `const fn` can search the subcommand list
    // during const evaluation, which makes a name that answers to nothing a compile error.
    // Nothing to assert at run time beyond the resolution having happened: the pointer is
    // the same static the subcommand list holds.
    let root = Cli_::command();
    let default = root.default_subcommand.expect("declared");
    assert_eq!(default.name, "run");
    assert!(
        root.subcommands
            .iter()
            .any(|sub| ::core::ptr::eq(*sub, default)),
        "resolved to the table's own entry rather than a copy of it"
    );
}
