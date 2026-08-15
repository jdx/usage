//! What a parse cannot decide until the last token has been read.
//!
//! The parser binds tokens; whether what it bound is *acceptable* needs to know the
//! declared type — so required-ness, choices, and how many values a variadic got are
//! checked here, by the code the derive generates. These are the corpus's
//! `post-binding` vectors, exercised through a derived struct rather than through the
//! harness's spec-built tables.

use std::ffi::OsStr;

use usage_argv::Error;
use usage_derive::{Args, Cli, Subcommands};

fn argv<const N: usize>(tokens: [&str; N]) -> [&OsStr; N] {
    tokens.map(OsStr::new)
}

/// A CLI exercising each check
#[derive(Cli)]
#[usage(bin = "ex")]
struct Ex {
    /// Which shell
    #[usage(long, choices("bash", "zsh", "fish"))]
    shell: Option<String>,
    /// Files to include
    #[usage(long, var_min = 1, var_max = 2)]
    include: Vec<String>,
    /// Where to write
    #[usage(long)]
    out: Option<String>,
    /// What to act on
    target: String,
}

#[test]
fn a_required_argument_has_to_be_given() {
    let a = argv([]);
    assert!(matches!(
        Ex::parse_from(&a),
        Err(Error::MissingRequired { name: "target" })
    ));

    let a = argv(["x"]);
    assert_eq!(Ex::parse_from(&a).expect("should parse").target, "x");
}

#[test]
fn an_empty_value_still_counts_as_given() {
    // `--out=` supplies an empty string, which is a value somebody typed — the check
    // is whether a token arrived, not whether it was non-empty.
    let a = argv(["--out=", "x"]);
    let ex = Ex::parse_from(&a).expect("should parse");
    assert_eq!(ex.out.as_deref(), Some(""));
}

#[test]
fn a_field_with_no_env_is_unaffected_by_the_environment() {
    // The environment cases live in their own test binary; nothing here reads it.
    let a = argv(["x"]);
    assert!(Ex::parse_from(&a).expect("should parse").out.is_none());
}

#[test]
fn a_value_outside_the_choices_is_refused() {
    let a = argv(["--shell", "csh", "x"]);
    assert!(matches!(
        Ex::parse_from(&a),
        Err(Error::InvalidChoice { name: "shell", .. })
    ));

    let a = argv(["--shell", "zsh", "x"]);
    assert_eq!(
        Ex::parse_from(&a).expect("should parse").shell.as_deref(),
        Some("zsh")
    );
}

#[test]
fn variadic_bounds_are_counted() {
    let a = argv(["--include", "a", "x"]);
    assert_eq!(Ex::parse_from(&a).expect("one is enough").include, ["a"]);

    // Three, where two is the most: the flag is repeatable, so each occurrence adds.
    let a = argv(["--include", "a", "--include", "b", "--include", "c", "x"]);
    assert!(matches!(
        Ex::parse_from(&a),
        Err(Error::VarTooMany {
            name: "include",
            max: 2,
            got: 3
        })
    ));
}

#[test]
fn a_bound_is_only_checked_when_the_flag_was_used() {
    // `var_min` counts the values a flag was given, not whether it was given at all —
    // an absent optional flag is absent, not a violation.
    let a = argv(["x"]);
    let ex = Ex::parse_from(&a).expect("an unused flag has no values to count");
    assert!(ex.include.is_empty());
}

/// A CLI whose subcommand is required
#[derive(Cli)]
#[usage(bin = "req")]
struct Req {
    /// What to do
    #[usage(subcommand)]
    command: Commands,
}

#[derive(Subcommands)]
enum Commands {
    /// Install something
    Install(Install),
    /// Do nothing in particular
    Noop(Noop),
}

/// Install something
#[derive(Args)]
struct Install {
    /// Which tool
    tool: String,
    /// How
    #[usage(long, choices("fast", "careful"))]
    how: Option<String>,
}

/// Do nothing
#[derive(Args)]
struct Noop {
    /// Unused
    #[usage(long)]
    quiet: bool,
}

#[test]
fn a_bare_subcommand_field_requires_one() {
    let a = argv([]);
    assert!(matches!(Req::parse_from(&a), Err(Error::MissingSubcommand)));

    let a = argv(["install", "node"]);
    let req = Req::parse_from(&a).expect("should parse");
    let Commands::Install(install) = req.command else {
        panic!("expected install");
    };
    assert_eq!(install.tool, "node");
}

