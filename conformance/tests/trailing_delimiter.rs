use std::ffi::OsStr;

use usage::Spec;
use usage_derive::{Args, Cli, Subcommands};

fn argv<const N: usize>(tokens: [&str; N]) -> [&OsStr; N] {
    tokens.map(OsStr::new)
}

#[derive(Cli)]
#[usage(bin = "ex", dont_delimit_trailing_values)]
struct Mixed {
    #[usage(arg, delimiter = ',')]
    values: Vec<String>,
}

#[derive(Cli)]
#[usage(bin = "ex", dont_delimit_trailing_values)]
struct Nested {
    #[usage(subcommand)]
    command: NestedCommands,
}

#[derive(Subcommands)]
enum NestedCommands {
    Run(Run),
}

#[derive(Args)]
struct Run {
    #[usage(arg, delimiter = ',')]
    values: Vec<String>,
}

#[test]
fn only_values_past_the_trailing_boundary_keep_the_delimiter() {
    let parsed = Mixed::parse_from(&argv(["a,b", "--", "c,d"])).expect("should parse");
    assert_eq!(parsed.values, ["a", "b", "c,d"]);
}

#[test]
fn the_command_policy_is_inherited_by_subcommands() {
    let parsed = Nested::parse_from(&argv(["run", "--", "a,b"])).expect("should parse");
    let NestedCommands::Run(run) = parsed.command;
    assert_eq!(run.values, ["a,b"]);
}

#[test]
fn the_policy_round_trips() {
    let emitted = Mixed::to_kdl();
    assert!(
        emitted.contains("dont_delimit_trailing_values #true"),
        "{emitted}"
    );
    let spec: Spec = emitted.parse().expect("valid emitted spec");
    assert!(spec.cmd.dont_delimit_trailing_values);
}
