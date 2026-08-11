//! Checks what the derive generates, end to end.
//!
//! Two claims are worth testing, and they are different claims. First, a derived
//! CLI parses a command line the way [the grammar] says — which is really a test
//! that the derive emits the tables it meant to. Second, the *same* declaration
//! emits a spec that usage-lib accepts, so one Rust type is enough to get docs,
//! manpages, and completions.
//!
//! [the grammar]: https://usage.jdx.dev/spec/argv

use std::ffi::OsStr;

use usage::Spec as LibSpec;
use usage_derive::Cli;

/// A tool that does things
///
/// With a second paragraph, so the long help differs from the short.
#[derive(Cli, Debug)]
#[usage(bin = "ex", version = "1.2.3")]
struct Ex {
    /// How many jobs to run at once
    ///
    /// Longer prose about jobs.
    #[usage(
        short = 'j',
        long,
        env = "EX_JOBS",
        default = "4",
        help_heading = "Performance"
    )]
    jobs: Option<String>,

    /// Print more
    #[usage(short = 'v', long, count)]
    verbose: u8,

    /// Colorize output
    #[usage(long, negate = "--no-color", default = "true")]
    color: bool,

    /// Overwrite what is there
    #[usage(short = 'f', long)]
    force: bool,

    /// Patterns to include
    #[usage(long, var)]
    include: Vec<String>,

    /// An option nobody should see
    #[usage(long, hide)]
    secret: bool,

    /// The file to work on
    file: String,

    /// Anything else
    rest: Vec<String>,
}

fn argv<const N: usize>(tokens: [&str; N]) -> [&OsStr; N] {
    tokens.map(OsStr::new)
}

#[test]
fn defaults_apply_when_the_command_line_is_empty() {
    let a = argv(["x.txt"]);
    let ex = Ex::parse_from(&a).expect("should parse");
    assert_eq!(ex.jobs.as_deref(), Some("4"), "the declared default");
    assert!(ex.color, "a negatable flag starts at its default");
    assert!(!ex.force);
    assert_eq!(ex.verbose, 0);
    assert_eq!(ex.file, "x.txt");
    assert!(ex.rest.is_empty());
    assert!(!ex.secret, "a hidden flag still parses like any other");
}

#[test]
fn flags_bind_in_every_form() {
    // Attached short, bundled shorts, attached long, and a repeated flag.
    let a = argv([
        "-j8",
        "-fv",
        "--include",
        "a",
        "--include=b",
        "-vv",
        "x.txt",
    ]);
    let ex = Ex::parse_from(&a).expect("should parse");
    assert_eq!(ex.jobs.as_deref(), Some("8"));
    assert!(ex.force);
    assert_eq!(ex.verbose, 3, "-v once and -vv again");
    assert_eq!(ex.include, ["a", "b"]);
    assert_eq!(ex.file, "x.txt");
}

/// The distinction #799 fixed in the harness, now pinned in the derive: a
/// repeatable flag takes one value per occurrence, and must not swallow the word
/// after it. My first version of this derive got it wrong, and the original test
/// missed it by always writing `--include` twice.
#[derive(Cli)]
struct Repeat {
    /// Patterns, one per occurrence
    #[usage(long, var)]
    include: Vec<String>,
    /// Patterns, several at once
    #[usage(long, variadic)]
    exclude: Vec<String>,
    /// Where to work
    target: String,
}

#[test]
fn a_repeatable_flag_leaves_the_next_word_alone() {
    let a = argv(["--include", "a", "b"]);
    let cli = Repeat::parse_from(&a).expect("should parse");
    assert_eq!(cli.include, ["a"], "one value per occurrence");
    assert_eq!(cli.target, "b", "the next word is still the positional's");
}

#[test]
fn a_variadic_flag_takes_several_values() {
    let a = argv(["--exclude", "a", "b", "--", "t"]);
    let cli = Repeat::parse_from(&a).expect("should parse");
    assert_eq!(cli.exclude, ["a", "b"], "greedy until the separator");
    assert_eq!(cli.target, "t");
}

