//! What a program calls itself, and what version it reports.
//!
//! Two papercuts the communique port hit, both of which made an adopter write something twice
//! or get an answer they did not mean.

use usage::Spec as LibSpec;
use usage_derive::{Cli, Subcommands};

/// A tool named after its binary
///
/// `bin` is given and `name` is not — which is the ordinary case, and used to banner itself
/// `cli 0.0.0` after the struct.
#[derive(Cli)]
#[usage(bin = "communique", version)]
struct Cli_ {
    #[usage(long)]
    plain: bool,
}

/// A tool that names itself something else
#[derive(Cli)]
#[usage(name = "the-tool", bin = "tool", version = "9.9")]
struct Renamed {
    #[usage(long)]
    plain: bool,
}

#[derive(Cli)]
#[usage(name = "busy", bin = "busy", multicall)]
struct Busy {
    #[usage(subcommand)]
    command: Option<BusyCommand>,
}

#[derive(Subcommands)]
enum BusyCommand {
    Run,
}

#[test]
fn a_program_is_called_what_its_binary_is_called() {
    // The name defaults to the struct's, and a struct is usually called `Cli`. So `bin` alone
    // produced `name cli`, the banner read `cli 1.3.1`, and communique had to declare the same
    // word twice to avoid it.
    let spec: LibSpec = Cli_::to_kdl().parse().expect("valid spec");
    assert_eq!(spec.name, "communique");
    assert_eq!(spec.bin, "communique");

    let page = usage_argv::help::render(Cli_::spec(), Cli_::spec().root.cmd, false).expect("page");
    assert!(page.starts_with("communique "), "{page}");
}

#[test]
fn a_declared_name_still_wins() {
    // The default is only a default: a CLI whose program name differs from its binary says so
    // and is believed.
    let spec: LibSpec = Renamed::to_kdl().parse().expect("valid spec");
    assert_eq!(spec.name, "the-tool");
    assert_eq!(spec.bin, "tool");
}

#[test]
fn a_bare_version_is_the_packages_own() {
    // `#[usage(version)]` with no value, as clap spells it. The expansion has to reach for
    // `CARGO_PKG_VERSION` in the *adopter's* crate — usage-derive has a version of its own and
    // it is not the answer — so this is the conformance crate's version, whatever it is.
    let spec: LibSpec = Cli_::to_kdl().parse().expect("valid spec");
    assert_eq!(spec.version.as_deref(), Some(env!("CARGO_PKG_VERSION")));

    // And it reaches the flag, which is the point of declaring one at all.
    use std::ffi::OsStr;
    let argv = [OsStr::new("--version")];
    assert!(matches!(
        Cli_::parse_from(&argv),
        Err(usage_argv::Error::Version)
    ));
}

#[test]
fn a_written_version_is_taken_as_written() {
    let spec: LibSpec = Renamed::to_kdl().parse().expect("valid spec");
    assert_eq!(spec.version.as_deref(), Some("9.9"));
}

#[test]
fn the_fields_are_bound() {
    use std::ffi::OsStr;
    let argv = [OsStr::new("--plain")];
    assert!(Cli_::parse_from(&argv).expect("should parse").plain);
    assert!(Renamed::parse_from(&argv).expect("should parse").plain);
}

#[test]
fn a_full_argv_helper_matches_clap_shaped_tests_and_multicall() {
    use std::ffi::OsStr;

    let ordinary = [OsStr::new("communique"), OsStr::new("--plain")];
    assert!(
        Cli_::parse_from_argv(&ordinary)
            .expect("should parse")
            .plain
    );

    let dispatcher = [OsStr::new("busy"), OsStr::new("run")];
    assert!(matches!(
        Busy::parse_from_argv(&dispatcher)
            .expect("dispatcher form should parse")
            .command,
        Some(BusyCommand::Run)
    ));

    let applet = [OsStr::new("/usr/local/bin/run")];
    assert!(matches!(
        Busy::parse_from_argv(&applet)
            .expect("applet form should parse")
            .command,
        Some(BusyCommand::Run)
    ));
}
