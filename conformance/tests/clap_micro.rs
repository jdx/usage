//! Minimal paired CLIs that turn the clap compatibility matrix into executable claims.

use std::ffi::OsStr;

use clap::{CommandFactory, Parser as _};
use usage_argv::Error;
use usage_derive::Cli;

#[derive(Debug, PartialEq, Eq)]
enum Stream {
    Stdout,
    Stderr,
}

#[derive(Debug, PartialEq, Eq)]
struct Exit {
    status: i32,
    stream: Stream,
    text: String,
}

fn clap_exit<T>(argv: &[&str]) -> Exit
where
    T: clap::Parser + CommandFactory + std::fmt::Debug,
{
    let result = T::try_parse_from(std::iter::once("micro").chain(argv.iter().copied()));
    let error = result.expect_err("the vector requests terminal output");
    Exit {
        status: error.exit_code(),
        stream: if error.use_stderr() {
            Stream::Stderr
        } else {
            Stream::Stdout
        },
        text: error.to_string(),
    }
}

fn usage_error(spec: &usage_argv::spec::Spec<'_>, words: &[&OsStr], error: Error<'_, '_>) -> Exit {
    match error {
        Error::Help { cmd, long } => Exit {
            status: 0,
            stream: Stream::Stdout,
            text: usage_argv::help::render(spec, cmd, long).expect("this CLI's command"),
        },
        Error::MissingArgsHelp { cmd } => Exit {
            status: 2,
            stream: Stream::Stderr,
            text: usage_argv::help::render(spec, cmd, false).expect("this CLI's command"),
        },
        Error::Version { long } => {
            let version = if long {
                spec.long_version.or(spec.version)
            } else {
                spec.version
            }
            .unwrap_or_default();
            Exit {
                status: 0,
                stream: Stream::Stdout,
                text: format!("{} {version}\n", spec.name),
            }
        }
        error => Exit {
            status: 2,
            stream: Stream::Stderr,
            text: usage_argv::diagnostic::render(
                spec,
                words,
                &error,
                usage_argv::diagnostic::Style::PLAIN,
            ),
        },
    }
}

mod require_equals {
    use super::*;

    #[derive(Debug, clap::Parser)]
    #[command(name = "micro", version = "1.2.3")]
    struct ClapCli {
        /// Value to record
        #[arg(long, require_equals = true)]
        value: String,
    }

    #[derive(Debug, Cli)]
    #[usage(bin = "micro", version = "1.2.3", unknown_flags = "error")]
    struct UsageCli {
        /// Value to record
        #[usage(long, require_equals)]
        value: String,
    }

    fn usage_exit(argv: &[&str]) -> Exit {
        let words: Vec<&OsStr> = argv.iter().map(OsStr::new).collect();
        let error = UsageCli::parse_from(&words).expect_err("the vector requests terminal output");
        usage_error(UsageCli::spec(), &words, error)
    }

    #[test]
    fn accepted_argv_binds_the_same_typed_value() {
        let clap = ClapCli::try_parse_from(["micro", "--value=kept"]).unwrap();
        let usage = UsageCli::parse_from(&[OsStr::new("--value=kept")]).unwrap();
        assert_eq!(usage.value, clap.value);
    }

    #[test]
    fn detached_values_have_the_same_terminal_contract() {
        let clap = clap_exit::<ClapCli>(&["--value", "kept"]);
        let usage = usage_exit(&["--value", "kept"]);
        assert_eq!(usage.status, clap.status);
        assert_eq!(usage.stream, clap.stream);
        let message = "equal sign is needed when assigning values to '--value=<VALUE>'";
        assert!(clap.text.contains(message), "{}", clap.text);
        assert!(usage.text.contains(message), "{}", usage.text);
    }

    #[test]
    fn help_and_version_use_the_same_stream_and_status() {
        for argv in [["-h"].as_slice(), ["--help"].as_slice()] {
            let clap = clap_exit::<ClapCli>(argv);
            let usage = usage_exit(argv);
            assert_eq!(usage.status, clap.status, "{argv:?}");
            assert_eq!(usage.stream, clap.stream, "{argv:?}");
            for text in [&clap.text, &usage.text] {
                assert!(text.contains("--value=<VALUE>"), "{argv:?}: {text}");
                assert!(text.contains("Value to record"), "{argv:?}: {text}");
            }
        }

        assert_eq!(
            usage_exit(&["--version"]),
            clap_exit::<ClapCli>(&["--version"])
        );
    }
}

mod default_value {
    use super::*;

    #[derive(Debug, clap::Parser)]
    #[command(name = "micro")]
    struct ClapCli {
        #[arg(long, default_value_t = 4)]
        jobs: u32,
    }

    #[derive(Debug, Cli)]
    #[usage(bin = "micro", unknown_flags = "error")]
    struct UsageCli {
        #[usage(long, default = "4")]
        jobs: u32,
    }

    #[test]
    fn omitted_values_bind_the_same_default() {
        assert_eq!(
            UsageCli::parse_from(&[]).unwrap().jobs,
            ClapCli::try_parse_from(["micro"]).unwrap().jobs
        );
        let clap_help = ClapCli::command().render_long_help().to_string();
        let usage_help =
            usage_argv::help::render(UsageCli::spec(), UsageCli::spec().root.cmd, true).unwrap();
        for help in [clap_help, usage_help] {
            assert!(help.contains("--jobs <JOBS>"), "{help}");
            assert!(help.contains("default: 4"), "{help}");
        }
    }
}

mod value_delimiter {
    use super::*;