#[test]
fn only_the_command_that_ran_is_judged() {
    // `install` requires a tool. Running `noop` must not be held to that.
    let a = argv(["noop"]);
    let Commands::Noop(noop) = Req::parse_from(&a).expect("noop takes nothing").command else {
        panic!("expected noop");
    };
    assert!(!noop.quiet, "nothing was given, so nothing is set");

    let a = argv(["install"]);
    assert!(matches!(
        Req::parse_from(&a),
        Err(Error::MissingRequired { name: "tool" })
    ));
}

#[test]
fn a_subcommands_own_choices_are_checked() {
    let a = argv(["install", "node", "--how", "sideways"]);
    assert!(matches!(
        Req::parse_from(&a),
        Err(Error::InvalidChoice { name: "how", .. })
    ));

    let a = argv(["install", "node", "--how", "careful"]);
    let req = Req::parse_from(&a).expect("should parse");
    let Commands::Install(install) = req.command else {
        panic!("expected install");
    };
    assert_eq!(install.how.as_deref(), Some("careful"));
}

#[test]
fn the_checks_reach_the_spec_too() {
    let spec: usage::Spec = Ex::to_kdl().parse().expect("valid spec");
    let shell = spec.cmd.flags.iter().find(|f| f.name == "shell").unwrap();
    let choices = shell.arg.as_ref().and_then(|a| a.choices.as_ref());
    assert_eq!(
        choices.map(|c| c.choices.clone()),
        Some(vec!["bash".into(), "zsh".into(), "fish".into()]),
        "a declared choice list has to be in the spec, or docs and completions \
         cannot offer it"
    );

    let include = spec.cmd.flags.iter().find(|f| f.name == "include").unwrap();
    assert_eq!(include.var_min, Some(1));
    assert_eq!(include.var_max, Some(2));

    let target = spec.cmd.args.iter().find(|a| a.name == "target").unwrap();
    assert!(target.required);
}

/// A CLI whose flags relate to each other.
///
/// `--stdin` conflicts with `--file` and `--url`; `--out` is required when writing
/// from `--stdin`; and reading has to come from somewhere, so `--file` is required
/// unless one of the other two says where.
#[derive(Cli)]
#[usage(bin = "rel")]
struct Rel {
    /// Read from a file
    #[usage(long, required_unless("--url", "--stdin"))]
    file: Option<String>,
    /// Read from a URL
    #[usage(long)]
    url: Option<String>,
    /// Read from standard input
    #[usage(long, conflicts("--file", "--url"), short = 's')]
    stdin: bool,
    /// Where to write
    #[usage(long, required_if = "--stdin")]
    out: Option<String>,
    /// How many at once
    #[usage(long, default = "4", required_if = "--stdin")]
    jobs: Option<String>,
}

#[test]
fn conflicting_flags_cannot_both_be_given() {
    let a = argv(["--stdin", "--out", "o", "--file", "f"]);
    assert!(matches!(
        Rel::parse_from(&a),
        Err(Error::ConflictingFlags {
            name: "stdin",
            other: "file"
        })
    ));

    // The other target of the same declaration, and by its short form, since the
    // conflict is between flags rather than between spellings.
    let a = argv(["-s", "--out", "o", "--url", "u"]);
    assert!(matches!(
        Rel::parse_from(&a),
        Err(Error::ConflictingFlags {
            name: "stdin",
            other: "url"
        })
    ));

    // Either side alone is fine.
    let a = argv(["--stdin", "--out", "o"]);
    assert!(Rel::parse_from(&a).expect("should parse").stdin);
    let a = argv(["--file", "f"]);
    assert_eq!(
        Rel::parse_from(&a).expect("should parse").file.as_deref(),
        Some("f")
    );
}

#[test]
fn a_conflict_is_reported_before_what_it_left_unfilled() {
    // `--stdin` without `--out` is also a missing-required error. The conflict is the
    // more useful answer: it says which flag not to have typed, where the other would
    // ask for one more.
    let a = argv(["--stdin", "--file", "f"]);
    assert!(matches!(
        Rel::parse_from(&a),
        Err(Error::ConflictingFlags { name: "stdin", .. })
    ));
}