#[test]
fn a_repeatable_flag_still_repeats() {
    let a = argv(["--include", "a", "--include=b", "t"]);
    let cli = Repeat::parse_from(&a).expect("should parse");
    assert_eq!(cli.include, ["a", "b"]);
    assert_eq!(cli.target, "t");
}

#[test]
fn a_count_saturates_rather_than_overflowing() {
    // A `u8` given more than 255 occurrences would otherwise panic in debug and
    // wrap to zero in release.
    let tokens = vec!["-v"; 300];
    let raw: Vec<&OsStr> = tokens.iter().map(|t| OsStr::new(*t)).collect();
    let mut with_file = raw.clone();
    with_file.push(OsStr::new("x.txt"));
    let ex = Ex::parse_from(&with_file).expect("should parse");
    assert_eq!(ex.verbose, u8::MAX);
}

#[test]
fn a_hidden_flag_still_works() {
    // Hidden means absent from help and docs, not unusable.
    let a = argv(["--secret", "x.txt"]);
    let ex = Ex::parse_from(&a).expect("should parse");
    assert!(ex.secret);
}

#[test]
fn a_negation_turns_a_default_off() {
    let a = argv(["--no-color", "x.txt"]);
    let ex = Ex::parse_from(&a).expect("should parse");
    assert!(!ex.color);
}

#[test]
fn positionals_fill_in_order_and_the_variadic_takes_the_rest() {
    let a = argv(["one", "two", "three"]);
    let ex = Ex::parse_from(&a).expect("should parse");
    assert_eq!(ex.file, "one");
    assert_eq!(ex.rest, ["two", "three"]);
}

#[test]
fn a_typo_is_reported_rather_than_bound() {
    let a = argv(["--forse", "x.txt"]);
    let err = Ex::parse_from(&a).expect_err("an unknown flag should not parse");
    assert!(
        matches!(err, usage_argv::Error::UnknownFlag { token } if token == b"--forse"),
        "got {err:?}"
    );
}

#[test]
fn everything_after_a_separator_is_a_value() {
    let a = argv(["--", "--not-a-flag", "-x"]);
    let ex = Ex::parse_from(&a).expect("should parse");
    assert_eq!(ex.file, "--not-a-flag");
    assert_eq!(ex.rest, ["-x"]);
}

#[test]
fn the_spec_is_valid_and_says_what_was_declared() {
    let kdl = Ex::to_kdl();
    let spec: LibSpec = kdl
        .parse()
        .unwrap_or_else(|e| panic!("usage-lib should parse the derived spec: {e}\n\n{kdl}"));

    assert_eq!(spec.bin, "ex");
    assert_eq!(spec.version.as_deref(), Some("1.2.3"));
    // The struct's doc comment: first paragraph short, whole thing long.
    assert_eq!(spec.about.as_deref(), Some("A tool that does things"));
    assert!(
        spec.about_long
            .as_deref()
            .is_some_and(|l| l.contains("second paragraph")),
        "the long form should keep the rest of the comment"
    );

    let jobs = spec
        .cmd
        .flags
        .iter()
        .find(|f| f.name == "jobs")
        .expect("--jobs should be in the spec");
    assert_eq!(jobs.short, vec!['j']);
    assert_eq!(jobs.long, vec!["jobs".to_string()]);
    assert_eq!(jobs.env.as_deref(), Some("EX_JOBS"));
    assert_eq!(jobs.default, vec!["4".to_string()]);
    assert_eq!(jobs.help_heading.as_deref(), Some("Performance"));
    assert_eq!(jobs.help.as_deref(), Some("How many jobs to run at once"));
    assert_eq!(
        jobs.help_long.as_deref(),
        Some("How many jobs to run at once\n\nLonger prose about jobs.")
    );
    assert!(jobs.arg.is_some(), "an Option<String> flag takes a value");

    let verbose = spec.cmd.flags.iter().find(|f| f.name == "verbose").unwrap();
    assert!(verbose.count);
    assert!(verbose.arg.is_none(), "a count flag takes no value");

    let color = spec.cmd.flags.iter().find(|f| f.name == "color").unwrap();
    assert_eq!(color.negate.as_deref(), Some("--no-color"));

    let secret = spec.cmd.flags.iter().find(|f| f.name == "secret").unwrap();
    assert!(secret.hide);

    // A `String` positional must be filled; a `Vec<String>` need not be.
    assert_eq!(spec.cmd.args[0].name, "file");
    assert!(spec.cmd.args[0].required);
    assert_eq!(spec.cmd.args[1].name, "rest");
    assert!(spec.cmd.args[1].var);
    assert!(!spec.cmd.args[1].required);
}

