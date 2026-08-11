//! Subcommands, end to end: routing, per-command values, and the emitted spec.

use std::ffi::OsStr;

use usage::Spec as LibSpec;
use usage_derive::{Args, Cli, Subcommands};

/// A tool that does things
#[derive(Cli)]
#[usage(bin = "ex", version = "1.0")]
struct Ex {
    /// Say more
    #[usage(short = 'v', long, global)]
    verbose: bool,
    /// What to do
    #[usage(subcommand)]
    command: Option<Commands>,
}

#[derive(Subcommands)]
enum Commands {
    /// Install a tool
    Install(Install),
    /// Run a task
    #[usage(name = "run")]
    RunTask(Run),
}

/// The struct's own description, which the variant's overrides
#[derive(Args)]
struct Install {
    /// Overwrite an existing install
    #[usage(short = 'f', long)]
    force: bool,
    /// How many at once
    #[usage(short = 'j', long, default = "4")]
    jobs: Option<String>,
    /// What to install
    tools: Vec<String>,
}

/// Run a task
#[derive(Args)]
struct Run {
    /// The task to run
    task: String,
}

fn argv<const N: usize>(tokens: [&str; N]) -> [&OsStr; N] {
    tokens.map(OsStr::new)
}

#[test]
fn a_word_selects_a_command_and_its_own_fields_are_filled() {
    let a = argv(["install", "--force", "node@20", "go@1.22"]);
    let ex = Ex::parse_from(&a).expect("should parse");

    let Some(Commands::Install(install)) = ex.command else {
        panic!("expected the install command");
    };
    assert!(install.force);
    assert_eq!(install.tools, ["node@20", "go@1.22"]);
    // A declared default is in place before parsing starts, so an untouched flag
    // still has it.
    assert_eq!(install.jobs.as_deref(), Some("4"));
}

#[test]
fn another_variant_routes_by_its_own_name() {
    // `run` rather than `run-task`: the variant renames itself.
    let a = argv(["run", "build"]);
    let ex = Ex::parse_from(&a).expect("should parse");
    let Some(Commands::RunTask(run)) = ex.command else {
        panic!("expected the run command");
    };
    assert_eq!(run.task, "build");
}

#[test]
fn no_subcommand_leaves_the_field_empty() {
    let a = argv(["--verbose"]);
    let ex = Ex::parse_from(&a).expect("should parse");
    assert!(ex.verbose);
    assert!(ex.command.is_none());
}

#[test]
fn a_global_flag_works_on_either_side_of_the_command() {
    for tokens in [["--verbose", "run", "build"], ["run", "build", "--verbose"]] {
        let a = argv(tokens);
        let ex = Ex::parse_from(&a).expect("should parse");
        assert!(ex.verbose, "{tokens:?}");
        assert!(
            matches!(ex.command, Some(Commands::RunTask(_))),
            "{tokens:?}"
        );
    }
}

#[test]
fn one_command_does_not_see_another_ones_flag() {
    // `--force` belongs to `install`, so `run` does not answer to it. Leniently it
    // becomes a word, and `run` has one argument to hold it.
    let a = argv(["run", "--force"]);
    let ex = Ex::parse_from(&a).expect("should parse");
    let Some(Commands::RunTask(run)) = ex.command else {
        panic!("expected the run command");
    };
    assert_eq!(run.task, "--force");
}

#[test]
fn the_spec_carries_the_commands() {
    let kdl = Ex::to_kdl();
    let spec: LibSpec = kdl.parse().unwrap_or_else(|e| panic!("{e}\n\n{kdl}"));

    assert_eq!(spec.bin, "ex");
    let mut names: Vec<&str> = spec.cmd.subcommands.keys().map(String::as_str).collect();
    names.sort();
    assert_eq!(names, ["install", "run"]);

    let install = &spec.cmd.subcommands["install"];
    // The variant's doc comment wins: it is where a reader of the enum expects to
    // describe the command, and ignoring it would lose the description silently.
    assert_eq!(install.help.as_deref(), Some("Install a tool"));
    assert_eq!(install.flags.len(), 2);
    assert_eq!(install.args.len(), 1);
    assert!(install.args[0].var, "`tools` is a Vec, so it takes several");

    let run = &spec.cmd.subcommands["run"];
    assert_eq!(run.help.as_deref(), Some("Run a task"));
    assert_eq!(run.args[0].name, "task");
}

#[test]
fn keys_are_unique_across_independently_expanded_types() {
    // Each derive assigns keys without seeing the others, so a collision would bind
    // the wrong field. Checked here rather than trusted.
    let mut keys = vec![Ex::command().key];
    for cmd in Ex::command().subcommands {
        keys.push(cmd.key);
        for flag in cmd.flags {
            keys.push(flag.key);
        }
        for arg in cmd.args {
            keys.push(arg.key);
        }
    }
    for flag in Ex::command().flags {
        keys.push(flag.key);
    }

    let mut sorted = keys.clone();
    sorted.sort_unstable();
    sorted.dedup();
    assert_eq!(sorted.len(), keys.len(), "duplicate keys in {keys:?}");
}

#[test]
fn the_emitted_spec_reads_the_way_a_handwritten_one_would() {
    // The whole point of emitting KDL: what comes out is what a person would have
    // written, and `usage g markdown|manpage` reads it without knowing a derive was
    // involved.
    insta::assert_snapshot!(Ex::to_kdl());
}
