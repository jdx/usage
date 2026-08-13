//! Real mise command lines, parsed by the generated shadow.
//!
//! Kept here rather than beside the generated crate: this is hand-written, and what
//! the generator produces is only worth
//! benchmarking if it parses the same words mise does. These are invocations out of
//! mise's own docs.

use std::ffi::OsStr;

use shadow_mise::{Cli, Commands, TasksCommands};

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
    let a = argv([
        "tasks",
        "run",
        "build",
        "extra",
        "--dry-run",
        "--",
        "--verbose",
    ]);
    let cli = Cli::parse_from(&a).expect("should parse");
    let Some(Commands::Tasks(tasks)) = cli.command else {
        panic!("expected `tasks`")
    };
    let Some(TasksCommands::Run(run)) = tasks.command else {
        panic!("expected `tasks run`")
    };
    // The words, not just that a command was selected: a regression that merged the two
    // sides of the `--` or dropped either would otherwise leave this green.
    assert_eq!(run.task.as_deref(), Some("build"));
    assert_eq!(run.args, ["extra"]);
    assert_eq!(run.args_last, ["--verbose"]);
    assert!(run.dry_run);
}

#[test]
fn a_bare_task_routes_through_run_and_then_needs_the_mount() {
    // `mise build` used to fill the root's own `[TASK]` here, because the derive could not
    // declare `default_subcommand`. Now it can, so the shadow does what mise's spec says:
    // `build` names no subcommand, so the parser descends into `run`.
    //
    // And there it stops, because mise's spec gives `run` no positional arguments at all —
    // `src/cli/usage.rs` clears them and adds `mount run="mise tasks --usage"`, so the task
    // names are supposed to come from running that. usage-argv does not execute mounts (a
    // subprocess mid-parse is not something a hot path should do), so there is nothing for
    // the word to bind to.
    //
    // Worth being plain about, because it bounds what routing alone buys: `mise build`
    // working end to end needs the mount as well, and mise's own hand-rolled routing cannot
    // be deleted on the strength of this rule by itself.
    let a = argv(["build"]);
    match Cli::parse_from(&a) {
        Err(usage_argv::Error::UnexpectedArg { token }) => {
            assert_eq!(
                token, b"build",
                "descended into `run`, which has no argument"
            );
        }
        Err(other) => panic!("wrong error: {other:?}"),
        Ok(_) => panic!("`run` has no positional in mise's spec, so this cannot bind"),
    }

    // The separator behaves the same way: routing happens at the word, and the words after
    // `--` were never candidates for it.
    let a = argv(["build", "--", "--verbose"]);
    assert!(matches!(
        Cli::parse_from(&a),
        Err(usage_argv::Error::UnexpectedArg { .. })
    ));

    // A word that *does* name a command is unaffected, which is the case that matters for
    // every other invocation in this file.
    let a = argv(["ls"]);
    let cli = Cli::parse_from(&a).expect("`ls` names a command");
    assert!(cli.command.is_some());
}

#[test]
fn a_global_flag_is_accepted_before_or_after_the_command() {
    let before = argv(["-C", "/tmp", "ls", "--installed"]);
    let cli = Cli::parse_from(&before).expect("should parse");
    assert_eq!(cli.cd.as_deref(), Some("/tmp"));

    // Global means the subcommand takes it too, which is how `mise ls -C /tmp` works.
    // Asserting the *value* matters here: unknown flag-like words are values by default
    // and `ls` has a variadic positional, so a global that stopped being recognized
    // after the command would still parse — `-C` and `/tmp` would land in the variadic
    // and nothing would complain.
    let after = argv(["ls", "-C", "/tmp"]);
    let cli = Cli::parse_from(&after).expect("should parse");
    assert_eq!(cli.cd.as_deref(), Some("/tmp"));
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

#[test]
fn a_command_that_requires_a_subcommand_refuses_to_stand_alone() {
    // 27 of mise's commands set `subcommand_required`, and the shadow has to answer the
    // same grammar: `mise bootstrap accounts` on its own is an error, not an empty
    // invocation. (`bootstrap` itself does not require one, which is why the shadow has
    // to read the spec rather than assume.)
    // `Cli` is generated without `Debug` — 211 commands' worth of it would be dead
    // weight — so the error is matched rather than unwrapped.
    let a = argv(["bootstrap", "accounts"]);
    match Cli::parse_from(&a) {
        Err(usage_argv::Error::MissingSubcommand) => {}
        Err(other) => panic!("wrong error: {other:?}"),
        Ok(_) => panic!("`bootstrap accounts` needs a subcommand"),
    }

    // With one, it parses.
    let a = argv(["bootstrap", "accounts", "status"]);
    Cli::parse_from(&a).expect("`accounts status` should parse");

    // And the root does not require one: `mise` alone is a whole invocation.
    let a: [&std::ffi::OsStr; 0] = [];
    Cli::parse_from(&a).expect("a bare `mise` should parse");
}

#[test]
fn a_command_answers_to_its_alias() {
    // 91 of mise's commands have a second name, and until the derive could declare one
    // the shadow rejected invocations the real thing accepts. `mise i` is `mise install`.
    let a = argv(["i", "node@20"]);
    let cli = Cli::parse_from(&a).expect("`mise i` should parse");
    let Some(Commands::Install(install)) = cli.command else {
        panic!("`i` should have selected install")
    };
    assert_eq!(install.tool_version, ["node@20"]);

    // And a hidden one, which is matched but kept out of help.
    let a = argv(["x", "--", "node", "-e", "1"]);
    let cli = Cli::parse_from(&a).expect("`mise x` should parse");
    assert!(matches!(cli.command, Some(Commands::Exec(_))));
}
