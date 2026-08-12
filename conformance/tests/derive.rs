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
fn a_bare_short_uses_the_renamed_name() {
    // `short` is written before `name`, and the two names start with different
    // letters — which is the only way to tell the bug from a coincidence.
    #[derive(Cli)]
    struct Renamed {
        /// Say less
        #[usage(short, name = "quiet", long)]
        verbose: bool,
    }

    let a = argv(["-q"]);
    assert!(
        Renamed::parse_from(&a)
            .expect("-q should be the short form")
            .verbose,
        "the short comes from the renamed name"
    );

    let a = argv(["-v"]);
    assert!(
        Renamed::parse_from(&a).is_err(),
        "-v is the field's letter, which is not what was declared"
    );

    let spec: LibSpec = Renamed::to_kdl().parse().expect("valid spec");
    assert_eq!(spec.cmd.flags[0].short, vec!['q']);
    assert_eq!(spec.cmd.flags[0].long, vec!["quiet".to_string()]);
}

#[test]
fn a_variadic_flag_is_not_also_repeatable() {
    // Two different claims: `var` says the flag may be repeated, a variadic
    // argument says one occurrence keeps taking values. Emitting both would
    // contradict the grammar.
    let spec: LibSpec = Repeat::to_kdl().parse().expect("valid spec");
    let exclude = spec
        .cmd
        .flags
        .iter()
        .find(|f| f.name == "exclude")
        .expect("--exclude should be in the spec");
    assert!(!exclude.var, "a variadic flag is not marked repeatable");
    assert!(
        exclude.arg.as_ref().is_some_and(|a| a.var),
        "its argument is the variadic one"
    );

    let include = spec.cmd.flags.iter().find(|f| f.name == "include").unwrap();
    assert!(include.var, "a repeatable flag is marked repeatable");
    assert!(
        include.arg.as_ref().is_some_and(|a| !a.var),
        "and its argument takes one value"
    );
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

/// A CLI that owns every flag it accepts, which is the usual case for a Rust
/// binary — as opposed to a script forwarding options to something else.
#[derive(Cli, Debug)]
#[usage(unknown_flags = "error")]
struct Strict {
    /// Overwrite
    #[usage(long)]
    force: bool,
    /// The file
    file: String,
}

#[test]
fn a_typo_is_a_value_by_default() {
    // The default: an unrecognized flag is data, because a spec is often parsing a
    // command line whose flags belong to something else.
    let a = argv(["--forse", "x.txt"]);
    let ex = Ex::parse_from(&a).expect("should parse");
    assert_eq!(ex.file, "--forse");
    assert_eq!(ex.rest, ["x.txt"]);
}

#[test]
fn a_typo_is_reported_when_the_cli_owns_its_flags() {
    let a = argv(["--forse", "x.txt"]);
    let err = Strict::parse_from(&a).expect_err("an unknown flag should not parse");
    assert!(
        matches!(err, usage_argv::Error::UnknownFlag { token } if token == b"--forse"),
        "got {err:?}"
    );

    // And the choice reaches the spec, so docs and completions see it too.
    let spec: LibSpec = Strict::to_kdl().parse().expect("valid spec");
    assert_eq!(spec.unknown_flags, Some(usage::UnknownFlags::Error));

    // A flag it does know still works, so strictness has not broken the parse.
    let a = argv(["--force", "x.txt"]);
    let strict = Strict::parse_from(&a).expect("should parse");
    assert!(strict.force);
    assert_eq!(strict.file, "x.txt");
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

/// A dashed `name` with a bare `long` derived an unreachable long form, since a
/// token has its dashes taken off before matching.
#[derive(Cli)]
struct DashedName {
    /// Colorize output
    #[usage(name = "--color", long)]
    c: bool,
}

#[test]
fn a_dashed_name_is_normalized_too() {
    let a = argv(["--color"]);
    let cli = DashedName::parse_from(&a).expect("should parse");
    assert!(cli.c);

    let spec: LibSpec = DashedName::to_kdl()
        .parse()
        .expect("should be a valid spec");
    assert_eq!(spec.cmd.flags[0].name, "color");
    assert_eq!(spec.cmd.flags[0].long, vec!["color".to_string()]);
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

/// Field names that collide with everything the generated code needs. A parser
/// people actually use will meet a field called `text` or `argv` eventually.
#[derive(Cli)]
struct Hostile {
    /// A field named after a helper
    #[usage(long)]
    text: bool,
    /// And another
    #[usage(long)]
    value_text: bool,
    /// And the parser itself
    #[usage(long)]
    parser: bool,
    /// And the input
    #[usage(long)]
    argv: bool,
    /// And the loop variable
    #[usage(long)]
    event: bool,
    /// And the raw args
    raw: String,
}

#[test]
fn field_names_cannot_collide_with_generated_code() {
    let a = argv(["--text", "--parser", "--event", "x"]);
    let cli = Hostile::parse_from(&a).expect("should parse");
    assert!(cli.text);
    assert!(cli.parser);
    assert!(cli.event);
    assert!(!cli.value_text);
    assert!(!cli.argv);
    assert_eq!(cli.raw, "x");
}

/// A CamelCase struct name becomes a kebab-case command name, since that is what a
/// CLI is called on a command line.
#[derive(Cli)]
struct MyLittleCli {
    /// What to do
    target: String,
}

#[test]
fn a_camel_case_struct_name_is_kebab_cased() {
    let spec: LibSpec = MyLittleCli::to_kdl().parse().expect("valid spec");
    assert_eq!(spec.name, "my-little-cli");
    assert_eq!(spec.bin, "my-little-cli");

    // And it still parses, so the field is not just decoration.
    let a = argv(["something"]);
    let cli = MyLittleCli::parse_from(&a).expect("should parse");
    assert_eq!(cli.target, "something");
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

/// The crate-level example from usage-derive's documentation, kept here because
/// that crate cannot dev-depend on usage-argv without making itself unpublishable
/// — see the note in `derive/Cargo.toml`. If this changes, change the docs too.
mod docs_example {
    use super::argv;
    use usage_derive::Cli;

    /// A tool that does things
    #[derive(Cli)]
    #[usage(bin = "ex", version = "1.0")]
    struct Cli {
        /// How many jobs to run at once
        #[usage(short = 'j', long, env = "EX_JOBS", default = "4")]
        jobs: Option<String>,

        /// Print more
        #[usage(short = 'v', long, count)]
        verbose: u8,

        /// Colorize output
        #[usage(long, negate = "--no-color", default = "true")]
        color: bool,

        /// Files to process
        files: Vec<String>,
    }

    #[test]
    fn the_crate_level_example_from_the_docs() {
        let a = argv(["-j8", "--no-color", "a.txt"]);
        let cli = Cli::parse_from(&a).unwrap();
        assert_eq!(cli.jobs.as_deref(), Some("8"));
        assert!(!cli.color);
        assert_eq!(cli.files, ["a.txt"]);
        assert_eq!(cli.verbose, 0);

        // The same declaration is also the spec, which is what generates docs,
        // manpages, and completions.
        assert!(Cli::to_kdl().contains(r#"flag "-j --jobs""#));
    }
}

/// A CLI whose flags relate to one another.
///
/// Separate from `Ex` so the relationships do not have to hold for every other test's
/// command line. Enforcement is checked in `post_binding.rs`; what matters here is
/// that the declarations reach the spec, since a conflict nobody wrote down is a
/// conflict `usage g markdown` and the completions cannot mention.
#[derive(Cli, Debug)]
#[usage(bin = "rel")]
struct Related {
    /// Read from a file
    #[usage(long, required_unless("--url", "--stdin"))]
    file: Option<String>,
    /// Read from a URL
    #[usage(long)]
    url: Option<String>,
    /// Read from standard input
    #[usage(short = 's', long, conflicts("--file", "--url"))]
    stdin: bool,
    /// Colorize output
    #[usage(long, default = "true", overrides = "--plain")]
    color: bool,
    /// No decoration
    #[usage(long)]
    plain: bool,
    /// Where to write
    #[usage(long, required_if = "--stdin")]
    out: Option<String>,
}

#[test]
fn flag_relationships_reach_the_spec() {
    let spec: LibSpec = Related::to_kdl().parse().expect("valid spec");
    let flag = |name: &str| {
        spec.cmd
            .flags
            .iter()
            .find(|f| f.name == name)
            .unwrap_or_else(|| panic!("--{name} should be in the spec"))
            .clone()
    };

    assert_eq!(
        flag("stdin").conflicts,
        vec!["--file".to_string(), "--url".to_string()]
    );
    assert_eq!(flag("out").required_if, vec!["--stdin".to_string()]);
    assert_eq!(flag("color").overrides, vec!["--plain".to_string()]);
    assert_eq!(
        flag("file").required_unless,
        vec!["--url".to_string(), "--stdin".to_string()]
    );

    // Written as the declaration spelled them, not normalized to the field name: a
    // selector is how the spec refers to a flag, and `-s` would be just as valid.
    assert!(
        Related::to_kdl().contains(r#"conflicts "--file" "--url""#),
        "{}",
        Related::to_kdl()
    );

    // And the same declaration still parses: a satisfied set of relationships is
    // invisible, which is the point.
    let a = argv(["--stdin", "--out", "o"]);
    let rel = Related::parse_from(&a).expect("should parse");
    assert!(rel.stdin);
    assert!(rel.color && !rel.plain);
    assert_eq!(rel.out.as_deref(), Some("o"));
    assert!(rel.file.is_none());
    assert!(rel.url.is_none());
}

/// A command whose trailing words are only reachable after a `--`.
///
/// mise declares this on `run`, `exec` and `git`: `[ARGS]…` takes the words before the
/// separator, `[-- ARGS_LAST]…` the ones after. It reads like an argument after a
/// variadic, which usually cannot be filled — but the `--` is what stops the variadic,
/// so this one can.
#[derive(Cli, Debug)]
#[usage(bin = "sep")]
struct Separated {
    /// Words before the separator
    #[usage(arg, name = "ARGS")]
    args: Vec<String>,
    /// Words after it
    #[usage(arg, name = "ARGS_LAST", double_dash = "required")]
    args_last: Vec<String>,
}

#[test]
fn a_double_dash_argument_can_follow_a_variadic() {
    let a = argv(["a", "b", "--", "c", "d"]);
    let sep = Separated::parse_from(&a).expect("should parse");
    assert_eq!(sep.args, ["a", "b"]);
    assert_eq!(sep.args_last, ["c", "d"]);

    // Without the separator the first variadic keeps everything.
    let a = argv(["a", "b", "c"]);
    let sep = Separated::parse_from(&a).expect("should parse");
    assert_eq!(sep.args, ["a", "b", "c"]);
    assert!(sep.args_last.is_empty());

    // And the spec says which is which, so docs and completions agree with the parse.
    // usage-lib writes the mode as a property rather than in the placeholder; either
    // spelling reads back the same, and this is the one the writer chose.
    let kdl = Separated::to_kdl();
    assert!(
        kdl.contains(r#"arg "[ARGS_LAST]..." help="Words after it" double_dash="required""#),
        "{kdl}"
    );
    let spec: LibSpec = kdl.parse().expect("valid spec");
    let last = spec.cmd.args.last().expect("two args");
    assert_eq!(last.double_dash, usage::SpecDoubleDashChoices::Required);
    assert!(last.var);
}
