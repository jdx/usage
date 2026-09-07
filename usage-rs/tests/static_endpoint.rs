#![cfg(feature = "spec")]

use std::ffi::{OsStr, OsString};

#[derive(Debug, usage_rs::Cli)]
#[usage(
    bin = "static-spec",
    spec_endpoint_file = "tests/fixtures/static.usage.kdl"
)]
struct StaticSpec {
    #[usage(long)]
    verbose: bool,
}

#[test]
fn embedded_spec_matches_the_live_serializer() {
    let expected = StaticSpec::to_kdl();
    let request = [OsStr::new(usage_rs::SPEC_REQUEST)];
    assert_eq!(
        StaticSpec::spec_request(&request).as_deref(),
        Some(expected.as_str())
    );
    let outcome = StaticSpec::embedded_outcome(&[OsString::from(usage_rs::SPEC_REQUEST)]);
    let exit = outcome.exit().unwrap();
    assert_eq!(exit.text, expected);
    assert_eq!(exit.code, 0);
    assert!(!exit.stderr);
}

#[test]
fn ordinary_arguments_still_parse() {
    assert!(StaticSpec::spec_request(&[OsStr::new("--verbose")]).is_none());
    assert!(
        StaticSpec::parse_from(&[OsStr::new("--verbose")])
            .unwrap()
            .verbose
    );
}

#[cfg(not(feature = "help-advanced"))]
#[test]
#[should_panic(expected = "require the `help-advanced` feature")]
fn recursive_help_is_explicitly_unavailable_in_the_small_build() {
    usage_rs::help::render_all(StaticSpec::spec(), StaticSpec::command());
}

#[cfg(not(feature = "help-advanced"))]
#[test]
#[should_panic(expected = "require the `help-advanced` feature")]
fn hand_written_flattened_metadata_is_not_silently_ignored() {
    let root = usage_rs::spec::CommandMeta {
        flatten_help: true,
        ..*StaticSpec::spec().root
    };
    let spec = usage_rs::spec::Spec {
        root: &root,
        ..*StaticSpec::spec()
    };
    usage_rs::help::render(&spec, StaticSpec::command(), false);
}
