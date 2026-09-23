//! Plus spellings agree between the typed and interpreted parsers.

use std::ffi::OsStr;

use usage::parse::ParseValue;
use usage::Spec as LibSpec;
use usage_derive::Cli;

#[derive(Debug, Cli)]
#[usage(bin = "sh")]
struct Shell {
    /// Trace commands as they run.
    #[usage(short, negate = "+x")]
    xtrace: bool,
    /// Set a shell option.
    #[usage(short = 'o', var)]
    option: Vec<String>,
    /// Unset a shell option.
    #[usage(plus_short = 'o', var)]
    unset_option: Vec<String>,
}

fn interpreted(spec: &LibSpec, argv: &[&str], name: &str) -> Option<ParseValue> {
    let argv = std::iter::once("sh")
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
fn a_plus_spelling_negates_a_switch_and_names_a_flag_of_its_own() {
    let argv = [
        OsStr::new("-x"),
        OsStr::new("-o"),
        OsStr::new("errexit"),
        OsStr::new("+o"),
        OsStr::new("pipefail"),
    ];
    let typed = Shell::parse_from(&argv).expect("typed parser reads plus spellings");
    assert!(typed.xtrace);
    assert_eq!(typed.option, ["errexit"]);
    assert_eq!(typed.unset_option, ["pipefail"]);

    let spec: LibSpec = Shell::to_kdl().parse().expect("derived KDL is valid");
    // A round trip names each flag after its spelling, as it does for any short.
    let xtrace = spec.cmd.flags.iter().find(|f| f.name == "x").unwrap();
    assert_eq!(xtrace.negate.as_deref(), Some("+x"));
    // The derive names the flag after its field; a spec that writes only `flag "+o"` gets
    // `plus-o`, so that `-o` and `+o` stay two flags rather than one.
    let unset = spec.cmd.flags.iter().find(|f| f.name == "plus-o").unwrap();
    assert_eq!(unset.plus_short, ['o']);

    let argv = ["-x", "-o", "errexit", "+o", "pipefail"];
    assert!(matches!(
        interpreted(&spec, &argv, "x"),
        Some(ParseValue::Bool(true))
    ));
    assert!(matches!(
        interpreted(&spec, &argv, "plus-o"),
        Some(ParseValue::MultiString(values)) if values == ["pipefail"]
    ));
}

#[test]
fn the_plus_form_turns_the_switch_off() {
    let typed = Shell::parse_from(&[OsStr::new("-x"), OsStr::new("+x")]).expect("both spellings");
    assert!(!typed.xtrace);
}
