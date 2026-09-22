//! Single-dash longs agree between the typed and interpreted parsers.

use std::ffi::OsStr;

use usage::parse::ParseValue;
use usage::Spec as LibSpec;
use usage_derive::Cli;

#[derive(Debug, Cli)]
#[usage(bin = "ld", single_dash_long)]
struct Linker {
    /// Build a shared object.
    #[usage(long)]
    shared: bool,
    /// Write the output here.
    #[usage(short, long)]
    output: Option<String>,
    /// Set NMAGIC, which ld spells with two dashes so `-omagic` stays `-o magic`.
    #[usage(long, single_dash_long = false)]
    omagic: bool,
}

fn interpreted(spec: &LibSpec, argv: &[&str], name: &str) -> Option<ParseValue> {
    let argv = std::iter::once("ld")
        .chain(argv.iter().copied())
        .map(str::to_string)
        .collect::<Vec<_>>();
    let parsed = usage::Parser::new(spec)
        .parse(&argv)
        .expect("valid invocation");
    parsed
        .flags
        .iter()
        .find(|(flag, _)| flag.name == name)
        .map(|(_, value)| value.clone())
}

#[test]
fn a_single_dash_names_a_long_unless_the_flag_opts_out() {
    let argv = [OsStr::new("-shared"), OsStr::new("-omagic")];
    let typed = Linker::parse_from(&argv).expect("typed parser reads single-dash longs");
    assert!(typed.shared);
    assert!(!typed.omagic);
    assert_eq!(typed.output.as_deref(), Some("magic"));

    let spec: LibSpec = Linker::to_kdl().parse().expect("derived KDL is valid");
    assert_eq!(spec.cmd.single_dash_long, Some(true));
    let omagic = spec.cmd.flags.iter().find(|f| f.name == "omagic").unwrap();
    assert_eq!(omagic.single_dash_long, Some(false));

    let argv = ["-shared", "-omagic"];
    assert!(matches!(
        interpreted(&spec, &argv, "shared"),
        Some(ParseValue::Bool(true))
    ));
    assert!(interpreted(&spec, &argv, "omagic").is_none());
    assert!(matches!(
        interpreted(&spec, &argv, "output"),
        Some(ParseValue::String(value)) if value == "magic"
    ));
}

#[test]
fn the_opted_out_flag_keeps_two_dashes() {
    let typed = Linker::parse_from(&[OsStr::new("--omagic")]).expect("double dash binds");
    assert!(typed.omagic);
}
