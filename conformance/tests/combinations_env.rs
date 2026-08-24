//! Environment-mutating interaction coverage lives in its own test process.

use std::ffi::OsStr;

use usage::Spec as LibSpec;
use usage_derive::{Args, Cli, Subcommands};

fn argv<const N: usize>(words: [&str; N]) -> [&OsStr; N] {
    words.map(OsStr::new)
}

#[derive(Cli)]
#[usage(bin = "layered-list")]
struct LayeredList {
    #[usage(
        long,
        env = "USAGE_COMBINATION_VALUES",
        default = "fallback-a,fallback-b",
        delimiter = ','
    )]
    values: Vec<String>,
}

#[derive(Cli)]
#[usage(bin = "ordered-env")]
struct OrderedEnv {
    #[usage(
        long,
        env = "USAGE_ENV_CANONICAL",
        env_fallback("USAGE_ENV_FALLBACK_A", "USAGE_ENV_FALLBACK_B"),
        deprecated_env("USAGE_ENV_DEPRECATED"),
        default = "fallback"
    )]
    value: Option<String>,
}

#[derive(Cli)]
#[usage(bin = "ordered-positional-env")]
struct OrderedPositionalEnv {
    #[usage(
        env = "USAGE_ARG_ENV_CANONICAL",
        env_fallback("USAGE_ARG_ENV_FALLBACK_A", "USAGE_ARG_ENV_FALLBACK_B"),
        deprecated_env("USAGE_ARG_ENV_DEPRECATED"),
        default = "fallback"
    )]
    input: Option<String>,
}

#[allow(dead_code)]
#[derive(Args)]
struct FlattenedEnvironment {
    #[usage(env_fallback("USAGE_FLAT_ARG_FALLBACK"), default = "fallback")]
    input: Option<String>,
    #[usage(long, env_fallback("USAGE_FLAT_FLAG_FALLBACK"), default = "fallback")]
    value: Option<String>,
}

#[allow(dead_code)]
#[derive(Subcommands)]
enum FlattenedCommands {
    Run(Box<FlattenedEnvironment>),
}

#[allow(dead_code)]
#[derive(Cli)]
#[usage(bin = "flattened-env", flatten_help, next_line_help)]
struct FlattenedEnvCli {
    #[usage(subcommand)]
    command: Option<FlattenedCommands>,
}

#[test]
fn argv_env_defaults_and_delimiters_keep_their_precedence() {
    unsafe { std::env::remove_var("USAGE_COMBINATION_VALUES") };
    assert_eq!(
        LayeredList::parse_from(&[]).unwrap().values,
        ["fallback-a", "fallback-b"]
    );

    unsafe { std::env::set_var("USAGE_COMBINATION_VALUES", "env-a,env-b") };
    assert_eq!(
        LayeredList::parse_from(&[]).unwrap().values,
        ["env-a", "env-b"]
    );
    assert_eq!(
        LayeredList::parse_from(&argv(["--values", "argv-a,argv-b"]))
            .unwrap()
            .values,
        ["argv-a", "argv-b"]
    );
    unsafe { std::env::remove_var("USAGE_COMBINATION_VALUES") };
}

#[test]
fn ordered_environment_names_use_first_set_value() {
    for name in [
        "USAGE_ENV_CANONICAL",
        "USAGE_ENV_FALLBACK_A",
        "USAGE_ENV_FALLBACK_B",
        "USAGE_ENV_DEPRECATED",
    ] {
        unsafe { std::env::remove_var(name) };
    }

    unsafe {
        std::env::set_var("USAGE_ENV_DEPRECATED", "deprecated");
        std::env::set_var("USAGE_ENV_FALLBACK_B", "fallback-b");
        std::env::set_var("USAGE_ENV_FALLBACK_A", "fallback-a");
        std::env::set_var("USAGE_ENV_CANONICAL", "canonical");
    }
    assert_eq!(
        OrderedEnv::parse_from(&[]).unwrap().value.as_deref(),
        Some("canonical")
    );

    unsafe { std::env::remove_var("USAGE_ENV_CANONICAL") };
    assert_eq!(
        OrderedEnv::parse_from(&[]).unwrap().value.as_deref(),
        Some("fallback-a")
    );

    unsafe { std::env::remove_var("USAGE_ENV_FALLBACK_A") };
    assert_eq!(
        OrderedEnv::parse_from(&[]).unwrap().value.as_deref(),
        Some("fallback-b")
    );

    unsafe { std::env::remove_var("USAGE_ENV_FALLBACK_B") };
    assert_eq!(
        OrderedEnv::parse_from(&[]).unwrap().value.as_deref(),
        Some("deprecated")
    );

    let spec: LibSpec = OrderedEnv::to_kdl().parse().expect("valid generated spec");
    let flag = spec.cmd.flags.first().expect("generated value flag");
    assert_eq!(
        flag.env_fallback,
        ["USAGE_ENV_FALLBACK_A", "USAGE_ENV_FALLBACK_B"]
    );
    assert_eq!(flag.deprecated_env, ["USAGE_ENV_DEPRECATED"]);
    let typed_help =
        usage_argv::help::render(OrderedEnv::spec(), OrderedEnv::spec().root.cmd, false)
            .expect("root page");
    let fallback_notes = " [env fallback: USAGE_ENV_FALLBACK_A] [env fallback: USAGE_ENV_FALLBACK_B] [deprecated env: USAGE_ENV_DEPRECATED]";
    let ordered_notes = format!("{fallback_notes} (default: fallback)");
    // Read with the layout collapsed: the narrow page wraps, so a run of notes this long is
    // split across lines. What is under test is that they arrive in this order, not where the
    // break falls.
    assert!(
        flattened(&typed_help).contains(&ordered_notes),
        "{typed_help}"
    );
    let reference_help = usage::docs::cli::render_help(&spec, &spec.cmd, false);
    assert!(
        flattened(&reference_help).contains(&ordered_notes),
        "{reference_help}"
    );

    unsafe { std::env::remove_var("USAGE_ENV_DEPRECATED") };
}

