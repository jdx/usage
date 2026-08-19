#![cfg(feature = "spec")]

use std::ffi::OsStr;
use std::path::{Path, PathBuf};

use usage_rs as usage;
use usage_rs::{Args, Cli, Subcommands, ValueEnum};

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

#[derive(ValueEnum)]
#[usage(ignore_case)]
enum Shell {
    Bash,
    #[usage(alias = "shell-z")]
    Zsh,
}

#[derive(Cli)]
#[usage(bin = "choice-ex")]
struct ChoiceEx {
    #[usage(long, value_enum)]
    shell: Shell,
}

#[derive(Cli)]
#[usage(bin = "strict-ex", unknown_flags = "error")]
struct StrictEx {}

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
    assert!(Ex::to_kdl().contains("complete \"file\" type=\"path\""));
}

#[test]
fn unit_subcommands_use_the_facade_derive() {
    let cli = Ex::parse_from(&[OsStr::new("version")]).expect("valid unit subcommand");
    assert!(matches!(cli.command, Command::Version));
}

#[test]
fn defaults_render_clap_shaped_parse_errors() {
    let argv = [OsStr::new("--wat")];
    let Err(err) = Ex::parse_from(&argv) else {
        panic!("unknown flag should fail");
    };
    // `render_failure` colours via `Style::auto()` when stderr is a TTY or
    // `CLICOLOR_FORCE` is set, which would put ANSI codes inside the quotes and
    // break a literal substring check. Plain style is what a pipe (and this
    // assertion) wants.
    let message =
        usage::diagnostic::render(Ex::spec(), &argv, &err, usage::diagnostic::Style::PLAIN);
    assert!(
        message.contains("unexpected argument '--wat'"),
        "defaults should enable diagnostics; got:\n{message}"
    );
}

#[test]
fn emitted_specs_preserve_value_enum_aliases_and_case_policy() {
    let cli = ChoiceEx::parse_from(&[OsStr::new("--shell"), OsStr::new("SHELL-Z")])
        .expect("aliases and ASCII case folding should parse");
    assert!(matches!(cli.shell, Shell::Zsh));

    let kdl = ChoiceEx::to_kdl();
    assert!(kdl.contains("choices ignore_case=#true"), "{kdl}");
    assert!(kdl.contains("alias \"shell-z\" hide=#true"), "{kdl}");
}

#[test]
fn emitted_parser_settings_are_portable_spec_metadata() {
    let Err(_) = StrictEx::parse_from(&[OsStr::new("--wat")]) else {
        panic!("strict parsing should reject an unknown flag");
    };

    let kdl = StrictEx::to_kdl();
    assert!(kdl.contains("unknown_flags \"error\""), "{kdl}");

    let spec: usage_parser::Spec = kdl.parse().expect("usage-lib should read derive output");
    assert_eq!(
        spec.unknown_flags,
        Some(usage_parser::UnknownFlags::Error),
        "the portable spec should retain the runtime setting"
    );
}
