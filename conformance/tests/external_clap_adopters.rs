//! Reduced, source-derived ports of three maintained clap CLIs.
//!
//! The upstream revisions and full-surface audit live in `benches/external/README.md`. These
//! probes keep the migration boundaries executable without vendoring three applications.

use std::ffi::OsStr;
use std::str::FromStr;

use clap::Parser as _;
use usage_derive::{Cli, Subcommands, ValueEnum};

#[test]
fn full_external_surfaces_are_captured_and_compile_as_typed_shadows() {
    let tokei: usage::Spec = include_str!("../../benches/external/tokei.usage.kdl")
        .parse()
        .expect("the pinned tokei capture should parse");
    assert_eq!(tokei.cmd.flags.len(), 18);
    assert_eq!(tokei.cmd.args.len(), 1);
    assert_eq!(
        include_str!("../../benches/external/tokei.losses.txt").trim(),
        "[]"
    );

    let starship: usage::Spec = include_str!("../../benches/external/starship.usage.kdl")
        .parse()
        .expect("the pinned starship capture should parse");
    assert_eq!(starship.cmd.subcommands.len(), 14);
    assert_eq!(
        starship
            .cmd
            .subcommands
            .values()
            .map(|command| command.flags.len())
            .sum::<usize>(),
        55
    );
    let report = include_str!("../../benches/external/starship.losses.txt");
    assert_eq!(report.matches("FidelityLoss").count(), 5, "{report}");
    assert_eq!(report.matches("ValueArity").count(), 5, "{report}");
    assert_eq!(report.matches("\"pipestatus\"").count(), 5, "{report}");
}

mod fd {
    use super::*;

    #[derive(Debug, Clone, PartialEq, Eq)]
    struct Size(String);

    impl FromStr for Size {
        type Err = &'static str;

        fn from_str(value: &str) -> Result<Self, Self::Err> {
            value
                .starts_with(['+', '-'])
                .then(|| Self(value.to_owned()))
                .ok_or("size needs a + or - prefix")
        }
    }

    #[derive(Debug, clap::Parser)]
    #[command(name = "fd", args_override_self = true)]
    struct ClapCli {
        #[arg(short = 'H', long)]
        hidden: bool,
        #[arg(short = 'S', long, value_parser = Size::from_str, allow_hyphen_values = true)]
        size: Option<Size>,
        #[arg(short = 'x', long, conflicts_with_all = ["max_results", "list_details"])]
        exec: Option<String>,
        #[arg(long, conflicts_with_all = ["exec", "list_details"])]
        max_results: Option<usize>,
        #[arg(short = 'l', long, conflicts_with_all = ["exec", "max_results"])]
        list_details: bool,
    }

    #[derive(Debug, Cli)]
    #[usage(bin = "fd", unknown_flags = "error")]
    struct UsageCli {
        #[arg(short = 'H', long)]
        hidden: bool,
        #[arg(short = 'S', long, allow_hyphen_values)]
        size: Option<Size>,
        #[arg(short = 'x', long, conflicts("--max-results", "--list-details"))]
        exec: Option<String>,
        #[arg(long, conflicts("--exec", "--list-details"))]
        max_results: Option<usize>,
        #[arg(short = 'l', long, conflicts("--exec", "--max-results"))]
        list_details: bool,
    }

    #[test]
    fn derive_heavy_custom_values_and_conflicts_port() {
        let argv = ["--hidden", "--size=-10"];
        let clap = ClapCli::try_parse_from(std::iter::once("fd").chain(argv)).unwrap();
        let usage = UsageCli::parse_from(&argv.map(OsStr::new)).unwrap();
        assert_eq!(usage.hidden, clap.hidden);
        assert_eq!(usage.size, clap.size);

        assert!(ClapCli::try_parse_from(["fd", "--exec", "echo", "--list-details"]).is_err());
        assert!(UsageCli::parse_from(&[
            OsStr::new("--exec"),
            OsStr::new("echo"),
            OsStr::new("--list-details"),
        ])
        .is_err());
    }
}