#[test]
fn positional_environment_names_round_trip_and_use_first_set_value() {
    for name in [
        "USAGE_ARG_ENV_CANONICAL",
        "USAGE_ARG_ENV_FALLBACK_A",
        "USAGE_ARG_ENV_FALLBACK_B",
        "USAGE_ARG_ENV_DEPRECATED",
    ] {
        unsafe { std::env::remove_var(name) };
    }

    unsafe {
        std::env::set_var("USAGE_ARG_ENV_DEPRECATED", "deprecated");
        std::env::set_var("USAGE_ARG_ENV_FALLBACK_B", "fallback-b");
        std::env::set_var("USAGE_ARG_ENV_FALLBACK_A", "fallback-a");
        std::env::set_var("USAGE_ARG_ENV_CANONICAL", "canonical");
    }
    assert_eq!(
        OrderedPositionalEnv::parse_from(&[])
            .unwrap()
            .input
            .as_deref(),
        Some("canonical")
    );
    unsafe { std::env::remove_var("USAGE_ARG_ENV_CANONICAL") };
    assert_eq!(
        OrderedPositionalEnv::parse_from(&[])
            .unwrap()
            .input
            .as_deref(),
        Some("fallback-a")
    );

    let kdl = OrderedPositionalEnv::to_kdl();
    let spec: LibSpec = kdl.parse().expect("valid generated spec");
    let arg = spec.cmd.args.first().expect("generated positional");
    assert_eq!(
        arg.env_fallback,
        ["USAGE_ARG_ENV_FALLBACK_A", "USAGE_ARG_ENV_FALLBACK_B"],
        "{kdl}"
    );
    assert_eq!(arg.deprecated_env, ["USAGE_ARG_ENV_DEPRECATED"]);
    let typed_help = usage_argv::help::render(
        OrderedPositionalEnv::spec(),
        OrderedPositionalEnv::spec().root.cmd,
        false,
    )
    .expect("root page");
    let fallback_notes = " [env fallback: USAGE_ARG_ENV_FALLBACK_A] [env fallback: USAGE_ARG_ENV_FALLBACK_B] [deprecated env: USAGE_ARG_ENV_DEPRECATED]";
    let ordered_notes = format!("{fallback_notes} (default: fallback)");
    // Read with the layout collapsed: the narrow page wraps, so a run of notes this long is
    // split across lines. What is under test is that they arrive in this order, not where the
    // break falls.
    assert!(
        flattened(&typed_help).contains(&ordered_notes),
        "{typed_help}"
    );
    let reference_help = usage::docs::cli::render_help(&spec, &spec.cmd, false);
    assert!(
        flattened(&reference_help).contains(&ordered_notes),
        "{reference_help}"
    );

    for name in [
        "USAGE_ARG_ENV_FALLBACK_A",
        "USAGE_ARG_ENV_FALLBACK_B",
        "USAGE_ARG_ENV_DEPRECATED",
    ] {
        unsafe { std::env::remove_var(name) };
    }
}

#[test]
fn flattened_next_line_help_indents_environment_notes() {
    let page = usage_argv::help::render(
        FlattenedEnvCli::spec(),
        FlattenedEnvCli::spec().root.cmd,
        false,
    )
    .expect("root page");
    for note in [
        "    [env fallback: USAGE_FLAT_ARG_FALLBACK]\n    (default: fallback)",
        "    [env fallback: USAGE_FLAT_FLAG_FALLBACK]\n    (default: fallback)",
    ] {
        assert!(page.contains(note), "{page}");
    }
}

/// A page with its wrapping taken back out, for a test about what it says rather than how it
/// is laid out.
fn flattened(page: &str) -> String {
    page.split_whitespace().collect::<Vec<_>>().join(" ")
}
