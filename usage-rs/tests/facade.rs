#![cfg(feature = "spec")]
#![deny(unused_variables)]

use std::ffi::OsStr;
use std::path::{Path, PathBuf};

use usage_rs as usage;
use usage_rs::{Args, Cli, Subcommands, ValueEnum};

const INLINE_AFTER_HELP: &str = "Inline command details from a Rust constant.";

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

#[derive(Subcommands, serde::Deserialize)]
enum InlineCommand {
    /// Run a named benchmark.
    #[usage(after_long_help = INLINE_AFTER_HELP)]
    Run {
        #[serde(default)]
        #[arg(long)]
        bench: Option<String>,
        #[usage(long)]
        runs: Option<u32>,
        #[cfg_attr(all(), arg(long))]
        iterations: Option<u32>,
        #[arg]
        label: Option<String>,
        #[cfg(any())]
        #[arg(long)]
        platform_only: Option<String>,
    },
    Empty {},
    PlatformOnly {
        #[cfg(any())]
        #[arg(long)]
        platform_only: Option<String>,
    },
}

#[derive(Cli)]
#[usage(bin = "inline-ex")]
struct InlineEx {
    #[usage(subcommand)]
    command: InlineCommand,
}

#[derive(ValueEnum)]
#[usage(ignore_case)]
enum Shell {
    #[value(aliases(["bourne-again", "bash-shell"]))]
    Bash,
    #[value(aliases = ["shell-z", "z-shell", "zsh-shell"])]
    Zsh,
    #[cfg(windows)]
    PowerShell,
}

impl std::str::FromStr for Shell {
    type Err = String;

    fn from_str(value: &str) -> Result<Self, Self::Err> {
        #[cfg(windows)]
        if value.eq_ignore_ascii_case("power-shell") {
            return Ok(Self::PowerShell);
        }
        if value.eq_ignore_ascii_case("bash") {
            Ok(Self::Bash)
        } else if value.eq_ignore_ascii_case("zsh") || value.eq_ignore_ascii_case("shell-z") {
            Ok(Self::Zsh)
        } else {
            Err(format!("unsupported shell: {value}"))
        }
    }
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

#[derive(Cli)]
#[usage(bin = "unit-root")]
struct UnitRoot;

#[derive(Args)]
struct UnitArgs;

#[derive(Subcommands)]
enum UnitArgsCommand {
    Empty(UnitArgs),
}

#[derive(Cli)]
#[usage(bin = "unit-args")]
struct UnitArgsCli {
    #[usage(subcommand)]
    command: UnitArgsCommand,
}

const DEFAULT_RUNS: u32 = 7;
const DYNAMIC_ABOUT: &str = "Metadata from a Rust constant.";
const DYNAMIC_AFTER_HELP: &str = "More details from a Rust constant.";

fn computed_version() -> &'static str {
    "1.2.3+runtime"
}

#[derive(Cli)]
#[usage(
    bin = "dynamic-ex",
    version = computed_version(),
    version_spec = "1.2.3",
    about = DYNAMIC_ABOUT,
    after_long_help = DYNAMIC_AFTER_HELP
)]
struct DynamicEx {
    #[usage(long, default_value_t = DEFAULT_RUNS, default = "7")]
    runs: u32,
    #[usage(long, default_value_t, default = "0")]
    retries: u16,
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
    assert!(Ex::to_kdl().contains("complete \"file\" type=\"path\""));

    let embedded = Ex::app().name("embedded").bin("embedded").spec();
    assert_eq!(embedded.name, "embedded");
    assert_eq!(embedded.bin, Some("embedded"));
}

#[test]
fn unit_subcommands_use_the_facade_derive() {
    let cli = Ex::parse_from(&[OsStr::new("version")]).expect("valid unit subcommand");
    assert!(matches!(cli.command, Command::Version));
}

