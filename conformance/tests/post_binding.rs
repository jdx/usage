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
        Err(Error::MissingRequired { name: "TARGET" })
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

/// A missing default is a bound value, so `choices` has to accept it the same
/// way it accepts `--color=always`.
#[derive(Cli, Debug)]
#[usage(bin = "color")]
struct ColorWhen {
    #[usage(long, default_missing = "always", choices("auto", "always", "never"))]
    color: Option<String>,
}

#[test]
fn a_default_missing_value_has_to_be_a_choice() {
    let a = argv(["--color"]);
    assert_eq!(
        ColorWhen::parse_from(&a)
            .expect("always is on the list")
            .color
            .as_deref(),
        Some("always")
    );

    let a = argv(["--color=never"]);
    assert_eq!(
        ColorWhen::parse_from(&a)
            .expect("an attached choice still binds")
            .color
            .as_deref(),
        Some("never")
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
        Err(Error::MissingRequired { name: "TOOL" })
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

    let target = spec.cmd.args.iter().find(|a| a.name == "TARGET").unwrap();
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
    /// Sign the output
    #[usage(long, requires("--key"))]
    sign: bool,
    /// Tag the output, which needs a job count — and one is always there
    #[usage(long, requires("--jobs"))]
    tag: bool,
    /// The key to sign with
    #[usage(long)]
    key: Option<String>,
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
fn a_requirement_names_the_flag_that_was_not_given() {
    // Reported as the *other* flag missing rather than as something wrong with
    // `--sign`, which is what clap says for an unmet `requires` and the thing a user
    // can act on: the fix is to type `--key`, not to delete `--sign`.
    let a = argv(["--file", "f", "--sign"]);
    assert!(matches!(
        Rel::parse_from(&a),
        Err(Error::MissingRequired { name: "key" })
    ));

    // Satisfied.
    let a = argv(["--file", "f", "--sign", "--key", "k"]);
    let rel = Rel::parse_from(&a).expect("should parse");
    assert!(rel.sign);
    assert_eq!(rel.key.as_deref(), Some("k"));

    // Nothing is imposed when the flag that declares the requirement is absent, which
    // is what makes this different from plain required-ness: `--key` is optional until
    // `--sign` asks for it.
    let a = argv(["--file", "f"]);
    let rel = Rel::parse_from(&a).expect("should parse");
    assert!(!rel.sign);
    assert!(rel.key.is_none());
}

#[test]
fn a_default_on_the_required_flag_satisfies_the_requirement() {
    // The check is not emitted at all when the flag a requirement names has a default,
    // because such a flag can never be missing — the same rule plain required-ness
    // follows, and usage-lib agrees.
    let a = argv(["--file", "f", "--tag"]);
    let rel = Rel::parse_from(&a).expect("the defaulted flag is not missing");
    assert!(rel.tag);
    assert_eq!(rel.jobs.as_deref(), Some("4"));
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
    #[usage(long, negate = "--no-color", default = "true", overrides = "--plain")]
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
fn a_later_override_clears_an_earlier_duplicate() {
    let a = argv(["--file", "a", "--file", "b", "--stdin"]);
    let ovr = Ovr::parse_from(&a).expect("the final overriding flag should win");
    assert!(ovr.stdin);
    assert_eq!(ovr.file, None);
}

#[test]
fn positive_and_negative_spellings_override_instead_of_duplicate() {
    let a = argv(["--color", "--no-color"]);
    assert!(
        !Ovr::parse_from(&a)
            .expect("the negative form should win")
            .color
    );

    let a = argv(["--no-color", "--color"]);
    assert!(
        Ovr::parse_from(&a)
            .expect("the positive form should win")
            .color
    );

    let a = argv(["--no-color", "--no-color"]);
    assert!(matches!(
        Ovr::parse_from(&a),
        Err(usage_argv::Error::DuplicateFlag { name: "color" })
    ));
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
    /// Read from the environment when nobody says otherwise
    #[usage(long, var, default = "one", default = "two", env = "EX_FROM_ENV")]
    from_env: Vec<String>,
    /// Never `None`, because it always has something
    #[usage(long, var, default = "x", default = "y")]
    maybe: Option<Vec<String>>,
    /// `None` until it is given, because it declares nothing
    #[usage(long, var)]
    plain: Option<Vec<String>>,
}

#[test]
fn a_plain_flag_cannot_be_given_twice() {
    let a = argv(["--jobs", "2", "--jobs", "3"]);
    assert!(matches!(
        Defaulted::parse_from(&a),
        Err(usage_argv::Error::DuplicateFlag { name: "jobs" })
    ));

    let a = argv(["--all-events", "--all-events"]);
    assert!(matches!(
        Defaulted::parse_from(&a),
        Err(usage_argv::Error::DuplicateFlag { name: "all-events" })
    ));
}

#[test]
fn a_repeatable_flag_still_accepts_several_occurrences() {
    let a = argv(["--fs-events", "access", "--fs-events", "remove"]);
    let parsed = Defaulted::parse_from(&a).expect("var permits another occurrence");
    assert_eq!(parsed.fs_events, ["access", "remove"]);
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

#[test]
fn the_environment_replaces_a_collections_defaults() {
    // Every other shape assigns, so the environment overrides a default. A collection pushed,
    // so it ended up holding both — three events when the variable named one, and the extra two
    // are the ones the user's variable was chosen instead of.
    //
    // Serialized against the other environment test by reading a variable of its own.
    unsafe { std::env::set_var("EX_FROM_ENV", "three") };
    let a = argv([]);
    let d = Defaulted::parse_from(&a).expect("should parse");
    assert_eq!(d.from_env, ["three"]);
    unsafe { std::env::remove_var("EX_FROM_ENV") };

    // And with the variable unset the defaults stand, which is the other half of "replaces".
    let a = argv([]);
    let d = Defaulted::parse_from(&a).expect("should parse");
    assert_eq!(d.from_env, ["one", "two"]);
}

#[test]
fn an_optional_collection_with_defaults_is_never_none() {
    // `Option<Vec<T>>` says `None` for "never given", and a default is a value — so a field that
    // declares one always has something to hold. Seeding it left `__given_*` alone (the
    // environment still has to be able to replace it), so `None` came back and every declared
    // default was discarded on the way out of the partial.
    let a = argv([]);
    let d = Defaulted::parse_from(&a).expect("should parse");
    assert_eq!(
        d.maybe.as_deref(),
        Some(&["x".to_string(), "y".to_string()][..])
    );

    // Given, it is what was given.
    let a = argv(["--maybe", "z"]);
    let d = Defaulted::parse_from(&a).expect("should parse");
    assert_eq!(d.maybe.as_deref(), Some(&["z".to_string()][..]));

    // And one that declares no default still tells "never given" from "given nothing".
    assert_eq!(d.plain, None);
}

/// A CLI whose flags are grouped.
///
/// `--file`/`--url`/`--stdin` are one exclusive, required group — exactly one source —
/// and `--json`/`--yaml` are an ordinary exclusive one, where saying nothing is fine.
#[derive(Cli)]
#[usage(bin = "grp")]
#[usage(group("input", required))]
struct Grp {
    /// Read from a file
    #[usage(long, group = "input")]
    file: Option<String>,
    /// Read from a URL
    #[usage(long, group = "input")]
    url: Option<String>,
    /// Read from standard input
    #[usage(short = 's', long, group = "input")]
    stdin: bool,
    /// Emit JSON
    #[usage(long, group = "format")]
    json: bool,
    /// Emit YAML
    #[usage(long, group = "format")]
    yaml: bool,
}

#[test]
fn a_required_group_needs_one_of_its_members() {
    let a = argv([]);
    assert!(matches!(
        Grp::parse_from(&a),
        Err(Error::MissingGroup {
            group: "input",
            members: ["--file", "--url", "--stdin"]
        })
    ));

    let a = argv(["--url", "u"]);
    assert_eq!(
        Grp::parse_from(&a).expect("one member").url.as_deref(),
        Some("u")
    );
}

#[test]
fn two_members_of_an_exclusive_group_cannot_both_be_given() {
    let a = argv(["--file", "f", "--stdin"]);
    assert!(matches!(
        Grp::parse_from(&a),
        Err(Error::ConflictingFlags {
            name: "stdin",
            other: "file"
        })
    ));

    // By its short form too, since the group is between flags rather than spellings.
    let a = argv(["--url", "u", "-s"]);
    assert!(matches!(
        Grp::parse_from(&a),
        Err(Error::ConflictingFlags { name: "stdin", .. })
    ));

    // One member alone still parses, and lands where it was declared.
    let a = argv(["--file", "f"]);
    let grp = Grp::parse_from(&a).expect("one member");
    assert_eq!(grp.file.as_deref(), Some("f"));
    assert!(!grp.stdin);
}

#[test]
fn a_group_that_is_not_required_may_be_left_alone() {
    // `--json`/`--yaml` exclude each other and neither is needed.
    let a = argv(["--url", "u"]);
    let grp = Grp::parse_from(&a).expect("saying nothing about format is fine");
    assert!(!grp.json && !grp.yaml);

    let a = argv(["--url", "u", "--json"]);
    assert!(Grp::parse_from(&a).expect("one of them").json);

    let a = argv(["--url", "u", "--json", "--yaml"]);
    assert!(matches!(
        Grp::parse_from(&a),
        Err(Error::ConflictingFlags { .. })
    ));
}

#[test]
fn a_group_reaches_the_emitted_spec_and_usage_lib_agrees() {
    let kdl = Grp::to_kdl();
    assert!(
        kdl.contains(r#"group "input" "--file" "--url" "--stdin" required=#true"#),
        "{kdl}"
    );
    assert!(kdl.contains(r#"group "format" "--json" "--yaml""#), "{kdl}");

    // The reference implementation reads what the derive wrote, and enforces the same
    // rule — which is the point of the spec being the definition rather than a summary.
    let spec: usage::Spec = kdl.parse().expect("the emitted spec should parse");
    let group = spec.cmd.groups.iter().find(|g| g.name == "input").unwrap();
    assert!(group.required);
    assert_eq!(group.members.len(), 3);
}

/// Two groups on one command, one required and one exclusive.
#[derive(Cli)]
#[usage(bin = "ex4")]
#[usage(group("input", required))]
struct TwoGroups {
    #[usage(long, group = "input")]
    file: Option<String>,
    #[usage(long, group = "input")]
    url: Option<String>,
    #[usage(long, group = "format")]
    json: bool,
    #[usage(long, group = "format")]
    yaml: bool,
}

#[test]
fn a_conflict_answers_before_an_unsatisfied_group_does() {
    // Both are wrong here: `input` has no member, and `format` has two. The conflict is
    // the more useful answer — it says which flag not to have typed, where the other
    // asks for one more — and it is the order the rest of the checks already follow.
    let a = argv(["--json", "--yaml"]);
    assert!(
        matches!(
            TwoGroups::parse_from(&a),
            Err(Error::ConflictingFlags { .. })
        ),
        "the exclusivity of a later group should answer before an earlier group's requiredness"
    );

    // With the conflict gone, the unsatisfied group is what is left to say.
    let a = argv(["--json"]);
    assert!(matches!(
        TwoGroups::parse_from(&a),
        Err(Error::MissingGroup { group: "input", .. })
    ));

    // And with both satisfied, the values land where they were declared.
    let a = argv(["--file", "f", "--yaml"]);
    let two = TwoGroups::parse_from(&a).expect("one from each group");
    assert_eq!(two.file.as_deref(), Some("f"));
    assert!(two.url.is_none() && two.yaml && !two.json);
}

/// A CLI with a flag that has to be alone.
#[derive(Cli)]
#[usage(bin = "ex2")]
struct Exclusively {
    /// Dump the spec and leave
    #[usage(long, exclusive)]
    dump: bool,
    /// Print more
    #[usage(short = 'v', long)]
    verbose: bool,
    /// What to act on
    target: Option<String>,
}

#[test]
fn an_exclusive_flag_has_to_be_alone() {
    let a = argv(["--dump"]);
    let ex = Exclusively::parse_from(&a).expect("alone is the point");
    assert!(ex.dump && !ex.verbose && ex.target.is_none());

    // Another flag.
    let a = argv(["--dump", "-v"]);
    assert!(matches!(
        Exclusively::parse_from(&a),
        Err(Error::ConflictingFlags { other: "dump", .. })
    ));

    // And a positional, which is what makes this more than a conflict with every other
    // flag: `conflicts` has nowhere to name an argument.
    let a = argv(["--dump", "t"]);
    assert!(matches!(
        Exclusively::parse_from(&a),
        Err(Error::ConflictingFlags { other: "dump", .. })
    ));

    // Without it, nothing changes.
    let a = argv(["-v", "t"]);
    let ex = Exclusively::parse_from(&a).expect("the rest of the CLI is unaffected");
    assert!(ex.verbose);
    assert_eq!(ex.target.as_deref(), Some("t"));
}

#[derive(Cli)]
#[usage(bin = "required-ex")]
struct ExclusiveWithRequiredSiblings {
    #[usage(long, exclusive)]
    dump: bool,
    #[usage(long)]
    output: String,
    target: String,
}

#[test]
fn an_exclusive_flag_bypasses_required_siblings() {
    let a = argv(["--dump"]);
    let parsed = ExclusiveWithRequiredSiblings::parse_from(&a)
        .expect("exclusive is the command's requiredness escape");
    assert!(parsed.dump);
    assert!(parsed.output.is_empty());
    assert!(parsed.target.is_empty());
}

#[test]
fn exclusive_reaches_the_spec() {
    let kdl = Exclusively::to_kdl();
    assert!(kdl.contains("exclusive=#true"), "{kdl}");
    let spec: usage::Spec = kdl.parse().expect("the emitted spec should parse");
    let dump = spec.cmd.flags.iter().find(|f| f.name == "dump").unwrap();
    assert!(dump.exclusive);
}

#[derive(Args)]
struct ExtraOutput {
    /// Write somewhere
    #[usage(long)]
    output: Option<String>,
}

#[derive(Cli)]
#[usage(bin = "flat-ex")]
struct ExclusiveBesideFlatten {
    /// Dump and leave
    #[usage(long, exclusive)]
    dump: bool,
    #[usage(flatten)]
    extra: ExtraOutput,
}

#[derive(Args)]
struct FlattenedDefault {
    /// How many jobs to run
    #[usage(long, default = "4")]
    jobs: u8,
}

#[derive(Cli)]
#[usage(bin = "flat-default-ex")]
struct ExclusiveBesideFlattenedDefault {
    /// Dump and leave
    #[usage(long, exclusive)]
    dump: bool,
    #[usage(flatten)]
    extra: FlattenedDefault,
}

#[derive(Args)]
struct FlattenedExclusive {
    /// Dump and leave
    #[usage(long, exclusive)]
    dump: bool,
}

#[derive(Cli)]
#[usage(bin = "flat-ex-reverse")]
struct FlattenBesideOther {
    /// Print more
    #[usage(long)]
    verbose: bool,
    #[usage(flatten)]
    extra: FlattenedExclusive,
}

#[test]
fn flattening_does_not_hide_either_side_of_exclusivity() {
    let a = argv(["--dump", "--output", "somewhere"]);
    assert!(matches!(
        ExclusiveBesideFlatten::parse_from(&a),
        Err(Error::ConflictingFlags { other: "dump", .. })
    ));

    let a = argv(["--dump", "--verbose"]);
    assert!(matches!(
        FlattenBesideOther::parse_from(&a),
        Err(Error::ConflictingFlags { other: "dump", .. })
    ));

    let a = argv(["--output", "somewhere"]);
    let parsed = ExclusiveBesideFlatten::parse_from(&a).expect("without --dump");
    assert!(!parsed.dump);
    assert_eq!(parsed.extra.output.as_deref(), Some("somewhere"));

    let a = argv(["--dump"]);
    let parsed = FlattenBesideOther::parse_from(&a).expect("the flattened flag is alone");
    assert!(!parsed.verbose);
    assert!(parsed.extra.dump);
}

#[test]
fn an_exclusive_flag_does_not_skip_flattened_defaults() {
    let a = argv(["--dump"]);
    let parsed = ExclusiveBesideFlattenedDefault::parse_from(&a)
        .expect("exclusive suppresses requiredness, not declared defaults");
    assert!(parsed.dump);
    assert_eq!(parsed.extra.jobs, 4);
}

#[derive(Cli)]
#[usage(bin = "sub-ex")]
struct ExclusiveBesideSubcommand {
    /// Print the version and leave
    #[usage(long, global, exclusive)]
    version: bool,
    #[usage(subcommand)]
    command: Option<ExclusiveCommands>,
}

#[derive(Subcommands)]
enum ExclusiveCommands {
    /// Run something
    Run,
}

#[test]
fn selecting_a_subcommand_counts_as_company_for_a_parent_exclusive_flag() {
    let a = argv(["--version"]);
    let parsed = ExclusiveBesideSubcommand::parse_from(&a).expect("alone is allowed");
    assert!(parsed.version);
    assert!(parsed.command.is_none());

    let a = argv(["--version", "run"]);
    assert!(matches!(
        ExclusiveBesideSubcommand::parse_from(&a),
        Err(Error::ConflictingFlags {
            other: "version",
            ..
        })
    ));
}
#[allow(dead_code)]
#[derive(Args)]
struct ChildExclusive {
    #[usage(long, exclusive)]
    dump: bool,
}

#[allow(dead_code)]
#[derive(Subcommands)]
enum ChildExclusiveCommands {
    Run(ChildExclusive),
}

#[allow(dead_code)]
#[derive(Cli)]
#[usage(bin = "child-ex")]
struct ParentBesideChildExclusive {
    #[usage(long)]
    verbose: bool,
    #[usage(subcommand)]
    command: Option<ChildExclusiveCommands>,
}

#[test]
fn a_child_exclusive_flag_counts_parent_flags_as_company() {
    let a = argv(["run", "--dump"]);
    ParentBesideChildExclusive::parse_from(&a).expect("the child flag is alone");

    let a = argv(["--verbose", "run", "--dump"]);
    assert!(matches!(
        ParentBesideChildExclusive::parse_from(&a),
        Err(Error::ConflictingFlags { other: "dump", .. })
            | Err(Error::ConflictingFlags { name: "dump", .. })
    ));
}

#[allow(dead_code)]
#[derive(Args)]
struct RedeclaredClean {
    /// Clean, as this command means it
    #[usage(long = "clean")]
    clean: bool,
    /// Say more
    #[usage(long)]
    verbose: bool,
}

#[allow(dead_code)]
#[derive(Subcommands)]
enum RedeclaredCleanCommands {
    Run(RedeclaredClean),
}

#[allow(dead_code)]
#[derive(Cli)]
#[usage(bin = "orphan-alias-ex")]
struct OrphanAliasExclusive {
    /// Clean everything and leave
    #[usage(short = 'c', long, global, exclusive)]
    clean: bool,
    #[usage(subcommand)]
    command: Option<RedeclaredCleanCommands>,
}

/// A child that re-declares only the long form of an inherited global leaves the short alias
/// with the ancestor — and the ancestor's `exclusive` goes with it. The derive keeps the two
/// declarations as separate fields, so it never had to reconcile them; this holds usage-lib,
/// which merges them into one flag, to the same answer.
#[test]
fn an_orphan_ancestor_alias_keeps_its_exclusivity_past_a_child_redeclaration() {
    let a = argv(["run", "-c"]);
    assert!(
        matches!(
            OrphanAliasExclusive::parse_from(&a),
            Err(Error::ConflictingFlags { .. })
        ),
        "the ancestor's own spelling is still its exclusive flag"
    );

    let a = argv(["run", "--clean", "--verbose"]);
    OrphanAliasExclusive::parse_from(&a)
        .expect("the child's spelling drops the exclusivity the child did not restate");
}

#[allow(dead_code)]
#[derive(Args)]
struct ExclusiveRedeclaredClean {
    /// Clean, and nothing else
    #[usage(long = "clean", exclusive)]
    clean: bool,
    /// Say more
    #[usage(long)]
    verbose: bool,
}

#[allow(dead_code)]
#[derive(Subcommands)]
enum ExclusiveRedeclaredCleanCommands {
    Run(ExclusiveRedeclaredClean),
}

#[allow(dead_code)]
#[derive(Cli)]
#[usage(bin = "mixed-alias-ex")]
struct MixedAliasExclusive {
    /// Clean everything
    #[usage(short = 'c', long, global)]
    clean: bool,
    #[usage(subcommand)]
    command: Option<ExclusiveRedeclaredCleanCommands>,
}

/// The other direction, and both aliases at once: the child's spelling is exclusive whatever it
/// was typed beside, so an ancestor-only alias in the same invocation cannot excuse a companion.
#[test]
fn a_child_spelling_stays_exclusive_beside_an_ancestor_spelling() {
    let a = argv(["run", "-c", "--clean", "--verbose"]);
    assert!(
        matches!(
            MixedAliasExclusive::parse_from(&a),
            Err(Error::ConflictingFlags { .. })
        ),
        "the child's exclusive spelling was given, so --verbose is company"
    );

    let a = argv(["run", "-c", "--verbose"]);
    MixedAliasExclusive::parse_from(&a).expect("the ancestor's own spelling was never exclusive");
}

/// A CLI whose values arrive several to a word.
#[derive(Cli)]
#[usage(bin = "ex3")]
struct Splitting {
    /// Tags to apply
    #[usage(long, delimiter = ',', var_max = 3)]
    tags: Vec<String>,
    /// Where to look
    #[usage(long, delimiter = ':', choices("src", "docs"))]
    paths: Vec<String>,
    /// Labels to attach
    #[usage(arg, delimiter = ';')]
    labels: Vec<String>,
}

#[test]
fn a_delimiter_makes_one_word_several_values() {
    let a = argv(["--tags", "a,b,c"]);
    assert_eq!(
        Splitting::parse_from(&a).expect("split").tags,
        ["a", "b", "c"]
    );

    // Several occurrences, each split.
    let a = argv(["--tags", "a,b", "--tags", "c"]);
    assert_eq!(
        Splitting::parse_from(&a).expect("split").tags,
        ["a", "b", "c"]
    );

    // A word with no separator in it is one value, as it was before.
    let a = argv(["--tags", "a"]);
    assert_eq!(Splitting::parse_from(&a).expect("split").tags, ["a"]);

    let a = argv(["one;two"]);
    assert_eq!(
        Splitting::parse_from(&a).expect("split positional").labels,
        ["one", "two"]
    );
}

#[test]
fn split_values_are_judged_and_counted_as_values() {
    // The split runs before every check, so `choices` sees each value rather than the
    // word that carried them, and the bounds count what the user meant.
    let a = argv(["--paths", "src:docs"]);
    assert_eq!(
        Splitting::parse_from(&a).expect("both are choices").paths,
        ["src", "docs"]
    );

    let a = argv(["--paths", "src:nowhere"]);
    assert!(matches!(
        Splitting::parse_from(&a),
        Err(Error::InvalidChoice { .. })
    ));

    let a = argv(["--tags", "a,b,c,d"]);
    assert!(matches!(
        Splitting::parse_from(&a),
        Err(Error::VarTooMany { got: 4, .. })
    ));
}

#[test]
fn a_delimiter_reaches_the_spec() {
    let kdl = Splitting::to_kdl();
    assert!(kdl.contains(r#"delimiter=",""#), "{kdl}");
    let spec: usage::Spec = kdl.parse().expect("the emitted spec should parse");
    let tags = spec.cmd.flags.iter().find(|f| f.name == "tags").unwrap();
    assert_eq!(tags.arg.as_ref().unwrap().delimiter, Some(','));
    assert_eq!(spec.cmd.args[0].delimiter, Some(';'));
}

/// A CLI whose bounded collections take their values several to a word.
#[allow(dead_code)]
#[derive(Cli)]
#[usage(bin = "ex4")]
struct BoundedSplitting {
    /// Patterns, at most two per occurrence
    #[usage(long, variadic, delimiter = ',', var_max = 2)]
    include: Vec<String>,
    /// Targets, at most two
    #[usage(delimiter = ':', var_max = 2)]
    targets: Vec<String>,
}

#[test]
fn a_bound_counts_the_values_a_word_carried() {
    // `var_max` bounds values, and a delimiter is what decides how many values a word is.
    // Counting words instead let one word carry an occurrence straight past its bound.
    let a = argv(["--include", "a,b,c"]);
    assert!(
        matches!(
            BoundedSplitting::parse_from(&a),
            Err(Error::VarTooMany { max: 2, got: 3, .. })
        ),
        "three values out of one word is still three values"
    );

    let a = argv(["x:y:z"]);
    assert!(
        matches!(
            BoundedSplitting::parse_from(&a),
            Err(Error::VarTooMany { max: 2, got: 3, .. })
        ),
        "a positional counts the same way"
    );
}

#[test]
fn a_split_bound_still_counts_one_occurrence_at_a_time() {
    // The rule the corpus documents for plain words holds for split ones: the bound is on
    // what one occurrence takes, not on the list the occurrences build up. Reading the
    // total would make the same declaration mean fewer values the more often it is given.
    let a = argv(["--include", "a,b", "--include", "c,d"]);
    assert_eq!(
        BoundedSplitting::parse_from(&a)
            .expect("two per occurrence is within the bound")
            .include,
        ["a", "b", "c", "d"]
    );

    // And a word carrying exactly the bound is still allowed.
    let a = argv(["--include", "a,b"]);
    assert_eq!(
        BoundedSplitting::parse_from(&a)
            .expect("exactly two")
            .include,
        ["a", "b"]
    );
}
