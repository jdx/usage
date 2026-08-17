use std::ffi::OsStr;
use std::path::{Path, PathBuf};

use usage_rs as usage;
use usage_rs::{Args, Cli, Subcommands};

#[derive(Cli)]
#[usage(bin = "ex")]
struct Ex {
    #[usage(subcommand)]
    command: Command,
}

#[derive(Subcommands)]
enum Command {
    Show(Show),
    /// Print version information
    Version,
}

/// Show one file
#[derive(Args)]
struct Show {
    #[usage(long, value_hint = usage::ValueHint::FilePath)]
    file: PathBuf,
}

#[test]
fn one_dependency_provides_derives_runtime_and_value_hints() {
    let _hint_from_facade = usage::ValueHint::FilePath;
    let argv = [
        OsStr::new("show"),
        OsStr::new("--file"),
        OsStr::new("input.txt"),
    ];
    let cli = Ex::parse_from(&argv).expect("valid command line");
    let Command::Show(show) = cli.command else {
        panic!("show command should be selected");
    };
    assert_eq!(show.file, Path::new("input.txt"));
    let spec = Ex::to_kdl();
    assert!(spec.contains("complete \"file\""), "{spec}");
    assert!(spec.contains("type=\"path\""), "{spec}");
}

#[test]
fn unit_subcommands_use_the_facade_derive() {
    let cli = Ex::parse_from(&[OsStr::new("version")]).expect("valid unit subcommand");
    assert!(matches!(cli.command, Command::Version));
}
