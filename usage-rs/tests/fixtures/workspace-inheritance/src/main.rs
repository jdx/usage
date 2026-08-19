use std::ffi::OsStr;

use usage::{Cli, ValueEnum};

#[derive(ValueEnum)]
enum Shell {
    Bash,
    Zsh,
}

impl std::str::FromStr for Shell {
    type Err = String;

    fn from_str(value: &str) -> Result<Self, Self::Err> {
        match value {
            "bash" => Ok(Self::Bash),
            "zsh" => Ok(Self::Zsh),
            _ => Err(format!("unsupported shell: {value}")),
        }
    }
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
