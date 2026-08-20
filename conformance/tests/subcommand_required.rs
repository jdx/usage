//! A command that cannot be run on its own, said in the spec it emits.
//!
//! The derive has always known this — a bare `T` subcommand field requires a subcommand and an
//! `Option<T>` does not, and the parser refuses the invocation either way — but `Spec::to_kdl`
//! did not write it, so the emitted KDL described `usage generate` as a command a user could
//! type alone. Found by converting usage-cli itself to the derive and diffing the spec it
//! prints against the one the clap bridge used to print.
//!
//! It matters past help text: docs, manpages, completions and the SDK generators all read the
//! emitted spec, and a command they think is runnable is one they offer.
//!
//! One shape could make the answer a lie, and is refused at compile time instead: a
//! `#[usage(flatten)]` group that declares subcommands of its own. Flatten joins flags and
//! arguments into the parent's tables and leaves subcommands behind, so the group's `build`
//! demanded one that no word could select while the parent — reading its own fields, which is
//! all an expansion can see — reported `subcommand_required=false`. `flatten_checks` in the
//! derive asserts the group's `COMMAND` has no subcommands, during const evaluation in the
//! parent's expansion, so that CLI stops compiling rather than shipping a command nobody can
//! run. There is no compile-fail harness here; the refusal is verified by hand, and the
//! working shape below is what keeps the rest of it honest.

use usage::Spec as LibSpec;
use usage_derive::{Args, Cli, Subcommands};

#[derive(Args)]
struct Leaf {
    /// Say more
    #[usage(long)]
    verbose: bool,
}

#[derive(Subcommands)]
enum Inner {
    /// The only thing under either parent
    Leaf(Box<Leaf>),
}

/// A group that is nothing but its subcommands
#[derive(Args)]
struct Strict {
    #[usage(subcommand)]
    command: Inner,
}

#[derive(Subcommands)]
enum InnerToo {
    /// The only thing under this one
    Leaf(Box<LeafToo>),
}

#[derive(Args)]
struct LeafToo {
    /// Say more
    #[usage(long)]
    verbose: bool,
}

/// A command that does something itself, and has subcommands too
#[derive(Args)]
struct Loose {
    #[usage(subcommand)]
    command: Option<InnerToo>,
}

#[derive(Subcommands)]
enum Commands {
    Strict(Box<Strict>),
    Loose(Box<Loose>),
}

#[derive(Cli)]
#[usage(bin = "ex")]
struct Ex {
    #[usage(subcommand)]
    command: Option<Commands>,
}

fn spec() -> LibSpec {
    Ex::to_kdl().parse().expect("valid spec")
}

#[test]
fn a_command_that_cannot_run_alone_says_so() {
    let spec = spec();
    let strict = spec.cmd.subcommands.get("strict").expect("declared");
    assert!(
        strict.subcommand_required,
        "a bare `T` subcommand field means one has to follow: {}",
        Ex::to_kdl()
    );
}

#[test]
fn a_command_that_can_stays_quiet() {
    // Not merely absent from the KDL — read back as false, which is what a consumer asks.
    // Writing it unconditionally would be the same bug in the other direction.
    let spec = spec();
    let loose = spec.cmd.subcommands.get("loose").expect("declared");
    assert!(!loose.subcommand_required);
    assert!(
        !Ex::to_kdl().contains("cmd loose subcommand_required"),
        "an `Option<T>` field writes nothing: {}",
        Ex::to_kdl()
    );
}

#[test]
fn a_leaf_command_never_claims_it() {
    // `subcommand_required` on a command with no subcommands is a spec the linter reports and
    // nothing could satisfy, so the writer holds both conditions rather than just the flag.
    let kdl = Ex::to_kdl();
    let leaf_lines: Vec<&str> = kdl
        .lines()
        .filter(|line| line.trim_start().starts_with("cmd leaf"))
        .collect();
    assert_eq!(leaf_lines.len(), 2, "one under each parent: {kdl}");
    for line in leaf_lines {
        assert!(!line.contains("subcommand_required"), "{line}");
    }
}

#[test]
fn the_parser_and_the_spec_agree() {
    // The property is emission-only, so this is the half that was already true: the derive
    // refuses the invocation from the type. If these ever disagree, the spec is describing a
    // grammar the binary does not have.
    use std::ffi::OsStr;
    let argv = [OsStr::new("strict")];
    assert!(
        Ex::parse_from(&argv).is_err(),
        "`strict` alone is not an invocation"
    );

    let argv = [OsStr::new("loose")];
    let ex = Ex::parse_from(&argv).expect("`loose` alone is, and says so both ways");
    assert!(matches!(ex.command, Some(Commands::Loose(_))));

    // And both still route to what is under them, which is what makes the distinction about
    // requiredness rather than about reachability.
    let argv = ["strict", "leaf", "--verbose"].map(OsStr::new);
    let Some(Commands::Strict(strict)) = Ex::parse_from(&argv).expect("should parse").command
    else {
        panic!("expected `strict`")
    };
    let Inner::Leaf(leaf) = strict.command;
    assert!(leaf.verbose);

    let argv = ["loose", "leaf", "--verbose"].map(OsStr::new);
    let Some(Commands::Loose(loose)) = Ex::parse_from(&argv).expect("should parse").command else {
        panic!("expected `loose`")
    };
    let Some(InnerToo::Leaf(leaf)) = loose.command else {
        panic!("expected `leaf`")
    };
    assert!(leaf.verbose);
}
