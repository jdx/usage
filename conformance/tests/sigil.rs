//! Sigil-classified positional arguments agree between the typed and interpreted parsers.

use std::ffi::OsStr;

use usage::parse::ParseValue;
use usage::Spec as LibSpec;
use usage_derive::Cli;

#[derive(Debug, Cli)]
#[usage(bin = "overlay")]
struct Overlay {
    /// Temporary tool selections.
    #[usage(sigil = "+")]
    tools: Vec<String>,
    /// Command to execute.
    command: String,
    /// Arguments passed to the command.
    args: Vec<String>,
}

fn interpreted_values(spec: &LibSpec, argv: &[&str], name: &str) -> Vec<String> {
    let argv = std::iter::once("overlay")
        .chain(argv.iter().copied())
        .map(str::to_string)
        .collect::<Vec<_>>();
    let parsed = usage::Parser::new(spec).parse(&argv).expect("valid invocation");
    parsed
        .args
        .iter()
        .find(|(arg, _)| arg.name.eq_ignore_ascii_case(name))
        .map(|(_, value)| match value {
            ParseValue::String(value) => vec![value.clone()],
            ParseValue::MultiString(values) => values.clone(),
            other => panic!("unexpected value for {name}: {other:?}"),
        })
        .unwrap_or_default()
}

#[test]
fn sigils_strip_and_do_not_advance_the_positional_cursor() {
    let argv = [
        OsStr::new("+node@27"),
        OsStr::new("+python@3.14"),
        OsStr::new("node"),
        OsStr::new("-v"),
    ];
    let typed = Overlay::parse_from(&argv).expect("typed parser accepts overlays");
    assert_eq!(typed.tools, ["node@27", "python@3.14"]);
    assert_eq!(typed.command, "node");
    assert_eq!(typed.args, ["-v"]);

    let spec: LibSpec = Overlay::to_kdl().parse().expect("derived KDL is valid");
    assert_eq!(spec.cmd.args[0].sigil.as_deref(), Some("+"));
    assert_eq!(interpreted_values(&spec, &["+node@27", "node"], "tools"), ["node@27"]);
    assert_eq!(interpreted_values(&spec, &["+node@27", "node"], "command"), ["node"]);
}

#[test]
fn double_dash_protects_a_literal_sigil_word() {
    let argv = [OsStr::new("node"), OsStr::new("--"), OsStr::new("+literal")];
    let typed = Overlay::parse_from(&argv).expect("literal reaches ordinary args");
    assert!(typed.tools.is_empty());
    assert_eq!(typed.args, ["+literal"]);

    let spec: LibSpec = Overlay::to_kdl().parse().expect("derived KDL is valid");
    assert!(interpreted_values(&spec, &["node", "--", "+literal"], "tools").is_empty());
    assert_eq!(
        interpreted_values(&spec, &["node", "--", "+literal"], "args"),
        ["+literal"]
    );
}