    #[derive(Debug, clap::Parser)]
    #[command(name = "micro")]
    struct ClapCli {
        #[arg(long, value_delimiter = ',')]
        values: Vec<String>,
    }

    #[derive(Debug, Cli)]
    #[usage(bin = "micro", unknown_flags = "error")]
    struct UsageCli {
        #[usage(long, var, delimiter = ',')]
        values: Vec<String>,
    }

    #[test]
    fn one_token_binds_the_same_values() {
        let clap = ClapCli::try_parse_from(["micro", "--values", "a,b"]).unwrap();
        let usage = UsageCli::parse_from(&[OsStr::new("--values"), OsStr::new("a,b")]).unwrap();
        assert_eq!(usage.values, clap.values);
    }
}

mod allow_negative_numbers {
    use super::*;

    #[derive(Debug, clap::Parser)]
    #[command(name = "micro")]
    struct ClapCli {
        #[arg(long, allow_negative_numbers = true)]
        number: i32,
    }

    #[derive(Debug, Cli)]
    #[usage(bin = "micro", unknown_flags = "error")]
    struct UsageCli {
        #[usage(long, allow_negative_numbers)]
        number: i32,
    }

    #[test]
    fn a_negative_token_binds_as_the_value() {
        let clap = ClapCli::try_parse_from(["micro", "--number", "-7"]).unwrap();
        let usage = UsageCli::parse_from(&[OsStr::new("--number"), OsStr::new("-7")]).unwrap();
        assert_eq!(usage.number, clap.number);
    }
}

mod value_enum {
    use super::*;

    #[derive(Clone, Debug, PartialEq, Eq, clap::ValueEnum)]
    enum ClapMode {
        Fast,
        Slow,
    }

    #[derive(Clone, Debug, PartialEq, Eq, usage_derive::ValueEnum)]
    enum UsageMode {
        Fast,
        Slow,
    }

    #[derive(Debug, clap::Parser)]
    #[command(name = "micro")]
    struct ClapCli {
        #[arg(long, value_enum)]
        mode: ClapMode,
    }

    #[derive(Debug, Cli)]
    #[usage(bin = "micro", unknown_flags = "error")]
    struct UsageCli {
        #[usage(long, value_enum)]
        mode: UsageMode,
    }

    #[test]
    fn declared_words_bind_and_invalid_choices_fail() {
        let clap = ClapCli::try_parse_from(["micro", "--mode", "slow"]).unwrap();
        let usage = UsageCli::parse_from(&[OsStr::new("--mode"), OsStr::new("slow")]).unwrap();
        assert_eq!(format!("{:?}", usage.mode), format!("{:?}", clap.mode));

        let clap = ClapCli::try_parse_from(["micro", "--mode", "medium"]).unwrap_err();
        let usage =
            UsageCli::parse_from(&[OsStr::new("--mode"), OsStr::new("medium")]).unwrap_err();
        assert_eq!(clap.kind(), clap::error::ErrorKind::InvalidValue);
        assert!(matches!(usage, Error::InvalidChoice { .. }));
    }

    #[test]
    fn declared_words_offer_the_same_completion_candidates() {
        let clap: std::collections::BTreeSet<_> = ClapCli::command()
            .get_arguments()
            .find(|arg| arg.get_id() == "mode")
            .expect("mode")
            .get_possible_values()
            .into_iter()
            .filter(|value| !value.is_hide_set())
            .map(|value| value.get_name().to_string())
            .collect();
        let line = "micro --mode ";
        let split =
            usage_argv::complete::split(line, line.len(), usage_argv::complete::Shell::Bash);
        let usage: std::collections::BTreeSet<_> =
            usage_argv::complete::candidates(UsageCli::spec(), &split)
                .into_iter()
                .map(|candidate| candidate.value)
                .collect();
        assert_eq!(usage, clap);
    }
}

mod args_override_self {
    use super::*;

    #[derive(Debug, clap::Parser)]
    #[command(name = "micro", args_override_self = true)]
    struct ClapCli {
        #[arg(long)]
        value: String,
        #[arg(long, value_parser = ["good", "better"])]
        mode: Option<String>,
    }

    #[derive(Debug, Cli)]
    #[usage(bin = "micro", unknown_flags = "error")]
    struct UsageCli {
        #[usage(long)]
        value: String,
        #[usage(long, choices("good", "better"))]
        mode: Option<String>,
    }

    #[test]
    fn the_last_scalar_value_wins() {
        let clap = ClapCli::try_parse_from(["micro", "--value", "old", "--value", "new"]).unwrap();
        let usage = UsageCli::parse_from(&[
            OsStr::new("--value"),
            OsStr::new("old"),
            OsStr::new("--value"),
            OsStr::new("new"),
        ])
        .unwrap();
        assert_eq!(usage.value, clap.value);
    }

    #[test]
    fn the_last_scalar_choice_controls_validation() {
        let argv = ["--value", "kept", "--mode", "bad", "--mode", "good"];
        let usage = UsageCli::parse_from(&argv.map(OsStr::new)).unwrap();
        assert_eq!(usage.mode.as_deref(), Some("good"));

        let argv = ["--value", "kept", "--mode", "good", "--mode", "bad"];
        let usage = UsageCli::parse_from(&argv.map(OsStr::new)).unwrap_err();
        assert!(matches!(usage, Error::InvalidChoice { .. }));
    }
}

mod allow_hyphen_values {
    use super::*;

    #[derive(Debug, clap::Parser)]
    #[command(name = "micro")]
    struct ClapCli {
        #[arg(long, allow_hyphen_values = true)]
        value: String,
    }

