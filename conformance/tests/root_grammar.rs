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
//! `default_subcommand` is *not* in that category, though this derive currently treats it as
//! if it were — usage-lib routes on it while parsing. See
//! `a_default_subcommand_does_not_route_yet`, which records the difference rather than
//! asserting the gap is intended.

use usage::Spec as LibSpec;
use usage_derive::{Args, Cli, Subcommands};

/// Run task(s)
#[derive(Args)]
#[usage(restart_token = ":::", mount = "mise tasks --usage")]
struct Run {
    /// Do not actually run anything
    #[usage(long)]
    dry_run: bool,
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
fn a_default_subcommand_does_not_route_yet() {
    use std::ffi::OsStr;

    // **A recorded divergence, not the intended end state.** usage-lib routes here: given
    // `default_subcommand "run"`, a word that names no subcommand makes its parser descend
    // into `run` and bind the word as *`run`'s* argument, even when the root declares an
    // argument of its own. Verified against usage-lib 4.0.0 directly — `mise build` comes
    // back as commands `["mise", "run"]` with `TASK = "build"`.
    //
    // This parser keeps the word at the root, so `default_subcommand` reaches the spec and
    // nothing more. That is a gap in the derive rather than a decision: routing needs the
    // table to carry a pointer to the default subcommand and the parser to descend on a word
    // that matches none, which is more than emitting a property. Asserted as it stands so
    // the difference is visible and cannot be mistaken for intent.
    //
    // It matters to mise: `mise build` meaning `mise run build` is exactly this rule, and
    // mise routes it by hand today.
    let argv = [OsStr::new("build")];
    let parsed = Cli_::parse_from(&argv).expect("should parse");
    assert_eq!(parsed.task.as_deref(), Some("build"), "bound at the root");
    assert!(
        parsed.command.is_none(),
        "usage-lib would have descended into `run` here"
    );
}