#[test]
fn required_if_applies_only_when_the_other_flag_is_given() {
    let a = argv(["--stdin"]);
    assert!(matches!(
        Rel::parse_from(&a),
        Err(Error::MissingRequired { name: "out" })
    ));

    // Without `--stdin`, `--out` is optional.
    let a = argv(["--file", "f"]);
    assert!(Rel::parse_from(&a).expect("should parse").out.is_none());
}

#[test]
fn a_default_satisfies_a_condition_that_would_have_required_the_flag() {
    // `--jobs` is required when `--stdin` is given, and it has a default — so it is
    // already filled and no condition can make it missing. Plain required-ness works
    // the same way, and so does usage-lib.
    let a = argv(["--stdin", "--out", "o"]);
    let rel = Rel::parse_from(&a).expect("should parse");
    assert_eq!(rel.jobs.as_deref(), Some("4"));
}

#[test]
fn required_unless_is_satisfied_by_the_other_flag() {
    // Neither given: `--file` is missing.
    let a = argv([]);
    assert!(matches!(
        Rel::parse_from(&a),
        Err(Error::MissingRequired { name: "file" })
    ));

    // `--url` stands in for it.
    let a = argv(["--url", "u"]);
    assert_eq!(
        Rel::parse_from(&a).expect("should parse").url.as_deref(),
        Some("u")
    );
}

/// A CLI where flags displace one another.
///
/// Only `--file` declares the override, which is enough: the relationship holds
/// between the two flags, so the later one wins whichever way round they are given.
#[derive(Cli)]
#[usage(bin = "ovr")]
struct Ovr {
    /// Read from a file
    #[usage(long, overrides("--stdin", "--url"))]
    file: Option<String>,
    /// Read from standard input
    #[usage(long)]
    stdin: bool,
    /// Read from a URL
    #[usage(long)]
    url: Option<String>,
    /// Colorize output, unless told otherwise
    #[usage(long, default = "true", overrides = "--plain")]
    color: bool,
    /// No decoration at all
    #[usage(long)]
    plain: bool,
    /// Patterns to include
    #[usage(long, var, overrides = "--all")]
    include: Vec<String>,
    /// Everything
    #[usage(long)]
    all: bool,
}

#[test]
fn the_last_of_two_overriding_flags_wins() {
    // Declared on `--file`, given `--file` first: `--stdin` is the survivor.
    let a = argv(["--file", "f", "--stdin"]);
    let ovr = Ovr::parse_from(&a).expect("should parse");
    assert!(ovr.stdin);
    assert_eq!(ovr.file, None, "displaced by the flag that came after it");

    // The same pair the other way round, which the declaration does not mention.
    let a = argv(["--stdin", "--file", "f"]);
    let ovr = Ovr::parse_from(&a).expect("should parse");
    assert_eq!(ovr.file.as_deref(), Some("f"));
    assert!(!ovr.stdin, "displaced by the flag that came after it");
}

#[test]
fn a_displaced_flag_goes_back_to_its_default_rather_than_to_nothing() {
    // `--color` defaults to on. `--plain` displaces it, and what it displaces it to
    // is the default: a flag that was never given reads the same as one that was
    // taken back.
    let a = argv(["--color", "--plain"]);
    let ovr = Ovr::parse_from(&a).expect("should parse");
    assert!(ovr.plain);
    assert!(ovr.color, "back to its declared default, not to false");

    // And a `--color` after `--plain` displaces the other way.
    let a = argv(["--plain", "--color"]);
    let ovr = Ovr::parse_from(&a).expect("should parse");
    assert!(ovr.color);
    assert!(!ovr.plain);
}

#[test]
fn a_displaced_collection_is_emptied() {
    let a = argv(["--include", "a", "--include", "b", "--all"]);
    let ovr = Ovr::parse_from(&a).expect("should parse");
    assert!(ovr.all);
    assert!(ovr.include.is_empty(), "got {:?}", ovr.include);

    // Values given after the flag that displaced them are kept: it is the order
    // they arrived in that decides, not which flag was mentioned in a declaration.
    let a = argv(["--include", "a", "--all", "--include", "b"]);
    let ovr = Ovr::parse_from(&a).expect("should parse");
    assert_eq!(ovr.include, ["b"]);
    assert!(!ovr.all);
}

#[test]
fn displacing_one_flag_leaves_the_others_alone() {
    let a = argv(["--url", "u", "--stdin"]);
    let ovr = Ovr::parse_from(&a).expect("should parse");
    assert_eq!(
        ovr.url.as_deref(),
        Some("u"),
        "--stdin does not touch --url"
    );
    assert!(ovr.stdin);
}