    #[derive(Debug, Cli)]
    #[usage(bin = "micro", unknown_flags = "error")]
    struct UsageCli {
        #[usage(long, allow_hyphen_values)]
        value: String,
    }

    #[test]
    fn a_flaglike_token_binds_as_the_value() {
        let clap = ClapCli::try_parse_from(["micro", "--value", "--literal"]).unwrap();
        let usage =
            UsageCli::parse_from(&[OsStr::new("--value"), OsStr::new("--literal")]).unwrap();
        assert_eq!(usage.value, clap.value);
    }
}

mod fixed_arity {
    use super::*;

    #[derive(Debug, clap::Parser)]
    #[command(name = "micro")]
    struct ClapCli {
        #[arg(long, num_args = 2, value_names = ["START", "END"])]
        range: Vec<String>,
    }

    #[derive(Debug, Cli)]
    #[usage(bin = "micro", unknown_flags = "error")]
    struct UsageCli {
        #[arg(long, num_args = 2, value_names = ["START", "END"])]
        range: Vec<String>,
    }

    #[test]
    fn one_occurrence_consumes_the_same_number_of_values() {
        let clap = ClapCli::try_parse_from(["micro", "--range", "1", "9"]).unwrap();
        let usage =
            UsageCli::parse_from(&[OsStr::new("--range"), OsStr::new("1"), OsStr::new("9")])
                .unwrap();
        assert_eq!(usage.range, clap.range);

        let clap = ClapCli::try_parse_from(["micro", "--range", "1"]).unwrap_err();
        let usage = UsageCli::parse_from(&[OsStr::new("--range"), OsStr::new("1")]).unwrap_err();
        assert_eq!(clap.kind(), clap::error::ErrorKind::WrongNumberOfValues);
        assert!(matches!(usage, Error::VarTooFew { .. }));
    }
}

mod global_subcommand {
    use super::*;

    #[derive(Debug, clap::Subcommand)]
    enum ClapCommand {
        Run {
            #[arg(long)]
            task: String,
        },
    }

    #[derive(Debug, clap::Parser)]
    #[command(name = "micro")]
    struct ClapCli {
        #[arg(long, global = true)]
        verbose: bool,
        #[command(subcommand)]
        command: ClapCommand,
    }

    #[derive(Debug, usage_derive::Subcommands)]
    enum UsageCommand {
        Run {
            #[usage(long)]
            task: String,
        },
    }

    #[derive(Debug, Cli)]
    #[usage(bin = "micro", unknown_flags = "error")]
    struct UsageCli {
        #[usage(long, global)]
        verbose: bool,
        #[usage(subcommand)]
        command: UsageCommand,
    }

    #[test]
    fn a_root_global_binds_after_the_selected_child() {
        let clap =
            ClapCli::try_parse_from(["micro", "run", "--task", "build", "--verbose"]).unwrap();
        let usage = UsageCli::parse_from(&[
            OsStr::new("run"),
            OsStr::new("--task"),
            OsStr::new("build"),
            OsStr::new("--verbose"),
        ])
        .unwrap();
        assert_eq!(usage.verbose, clap.verbose);
        let (ClapCommand::Run { task: clap_task }, UsageCommand::Run { task: usage_task }) =
            (clap.command, usage.command);
        assert_eq!(usage_task, clap_task);
    }

    #[test]
    fn the_root_offers_the_same_subcommand_candidates() {
        let clap: std::collections::BTreeSet<_> = ClapCli::command()
            .get_subcommands()
            .filter(|command| command.get_name() != "help" && !command.is_hide_set())
            .map(|command| command.get_name().to_string())
            .collect();
        let line = "micro ";
        let split =
            usage_argv::complete::split(line, line.len(), usage_argv::complete::Shell::Bash);
        let usage: std::collections::BTreeSet<_> =
            usage_argv::complete::candidates(UsageCli::spec(), &split)
                .into_iter()
                .map(|candidate| candidate.value)
                .collect();
        assert_eq!(usage, clap);
    }
}

mod required_flag {
    use super::*;

    #[derive(Debug, clap::Parser)]
    #[command(name = "micro")]
    struct ClapCli {
        #[arg(long)]
        config: String,
    }

    #[derive(Debug, Cli)]
    #[usage(bin = "micro", unknown_flags = "error")]
    struct UsageCli {
        #[usage(long)]
        config: String,
    }

    #[test]
    fn required_values_accept_and_reject_the_same_shapes() {
        let clap = ClapCli::try_parse_from(["micro", "--config", "config.toml"]).unwrap();
        let usage =
            UsageCli::parse_from(&[OsStr::new("--config"), OsStr::new("config.toml")]).unwrap();
        assert_eq!(usage.config, clap.config);

        let clap = ClapCli::try_parse_from(["micro"]).unwrap_err();
        let usage = UsageCli::parse_from(&[]).unwrap_err();
        assert_eq!(clap.kind(), clap::error::ErrorKind::MissingRequiredArgument);
        assert!(matches!(usage, Error::MissingRequired { .. }));
    }
}

mod conflicts_and_requires {
    use super::*;

    #[derive(Debug, clap::Parser)]
    #[command(name = "micro")]
    struct ClapCli {
        #[arg(long, conflicts_with = "quiet")]
        verbose: bool,
        #[arg(long)]
        quiet: bool,
        #[arg(long, requires = "key")]
        sign: bool,
        #[arg(long)]
        key: Option<String>,
    }

