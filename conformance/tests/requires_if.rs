//! Value-conditional requirements through the compiled derive path.

use std::ffi::OsStr;

use usage::Spec as LibSpec;
use usage_argv::Error;
use usage_derive::Cli;

fn argv<const N: usize>(tokens: [&str; N]) -> [&OsStr; N] {
    tokens.map(OsStr::new)
}

#[derive(Cli)]
#[usage(bin = "conditional")]
struct Conditional {
    #[usage(
        long,
        requires_if("json", "--schema"),
        requires_ifs(("signed", "--key"), ("remote", "--token"))
    )]
    format: Option<String>,
    #[usage(long)]
    schema: Option<String>,
    #[usage(long)]
    key: Option<String>,
    #[usage(long)]
    token: Option<String>,
}

#[test]
fn each_value_activates_only_its_requirement() {
    let unrelated = argv(["--format", "text"]);
    Conditional::parse_from(&unrelated).expect("an unrelated value requires nothing");

    for (value, missing) in [("json", "schema"), ("signed", "key"), ("remote", "token")] {
        let words = argv(["--format", value]);
        assert!(matches!(
            Conditional::parse_from(&words),
            Err(Error::MissingRequired { name }) if name == missing
        ));
    }

    let satisfied = argv(["--format", "json", "--schema", "schema.json"]);
    let parsed = Conditional::parse_from(&satisfied).expect("the matching flag satisfies it");
    assert_eq!(parsed.format.as_deref(), Some("json"));
    assert_eq!(parsed.schema.as_deref(), Some("schema.json"));
    assert!(parsed.key.is_none());
    assert!(parsed.token.is_none());
}

#[test]
fn the_conditions_reach_the_emitted_spec() {
    let spec: LibSpec = Conditional::to_kdl().parse().expect("valid emitted spec");
    let format = spec
        .cmd
        .flags
        .iter()
        .find(|flag| flag.name == "format")
        .expect("format flag");
    assert_eq!(format.requires_if.len(), 3);
    assert_eq!(format.requires_if[0].value, "json");
    assert_eq!(format.requires_if[0].requires, "--schema");
    assert_eq!(format.requires_if[2].value, "remote");
    assert_eq!(format.requires_if[2].requires, "--token");
}

#[derive(Cli)]
#[usage(bin = "defaulted")]
struct Defaulted {
    #[usage(long, default = "json", requires_if("json", "--schema"))]
    format: Option<String>,
    #[usage(long)]
    schema: Option<String>,
}

#[test]
fn a_default_does_not_activate_a_conditional_requirement() {
    let empty = argv([]);
    let parsed = Defaulted::parse_from(&empty).expect("defaults are not explicit");
    assert_eq!(parsed.format.as_deref(), Some("json"));
    assert!(parsed.schema.is_none());
}
