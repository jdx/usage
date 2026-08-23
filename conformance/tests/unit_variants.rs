//! A command that takes nothing.
//!
//! clap lets one be a bare variant — `Sponsors,` — and the derive needed
//! `Sponsors(Box<Sponsors>)` plus an empty struct plus, at the call site, a destructure to
//! keep the payload from reading as dead code. communique carries all three today.
//!
//! Everything generated speaks to a struct — its tables, its metadata, its `build` — so rather
//! than teach all of that about a variant with no struct, the derive writes the empty struct
//! such a variant implies. Only the construction differs.

use usage::Spec as LibSpec;
use usage_argv::help;
use usage_derive::{Args, Cli, Subcommands};

/// Set things up
#[derive(Args)]
struct Init {
    /// Overwrite what is there
    #[usage(long)]
    force: bool,
}

#[derive(Subcommands)]
enum Command {
    /// Set things up
    Init(Box<Init>),
    /// Show who pays for this
    #[usage(effect = "read")]
    Sponsors,
    /// Kept out of help, and still a command
    #[usage(hide)]
    Secret,
    /// Answers to another name too
    #[usage(name = "licence", alias = "license")]
    Licence,
}

/// Names that would collide if the two halves were run together
///
/// `Ambiguous::PairTwo` and the enum below spell `AmbiguousPairTwo` either way round, which is
/// why the pieces are separated. And `r#type` is a keyword a CLI may well want as a command:
/// its raw form prints as `r#type`, and `Ident::new` panics on the `#`.
// Every Rust keyword is lower case, so a variant named after one is lower case too — there is
// no way to write this fixture without the lint firing on the *fixture*. Nothing generated
// needs it; that is what the length-prefixed struct name is for.
#[allow(non_camel_case_types)]
#[derive(Subcommands)]
enum Ambiguous {
    /// One
    PairTwo,
    /// A command named after a keyword
    r#type,
}

/// A tool whose names are awkward
#[derive(Cli)]
#[usage(bin = "awkward")]
struct AwkwardCli {
    #[usage(subcommand)]
    command: Option<Ambiguous>,
}

/// The other half of the collision
#[derive(Subcommands)]
enum AmbiguousPair {
    /// Two
    Two,
}

/// A tool sharing the module with it
#[derive(Cli)]
#[usage(bin = "pair")]
struct PairCli {
    #[usage(subcommand)]
    command: Option<AmbiguousPair>,
}

/// A second set, with a bare variant spelled the same as the first's
///
/// The implied structs are named after the enum as well as the variant for exactly this: two
/// of one name in a module is a worse error than the one being avoided, and it would land in
/// the adopter's crate pointing at a struct they never wrote.
#[derive(Subcommands)]
enum Other {
    /// A different command that happens to share a name
    Sponsors,
}

/// A second tool sharing the module
#[derive(Cli)]
#[usage(bin = "other")]
struct OtherCli {
    #[usage(subcommand)]
    command: Option<Other>,
}

/// A tool with commands that take nothing
#[derive(Cli)]
#[usage(bin = "ex")]
struct Ex {
    #[usage(subcommand)]
    command: Option<Command>,
}

#[test]
fn a_bare_variant_is_a_command() {
    use std::ffi::OsStr;
    let argv = [OsStr::new("sponsors")];
    let parsed = Ex::parse_from(&argv).expect("should parse");
    assert!(
        matches!(parsed.command, Some(Command::Sponsors)),
        "a bare variant is built without a payload, which is the only thing that differs"
    );
}

#[test]
fn it_reaches_the_spec_like_any_other_command() {
    let spec: LibSpec = Ex::to_kdl().parse().expect("valid spec");
    let sponsors = spec.cmd.subcommands.get("sponsors").expect("sponsors");
    assert_eq!(sponsors.help.as_deref(), Some("Show who pays for this"));
    // Nothing of its own, which is the point — and it still declares itself.
    assert!(sponsors.flags.iter().all(|flag| flag.builtin));
    assert!(sponsors.args.is_empty());
}