    #[derive(Debug, Cli)]
    #[usage(bin = "micro", unknown_flags = "error")]
    struct UsageCli {
        #[usage(long, conflicts = "--quiet")]
        verbose: bool,
        #[usage(long)]
        quiet: bool,
        #[usage(long, requires = "--key")]
        sign: bool,
        #[usage(long)]
        key: Option<String>,
    }

    #[test]
    fn relationships_accept_and_reject_the_same_shapes() {
        let clap = ClapCli::try_parse_from(["micro", "--verbose"]).unwrap();
        let usage = UsageCli::parse_from(&[OsStr::new("--verbose")]).unwrap();
        assert_eq!((usage.verbose, usage.quiet), (clap.verbose, clap.quiet));

        let clap = ClapCli::try_parse_from(["micro", "--sign", "--key", "secret"]).unwrap();
        let usage = UsageCli::parse_from(&[
            OsStr::new("--sign"),
            OsStr::new("--key"),
            OsStr::new("secret"),
        ])
        .unwrap();
        assert_eq!(usage.sign, clap.sign);
        assert_eq!(usage.key, clap.key);

        let clap = ClapCli::try_parse_from(["micro", "--verbose", "--quiet"]).unwrap_err();
        let usage =
            UsageCli::parse_from(&[OsStr::new("--verbose"), OsStr::new("--quiet")]).unwrap_err();
        assert_eq!(clap.kind(), clap::error::ErrorKind::ArgumentConflict);
        assert!(matches!(usage, Error::ConflictingFlags { .. }));

        let clap = ClapCli::try_parse_from(["micro", "--sign"]).unwrap_err();
        let usage = UsageCli::parse_from(&[OsStr::new("--sign")]).unwrap_err();
        assert_eq!(clap.kind(), clap::error::ErrorKind::MissingRequiredArgument);
        assert!(matches!(usage, Error::MissingRequired { .. }));
    }
}

mod required_exclusive_group {
    use super::*;
    use clap::ArgGroup;

    #[derive(Debug, clap::Parser)]
    #[command(
        name = "micro",
        group(ArgGroup::new("input").required(true).multiple(false))
    )]
    struct ClapCli {
        #[arg(long, group = "input")]
        file: bool,
        #[arg(long, group = "input")]
        stdin: bool,
    }

    #[derive(Debug, Cli)]
    #[usage(bin = "micro", unknown_flags = "error", group("input", required))]
    struct UsageCli {
        #[usage(long, group = "input")]
        file: bool,
        #[usage(long, group = "input")]
        stdin: bool,
    }

    #[test]
    fn exactly_one_member_is_required() {
        let clap = ClapCli::try_parse_from(["micro", "--file"]).unwrap();
        let usage = UsageCli::parse_from(&[OsStr::new("--file")]).unwrap();
        assert_eq!((usage.file, usage.stdin), (clap.file, clap.stdin));

        let clap = ClapCli::try_parse_from(["micro"]).unwrap_err();
        let usage = UsageCli::parse_from(&[]).unwrap_err();
        assert_eq!(clap.kind(), clap::error::ErrorKind::MissingRequiredArgument);
        assert!(matches!(usage, Error::MissingGroup { .. }));

        let clap = ClapCli::try_parse_from(["micro", "--file", "--stdin"]).unwrap_err();
        let usage =
            UsageCli::parse_from(&[OsStr::new("--file"), OsStr::new("--stdin")]).unwrap_err();
        assert_eq!(clap.kind(), clap::error::ErrorKind::ArgumentConflict);
        assert!(matches!(usage, Error::ConflictingFlags { .. }));
    }
}

mod optional_value_with_equals {
    use super::*;

    #[derive(Debug, clap::Parser)]
    #[command(name = "micro")]
    struct ClapCli {
        #[arg(long, num_args = 0..=1, require_equals = true)]
        color: Option<Option<String>>,
        rest: Option<String>,
    }

    #[derive(Debug, Cli)]
    #[usage(bin = "micro", unknown_flags = "error")]
    struct UsageCli {
        #[usage(long, require_equals)]
        color: Option<Option<String>>,
        #[usage(arg)]
        rest: Option<String>,
    }

    #[test]
    fn detached_words_remain_positional() {
        let clap = ClapCli::try_parse_from(["micro", "--color", "input"]).unwrap();
        let usage = UsageCli::parse_from(&[OsStr::new("--color"), OsStr::new("input")]).unwrap();
        assert_eq!(usage.color, clap.color);
        assert_eq!(usage.rest, clap.rest);

        let clap = ClapCli::try_parse_from(["micro", "--color=always"]).unwrap();
        let usage = UsageCli::parse_from(&[OsStr::new("--color=always")]).unwrap();
        assert_eq!(usage.color, clap.color);
    }
}

mod external_subcommand {
    use super::*;
    use usage_derive::Subcommands;

    #[derive(Debug, clap::Subcommand)]
    enum ClapCommand {
        Run,
        #[command(external_subcommand)]
        External(Vec<String>),
    }

    #[derive(Debug, clap::Parser)]
    #[command(name = "micro")]
    struct ClapCli {
        #[command(subcommand)]
        command: ClapCommand,
    }

    #[derive(Debug, Subcommands)]
    enum UsageCommand {
        Run,
        #[usage(external_subcommand)]
        External(Vec<String>),
    }

    #[derive(Debug, Cli)]
    #[usage(bin = "micro", unknown_flags = "error")]
    struct UsageCli {
        #[usage(subcommand)]
        command: UsageCommand,
    }