/// A flag whose one occurrence collects, bounded.
///
/// `var_max` is a binding limit here — the occurrence stops at two values and the words
/// after belong to the positional — so a *second* occurrence starts its own count. Checking
/// the total afterwards would fail an invocation that never broke the limit.
#[derive(Cli)]
#[usage(bin = "bounded")]
struct BoundedFlag {
    /// Patterns, two at a time
    // `variadic` alone: one occurrence takes several values. `var` would be the other
    // shape — one value per occurrence — and the derive refuses both at once.
    #[usage(long, variadic, var_max = 2)]
    include: Vec<String>,
    /// Where to write
    #[usage(arg, name = "OUT")]
    out: Option<String>,
}

#[test]
fn each_occurrence_of_a_bounded_flag_counts_for_itself() {
    // Two occurrences, each within its bound, four values in total.
    let a = argv(["--include", "a", "b", "--include", "c", "d"]);
    let bounded = BoundedFlag::parse_from(&a).expect("two occurrences should parse");
    assert_eq!(bounded.include, ["a", "b", "c", "d"]);

    // And the bound still stops one occurrence, leaving the third word to the positional.
    let a = argv(["--include", "a", "b", "c"]);
    let bounded = BoundedFlag::parse_from(&a).expect("should parse");
    assert_eq!(bounded.include, ["a", "b"]);
    assert_eq!(bounded.out.as_deref(), Some("c"));
}

/// A watcher whose filter starts out set
///
/// mise's `mise watch --fs-events`, which is where this came from: a `Vec` flag that already
/// holds something when nobody gives it one. Its spec said so and the derive could not, so it
/// was the last thing mise's 211-command spec could express that the derive could not.
#[derive(Cli)]
#[usage(bin = "ex")]
struct Defaulted {
    /// Filesystem events to filter to
    #[usage(long, var, default = "create", default = "remove", default = "modify")]
    fs_events: Vec<String>,
    /// Watch everything again
    #[usage(long, overrides = "--fs-events")]
    all_events: bool,
    /// One value, and it may be given once
    #[usage(long, default = "4")]
    jobs: Option<String>,
}

#[test]
fn a_collecting_flag_starts_out_holding_its_defaults() {
    // Absent: all of them, in the order written. A `Vec` is the one shape that can hold
    // several, so it is the one shape that may be given several.
    let a = argv([]);
    let d = Defaulted::parse_from(&a).expect("should parse");
    assert_eq!(d.fs_events, ["create", "remove", "modify"]);
    assert_eq!(d.jobs.as_deref(), Some("4"));

    // Given: *replaced*, not added to. A default says what the flag means when nobody said
    // anything, so appending would make `--fs-events access` mean four events, three of which
    // the user asked to filter out.
    let a = argv(["--fs-events", "access"]);
    let d = Defaulted::parse_from(&a).expect("should parse");
    assert_eq!(d.fs_events, ["access"]);
}

#[test]
fn the_defaults_reach_the_spec_in_order() {
    // The spec is the interface: a default the parser applies and the spec omits is a CLI whose
    // help, completions and manpage describe different behaviour from the binary.
    let spec: usage::Spec = Defaulted::to_kdl().parse().expect("valid spec");
    let flag = spec
        .cmd
        .flags
        .iter()
        .find(|f| f.name == "fs-events")
        .expect("the flag");
    let defaults = if flag.default.is_empty() {
        &flag.arg.as_ref().expect("takes a value").default
    } else {
        &flag.default
    };
    assert_eq!(defaults, &["create", "remove", "modify"]);
}

#[test]
fn a_displaced_collecting_flag_goes_back_to_its_defaults() {
    // The other half of "replaced, not added to", and the half the guard on `__given_*` hides:
    // when the flag *was* given, the defaults are never reached — except here. `--all-events`
    // displaces `--fs-events`, and what a displaced flag reads as is its declared default, so
    // the values the user gave have to go. Appending would leave `access` standing beside the
    // three it was meant to replace, in a flag the user asked to be overridden.
    let a = argv(["--fs-events", "access", "--all-events"]);
    let d = Defaulted::parse_from(&a).expect("should parse");
    assert!(d.all_events);
    assert_eq!(
        d.fs_events,
        ["create", "remove", "modify"],
        "back to its declared defaults, and only those"
    );
}
