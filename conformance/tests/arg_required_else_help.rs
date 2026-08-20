use std::ffi::OsStr;

use usage_argv::Error;
use usage_derive::{Args, Cli, Subcommands};

fn argv<const N: usize>(tokens: [&str; N]) -> [&OsStr; N] {
    tokens.map(OsStr::new)
}

#[derive(Cli)]
#[command(bin = "ex", arg_required_else_help)]
struct RootPolicy {
    #[arg(long, default = "defaulted")]
    value: String,
}

#[derive(Cli)]
#[command(bin = "ex")]
struct NestedPolicy {
    #[command(subcommand)]
    command: Commands,
}

#[derive(Subcommands)]
enum Commands {
    Run(Run),
}

#[derive(Args)]
#[command(arg_required_else_help)]
struct Run {
    #[arg(long)]
    all: bool,
}

#[derive(Cli)]
#[command(bin = "ex", default_subcommand = "run")]
struct DefaultPolicy {
    #[command(subcommand)]
    command: Option<DefaultCommands>,
}

#[derive(Subcommands)]
enum DefaultCommands {
    Run(DefaultRun),
}

#[derive(Args)]
#[command(arg_required_else_help)]
struct DefaultRun {
    task: String,
}

#[test]
fn a_bare_root_asks_for_short_help_even_when_a_default_fills_a_field() {
    let Err(Error::Help { cmd, long }) = RootPolicy::parse_from(&[]) else {
        panic!("a bare invocation should ask for help");
    };
    assert_eq!(cmd.name, "ex");
    assert!(!long);

    let parsed = RootPolicy::parse_from(&argv(["--value", "given"])).expect("argv was supplied");
    assert_eq!(parsed.value, "given");
}

#[test]
fn a_nested_command_counts_only_tokens_after_its_own_name() {
    let Err(Error::Help { cmd, long }) = NestedPolicy::parse_from(&argv(["run"])) else {
        panic!("the command name selects run but is not one of run's arguments");
    };
    assert_eq!(cmd.name, "run");
    assert!(!long);

    let parsed = NestedPolicy::parse_from(&argv(["run", "--all"])).expect("run has an argument");
    let Commands::Run(run) = parsed.command;
    assert!(run.all);
}

#[test]
fn a_default_command_counts_the_unmatched_word_routed_into_it() {
    let parsed = DefaultPolicy::parse_from(&argv(["build"])).expect("run received argv");
    let Some(DefaultCommands::Run(run)) = parsed.command else {
        panic!("the default command should be selected");
    };
    assert_eq!(run.task, "build");
}

#[test]
fn the_policy_is_emitted_in_the_portable_spec() {
    let root = RootPolicy::to_kdl();
    assert!(root.contains("arg_required_else_help #true"), "{root}");

    let nested = NestedPolicy::to_kdl();
    assert!(
        nested.contains("cmd \"run\" arg_required_else_help=#true"),
        "{nested}"
    );
}