    #[test]
    fn unmatched_commands_forward_the_same_words() {
        let clap = ClapCli::try_parse_from(["micro", "plugin", "--literal", "value"]).unwrap();
        let usage = UsageCli::parse_from(&[
            OsStr::new("plugin"),
            OsStr::new("--literal"),
            OsStr::new("value"),
        ])
        .unwrap();
        let (ClapCommand::External(clap), UsageCommand::External(usage)) =
            (clap.command, usage.command)
        else {
            panic!("the unknown command should select the external variant")
        };
        assert_eq!(usage, clap);
    }
}

mod arg_required_else_help {
    use super::*;

    #[derive(Debug, clap::Parser)]
    #[command(name = "micro", arg_required_else_help = true)]
    struct ClapCli {
        #[arg(long)]
        all: bool,
    }

    #[derive(Debug, Cli)]
    #[usage(bin = "micro", unknown_flags = "error", arg_required_else_help)]
    struct UsageCli {
        #[usage(long)]
        all: bool,
    }

    fn usage_exit(argv: &[&str]) -> Exit {
        let words: Vec<&OsStr> = argv.iter().map(OsStr::new).collect();
        let error = UsageCli::parse_from(&words).expect_err("bare argv requests help");
        usage_error(UsageCli::spec(), &words, error)
    }

    #[test]
    fn bare_argv_has_the_same_terminal_contract() {
        let clap = clap_exit::<ClapCli>(&[]);
        let usage = usage_exit(&[]);
        assert_eq!(usage.status, clap.status);
        assert_eq!(usage.stream, clap.stream);
        for help in [usage.text, clap.text] {
            assert!(help.contains("--all"), "{help}");
        }

        assert_eq!(
            UsageCli::parse_from(&[OsStr::new("--all")]).unwrap().all,
            ClapCli::try_parse_from(["micro", "--all"]).unwrap().all
        );
    }
}

mod subcommand_policies {
    use super::*;
    use usage_derive::Subcommands;

    #[derive(Debug, clap::Subcommand)]
    enum ClapCommand {
        Run,
    }

    #[derive(Debug, clap::Parser)]
    #[command(
        name = "micro",
        subcommand_negates_reqs = true,
        subcommand_precedence_over_arg = true
    )]
    struct ClapNegates {
        #[command(subcommand)]
        command: Option<ClapCommand>,
        #[arg(long, required = true)]
        profile: Option<String>,
    }

    #[derive(Debug, clap::Parser)]
    #[command(name = "micro", args_conflicts_with_subcommands = true)]
    struct ClapConflicts {
        #[arg(long)]
        verbose: bool,
        #[command(subcommand)]
        command: Option<ClapCommand>,
    }

    #[derive(Debug, Subcommands)]
    enum UsageCommand {
        Run,
    }

    #[derive(Debug, Cli)]
    #[usage(
        bin = "micro",
        unknown_flags = "error",
        subcommand_negates_reqs,
        subcommand_precedence_over_arg
    )]
    struct UsageNegates {
        #[usage(subcommand)]
        command: Option<UsageCommand>,
        #[usage(long, required)]
        profile: Option<String>,
    }

    #[derive(Debug, Cli)]
    #[usage(
        bin = "micro",
        unknown_flags = "error",
        args_conflicts_with_subcommands
    )]
    struct UsageConflicts {
        #[usage(long)]
        verbose: bool,
        #[usage(subcommand)]
        command: Option<UsageCommand>,
    }

    #[test]
    fn a_subcommand_can_suppress_parent_requirements() {
        assert!(ClapNegates::command().is_subcommand_negates_reqs_set());
        assert!(ClapNegates::command().is_subcommand_precedence_over_arg_set());
        let clap = ClapNegates::try_parse_from(["micro", "run"]).unwrap();
        let usage = UsageNegates::parse_from(&[OsStr::new("run")]).unwrap();
        assert!(clap.profile.is_none());
        assert!(usage.profile.is_none());
        assert!(matches!(clap.command, Some(ClapCommand::Run)));
        assert!(matches!(usage.command, Some(UsageCommand::Run)));
    }

    #[test]
    fn parent_arguments_can_conflict_with_a_subcommand() {
        let clap = ClapConflicts::try_parse_from(["micro", "--verbose"]).unwrap();
        let usage = UsageConflicts::parse_from(&[OsStr::new("--verbose")]).unwrap();
        assert_eq!(usage.verbose, clap.verbose);
        assert!(usage.command.is_none());

        let clap = ClapConflicts::try_parse_from(["micro", "--verbose", "run"]).unwrap_err();
        let usage =
            UsageConflicts::parse_from(&[OsStr::new("--verbose"), OsStr::new("run")]).unwrap_err();
        assert_eq!(clap.kind(), clap::error::ErrorKind::ArgumentConflict);
        assert!(matches!(usage, Error::SubcommandConflict { .. }));
    }
}

mod conditional_defaults {
    use super::*;

    #[derive(Debug, clap::Parser)]
    #[command(name = "micro")]
    struct ClapCli {
        #[arg(long)]
        json: bool,
        #[arg(long, default_value_if("json", "true", "json"))]
        format: Option<String>,
    }

    #[derive(Debug, Cli)]
    #[usage(bin = "micro", unknown_flags = "error")]
    struct UsageCli {
        #[usage(long)]
        json: bool,
        #[usage(long, default_if("--json", "json"))]
        format: Option<String>,
    }

