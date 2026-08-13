//! Three things a spec can say that the derive could not.
//!
//! Each was found the same way: rendering mise's help from the shadow's metadata and comparing
//! it against usage-lib's, over all 211 commands. Every difference traced back to something
//! the KDL declared, the derive had no vocabulary for, and `gen-shadow` therefore dropped —
//! without counting it, which is the part that made them hard to see.
//!
//! They matter beyond help text. The emitted spec feeds docs, manpages, completions and the
//! SDK generators, so a property that cannot survive the round trip is one every downstream
//! consumer is wrong about.

use usage::Spec as LibSpec;
use usage_derive::{Args, Cli, Subcommands};

/// A flag reachable only by its short form, whose value still needs a name.
#[derive(Cli)]
#[usage(bin = "shortonly")]
struct ShortOnly {
    /// How many at once
    #[usage(short = 'j')]
    jobs: Option<String>,
}

#[test]
fn a_short_only_flag_keeps_a_descriptive_placeholder() {
    // A flag is named after the form it answers to, and for a short-only flag that form is one
    // character — right for the flag's name and useless as the name of its *value*, since help
    // and the KDL both fall back to it. `-j <j>` for a field called `jobs`.
    let spec: LibSpec = ShortOnly::to_kdl().parse().expect("valid spec");
    let flag = &spec.cmd.flags[0];
    assert_eq!(
        flag.name, "j",
        "named after the form, as usage-lib names it"
    );
    assert_eq!(
        flag.arg.as_ref().expect("takes a value").name,
        "jobs",
        "but its value keeps the descriptive name"
    );

    // And it binds by the short form, which is the only form it has.
    use std::ffi::OsStr;
    let argv = [OsStr::new("-j"), OsStr::new("4")];
    let parsed = ShortOnly::parse_from(&argv).expect("should parse");
    assert_eq!(parsed.jobs.as_deref(), Some("4"));
}

/// A flag whose long form differs from the Rust field holding it.
#[derive(Cli)]
#[usage(bin = "renamed")]
struct Renamed {
    /// What sort of thing
    #[usage(long = "type", short = 't')]
    type_: Option<String>,
}

#[test]
fn a_renamed_flag_takes_its_placeholder_from_the_form_not_the_field() {
    // The value name falls back to the flag's name, and the flag is named after its long form —
    // so a field called `type_` must not drag its kebab-cased ident into the placeholder and
    // render `--type <type->`.
    let spec: LibSpec = Renamed::to_kdl().parse().expect("valid spec");
    let flag = &spec.cmd.flags[0];
    assert_eq!(flag.name, "type");
    assert_eq!(flag.arg.as_ref().expect("takes a value").name, "type");

    use std::ffi::OsStr;
    let argv = [OsStr::new("--type"), OsStr::new("toml")];
    let parsed = Renamed::parse_from(&argv).expect("should parse");
    assert_eq!(parsed.type_.as_deref(), Some("toml"));
}

/// A CLI declaring all three.
#[derive(Cli)]
#[usage(bin = "ex")]
struct Ex {
    /// How many at once
    ///
    /// The placeholder differs from the flag's name in case, which is the ordinary case:
    /// mise writes `--tool <TOOL>`, and without this the spec said `--tool <tool>`.
    #[usage(long, short = 'j', value_name = "JOBS")]
    jobs: Option<String>,
    /// Show more
    ///
    /// A count is repeatable by definition — `-vvv` is three occurrences — so the spec has
    /// to say `var` as well, which it now infers rather than needing told.
    #[usage(long, short = 'v', count)]
    verbose: u8,
    /// What to act on, at least one
    ///
    /// `<TARGET>…` in a spec. A `Vec` has no bare-versus-`Option` shape to carry
    /// required-ness, so this is the one place it is declared rather than inferred.
    #[usage(arg, name = "TARGET", required)]
    target: Vec<String>,
}

fn spec() -> LibSpec {
    Ex::to_kdl().parse().expect("valid spec")
}

#[test]
fn a_flags_value_keeps_the_name_it_was_given() {
    let spec = spec();
    let jobs = spec
        .cmd
        .flags
        .iter()
        .find(|f| f.name == "jobs")
        .expect("--jobs");
    let arg = jobs.arg.as_ref().expect("--jobs takes a value");
    assert_eq!(arg.name, "JOBS");

    // And it reaches the KDL as the placeholder, which is what help and completions read.
    let kdl = Ex::to_kdl();
    assert!(kdl.contains("<JOBS>"), "{kdl}");
}

