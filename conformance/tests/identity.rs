//! Two commands that are declared identically in different modules.
//!
//! A derive cannot see a module path, so it hashes the declaration it was handed — and
//! two byte-identical declarations hash alike. That used to be waved off as harmless,
//! on the grounds that a key only chooses which `match` arm to jump to and each arm
//! checks the event came from its own table. It was not harmless: `Spec::to_kdl`
//! asserts no two things in a CLI share a key, so a perfectly good CLI failed the
//! assertion. The module is now folded into the key where `module_path!()` can be read.

use std::ffi::OsStr;

use usage_derive::{Cli, Subcommands};

mod add {
    use usage_derive::Args;

    /// Add a thing
    #[derive(Args)]
    pub struct Op {
        /// Do it anyway
        #[usage(long)]
        pub force: bool,
    }
}

mod remove {
    use usage_derive::Args;

    /// Remove a thing
    ///
    /// Declared byte-for-byte like `add::Op` above, doc comment included, which is what
    /// makes the two hash alike.
    #[derive(Args)]
    pub struct Op {
        /// Do it anyway
        #[usage(long)]
        pub force: bool,
    }
}

#[derive(Subcommands)]
enum Commands {
    Add(add::Op),
    Remove(remove::Op),
}

/// A tool with two identical commands
#[derive(Cli)]
#[usage(bin = "ex")]
struct Ex {
    #[usage(subcommand)]
    command: Option<Commands>,
}

fn argv<const N: usize>(tokens: [&str; N]) -> [&OsStr; N] {
    tokens.map(OsStr::new)
}

#[test]
fn the_spec_holds_no_duplicate_keys() {
    // `to_kdl` asserts it, in the debug build tests run in.
    let kdl = Ex::to_kdl();
    assert!(kdl.contains(r#"cmd "add""#), "{kdl}");
    assert!(kdl.contains(r#"cmd "remove""#), "{kdl}");
}

#[test]
fn each_command_still_binds_its_own_flag() {
    let a = argv(["add", "--force"]);
    let Some(Commands::Add(op)) = Ex::parse_from(&a).expect("should parse").command else {
        panic!("expected `add`")
    };
    assert!(op.force);

    let a = argv(["remove", "--force"]);
    let Some(Commands::Remove(op)) = Ex::parse_from(&a).expect("should parse").command else {
        panic!("expected `remove`")
    };
    assert!(op.force);

    // And the flag is not silently unclaimed when it is absent.
    let a = argv(["remove"]);
    let Some(Commands::Remove(op)) = Ex::parse_from(&a).expect("should parse").command else {
        panic!("expected `remove`")
    };
    assert!(!op.force);
}