#[test]
fn the_attributes_a_variant_takes_still_work() {
    let spec: LibSpec = Ex::to_kdl().parse().expect("valid spec");
    // `hide` is a variant's own business and does not need a struct to say it.
    assert!(spec.cmd.subcommands.get("secret").expect("secret").hide);
    // As are `name` and `alias`.
    let licence = spec.cmd.subcommands.get("licence").expect("licence");
    assert!(licence.aliases.contains(&"license".to_string()));

    use std::ffi::OsStr;
    let argv = [OsStr::new("license")];
    assert!(
        matches!(
            Ex::parse_from(&argv).expect("should parse").command,
            Some(Command::Licence)
        ),
        "the alias reaches the same command"
    );
}

#[test]
fn it_is_listed_where_it_should_be_and_not_where_it_should_not() {
    let page = help::render(Ex::spec(), Ex::spec().root.cmd, false).expect("a page");
    assert!(page.contains("sponsors"), "{page}");
    assert!(page.contains("Show who pays for this"), "{page}");
    assert!(!page.contains("secret"), "hidden means hidden: {page}");

    // And it has a page of its own, which says what it does and offers nothing to give it.
    let sponsors = Ex::spec()
        .root
        .cmd
        .subcommands
        .iter()
        .find(|c| c.name == "sponsors")
        .expect("sponsors");
    let page = help::render(Ex::spec(), sponsors, false).expect("a page");
    assert!(page.starts_with("Show who pays for this"), "{page}");
}

#[test]
fn the_struct_it_wraps_is_still_bound() {
    // The other variants are unaffected: a bare one costs its siblings nothing.
    use std::ffi::OsStr;
    let argv = ["init", "--force"].map(OsStr::new);
    let parsed = Ex::parse_from(&argv).expect("should parse");
    let Some(Command::Init(init)) = parsed.command else {
        panic!("expected init")
    };
    assert!(init.force);
}

#[test]
fn two_enums_may_each_have_one_of_the_same_name() {
    // Both compile, which is the assertion — an implied struct named only after the variant
    // would collide here, in the adopter's crate, pointing at something they never wrote.
    use std::ffi::OsStr;
    let argv = [OsStr::new("sponsors")];
    assert!(matches!(
        Ex::parse_from(&argv).expect("should parse").command,
        Some(Command::Sponsors)
    ));
    assert!(matches!(
        OtherCli::parse_from(&argv).expect("should parse").command,
        Some(Other::Sponsors)
    ));
}

#[test]
fn a_bare_variant_can_say_what_it_does_to_the_world() {
    // `effect` is otherwise an `Args` attribute, because that is where a command declares
    // itself — and a bare variant has no `Args`. The variant *is* the whole declaration, so it
    // says it there and the struct written for it carries it. Without this, moving a command
    // to a bare variant silently dropped its effect, which is how communique lost `sponsors`.
    let spec: LibSpec = Ex::to_kdl().parse().expect("valid spec");
    assert_eq!(
        spec.cmd
            .subcommands
            .get("sponsors")
            .expect("sponsors")
            .effect,
        Some(usage::SpecCommandEffect::Read)
    );
}

#[test]
fn awkward_names_do_not_collide_or_panic() {
    // All four compile, which is the assertion. `Ambiguous::PairTwo` and
    // `AmbiguousPair::Two` both read as `AmbiguousPairTwo` if the halves are simply run
    // together, and `r#type` panics the macro outright if the raw prefix is not stripped —
    // both landing in the adopter's crate, about a struct they never wrote.
    use std::ffi::OsStr;
    let argv = [OsStr::new("pair-two")];
    assert!(matches!(
        AwkwardCli::parse_from(&argv).expect("should parse").command,
        Some(Ambiguous::PairTwo)
    ));
    let argv = [OsStr::new("type")];
    assert!(matches!(
        AwkwardCli::parse_from(&argv).expect("should parse").command,
        Some(Ambiguous::r#type)
    ));
    let argv = [OsStr::new("two")];
    assert!(matches!(
        PairCli::parse_from(&argv).expect("should parse").command,
        Some(AmbiguousPair::Two)
    ));
}