    #[test]
    fn a_present_flag_supplies_the_same_conditional_default() {
        let clap = ClapCli::try_parse_from(["micro", "--json"]).unwrap();
        let usage = UsageCli::parse_from(&[OsStr::new("--json")]).unwrap();
        assert_eq!(usage.format, clap.format);

        let clap = ClapCli::try_parse_from(["micro", "--json", "--format", "text"]).unwrap();
        let usage = UsageCli::parse_from(&[
            OsStr::new("--json"),
            OsStr::new("--format"),
            OsStr::new("text"),
        ])
        .unwrap();
        assert_eq!(usage.format, clap.format);

        assert_eq!(
            UsageCli::parse_from(&[]).unwrap().format,
            ClapCli::try_parse_from(["micro"]).unwrap().format
        );
    }
}

mod value_terminator {
    use super::*;

    #[derive(Debug, clap::Parser)]
    #[command(name = "micro")]
    struct ClapCli {
        #[arg(long, num_args = 1.., value_terminator = ";")]
        include: Vec<String>,
        rest: Option<String>,
    }

    #[derive(Debug, Cli)]
    #[usage(bin = "micro", unknown_flags = "error")]
    struct UsageCli {
        #[arg(long, num_args = 1.., value_terminator = ";")]
        include: Vec<String>,
        #[usage(arg)]
        rest: Option<String>,
    }

    #[test]
    fn the_terminator_ends_the_value_without_becoming_a_positional() {
        assert!(UsageCli::spec().root.cmd.flags[0].variadic);
        assert_eq!(
            UsageCli::spec().root.cmd.flags[0].value_terminator,
            Some(b";".as_slice())
        );
        let clap =
            ClapCli::try_parse_from(["micro", "--include", "one", "two", ";", "tail"]).unwrap();
        let usage = UsageCli::parse_from(&[
            OsStr::new("--include"),
            OsStr::new("one"),
            OsStr::new("two"),
            OsStr::new(";"),
            OsStr::new("tail"),
        ])
        .unwrap();
        assert_eq!(usage.include, clap.include);
        assert_eq!(usage.rest, clap.rest);
    }
}

mod missing_positionals {
    use super::*;

    #[derive(Debug, clap::Parser)]
    #[command(name = "micro", allow_missing_positional = true)]
    struct ClapCli {
        optional: Option<String>,
        required: String,
    }

    #[derive(Debug, Cli)]
    #[usage(bin = "micro", unknown_flags = "error", allow_missing_positional)]
    struct UsageCli {
        #[usage(arg)]
        optional: Option<String>,
        #[usage(arg)]
        required: String,
    }

    #[test]
    fn the_last_word_can_skip_an_earlier_optional_position() {
        let clap = ClapCli::try_parse_from(["micro", "required"]).unwrap();
        let usage = UsageCli::parse_from(&[OsStr::new("required")]).unwrap();
        assert_eq!(usage.optional, clap.optional);
        assert_eq!(usage.required, clap.required);

        let clap = ClapCli::try_parse_from(["micro", "optional", "required"]).unwrap();
        let usage =
            UsageCli::parse_from(&[OsStr::new("optional"), OsStr::new("required")]).unwrap();
        assert_eq!(usage.optional, clap.optional);
        assert_eq!(usage.required, clap.required);
    }
}

mod actions {
    use super::*;

    #[derive(Debug, clap::Parser)]
    #[command(name = "micro")]
    struct ClapCli {
        #[arg(short, long, action = clap::ArgAction::Count)]
        verbose: u8,
        #[arg(long, action = clap::ArgAction::SetFalse, default_value_t = true)]
        color: bool,
    }

    #[derive(Debug, Cli)]
    #[usage(bin = "micro", unknown_flags = "error")]
    struct UsageCli {
        #[usage(short, long, count)]
        verbose: u8,
        #[arg(long, action = clap::ArgAction::SetFalse, default_value = "true")]
        color: bool,
    }

    #[test]
    fn count_and_set_false_actions_build_the_same_values() {
        let clap = ClapCli::try_parse_from(["micro", "-vvv", "--color"]).unwrap();
        let usage = UsageCli::parse_from(&[OsStr::new("-vvv"), OsStr::new("--color")]).unwrap();
        assert_eq!(usage.verbose, clap.verbose);
        assert_eq!(usage.color, clap.color);

        let clap = ClapCli::try_parse_from(["micro"]).unwrap();
        let usage = UsageCli::parse_from(&[]).unwrap();
        assert_eq!(usage.verbose, clap.verbose);
        assert_eq!(usage.color, clap.color);

        let kdl = UsageCli::to_kdl();
        assert!(kdl.contains("flag color: negate=--color"), "{kdl}");
        let spec: usage::Spec = kdl.parse().expect("the negative-only form round-trips");
        let color = spec
            .cmd
            .flags
            .iter()
            .find(|flag| flag.name == "color")
            .expect("color");
        assert!(color.long.is_empty());
        assert_eq!(color.negate.as_deref(), Some("--color"));

        let help = usage_argv::help::render(UsageCli::spec(), UsageCli::command(), false)
            .expect("root help");
        assert!(help.contains("--color"), "{help}");
        assert!(!help.contains("color: /"), "{help}");
    }
}

mod aliases_and_case {
    use super::*;

    #[derive(Clone, Debug, PartialEq, Eq, clap::ValueEnum)]
    enum ClapMode {
        #[value(alias = "quick")]
        Fast,
        Slow,
    }

    #[derive(Clone, Debug, PartialEq, Eq, usage_derive::ValueEnum)]
    #[usage(ignore_case)]
    enum UsageMode {
        #[value(alias = "quick")]
        Fast,
        Slow,
    }

