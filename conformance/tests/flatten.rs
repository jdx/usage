//! Sharing declarations between commands.
//!
//! mise writes `ConfigLs` once and gives it to both `config` and `config ls`, so that
//! `mise config --no-header` and `mise config ls --no-header` accept the same flags. Ten
//! commands do this. It is a Rust-side device with no counterpart in the spec: the emitted
//! KDL lists the flags inline, exactly as a hand-written command would, because a spec
//! describes what a CLI accepts and not how the declarations were organised.
//!
//! The tables are joined at compile time, so the parser walks one flat slice and flatten
//! costs nothing at run time.

use std::ffi::OsStr;

use usage::Spec as LibSpec;
use usage_argv::Error;
use usage_derive::{Args, Cli, Subcommands};

fn argv<const N: usize>(tokens: [&str; N]) -> [&OsStr; N] {
    tokens.map(OsStr::new)
}

/// The shared declarations, as mise's `ConfigLs` is.
#[derive(Args)]
struct Listing {
    /// Do not print a header
    #[usage(long)]
    no_header: bool,
    /// Output format
    #[usage(long, choices("json", "table"))]
    format: Option<String>,
    /// What to list
    #[usage(arg, name = "WHAT")]
    what: Option<String>,
}

/// The subcommand, which holds them directly.
#[derive(Args)]
struct ConfigLs {
    #[usage(flatten)]
    listing: Listing,
}

#[derive(Subcommands)]
enum ConfigCommands {
    /// List config
    Ls(Box<ConfigLs>),
}

/// The parent, which flattens the same struct so a bare `config` behaves like `config ls`.
#[derive(Args)]
struct Config {
    /// Only this file
    #[usage(long, short = 'f')]
    file: Option<String>,
    #[usage(subcommand)]
    command: Option<ConfigCommands>,
    #[usage(flatten)]
    listing: Listing,
}

#[derive(Subcommands)]
enum Commands {
    /// Manage config
    Config(Box<Config>),
}

#[derive(Cli)]
#[usage(bin = "ex")]
struct Ex {
    #[usage(subcommand)]
    command: Option<Commands>,
}

fn config_of(tokens: &[&OsStr]) -> Config {
    let Some(Commands::Config(config)) = Ex::parse_from(tokens).expect("should parse").command
    else {
        panic!("expected `config`")
    };
    *config
}

#[test]
fn a_flattened_flag_binds_on_the_command_that_flattened_it() {
    let a = argv(["config", "--no-header", "--format", "json"]);
    let config = config_of(&a);
    assert!(config.listing.no_header);
    assert_eq!(config.listing.format.as_deref(), Some("json"));

    // And the parent's own flags still work beside them.
    let a = argv(["config", "-f", "mise.toml", "--no-header"]);
    let config = config_of(&a);
    assert_eq!(config.file.as_deref(), Some("mise.toml"));
    assert!(config.listing.no_header);
}

#[test]
fn the_same_struct_serves_two_commands() {
    // The point of flatten: `config` and `config ls` accept the same flags, declared once.
    let a = argv(["config", "ls", "--no-header", "keys"]);
    let config = config_of(&a);
    let Some(ConfigCommands::Ls(ls)) = config.command else {
        panic!("expected `ls`")
    };
    assert!(ls.listing.no_header);
    assert_eq!(ls.listing.what.as_deref(), Some("keys"));

    // The parent's copy is untouched: two commands, two sets of values.
    assert!(!config.listing.no_header);
}

#[test]
fn a_flattened_positional_takes_its_declared_place() {
    let a = argv(["config", "keys"]);
    let config = config_of(&a);
    assert_eq!(config.listing.what.as_deref(), Some("keys"));
}

#[test]
fn a_flattened_declaration_is_still_checked() {
    // `choices` lives in the flattened struct, and only its own derive knows about it — so
    // this proves the delegation runs rather than the flags merely binding.
    let a = argv(["config", "--format", "yaml"]);
    match Ex::parse_from(&a) {
        Err(Error::InvalidChoice { name, choices }) => {
            assert_eq!(name, "format");
            assert_eq!(choices, ["json", "table"]);
        }
        Err(other) => panic!("wrong error: {other:?}"),
        Ok(_) => panic!("`yaml` is not one of the choices"),
    }
}

#[test]
fn the_spec_shows_the_flags_inline() {
    // A spec has no idea of flattening, and does not need one: what reaches the KDL is the
    // command with every flag it accepts, indistinguishable from one written out by hand.
    let kdl = Ex::to_kdl();
    let spec: LibSpec = kdl.parse().expect("valid spec");
    let config = spec.cmd.subcommands.get("config").expect("config");

    let mut flags: Vec<&str> = config
        .flags
        .iter()
        .flat_map(|f| f.long.iter().map(|l| l.as_str()))
        .collect();
    flags.sort_unstable();
    assert_eq!(flags, ["file", "format", "no-header"], "{kdl}");

    assert_eq!(
        config
            .args
            .iter()
            .map(|a| a.name.as_str())
            .collect::<Vec<_>>(),
        ["WHAT"],
        "{kdl}"
    );

    // Help text travels with the declaration, so the shared flags are documented on both.
    let ls = config.subcommands.get("ls").expect("ls");
    assert_eq!(
        ls.flags
            .iter()
            .find(|f| f.long.iter().any(|l| l == "no-header"))
            .and_then(|f| f.help.as_deref()),
        Some("Do not print a header")
    );

    // Nothing about the Rust-side arrangement leaks in.
    assert!(!kdl.contains("flatten"), "{kdl}");
    assert!(!kdl.contains("listing"), "{kdl}");
}

/// A struct that flattens something which itself flattens.
#[derive(Args)]
struct Outer {
    /// Whether to be loud
    #[usage(long)]
    verbose: bool,
    #[usage(flatten)]
    inner: ConfigLs,
}

#[derive(Cli)]
#[usage(bin = "nested")]
struct Nested {
    #[usage(flatten)]
    outer: Outer,
}

#[test]
fn flattening_nests() {
    // Nothing special makes this work: every level is the same delegation, so a flatten of a
    // flatten joins three tables into one.
    let a = argv(["--verbose", "--no-header", "keys"]);
    let nested = Nested::parse_from(&a).expect("should parse");
    assert!(nested.outer.verbose);
    assert!(nested.outer.inner.listing.no_header);
    assert_eq!(nested.outer.inner.listing.what.as_deref(), Some("keys"));
}
