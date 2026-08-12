//! The command-level properties mise patches into its spec by hand.
//!
//! `src/cli/usage.rs` exists because clap cannot say any of this: it clears `run`'s
//! arguments, adds a mount of `mise tasks --usage`, and sets `:::` as the restart token,
//! after the spec has been generated. Declared here instead, they travel with the code that
//! defines the command — which is the point of the spec being emitted rather than patched.
//!
//! None of the three changes how a word binds. A mount costs a subprocess and belongs to
//! completions, which is the cold path; a restart token is read by whoever splits an
//! invocation into several; and a default subcommand is what a completion engine consults to
//! decide that a bare `mise build` means `mise run build`.

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
fn none_of_it_changes_how_a_word_binds() {
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

    // And `default_subcommand` is a note for whoever completes the line, not a redirection:
    // a bare word still fills the root's own argument.
    let argv = [OsStr::new("build")];
    let parsed = Cli_::parse_from(&argv).expect("should parse");
    assert_eq!(parsed.task.as_deref(), Some("build"));
    assert!(parsed.command.is_none());
}
