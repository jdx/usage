//! Unambiguous long-flag and subcommand prefixes, through both Rust parsers.

use std::ffi::OsStr;

use usage::Spec as LibSpec;
use usage_derive::{Args, Cli, Subcommands};

#[derive(Cli)]
#[usage(
    name = "ex",
    unknown_flags = "error",
    infer_subcommands,
    infer_long_args
)]
struct Ex {
    #[usage(long)]
    verbose: bool,
    #[usage(long)]
    verify: bool,
    #[usage(subcommand)]
    command: Option<Commands>,
}

#[derive(Subcommands)]
enum Commands {
    Install(Install),
    Inspect(Inspect),
    #[usage(alias = "uninstall")]
    Remove(Remove),
}

#[derive(Args)]
struct Install {
    #[usage(long)]
    forceful: bool,
}

#[derive(Args)]
struct Inspect;

#[derive(Args)]
struct Remove;

fn argv<const N: usize>(tokens: [&str; N]) -> [&OsStr; N] {
    tokens.map(OsStr::new)
}

fn words<const N: usize>(tokens: [&str; N]) -> Vec<String> {
    tokens.iter().map(|token| token.to_string()).collect()
}

#[test]
fn the_typed_parser_accepts_only_unique_prefixes() {
    let parsed = Ex::parse_from(&argv(["insta", "--for"]))
        .expect("root inference should reach the child and stay enabled there");
    let Some(Commands::Install(install)) = parsed.command else {
        panic!("expected install")
    };
    assert!(install.forceful);

    assert!(
        matches!(
            Ex::parse_from(&argv(["uni"]))
                .expect("an alias prefix should route")
                .command,
            Some(Commands::Remove(_))
        ),
        "aliases participate in inference"
    );

    assert!(Ex::parse_from(&argv(["ins"])).is_err());
    assert!(Ex::parse_from(&argv(["--ver"])).is_err());

    let verbose = Ex::parse_from(&argv(["--verb"])).expect("a unique long prefix should bind");
    assert!(verbose.verbose);
    assert!(!verbose.verify);

    let exact = Ex::parse_from(&argv(["install"])).expect("exact names still outrank prefixes");
    assert!(matches!(exact.command, Some(Commands::Install(_))));
}

#[test]
fn emitted_kdl_gives_usage_lib_the_same_policy() {
    let kdl = Ex::to_kdl();
    assert!(kdl.contains("infer_subcommands #true"), "{kdl}");
    assert!(kdl.contains("infer_long_args #true"), "{kdl}");
    let spec: LibSpec = kdl.parse().expect("valid spec");

    let parsed = usage::parse::parse(&spec, &words(["ex", "insta", "--for"]))
        .expect("usage-lib should accept the same unique prefixes");
    assert_eq!(parsed.cmd.name, "install");

    assert!(usage::parse::parse(&spec, &words(["ex", "ins"])).is_err());
    assert!(usage::parse::parse(&spec, &words(["ex", "--ver"])).is_err());
}
