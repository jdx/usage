//! Real mise command lines, parsed by the generated shadow.
//!
//! Kept here rather than beside the generated crate: this is hand-written, and what
//! the generator produces is only worth
//! benchmarking if it parses the same words mise does. These are invocations out of
//! mise's own docs.

use std::ffi::OsStr;

use shadow_mise::{Cli, Commands};

fn argv<const N: usize>(tokens: [&str; N]) -> [&OsStr; N] {
    tokens.map(OsStr::new)
}

#[test]
fn a_tool_is_installed_globally() {
    let a = argv(["use", "-g", "node@20"]);
    let cli = Cli::parse_from(&a).expect("should parse");
    let Some(Commands::Use(use_args)) = cli.command else {
        panic!("expected `use`")
    };
    assert!(use_args.global);
    assert_eq!(use_args.tool_version, ["node@20"]);
}

#[test]
fn a_task_runs_with_arguments_after_a_separator() {
    // The shape that made the derive's validation wrong: `[ARGS]…` before the `--` and
    // `[-- ARGS_LAST]…` after it. `tasks run` is where mise's spec declares it; the
    // top-level `run` carries no positionals of its own — see the note in the PR.
    let a = argv(["tasks", "run", "build", "--dry-run", "--", "--verbose"]);
    let cli = Cli::parse_from(&a).expect("should parse");
    let Some(Commands::Tasks(tasks)) = cli.command else {
        panic!("expected `tasks`")
    };
    assert!(tasks.command.is_some(), "`run` should have been selected");
}

#[test]
fn a_bare_task_lands_on_the_root_positional() {
    // What `default_subcommand run` leaves behind: `mise build -- --verbose` fills the
    // root's own `[TASK]`, with the words after the separator kept apart.
    let a = argv(["build", "--", "--verbose"]);
    let cli = Cli::parse_from(&a).expect("should parse");
    assert_eq!(cli.task.as_deref(), Some("build"));
    assert_eq!(cli.task_args_last, ["--verbose"]);
    assert!(cli.command.is_none(), "`build` is not a subcommand");
}

#[test]
fn a_global_flag_is_accepted_before_or_after_the_command() {
    let before = argv(["-C", "/tmp", "ls", "--installed"]);
    let cli = Cli::parse_from(&before).expect("should parse");
    assert_eq!(cli.cd.as_deref(), Some("/tmp"));

    // Global means the subcommand takes it too, which is how `mise ls -C /tmp` works.
    let after = argv(["ls", "-C", "/tmp"]);
    let cli = Cli::parse_from(&after).expect("should parse");
    let Some(Commands::Ls(_)) = cli.command else {
        panic!("expected `ls`")
    };
}

#[test]
fn a_nested_command_reaches_three_levels() {
    let a = argv(["settings", "set", "experimental", "true"]);
    let cli = Cli::parse_from(&a).expect("should parse");
    let Some(Commands::Settings(settings)) = cli.command else {
        panic!("expected `settings`")
    };
    assert!(
        settings.command.is_some(),
        "`set` should have been selected"
    );
}

#[test]
fn counted_verbosity_accumulates() {
    let a = argv(["-vv", "ls"]);
    let cli = Cli::parse_from(&a).expect("should parse");
    assert_eq!(cli.verbose, 2);
}
