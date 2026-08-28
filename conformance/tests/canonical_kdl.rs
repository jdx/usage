#![allow(dead_code)]

use usage::Spec;
use usage_derive::{ArgGroup, Args, Cli, Subcommands};

#[derive(Cli)]
#[usage(
    name = "canonical",
    bin = "canonical",
    version = "1.2.3",
    unknown_flags = "error",
    example("canonical run build", header = "Run a task"),
    heading("Output Options", help = "Formats are stable across releases.")
)]
struct Ex {
    /// Number of jobs to run.
    #[usage(short, long, global, env = "JOBS", default = "4")]
    jobs: usize,

    /// Select the output format.
    #[usage(long, choices("human", "json"), help_heading = "Output Options")]
    format: Option<String>,

    #[usage(subcommand)]
    command: Commands,
}

#[derive(Subcommands)]
enum Commands {
    /// Run a task.
    Run(Run),
}

#[derive(Args)]
#[usage(
    example("canonical run build", header = "Build"),
    heading(
        "Task Options",
        help = "A task name is resolved against the config file."
    )
)]
struct Run {
    /// Configuration file.
    #[usage(long, help_heading = "Task Options")]
    config: Option<String>,

    /// Task name.
    task: String,
}

#[test]
fn direct_derive_output_is_the_canonical_portable_serialization() {
    let direct = Ex::to_kdl();
    let parsed: Spec = direct
        .parse()
        .unwrap_or_else(|error| panic!("derived KDL is valid: {error}\n{direct}"));
    assert_eq!(direct, parsed.to_string());
}

// A command declaring every node the two writers both emit, at the root and nested, because
// each disagreement found so far — `example` above the flags, a `heading` before it, an
// unquoted group member, `help` before `help_heading`, a preamble in the wrong place — was a
// combination no fixture happened to declare.
#[derive(Cli)]
#[usage(
    name = "maximal",
    bin = "maximal",
    version = "1.2.3",
    unknown_flags = "error",
    before_help = "before",
    after_help = "after",
    before_long_help = "before, at length",
    after_long_help = "after, at length",
    example("maximal run build", header = "Run", help = "Runs it"),
    heading("Output Options", help = "Formats are stable across releases."),
    output(
        "json",
        media_type = "application/json",
        framing = "json",
        help = "JSON"
    ),
    output("text", default, help = "Text"),
    exit_code(0, "fine"),
    exit_code(1, "not fine")
)]
struct Maximal {
    /// Number of jobs to run.
    #[usage(short, long, global, env = "JOBS", default = "4")]
    jobs: usize,

    /// Select the output format.
    #[usage(long, choices("human", "json"), help_heading = "Output Options")]
    format: Option<String>,

    #[usage(arg_group)]
    filters: Vec<MaximalFilter>,

    #[usage(subcommand)]
    command: MaximalCommands,
}

#[derive(Clone, ArgGroup)]
#[usage(name = "filter", multiple)]
enum MaximalFilter {
    /// Allow it.
    #[usage(short = 'A', value_name = "NAME")]
    Allow(String),

    /// Deny it.
    #[usage(short = 'D', value_name = "NAME")]
    Deny(String),
}

#[derive(Subcommands)]
enum MaximalCommands {
    /// Run a task.
    #[usage(alias = "r", help_heading = "Task Commands")]
    Run(MaximalRun),

    /// Something else.
    Other(MaximalRun),
}

#[derive(Args)]
#[usage(
    example("maximal run build", header = "Build"),
    heading(
        "Task Options",
        help = "A task name is resolved against the config file."
    ),
    output("junit", media_type = "application/xml", help = "JUnit"),
    exit_code(2, "the task failed")
)]
struct MaximalRun {
    /// Configuration file.
    #[usage(long, help_heading = "Task Options")]
    config: Option<String>,

    /// Temporary tool selections.
    #[usage(sigil = "+")]
    tools: Vec<String>,

    /// Task name.
    task: String,
}

#[test]
fn every_node_the_two_writers_share_serializes_identically() {
    let direct = Maximal::to_kdl();
    let parsed: Spec = direct
        .parse()
        .unwrap_or_else(|error| panic!("derived KDL is valid: {error}\n{direct}"));
    assert_eq!(direct, parsed.to_string());
}