mod tokei {
    use clap::{value_parser, Arg, ArgAction, Command};

    fn command() -> Command {
        Command::new("tokei")
            .arg(
                Arg::new("columns")
                    .long("columns")
                    .short('c')
                    .value_parser(value_parser!(usize))
                    .conflicts_with("output"),
            )
            .arg(
                Arg::new("output")
                    .long("output")
                    .short('o')
                    .value_parser(["json", "yaml"]),
            )
            .arg(
                Arg::new("exclude")
                    .long("exclude")
                    .short('e')
                    .action(ArgAction::Append),
            )
            .arg(Arg::new("input").num_args(1..))
    }

    #[test]
    fn builder_heavy_commands_have_an_interpreted_migration_path() {
        let mut clap = command();
        let (spec, report) = clap_usage::spec_with_report(&mut clap, "tokei");
        assert!(report.is_lossless(), "{:#?}", report.losses());
        assert_eq!(spec.bin, "tokei");
        assert!(spec.cmd.flags.iter().any(|flag| flag.name == "columns"));
        assert!(spec.cmd.flags.iter().any(|flag| flag.name == "exclude"));
        assert_eq!(spec.cmd.args.len(), 1);
    }
}

mod starship {
    use super::*;

    #[derive(Debug, Clone, Copy, PartialEq, Eq, ValueEnum, clap::ValueEnum)]
    enum Shell {
        Bash,
        Fish,
        #[value(alias = "pwsh")]
        #[clap(alias = "pwsh")]
        Powershell,
        Zsh,
    }

    #[derive(Debug, clap::Parser)]
    #[command(name = "starship", subcommand_required = true)]
    struct ClapCli {
        #[command(subcommand)]
        command: ClapCommand,
    }

    #[derive(Debug, clap::Subcommand)]
    enum ClapCommand {
        Completions {
            #[arg(value_enum)]
            shell: Shell,
        },
        Config {
            #[arg(requires = "value")]
            name: Option<String>,
            value: Option<String>,
        },
    }

    #[derive(Debug, Cli)]
    #[usage(bin = "starship", unknown_flags = "error")]
    struct UsageCli {
        #[command(subcommand)]
        command: UsageCommand,
    }

    #[derive(Debug, Subcommands)]
    enum UsageCommand {
        Completions {
            #[arg(value_enum)]
            shell: Shell,
        },
        Config {
            #[arg(requires = "value")]
            name: Option<String>,
            value: Option<String>,
        },
    }

    #[test]
    fn custom_completion_values_and_positional_relationships_port() {
        let clap = ClapCli::try_parse_from(["starship", "completions", "pwsh"]).unwrap();
        let usage = UsageCli::parse_from(&[OsStr::new("completions"), OsStr::new("pwsh")]).unwrap();
        assert!(matches!(
            (clap.command, usage.command),
            (
                ClapCommand::Completions {
                    shell: Shell::Powershell
                },
                UsageCommand::Completions {
                    shell: Shell::Powershell
                }
            )
        ));

        assert!(ClapCli::try_parse_from(["starship", "config", "format"]).is_err());
        assert!(UsageCli::parse_from(&[OsStr::new("config"), OsStr::new("format"),]).is_err());

        let clap = ClapCli::try_parse_from(["starship", "config", "format", "$all"]).unwrap();
        let usage = UsageCli::parse_from(&[
            OsStr::new("config"),
            OsStr::new("format"),
            OsStr::new("$all"),
        ])
        .unwrap();
        match (clap.command, usage.command) {
            (
                ClapCommand::Config {
                    name: clap_name,
                    value: clap_value,
                },
                UsageCommand::Config { name, value },
            ) => {
                assert_eq!(name, clap_name);
                assert_eq!(value, clap_value);
            }
            _ => panic!("both parsers should select config"),
        }
    }
}
