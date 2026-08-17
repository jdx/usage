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

/// A parent with a required flag of its own, to pit against a flattened rule.
#[derive(Args)]
struct Strict {
    /// Which file — a bare `String`, so it is required
    #[usage(long)]
    file: String,
    #[usage(flatten)]
    listing: Listing,
}

#[derive(Subcommands)]
enum StrictCommands {
    /// Be strict
    Strict(Box<Strict>),
}

#[derive(Cli)]
#[usage(bin = "strict")]
struct StrictCli {
    #[usage(subcommand)]
    command: Option<StrictCommands>,
}

#[test]
fn a_flattened_rule_is_reported_before_the_parent_notices_something_missing() {
    // Both fail: `yaml` is not a choice, and `--file` was never given. The choice error is
    // the useful one — it is about a word the user typed, where the other is about one they
    // did not. Same principle that already puts conflicts before required-ness.
    //
    // Asserted because the ordering is easy to get wrong by moving one line, and a comment
    // claiming it is not the same as a test holding it: the first version of this splice ran
    // the flattened checks last while its comment said otherwise.
    let a = argv(["strict", "--format", "yaml"]);
    match StrictCli::parse_from(&a) {
        Err(Error::InvalidChoice { name, .. }) => assert_eq!(name, "format"),
        Err(other) => panic!("the choice error should come first, got: {other:?}"),
        Ok(_) => panic!("neither rule was satisfied"),
    }

    // And with nothing typed wrong, the missing flag is still reported.
    let a = argv(["strict"]);
    assert!(matches!(
        StrictCli::parse_from(&a),
        Err(Error::MissingRequired { .. })
    ));

    // Satisfying both gives both: a required flag of the parent's own alongside the group it
    // flattened, which is the arrangement the two rules were competing over.
    let a = argv(["strict", "--file", "mise.toml", "--format", "json"]);
    let Some(StrictCommands::Strict(strict)) = StrictCli::parse_from(&a)
        .expect("both rules satisfied")
        .command
    else {
        panic!("expected `strict`")
    };
    assert_eq!(strict.file, "mise.toml");
    assert_eq!(strict.listing.format.as_deref(), Some("json"));
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

/// A group declared inside a struct that gets flattened somewhere else.
#[derive(Args)]
#[usage(group("output", required))]
struct Emitting {
    /// Emit JSON
    #[usage(long, group = "output")]
    json: bool,
    /// Emit YAML
    #[usage(long, group = "output")]
    yaml: bool,
}

#[derive(Cli)]
#[usage(bin = "fl")]
struct Flattened {
    #[usage(flatten)]
    emitting: Emitting,
    /// Where to write
    #[usage(long)]
    out: Option<String>,
}

#[test]
fn a_flattened_structs_group_is_enforced_and_emitted() {
    // Enforced: the child's own `check` runs, so the group holds on the command that
    // flattened it.
    let a = ["--json", "--out", "o"].map(OsStr::new);
    let fl = Flattened::parse_from(&a).expect("one member");
    assert!(fl.emitting.json && !fl.emitting.yaml);
    assert_eq!(fl.out.as_deref(), Some("o"));

    let a = ["--json", "--yaml"].map(OsStr::new);
    assert!(matches!(
        Flattened::parse_from(&a),
        Err(Error::ConflictingFlags { .. })
    ));

    let a: [&OsStr; 0] = [];
    assert!(matches!(
        Flattened::parse_from(&a),
        Err(Error::MissingGroup {
            group: "output",
            ..
        })
    ));

    // And emitted, which is the half that can silently rot: the flags are joined into
    // the parent's tables, so the group describing them has to be joined too, or the
    // spec would describe a CLI without a rule the CLI enforces.
    let kdl = Flattened::to_kdl();
    assert!(
        kdl.contains(r#"group "output" "--json" "--yaml" required=#true"#),
        "{kdl}"
    );
    let spec: LibSpec = kdl.parse().expect("the emitted spec should parse");
    assert_eq!(spec.cmd.groups.len(), 1);
    assert!(spec.cmd.groups[0].required);
}

/// A second group to flatten, so a struct can hold one on each side of one.
#[allow(dead_code)]
#[derive(Args)]
#[usage(group("format"))]
struct Formatting {
    /// Compact output
    #[usage(long, group = "format")]
    compact: bool,
    /// Pretty output
    #[usage(long, group = "format")]
    pretty: bool,
}

/// A group before the flattened field, and another after it.
#[allow(dead_code)]
#[derive(Cli)]
#[usage(bin = "ord", group("source"), group("sink"))]
struct GroupOrder {
    /// Read from a file
    #[usage(long, group = "source")]
    file: Option<String>,
    /// Read from a URL
    #[usage(long, group = "source")]
    url: Option<String>,
    #[usage(flatten)]
    formatting: Formatting,
    /// Write to a file
    #[usage(long, group = "sink")]
    out: Option<String>,
    /// Write to stdout
    #[usage(long, group = "sink")]
    stdout: bool,
}

#[test]
fn a_flattened_structs_groups_land_where_the_field_was_written() {
    // The order the flag and argument tables already promise, on the groups beside them:
    // a flattened struct's groups splice in at the field, rather than after everything this
    // struct declares. Emitting all the local ones first put `sink` — written below the
    // flattened field — above the group that field brought in.
    let kdl = GroupOrder::to_kdl();
    let at = |name: &str| kdl.find(name).unwrap_or_else(|| panic!("{name} in {kdl}"));
    assert!(
        at("\"source\"") < at("\"format\"") && at("\"format\"") < at("\"sink\""),
        "groups follow the fields that declare them: {kdl}"
    );

    let spec: usage::Spec = kdl.parse().expect("the emitted spec should parse");
    assert_eq!(
        spec.cmd
            .groups
            .iter()
            .map(|g| g.name.as_str())
            .collect::<Vec<_>>(),
        ["source", "format", "sink"],
    );
}