#[test]
fn the_spec_renders_as_docs() {
    // The reason the spec exists: `usage g markdown|manpage` at build time.
    let spec: LibSpec = Ex::to_kdl().parse().unwrap();
    let markdown = usage::docs::markdown::MarkdownRenderer::new(spec.clone())
        .render_index()
        .expect("should render markdown");
    assert!(markdown.contains("--jobs"));
    assert!(
        !markdown.contains("--secret"),
        "a hidden flag stays out of the docs"
    );

    let manpage = usage::docs::manpage::ManpageRenderer::new(spec)
        .render()
        .expect("should render a manpage");
    assert!(manpage.contains("ex"));
}

#[test]
fn help_output_groups_by_heading() {
    let spec: LibSpec = Ex::to_kdl().parse().unwrap();
    let help = usage::docs::cli::render_help(&spec, &spec.cmd, false);
    assert!(help.contains("Performance:"), "got:\n{help}");
    assert!(help.contains("Flags:"), "got:\n{help}");
}

/// A CLI with no flags at all still compiles: the generated code has to cope with
/// empty tables, which is exactly where an unused variable or an empty `match`
/// would break the build.
#[derive(Cli)]
struct Bare {
    /// The only thing it takes
    target: String,
}

#[test]
fn a_cli_with_only_a_positional_works() {
    let a = argv(["thing"]);
    let bare = Bare::parse_from(&a).expect("should parse");
    assert_eq!(bare.target, "thing");

    let spec: LibSpec = Bare::to_kdl().parse().expect("should be a valid spec");
    assert_eq!(spec.cmd.args.len(), 1);
    assert!(spec.cmd.flags.is_empty());
}

/// And a CLI with no positionals, for the same reason.
#[derive(Cli)]
struct FlagsOnly {
    /// Whether to hurry
    #[usage(long)]
    fast: bool,
}

#[test]
fn a_cli_with_only_a_flag_works() {
    let a = argv(["--fast"]);
    let cli = FlagsOnly::parse_from(&a).expect("should parse");
    assert!(cli.fast);

    let spec: LibSpec = FlagsOnly::to_kdl().parse().expect("should be a valid spec");
    assert!(spec.cmd.args.is_empty());
    assert_eq!(spec.cmd.flags.len(), 1);
}

/// An explicit `long` written with its dashes is the natural mistake, and would
/// have produced a flag no command line could reach.
#[derive(Cli)]
struct Dashed {
    /// Colorize output
    #[usage(long = "--color", negate = "--no-color", default = "true")]
    color: bool,
}

#[test]
fn an_explicit_long_may_be_written_with_dashes() {
    let a = argv(["--no-color"]);
    let cli = Dashed::parse_from(&a).expect("should parse");
    assert!(!cli.color);

    let a = argv(["--color"]);
    let cli = Dashed::parse_from(&a).expect("should parse");
    assert!(cli.color);

    // And the spec records it the way a spec does, without the dashes on the long
    // form and with them on the negation.
    let spec: LibSpec = Dashed::to_kdl().parse().expect("should be a valid spec");
    let color = &spec.cmd.flags[0];
    assert_eq!(color.long, vec!["color".to_string()]);
    assert_eq!(color.negate.as_deref(), Some("--no-color"));
}

#[test]
fn reaching_the_tables_is_free() {
    // The point of the whole exercise: `command()` hands back a `static`, so there
    // is nothing to build before parsing. This is a compile-time property, and the
    // assertion is that the reference is the same one every time.
    let first = Ex::command() as *const _;
    let second = Ex::command() as *const _;
    assert_eq!(
        first, second,
        "the tables should be one static, not a build"
    );
}
