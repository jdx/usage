//! Declared choices can be suggestions without becoming a closed vocabulary.

use std::ffi::{OsStr, OsString};

use usage::Spec;
use usage_argv::Error;
use usage_derive::Cli;

#[derive(Cli)]
#[usage(bin = "backend", completion)]
struct Backend {
    /// A built-in backend name or an external backend identifier
    #[usage(long, choices("core", "git"), choices_strict = false)]
    backend: Option<String>,
}

#[derive(Cli)]
#[usage(bin = "strict-backend")]
#[allow(dead_code)]
struct StrictBackend {
    #[usage(long, choices("core", "git"))]
    backend: Option<String>,
}

#[test]
fn typed_non_strict_choices_accept_values_outside_the_suggestions() {
    let argv = [OsStr::new("--backend"), OsStr::new("vendor:custom")];
    assert_eq!(
        Backend::parse_from(&argv).unwrap().backend.as_deref(),
        Some("vendor:custom")
    );
    assert!(matches!(
        StrictBackend::parse_from(&argv),
        Err(Error::InvalidChoice { .. })
    ));
}

#[test]
fn the_portable_spec_keeps_the_suggestions_without_enforcing_them() {
    let kdl = Backend::to_kdl();
    assert!(kdl.contains("choices strict=#false core git"), "{kdl}");

    let spec: Spec = kdl.parse().expect("the emitted spec should load");
    let argv = [
        "backend".to_string(),
        "--backend".to_string(),
        "other".to_string(),
    ];
    assert!(usage::parse::parse(&spec, &argv).is_ok());

    let help = usage_argv::help::render(Backend::spec(), Backend::command(), true).unwrap();
    assert!(help.contains("possible values: core, git"), "{help}");

    let request = [
        "__complete_word__",
        "--shell",
        "bash",
        "--line",
        "backend --backend ",
    ]
    .map(OsString::from);
    assert_eq!(
        Backend::completion_request(&request).unwrap(),
        "core\ngit\n"
    );
}
