#![allow(dead_code)]

use usage::Spec;
use usage_derive::{Args, Cli, Subcommands};

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
