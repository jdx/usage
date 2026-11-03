//! Settings bound by a flag inside a clause.
//!
//! A clause is a list of the same group, one item per `[flags] tool` the user wrote, so a flag in
//! it is declared once and given any number of times. Its binding belongs in the root's tables
//! the way a flattened group's does: it is a flag the CLI reads into a setting, wherever it sits.
//! What several items contribute is what one command line repeating the flag would: every item of
//! a list setting, and the last value of any other.

use std::ffi::OsStr;

use usage_config::{Layers, Ty, Value, resolve};
use usage_derive::{Args, Cli};

/// One tool to install
#[derive(Args, Debug)]
struct Tool {
    /// The script to run after installing
    #[usage(long, setting = "postinstall")]
    postinstall: Option<String>,

    /// Paths to leave alone
    #[usage(long, setting = "exclude")]
    exclude: Vec<String>,

    tool: String,
}

/// A tool whose only bound flags are in its clause
#[derive(Cli, Debug)]
#[usage(bin = "ex", settings)]
struct Ex {
    #[usage(clause)]
    tools: Vec<Tool>,
}

fn parse(tokens: &[&str]) -> (Ex, usage_config::CliLayer) {
    let argv: Vec<&OsStr> = tokens.iter().map(OsStr::new).collect();
    Ex::parse_from_with_settings(&argv).expect("should parse")
}

#[test]
fn a_clause_binds_the_flags_its_items_declare() {
    assert_eq!(
        Ex::SETTINGS_BINDINGS,
        &[("--postinstall", "postinstall"), ("--exclude", "exclude")],
    );
    let keys: Vec<_> = Ex::SETTINGS_PROPS.iter().map(|p| (p.key, p.ty)).collect();
    assert_eq!(
        keys,
        [
            ("postinstall", Ty::String),
            ("exclude", Ty::List(&Ty::String))
        ]
    );
    assert_eq!(
        Ex::SETTINGS_REGISTRY.drift(Ex::SETTINGS_BINDINGS),
        Vec::<String>::new()
    );
}

#[test]
fn one_item_contributes_what_it_was_given() {
    let (cli, layer) = parse(&["--postinstall", "npm i -g pnpm", "node"]);
    assert_eq!(cli.tools.len(), 1);
    assert_eq!(cli.tools[0].postinstall.as_deref(), Some("npm i -g pnpm"));

    let resolved = resolve(Ex::SETTINGS_REGISTRY, Layers::new().then(&layer)).expect("resolves");
    assert_eq!(
        resolved.get_key("postinstall"),
        Some(&Value::from("npm i -g pnpm"))
    );
    assert_eq!(
        resolved.origin_key("postinstall").map(|o| o.describe()),
        Some("--postinstall")
    );
}

#[test]
fn several_items_contribute_the_last_value_and_every_list_item() {
    let (cli, layer) = parse(&[
        "--postinstall",
        "first",
        "--exclude",
        "a",
        "node",
        "--exclude",
        "b",
        "--exclude",
        "c",
        "python",
        "--postinstall",
        "last",
        "ruby",
    ]);
    assert_eq!(cli.tools.len(), 3, "the parser saw three items");

    let resolved = resolve(Ex::SETTINGS_REGISTRY, Layers::new().then(&layer)).expect("resolves");
    assert_eq!(resolved.get_key("postinstall"), Some(&Value::from("last")));
    assert_eq!(
        resolved.get_key("exclude"),
        Some(&Value::List(vec![
            Value::from("a"),
            Value::from("b"),
            Value::from("c")
        ]))
    );
}

#[test]
fn items_that_were_given_nothing_contribute_nothing() {
    let (cli, layer) = parse(&["node", "python"]);
    assert_eq!(cli.tools.len(), 2);
    assert!(layer.is_empty(), "no flag, no entries");
}
