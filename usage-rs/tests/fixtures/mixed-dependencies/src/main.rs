use std::ffi::OsStr;

use usage_derive_direct::{Args, Cli, Subcommands};

#[derive(Cli)]
#[usage(bin = "ex")]
struct Ex {
    #[usage(subcommand)]
    command: Command,
}

#[derive(Subcommands)]
enum Command {
    Show(Show),
    Version,
}

#[derive(Args)]
struct Show {
    #[usage(long, value_hint = usage_argv_direct::ValueHint::FilePath)]
    file: std::path::PathBuf,
}

fn main() {
    let cli = Ex::parse_from(&[
        OsStr::new("show"),
        OsStr::new("--file"),
        OsStr::new("input.txt"),
    ])
    .expect("direct runtime should supply the spec feature");
    assert!(matches!(cli.command, Command::Show(_)));
    assert!(Ex::to_kdl().contains("complete \"file\" type=\"path\""));

    let cli = Ex::parse_from(&[OsStr::new("version")]).expect("valid unit subcommand");
    assert!(matches!(cli.command, Command::Version));
}
