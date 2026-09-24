//! `choices run=`: values listed by a command, run where the program runs and described
//! everywhere else.
//!
//! The command-running cases are Unix-only because their commands are `sh` pipelines, which
//! the `cmd /c` fallback on a Windows machine without `sh` cannot run.

use std::collections::HashMap;

use usage::docs::cli::{render_help, render_runtime_help, Style};
use usage::{Parser, Spec};

fn spec(choices: &str) -> Spec {
    format!(
        r#"
name "build"
bin "build"
arg "<service>" help="The service to build" {{
    {choices}
}}
"#
    )
    .parse()
    .unwrap()
}

fn args(words: &[&str]) -> Vec<String> {
    std::iter::once("build")
        .chain(words.iter().copied())
        .map(String::from)
        .collect()
}

#[test]
fn run_is_read_and_written_back() {
    let spec = spec(r#"choices "local" run="docker compose config --services""#);
    let choices = spec.cmd.args[0].choices.as_ref().unwrap();
    assert_eq!(choices.choices, ["local"]);
    assert_eq!(choices.run(), Some("docker compose config --services"));

    let written = spec.to_string();
    assert!(
        written.contains(r#"choices local run="docker compose config --services""#),
        "{written}"
    );
    let reparsed: Spec = written.parse().unwrap();
    assert_eq!(
        reparsed.cmd.args[0].choices.as_ref().unwrap().run(),
        Some("docker compose config --services")
    );
}

#[test]
fn a_command_alone_is_enough_to_declare_choices() {
    let spec = spec(r#"choices run="docker compose config --services""#);
    assert!(spec.cmd.args[0]
        .choices
        .as_ref()
        .unwrap()
        .choices
        .is_empty());

    let err = r#"arg "<service>" { choices; }"#.parse::<Spec>().unwrap_err();
    assert!(format!("{err:?}").contains("run property"), "{err:?}");
}

#[test]
fn values_never_runs_the_command() {
    // `values` and `matches` are what static callers use; a command that would exit
    // non-zero shows it was not run, since running it would be the only way to fail.
    let spec = spec(r#"choices "local" run="exit 1""#);
    let choices = spec.cmd.args[0].choices.as_ref().unwrap();
    assert_eq!(choices.values(), ["local"]);
    assert!(choices.matches("local"));
    assert!(!choices.matches("app"));
}

#[cfg(unix)]
#[test]
fn a_value_is_checked_against_what_the_command_prints() {
    let spec = spec(r#"choices "local" run="echo app; echo; echo '  database  '""#);
    let parsed = usage::parse(&spec, &args(&["database"])).unwrap();
    assert_eq!(parsed.as_env()["usage_service"], "database");
    usage::parse(&spec, &args(&["local"])).unwrap();

    let err = usage::parse(&spec, &args(&["nope"]))
        .unwrap_err()
        .to_string();
    assert_eq!(
        err,
        "Invalid choice for arg service: nope, expected one of local, app, database"
    );
}

#[cfg(unix)]
#[test]
fn ignore_case_and_strict_apply_to_command_values() {
    let spec_ = spec(r#"choices ignore_case=#true run="echo app""#);
    usage::parse(&spec_, &args(&["APP"])).unwrap();

    // Not strict: nothing is refused, so nothing needs to be run to find out.
    let spec_ = spec(r#"choices strict=#false run="exit 1""#);
    usage::parse(&spec_, &args(&["anything"])).unwrap();
}

#[cfg(unix)]
#[test]
fn a_command_that_fails_is_a_clear_parse_error() {
    let spec = spec(r#"choices run="exit 3""#);
    let err = usage::parse(&spec, &args(&["app"]))
        .unwrap_err()
        .to_string();
    assert!(
        err.starts_with("Could not check arg service: app against its choices: exited with code 3"),
        "{err}"
    );
}

#[cfg(unix)]
#[test]
fn a_command_that_prints_nothing_says_so() {
    let spec = spec(r#"choices run="true""#);
    let err = usage::parse(&spec, &args(&["app"]))
        .unwrap_err()
        .to_string();
    assert_eq!(
        err,
        "Invalid choice for arg service: app, `true` printed no choices"
    );
}

#[cfg(unix)]
#[test]
fn the_command_runs_once_per_parse() {
    let dir = tempfile::tempdir().unwrap();
    let log = dir.path().join("runs");
    let spec: Spec = format!(
        r#"
bin "build"
arg "<services>..." default="app" {{
    choices run="echo ran >> '{}'; echo app; echo database"
}}
flag "--also <service>" default="database" {{
    choices run="echo ran >> '{}'; echo app; echo database"
}}
"#,
        log.display(),
        log.display()
    )
    .parse()
    .unwrap();

    usage::parse(&spec, &args(&["app", "database", "app"])).unwrap();
    assert_eq!(std::fs::read_to_string(&log).unwrap().lines().count(), 1);

    // A second parse asks again: the answer is not kept on the spec.
    std::fs::remove_file(&log).unwrap();
    usage::parse(&spec, &args(&[])).unwrap();
    assert_eq!(std::fs::read_to_string(&log).unwrap().lines().count(), 1);
}

#[cfg(unix)]
#[test]
fn the_parse_environment_reaches_the_command() {
    let spec = spec(r#"choices run="echo $BUILD_SERVICES""#);
    let env = HashMap::from([("BUILD_SERVICES".to_string(), "worker".to_string())]);
    Parser::new(&spec)
        .with_env(env)
        .parse(&args(&["worker"]))
        .unwrap();
}

#[cfg(unix)]
#[test]
fn a_running_programs_help_lists_the_values_and_a_static_page_describes_them() {
    let spec = spec(r#"choices "local" run="echo app; echo database""#);

    let runtime = render_runtime_help(&spec, &spec.cmd, true, Style::PLAIN, None);
    assert!(
        runtime.contains("[possible values: local, app, database]"),
        "{runtime}"
    );
    assert!(!runtime.contains("output of"), "{runtime}");

    let short = render_runtime_help(&spec, &spec.cmd, false, Style::PLAIN, None);
    assert!(short.contains("[local, app, database]"), "{short}");

    // The static page runs nothing, so `app` cannot appear on it.
    let long = render_help(&spec, &spec.cmd, true);
    assert!(!long.contains("app,"), "{long}");
    assert!(
        long.contains("[possible values: output of `echo app; echo database`]"),
        "{long}"
    );
    let short = render_help(&spec, &spec.cmd, false);
    assert!(!short.contains("app,"), "{short}");
    assert!(short.contains("[local] [output of `echo app;"), "{short}");
}

#[cfg(unix)]
#[test]
fn a_parse_answers_help_with_the_running_programs_page() {
    let spec = spec(r#"choices run="echo app; echo database""#);
    let err = usage::parse(&spec, &args(&["--help"]))
        .unwrap_err()
        .to_string();
    assert!(err.contains("[possible values: app, database]"), "{err}");
}

#[cfg(unix)]
#[test]
fn help_describes_a_command_that_fails_instead_of_listing_nothing() {
    let spec = spec(r#"choices "local" run="exit 1""#);
    let page = render_runtime_help(&spec, &spec.cmd, true, Style::PLAIN, None);
    assert!(page.contains("[possible values: local]"), "{page}");
    assert!(
        page.contains("[possible values: output of `exit 1`]"),
        "{page}"
    );
}

#[test]
fn markdown_describes_the_command_without_running_it() {
    let spec = spec(r#"choices "local" run="echo app""#);
    let md = usage::docs::markdown::MarkdownRenderer::new(spec.clone())
        .render_arg(&spec.cmd.args[0])
        .unwrap();
    assert!(md.contains("**Choices:** output of `echo app`"), "{md}");
    assert!(!md.contains("- `app`"), "{md}");
}