#[test]
fn a_counted_flag_says_it_can_be_given_again() {
    let spec = spec();
    let verbose = spec
        .cmd
        .flags
        .iter()
        .find(|f| f.name == "verbose")
        .expect("--verbose");
    assert!(verbose.count, "it counts");
    assert!(
        verbose.var,
        "and counting only means something for a flag that may be repeated"
    );
}

#[test]
fn a_collecting_argument_can_require_a_value() {
    let spec = spec();
    let target = spec
        .cmd
        .args
        .iter()
        .find(|a| a.name == "TARGET")
        .expect("TARGET");
    assert!(target.var, "it collects");
    assert!(target.required, "and it needs at least one");

    // `<TARGET>…` rather than `[TARGET]…`, which is how a reader of the spec tells the two
    // apart — and what a usage line renders from.
    let kdl = Ex::to_kdl();
    assert!(kdl.contains("<TARGET>"), "{kdl}");
    assert!(!kdl.contains("[TARGET]"), "{kdl}");
}

#[test]
fn a_required_collection_still_binds_like_a_collection() {
    // Declaring it required is a statement about the spec and about what the post-binding
    // check demands — not about how words bind, which is unchanged.
    use std::ffi::OsStr;

    let argv = [
        OsStr::new("-j"),
        OsStr::new("4"),
        OsStr::new("-vv"),
        OsStr::new("one"),
        OsStr::new("two"),
    ];
    let ex = Ex::parse_from(&argv).expect("two values");
    assert_eq!(ex.target, ["one", "two"]);

    // The other two are declarations about the *spec*, and change nothing about binding: the
    // value name is a placeholder in help, and a counted flag counted before this too.
    assert_eq!(ex.jobs.as_deref(), Some("4"));
    assert_eq!(ex.verbose, 2);
}

/// A command whose help text has line breaks that matter, and a hidden sibling.
#[derive(Args)]
struct Shims {
    /// Undocumented on purpose: the help is declared below, not commented.
    #[usage(
        long,
        help = "Use shims instead of modifying PATH\nEffectively the same as:"
    )]
    shims: bool,
}

#[derive(Args)]
struct Internal {
    #[usage(long)]
    force: bool,
}

#[derive(Subcommands)]
enum Commands {
    /// Activate the thing
    Activate(Box<Shims>),
    /// Simulate something for compatibility
    #[usage(hide)]
    Asdf(Box<Internal>),
}

#[derive(Cli)]
#[usage(bin = "verbatim")]
struct Verbatim {
    #[usage(subcommand)]
    command: Option<Commands>,
}

#[test]
fn help_text_can_keep_line_breaks_a_comment_would_flow() {
    // A doc comment's first paragraph is read the way Rust reads one, so a line break inside
    // it becomes a space — which is right for prose and wrong for help whose shape is
    // deliberate. 37 of mise's flags and commands declare multi-line help, and every one came
    // back with its lines run together until this existed.
    let spec: LibSpec = Verbatim::to_kdl().parse().expect("valid spec");
    let activate = spec.cmd.subcommands.get("activate").expect("activate");
    let shims = activate
        .flags
        .iter()
        .find(|f| f.name == "shims")
        .expect("--shims");
    assert_eq!(
        shims.help.as_deref(),
        Some("Use shims instead of modifying PATH\nEffectively the same as:")
    );
}

#[test]
fn a_command_can_be_hidden() {
    // `hide=#true` on a `cmd`. The command still answers to its name; it is not offered.
    let spec: LibSpec = Verbatim::to_kdl().parse().expect("valid spec");
    let asdf = spec.cmd.subcommands.get("asdf").expect("asdf");
    assert!(asdf.hide, "declared hidden");
    assert!(
        !spec.cmd.subcommands.get("activate").expect("activate").hide,
        "and its sibling is not"
    );

    // Still reachable, which is the whole point of hidden rather than absent — and its flags
    // bind as any other command's do.
    use std::ffi::OsStr;
    let argv = [OsStr::new("asdf"), OsStr::new("--force")];
    let parsed = Verbatim::parse_from(&argv).expect("a hidden command still parses");
    let Some(Commands::Asdf(internal)) = parsed.command else {
        panic!("expected the hidden command")
    };
    assert!(internal.force);

    // And the visible sibling, whose declared help is the subject of the test above.
    let argv = [OsStr::new("activate"), OsStr::new("--shims")];
    let parsed = Verbatim::parse_from(&argv).expect("should parse");
    let Some(Commands::Activate(shims)) = parsed.command else {
        panic!("expected activate")
    };
    assert!(shims.shims);
}
