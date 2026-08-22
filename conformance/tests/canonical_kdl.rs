#![allow(dead_code)]

use usage::Spec;
use usage_derive::{Args, Cli, Subcommands};

#[derive(Cli)]
#[usage(
    name = "canonical",
    bin = "canonical",
    version = "1.2.3",
    unknown_flags = "error"
)]
struct Ex {
    /// Number of jobs to run.
    #[usage(short, long, global, env = "JOBS", default = "4")]
    jobs: usize,

    /// Select the output format.
    #[usage(long, choices("human", "json"))]
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
struct Run {
    /// Configuration file.
    #[usage(long)]
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
