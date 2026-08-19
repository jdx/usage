use std::ffi::OsStr;

use usage::{Cli, ValueEnum};

#[derive(ValueEnum)]
enum Shell {
    Bash,
    Zsh,
}

#[derive(Cli)]
#[usage(bin = "ex")]
struct Ex {
    #[usage(long, value_enum)]
    shell: Shell,
}

fn main() {
    let cli = Ex::parse_from(&[OsStr::new("--shell"), OsStr::new("zsh")])
        .expect("workspace-inherited facade should resolve");
    assert!(matches!(cli.shell, Shell::Zsh));
}