    #[derive(Debug, clap::Subcommand)]
    enum ClapCommand {
        #[command(visible_alias = "go", alias = "secret-run")]
        Run,
    }

    #[derive(Debug, usage_derive::Subcommands)]
    enum UsageCommand {
        #[command(visible_alias = "go", alias = "secret-run")]
        Run,
    }

    #[derive(Debug, clap::Parser)]
    #[command(name = "micro")]
    struct ClapCli {
        #[arg(long, visible_alias = "out", alias = "secret-output")]
        output: bool,
        #[arg(long, value_enum, ignore_case = true)]
        mode: Option<ClapMode>,
        #[command(subcommand)]
        command: Option<ClapCommand>,
    }

    #[derive(Debug, Cli)]
    #[usage(bin = "micro", unknown_flags = "error")]
    struct UsageCli {
        #[arg(long, visible_alias = "out", alias = "secret-output")]
        output: bool,
        #[usage(long, value_enum)]
        mode: Option<UsageMode>,
        #[usage(subcommand)]
        command: Option<UsageCommand>,
    }

    #[test]
    fn visible_and_hidden_aliases_bind_the_same_values() {
        for alias in ["--output", "--out", "--secret-output"] {
            let clap = ClapCli::try_parse_from(["micro", alias]).unwrap();
            let usage = UsageCli::parse_from(&[OsStr::new(alias)]).unwrap();
            assert_eq!(usage.output, clap.output, "{alias}");
        }

        for alias in ["run", "go", "secret-run"] {
            let clap = ClapCli::try_parse_from(["micro", alias]).unwrap();
            let usage = UsageCli::parse_from(&[OsStr::new(alias)]).unwrap();
            assert!(matches!(clap.command, Some(ClapCommand::Run)), "{alias}");
            assert!(matches!(usage.command, Some(UsageCommand::Run)), "{alias}");
        }

        for value in ["FAST", "quick"] {
            let clap = ClapCli::try_parse_from(["micro", "--mode", value]).unwrap();
            let usage = UsageCli::parse_from(&[OsStr::new("--mode"), OsStr::new(value)]).unwrap();
            assert!(matches!(clap.mode, Some(ClapMode::Fast)), "{value}");
            assert!(matches!(usage.mode, Some(UsageMode::Fast)), "{value}");
        }
    }

    #[test]
    fn completion_advertises_only_visible_aliases() {
        let line = "micro --";
        let split =
            usage_argv::complete::split(line, line.len(), usage_argv::complete::Shell::Bash);
        let flags: std::collections::BTreeSet<_> =
            usage_argv::complete::candidates(UsageCli::spec(), &split)
                .into_iter()
                .map(|candidate| candidate.value)
                .collect();
        assert!(flags.contains("--output"), "{flags:?}");
        assert!(flags.contains("--out"), "{flags:?}");
        assert!(!flags.contains("--secret-output"), "{flags:?}");

        let line = "micro ";
        let split =
            usage_argv::complete::split(line, line.len(), usage_argv::complete::Shell::Bash);
        let commands: std::collections::BTreeSet<_> =
            usage_argv::complete::candidates(UsageCli::spec(), &split)
                .into_iter()
                .map(|candidate| candidate.value)
                .collect();
        assert!(commands.contains("run"), "{commands:?}");
        assert!(commands.contains("go"), "{commands:?}");
        assert!(!commands.contains("secret-run"), "{commands:?}");

        let line = "micro --mode ";
        let split =
            usage_argv::complete::split(line, line.len(), usage_argv::complete::Shell::Bash);
        let values: std::collections::BTreeSet<_> =
            usage_argv::complete::candidates(UsageCli::spec(), &split)
                .into_iter()
                .map(|candidate| candidate.value)
                .collect();
        assert!(values.contains("fast"), "{values:?}");
        assert!(!values.contains("quick"), "{values:?}");
    }
}

mod help_metadata {
    use super::*;

    #[derive(Debug, clap::Subcommand)]
    enum ClapCommand {
        /// Run the task.
        Run,
        #[command(hide = true)]
        Secret,
    }

    #[derive(Debug, usage_derive::Subcommands)]
    enum UsageCommand {
        /// Run the task.
        Run,
        #[command(hide = true)]
        Secret,
    }

    #[derive(Debug, clap::Parser)]
    #[command(
        name = "micro",
        next_line_help = true,
        subcommand_help_heading = "Tasks",
        subcommand_value_name = "ACTION"
    )]
    struct ClapCli {
        /// Write JSON output.
        #[arg(long, help_heading = "Output")]
        json: bool,
        #[arg(long, hide = true)]
        secret: bool,
        #[command(subcommand)]
        command: Option<ClapCommand>,
    }

    #[derive(Debug, Cli)]
    #[allow(dead_code)]
    #[usage(
        bin = "micro",
        unknown_flags = "error",
        next_line_help = true,
        subcommand_help_heading = "Tasks",
        subcommand_value_name = "ACTION"
    )]
    struct UsageCli {
        /// Write JSON output.
        #[arg(long, help_heading = "Output")]
        json: bool,
        #[arg(long, hide = true)]
        secret: bool,
        #[usage(subcommand)]
        command: Option<UsageCommand>,
    }

    #[test]
    fn help_preserves_sections_placeholders_and_visibility() {
        let clap = ClapCli::command().render_long_help().to_string();
        let usage =
            usage_argv::help::render(UsageCli::spec(), UsageCli::spec().root.cmd, true).unwrap();

        for help in [clap, usage] {
            assert!(help.contains("ACTION"), "{help}");
            assert!(help.contains("Tasks:"), "{help}");
            assert!(help.contains("Output:"), "{help}");
            assert!(help.contains("--json"), "{help}");
            assert!(help.contains("Write JSON output"), "{help}");
            assert!(help.contains("run"), "{help}");
            assert!(!help.contains("--secret"), "{help}");
            assert!(!help.contains("secret"), "{help}");

            let lines = help.lines().collect::<Vec<_>>();
            let flag = lines
                .iter()
                .position(|line| line.trim() == "--json")
                .expect("the visible flag has its own line");
            assert!(
                lines
                    .get(flag + 1)
                    .is_some_and(|line| line.trim().starts_with("Write JSON output")),
                "next_line_help must put the description immediately below the flag:\n{help}"
            );
        }
    }
}

