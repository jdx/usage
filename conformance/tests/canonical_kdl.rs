#![allow(dead_code)]

use usage::Spec;
use usage_derive::{Args, Cli, Subcommands};

#[derive(Cli)]
#[command(
    name = "canonical",
    bin = "canonical",
    version = "1.2.3",
    unknown_flags = "error"
)]
struct Ex {
    /// Number of jobs to run.
    #[arg(short, long, global, env = "JOBS", default = "4")]
    jobs: usize,

    /// Select the output format.
    #[arg(long, choices("human", "json"))]
    format: Option<String>,

    #[command(subcommand)]
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
    #[arg(long)]
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
