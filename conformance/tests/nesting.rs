//! Commands inside commands, to any depth.
//!
//! mise is four levels deep in places (`mise bootstrap macos launchd-agents apply`),
//! so one level was never going to be enough. A nested command is not a special case
//! here: an `Args` struct carries a `subcommand` field exactly as the root does, and
//! generates the same code for it.

use std::ffi::OsStr;

use usage::Spec as LibSpec;
use usage_argv::Error;
use usage_derive::{Args, Cli, Subcommands};

fn argv<const N: usize>(tokens: [&str; N]) -> [&OsStr; N] {
    tokens.map(OsStr::new)
}

/// A tool with commands inside commands
#[derive(Cli)]
#[usage(bin = "ex")]
struct Ex {
    /// Say more
    #[usage(short = 'v', long, global)]
    verbose: bool,
    /// What to do
    #[usage(subcommand)]
    command: Commands,
}

#[derive(Subcommands)]
enum Commands {
    /// Manage settings
    Settings(Settings),
    /// Install a tool
    Install(Install),
}

/// Manage settings
#[derive(Args)]
struct Settings {
    /// Which settings file
    #[usage(long)]
    file: Option<String>,
    /// What to do with them
    #[usage(subcommand)]
    command: SettingsCommands,
}

#[derive(Subcommands)]
enum SettingsCommands {
    /// Set a value
    Set(SettingsSet),
    /// Show every value
    Ls(SettingsLs),
}

/// Set a value
#[derive(Args)]
struct SettingsSet {
    /// Which setting
    key: String,
    /// The value
    value: String,
}

/// Show every value
#[derive(Args)]
struct SettingsLs {
    /// As JSON
    #[usage(long)]
    json: bool,
}

/// Install a tool
#[derive(Args)]
struct Install {
    /// What to install
    tool: String,
}

#[test]
fn three_levels_route_to_the_deepest_command() {
    let a = argv(["settings", "set", "jobs", "8"]);
    let ex = Ex::parse_from(&a).expect("should parse");

    let Commands::Settings(settings) = ex.command else {
        panic!("expected settings");
    };
    let SettingsCommands::Set(set) = settings.command else {
        panic!("expected settings set");
    };
    assert_eq!((set.key.as_str(), set.value.as_str()), ("jobs", "8"));
}

#[test]
fn each_level_keeps_its_own_flags() {
    // `--file` belongs to `settings`, `--json` to `settings ls`, `--verbose` to the
    // root and inherited by both.
    let a = argv(["--verbose", "settings", "--file", "a.toml", "ls", "--json"]);
    let ex = Ex::parse_from(&a).expect("should parse");
    assert!(ex.verbose, "a global reaches any depth");

    let Commands::Settings(settings) = ex.command else {
        panic!("expected settings");
    };
    assert_eq!(settings.file.as_deref(), Some("a.toml"));
    let SettingsCommands::Ls(ls) = settings.command else {
        panic!("expected settings ls");
    };
    assert!(ls.json);
}

#[test]
fn a_global_works_after_the_deepest_command_too() {
    let a = argv(["settings", "ls", "--verbose"]);
    let ex = Ex::parse_from(&a).expect("should parse");
    assert!(ex.verbose);
}

#[test]
fn a_middle_command_can_require_one_of_its_own() {
    // `settings` alone is incomplete: its `subcommand` field is not an `Option`.
    let a = argv(["settings"]);
    assert!(matches!(Ex::parse_from(&a), Err(Error::MissingSubcommand)));
}

#[test]
fn a_deep_commands_requirements_are_its_own() {
    // `set` needs two arguments; reaching it without them is its failure, not the
    // root's, and running a sibling is not held to it.
    let a = argv(["settings", "set", "jobs"]);
    assert!(matches!(
        Ex::parse_from(&a),
        Err(Error::MissingRequired { name: "VALUE" })
    ));

    let a = argv(["settings", "ls"]);
    assert!(Ex::parse_from(&a).is_ok(), "ls requires nothing");
}

#[test]
fn a_sibling_at_the_top_still_works() {
    let a = argv(["install", "node"]);
    let ex = Ex::parse_from(&a).expect("should parse");
    let Commands::Install(install) = ex.command else {
        panic!("expected install");
    };
    assert_eq!(install.tool, "node");
}

#[test]
fn the_spec_nests_the_same_way() {
    let kdl = Ex::to_kdl();
    let spec: LibSpec = kdl.parse().unwrap_or_else(|e| panic!("{e}\n\n{kdl}"));

    let settings = &spec.cmd.subcommands["settings"];
    let mut inner: Vec<&str> = settings.subcommands.keys().map(String::as_str).collect();
    inner.sort();
    assert_eq!(inner, ["ls", "set"]);

    let set = &settings.subcommands["set"];
    assert_eq!(set.args.len(), 2);
    assert_eq!(set.help.as_deref(), Some("Set a value"));
}

#[test]
fn the_emitted_spec_reads_like_a_handwritten_one() {
    insta::assert_snapshot!(Ex::to_kdl());
}

/// Two commands whose argument structs share a Rust type *name*, in different
/// modules. Keys are derived from the whole declaration rather than the name, so
/// these do not collide — hashing the name alone gave both structs the same key and
/// selected the wrong command.
mod add {
    /// Add something
    #[derive(usage_derive::Args)]
    pub struct Op {
        /// What to add
        pub target: String,
    }
}

mod remove {
    /// Remove something
    #[derive(usage_derive::Args)]
    pub struct Op {
        /// What to remove
        pub target: String,
        /// Whether to force it
        #[usage(long)]
        pub force: bool,
    }
}

#[derive(Cli)]
#[usage(bin = "same")]
struct Same {
    #[usage(subcommand)]
    command: SameCommands,
}

#[derive(Subcommands)]
enum SameCommands {
    /// Add something
    #[usage(name = "add")]
    Add(add::Op),
    /// Remove something
    #[usage(name = "remove")]
    Remove(remove::Op),
}

#[test]
fn same_named_structs_in_different_modules_do_not_collide() {
    let a = argv(["remove", "x", "--force"]);
    let same = Same::parse_from(&a).expect("should parse");
    let SameCommands::Remove(op) = same.command else {
        panic!("expected remove, which means the keys collided");
    };
    assert_eq!(op.target, "x");
    assert!(op.force);

    let a = argv(["add", "y"]);
    let same = Same::parse_from(&a).expect("should parse");
    let SameCommands::Add(op) = same.command else {
        panic!("expected add");
    };
    assert_eq!(op.target, "y");

    // `to_kdl` asserts the tree holds no duplicate keys, so this would panic in debug
    // if the two still shared one.
    assert!(Same::to_kdl().contains("cmd remove"));
}