mod default_missing_value {
    use super::*;

    #[derive(Debug, clap::Parser)]
    #[command(name = "micro")]
    struct ClapCli {
        #[arg(
            long,
            num_args = 0..=1,
            require_equals = true,
            default_missing_value = "always"
        )]
        color: Option<String>,
    }

    #[derive(Debug, Cli)]
    #[usage(bin = "micro", unknown_flags = "error")]
    struct UsageCli {
        #[usage(long, require_equals, default_missing = "always")]
        color: Option<String>,
    }

    #[test]
    fn absent_bare_and_explicit_values_have_the_same_states() {
        for argv in [
            [].as_slice(),
            ["--color"].as_slice(),
            ["--color=never"].as_slice(),
        ] {
            let clap =
                ClapCli::try_parse_from(std::iter::once("micro").chain(argv.iter().copied()))
                    .unwrap();
            let words: Vec<&OsStr> = argv.iter().map(OsStr::new).collect();
            let usage = UsageCli::parse_from(&words).unwrap();
            assert_eq!(usage.color, clap.color, "{argv:?}");
        }
    }
}

mod flatten_and_skip {
    use super::*;

    #[derive(Debug, clap::Args)]
    struct ClapShared {
        #[arg(long)]
        verbose: bool,
        #[arg(skip)]
        cache_generation: u32,
    }

    #[derive(Debug, usage_derive::Args)]
    struct UsageShared {
        #[usage(long)]
        verbose: bool,
        #[usage(skip)]
        cache_generation: u32,
    }

    #[derive(Debug, clap::Parser)]
    #[command(name = "micro")]
    struct ClapCli {
        #[command(flatten)]
        shared: ClapShared,
        input: Option<String>,
    }

    #[derive(Debug, Cli)]
    #[usage(bin = "micro", unknown_flags = "error")]
    struct UsageCli {
        #[usage(flatten)]
        shared: UsageShared,
        #[usage(arg)]
        input: Option<String>,
    }

    #[test]
    fn flattened_values_bind_and_skipped_values_default() {
        let clap = ClapCli::try_parse_from(["micro", "--verbose", "input"]).unwrap();
        let usage = UsageCli::parse_from(&[OsStr::new("--verbose"), OsStr::new("input")]).unwrap();
        assert_eq!(usage.shared.verbose, clap.shared.verbose);
        assert_eq!(usage.shared.cache_generation, clap.shared.cache_generation);
        assert_eq!(usage.input, clap.input);
    }
}

mod trailing_values {
    use super::*;

    #[derive(Debug, clap::Parser)]
    #[command(name = "micro")]
    struct ClapCli {
        #[arg(last = true)]
        rest: Vec<String>,
    }

    #[derive(Debug, Cli)]
    #[usage(bin = "micro", unknown_flags = "error")]
    struct UsageCli {
        #[usage(arg, double_dash = "required")]
        rest: Vec<String>,
    }

    #[test]
    fn a_separator_is_required_and_not_collected() {
        let clap = ClapCli::try_parse_from(["micro", "--", "--literal", "tail"]).unwrap();
        let usage = UsageCli::parse_from(&[
            OsStr::new("--"),
            OsStr::new("--literal"),
            OsStr::new("tail"),
        ])
        .unwrap();
        assert_eq!(usage.rest, clap.rest);

        let clap = ClapCli::try_parse_from(["micro", "tail"]).unwrap_err();
        let usage = UsageCli::parse_from(&[OsStr::new("tail")]).unwrap_err();
        assert_eq!(clap.kind(), clap::error::ErrorKind::UnknownArgument);
        assert!(matches!(usage, Error::ArgRequiresDoubleDash { .. }));
    }
}

mod long_version {
    use super::*;

    #[derive(Debug, clap::Parser)]
    #[command(name = "micro", version = "1.2.3", long_version = "1.2.3\ncommit abc")]
    struct ClapCli {}

    #[derive(Debug, Cli)]
    #[usage(
        bin = "micro",
        version = "1.2.3",
        long_version = "1.2.3\ncommit abc",
        unknown_flags = "error"
    )]
    struct UsageCli {}

    fn usage_exit(argv: &[&str]) -> Exit {
        let words: Vec<&OsStr> = argv.iter().map(OsStr::new).collect();
        let error = UsageCli::parse_from(&words).expect_err("version exits");
        usage_error(UsageCli::spec(), &words, error)
    }

    #[test]
    fn short_and_long_actions_emit_the_same_versions() {
        assert_eq!(usage_exit(&["-V"]), clap_exit::<ClapCli>(&["-V"]));
        assert_eq!(
            usage_exit(&["--version"]),
            clap_exit::<ClapCli>(&["--version"])
        );
    }
}