#[test]
fn unit_cli_and_args_structs_parse_without_shape_rewrites() {
    let root = UnitRoot::parse_from(&[]).expect("unit root should parse");
    let UnitRoot = root;
    assert!(UnitRoot::to_kdl().contains("name \"unit-root\""));

    let cli =
        UnitArgsCli::parse_from(&[OsStr::new("empty")]).expect("unit Args command should parse");
    assert!(matches!(cli.command, UnitArgsCommand::Empty(UnitArgs)));
    assert!(UnitArgsCli::to_kdl().contains("cmd \"empty\""));
}

#[test]
fn struct_style_subcommands_bind_fields_in_place() {
    let cli = InlineEx::parse_from(&[
        OsStr::new("run"),
        OsStr::new("--bench"),
        OsStr::new("startup"),
        OsStr::new("--runs"),
        OsStr::new("5"),
        OsStr::new("--iterations"),
        OsStr::new("9"),
        OsStr::new("nightly"),
    ])
    .expect("inline fields should parse");
    let InlineCommand::Run {
        bench,
        runs,
        iterations,
        label,
    } = cli.command
    else {
        panic!("run command should be selected");
    };
    assert_eq!(bench.as_deref(), Some("startup"));
    assert_eq!(runs, Some(5));
    assert_eq!(iterations, Some(9));
    assert_eq!(label.as_deref(), Some("nightly"));

    let kdl = InlineEx::to_kdl();
    assert!(kdl.contains("cmd \"run\""), "{kdl}");
    assert!(kdl.contains("flag \"--bench\""), "{kdl}");
    assert!(kdl.contains("arg \"<BENCH>\""), "{kdl}");
    assert!(kdl.contains("flag \"--runs\""), "{kdl}");
    assert!(kdl.contains("arg \"<RUNS>\""), "{kdl}");
    assert!(kdl.contains("flag \"--iterations\""), "{kdl}");
    assert!(kdl.contains("arg \"[LABEL]\""), "{kdl}");
    assert!(kdl.contains(INLINE_AFTER_HELP), "{kdl}");
}

#[test]
fn empty_struct_style_subcommands_do_not_emit_unused_bindings() {
    let cli = InlineEx::parse_from(&[OsStr::new("empty")]).expect("empty command should parse");
    assert!(matches!(cli.command, InlineCommand::Empty {}));

    let cli = InlineEx::parse_from(&[OsStr::new("platform-only")])
        .expect("a command whose fields are cfg'd out should parse");
    assert!(matches!(cli.command, InlineCommand::PlatformOnly {}));
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
    assert!(kdl.contains("alias \"z-shell\" hide=#true"), "{kdl}");
    assert!(kdl.contains("alias \"zsh-shell\" hide=#true"), "{kdl}");
    assert!(kdl.contains("alias \"bourne-again\" hide=#true"), "{kdl}");
    #[cfg(not(windows))]
    assert!(!kdl.contains("power-shell"), "{kdl}");
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

#[test]
fn runtime_metadata_expressions_have_explicit_portable_values() {
    let _parse_entry = DynamicEx::parse as fn() -> DynamicEx;
    let cli = DynamicEx::parse_from(&[]).expect("the typed default should be evaluated");
    assert_eq!(cli.runs, DEFAULT_RUNS);
    assert_eq!(cli.retries, 0);

    let kdl = DynamicEx::to_kdl();
    assert!(kdl.contains("version \"1.2.3\""), "{kdl}");
    assert!(kdl.contains("default=\"7\""), "{kdl}");
    assert!(kdl.contains("default=\"0\""), "{kdl}");
    assert!(kdl.contains(DYNAMIC_ABOUT), "{kdl}");
    assert!(kdl.contains(DYNAMIC_AFTER_HELP), "{kdl}");
    let spec: usage_parser::Spec = kdl.parse().expect("the static values should be portable");
    assert_eq!(spec.version.as_deref(), Some("1.2.3"));
}
