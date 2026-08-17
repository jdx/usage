//! What an unrecognized flag means, and how far a command's answer reaches.
//!
//! usage-lib resolves this by walking outward from the command that ran and falling back to
//! the spec's own setting — `effective_unknown_flags` in `lib/src/parse.rs`. usage-argv held
//! the effective value per command instead, on the theory that whoever built the tables could
//! resolve it. A derive cannot: it expands one struct at a time and cannot see the command
//! above. So `#[usage(unknown_flags = "error")]` reached the root alone, and on an `Args` it
//! was accepted and then ignored — a declaration that compiled and did nothing.
//!
//! Found by converting usage-cli itself to the derive, where every command wants the strict
//! reading and the five that forward a command line to somebody else's script want the
//! lenient one.

use std::ffi::OsStr;

use usage::Spec as LibSpec;
use usage_argv::Error;
use usage_derive::{Args, Cli, Subcommands};

/// A command that says nothing, and so is as strict as the root.
#[derive(Args)]
struct Build {
    /// Say more
    #[usage(long)]
    verbose: bool,
    /// What to build
    target: Option<String>,
}

/// A command that forwards what it does not recognise, as `usage bash` does.
#[derive(Args)]
#[usage(unknown_flags = "value")]
struct Exec {
    /// The script and its own options
    args: Vec<String>,
}

/// A command inside a command, to check the answer reaches past one level.
#[derive(Args)]
struct Inner {
    /// Say more
    #[usage(long)]
    verbose: bool,
}

#[derive(Subcommands)]
enum Nested {
    /// Two levels down, and still strict
    Inner(Box<Inner>),
}

#[derive(Args)]
struct Outer {
    #[usage(subcommand)]
    command: Option<Nested>,
}

#[derive(Subcommands)]
enum Commands {
    /// Build something
    Build(Box<Build>),
    /// Run something else
    Exec(Box<Exec>),
    /// A group
    Outer(Box<Outer>),
}

/// A CLI that owns all of its flags.
#[derive(Cli)]
#[usage(bin = "ex", unknown_flags = "error")]
struct Ex {
    #[usage(subcommand)]
    command: Option<Commands>,
}

fn argv<const N: usize>(tokens: [&str; N]) -> [&OsStr; N] {
    tokens.map(OsStr::new)
}

#[test]
fn a_subcommand_inherits_the_roots_answer() {
    // The bug this is here for: `--nope` used to be offered to `build`'s positional, so the
    // *target* became `--nope` and the real target was the unexpected word — where clap, and
    // the root of this very CLI, name the flag.
    let a = argv(["build", "--nope"]);
    let Err(err) = Ex::parse_from(&a) else {
        panic!("the root said unknown flags are errors")
    };
    assert!(
        matches!(err, Error::UnknownFlag { .. }),
        "the flag is named, not swallowed by a positional: {err:?}"
    );

    // And the flag it *does* declare still binds, so this is strictness rather than a
    // command that stopped working.
    let a = argv(["build", "--verbose", "release"]);
    let Some(Commands::Build(build)) = Ex::parse_from(&a).expect("should parse").command else {
        panic!("expected `build`")
    };
    assert!(build.verbose);
    assert_eq!(build.target.as_deref(), Some("release"));
}

#[test]
fn it_reaches_past_one_level() {
    let a = argv(["outer", "inner", "--nope"]);
    let Err(err) = Ex::parse_from(&a) else {
        panic!("inherited two levels down")
    };
    assert!(matches!(err, Error::UnknownFlag { .. }), "{err:?}");

    let a = argv(["outer", "inner", "--verbose"]);
    let Some(Commands::Outer(outer)) = Ex::parse_from(&a).expect("should parse").command else {
        panic!("expected `outer`")
    };
    let Some(Nested::Inner(inner)) = outer.command else {
        panic!("expected `inner`")
    };
    assert!(inner.verbose);
}

#[test]
fn a_command_that_declares_its_own_keeps_it() {
    // And this is why inheritance is not enough on its own: a command that forwards a command
    // line to somebody else's program has to be able to say so.
    let a = argv(["exec", "./script.sh", "--its-own-flag"]);
    let ex = Ex::parse_from(&a).expect("a forwarding command takes it as a value");
    let Some(Commands::Exec(exec)) = ex.command else {
        panic!("expected `exec`")
    };
    assert_eq!(exec.args, ["./script.sh", "--its-own-flag"]);
}

#[test]
fn a_declaration_that_does_nothing_is_not_a_declaration() {
    // The emitted spec says the same thing the parser does, which is the whole claim: `exec`
    // differs from what it inherited and says so, and `build` inherits and stays quiet.
    let kdl = Ex::to_kdl();
    let spec: LibSpec = kdl.parse().expect("valid spec");
    assert_eq!(
        spec.unknown_flags,
        Some(usage::UnknownFlags::Error),
        "the root's own setting: {kdl}"
    );
    let cmd = |name: &str| spec.cmd.subcommands.get(name).expect("declared");
    assert_eq!(cmd("exec").unknown_flags, Some(usage::UnknownFlags::Value));
    assert_eq!(
        cmd("build").unknown_flags,
        None,
        "a command that says nothing writes nothing, and inherits when read back: {kdl}"
    );
}

#[test]
fn the_reference_reads_it_the_same_way() {
    // The oracle, on the spec this CLI emits: usage-lib walks outward from the command that
    // ran, so `build` is strict and `exec` is not. A disagreement here is the divergence this
    // change is about, in the direction that would matter.
    let spec: LibSpec = Ex::to_kdl().parse().expect("valid spec");

    let strict = usage::parse::parse(&spec, &words(["ex", "build", "--nope"]));
    assert!(strict.is_err(), "usage-lib inherits the root's `error` too");

    let forwarding = usage::parse::parse(&spec, &words(["ex", "exec", "--its-own-flag"]));
    assert!(
        forwarding.is_ok(),
        "and honours a command's own answer: {forwarding:?}"
    );
}

fn words<const N: usize>(tokens: [&str; N]) -> Vec<String> {
    tokens.iter().map(|t| t.to_string()).collect()
}
