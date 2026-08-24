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

#[derive(Cli)]
#[usage(bin = "column-cap", term_width = 80)]
#[allow(dead_code)]
struct CappedColumns {
    /// Disable reporting on warnings
    #[usage(long)]
    quiet: bool,
    /// Choose the severity for unused directives
    #[usage(
        long = "report-unused-disable-directives-severity",
        value_name = "SEVERITY"
    )]
    severity: Option<String>,
    /// Number of threads to use
    #[usage(long)]
    threads: Option<usize>,
}

#[derive(Cli)]
#[usage(
    bin = "oxlint-layout",
    term_width = 80,
    about = "A fast JavaScript and TypeScript linter with a deliberately long introduction that must wrap cleanly"
)]
#[allow(dead_code)]
struct OxcHelpLayout {
    /// Do not fail when a supplied pattern matches no files
    #[usage(long)]
    no_error_on_unmatched_pattern: bool,
    /// Disable the TypeScript language service plugin
    #[usage(long)]
    disable_typescript_plugin: bool,
    /// Add a pattern to the ignore list while retaining every previously configured pattern
    #[usage(long, value_name = "PATTERN")]
    ignore_pattern: Vec<String>,
    #[usage(
        long,
        value_name = "OPTIONS",
        help = "Enable debug output for selected subsystems",
        long_help = "Enable debug output for selected subsystems.\n\n- parser: trace parsed files and recovered syntax\n- resolver: trace module resolution and cache decisions"
    )]
    debug: Option<String>,
    #[usage(
        long,
        value_name = "PATH",
        help = "Use a configuration file",
        long_help = "Use a configuration file. Configuration discovered from parent directories is merged first.\n\nWarning: explicit files replace project-local defaults.\n\n    oxlint-layout --config ./oxlint.json"
    )]
    config: Option<String>,
}

#[test]
fn oxc_shaped_help_keeps_useful_prose_inline_and_wraps_the_rest() {
    let spec = OxcHelpLayout::spec();
    let portable: LibSpec = OxcHelpLayout::to_kdl().parse().expect("valid spec");

    for long in [false, true] {
        let page = usage_argv::help::render(spec, spec.root.cmd, long).unwrap();
        assert_eq!(
            page,
            usage::docs::cli::render_help(&portable, &portable.cmd, long)
        );
        assert!(page.lines().any(|line| {
            line.contains("--no-error-on-unmatched-pattern") && line.contains("Do not fail when")
        }));
        assert!(page.lines().any(|line| {
            line.contains("--disable-typescript-plugin") && line.contains("Disable the TypeScript")
        }));
        assert!(page.lines().all(|line| !line.ends_with(' ')), "{page}");
        for line in page.lines() {
            if !line.trim_start().starts_with("oxlint-layout --config")
                && !line.starts_with("Usage:")
            {
                assert!(
                    line.chars().count() <= 80,
                    "line exceeds width: {line:?}\n{page}"
                );
            }
        }
    }

    let long = usage_argv::help::render(spec, spec.root.cmd, true).unwrap();
    assert!(long.contains("\n\n"), "{long}");
    assert!(long.contains("- parser:"), "{long}");
    assert!(long.contains("- resolver:"), "{long}");
    assert!(
        long.contains("    oxlint-layout --config ./oxlint.json"),
        "{long}"
    );
}

#[derive(Args)]
#[usage(disable_help_flag, term_width = 0)]
#[allow(dead_code)]
struct CappedFlatArgs {
    /// Disable reporting on warnings
    #[usage(long)]
    quiet: bool,
    /// Choose the severity for unused directives while keeping every long explanation inside the bounded parent help page
    #[usage(
        long = "report-unused-disable-directives-severity",
        value_name = "SEVERITY",
        env = "COLUMN_CAP_SEVERITY",
        default = "warn"
    )]
    severity: Option<String>,
    #[usage(
        long = "an-extraordinarily-long-multiline-option-name",
        help = "Keep this line.\nAnd this one."
    )]
    multiline: bool,
}

#[derive(Subcommands)]
#[allow(dead_code)]
enum CappedFlatCommands {
    Run(CappedFlatArgs),
}

#[derive(Args)]
#[allow(dead_code)]
struct LongCommandArgs {}

#[derive(Subcommands)]
#[allow(dead_code)]
enum LongCommandNames {
    /// Explain this unusually named command with enough detail to wrap beneath its name on a bounded help page
    #[usage(name = "an-extraordinarily-long-subcommand-name")]
    Long(LongCommandArgs),
}

#[derive(Cli)]
#[usage(bin = "command-cap", term_width = 80)]
#[allow(dead_code)]
struct CappedCommandColumns {
    #[usage(subcommand)]
    command: Option<LongCommandNames>,
}

#[derive(Cli)]
#[usage(bin = "column-cap-flat", term_width = 80, flatten_help)]
#[allow(dead_code)]
struct CappedFlatColumns {
    #[usage(subcommand)]
    command: Option<CappedFlatCommands>,
}

#[test]
fn a_long_flag_only_moves_its_own_help_below_the_column() {
    let spec = CappedColumns::spec();
    let portable: LibSpec = CappedColumns::to_kdl().parse().expect("valid spec");

    for long in [false, true] {
        let page = usage_argv::help::render(spec, spec.root.cmd, long).unwrap();
        assert_eq!(
            page,
            usage::docs::cli::render_help(&portable, &portable.cmd, long)
        );

        let lines: Vec<_> = page.lines().collect();
        assert!(
            lines.iter().any(|line| {
                line.contains("--quiet") && line.contains("Disable reporting on warnings")
            }),
            "{page}"
        );
        assert!(
            lines.iter().any(|line| {
                line.contains("--threads <THREADS>") && line.contains("Number of threads to use")
            }),
            "{page}"
        );

        let outlier = lines
            .iter()
            .position(|line| line.contains("--report-unused-disable-directives-severity"))
            .expect("outlier flag");
        assert_eq!(
            lines[outlier].trim(),
            "--report-unused-disable-directives-severity <SEVERITY>"
        );
        assert_eq!(
            lines.get(outlier + 1).map(|line| line.trim()),
            Some("Choose the severity for unused directives")
        );
        assert!(lines[outlier + 1].starts_with("    "), "{page}");
    }
}

#[test]
fn flattened_help_caps_each_nested_commands_columns_too() {
    let spec = CappedFlatColumns::spec();
    let portable: LibSpec = CappedFlatColumns::to_kdl().parse().expect("valid spec");

    for long in [false, true] {
        let page = usage_argv::help::render(spec, spec.root.cmd, long).unwrap();
        assert_eq!(
            page,
            usage::docs::cli::render_help(&portable, &portable.cmd, long)
        );
        assert!(
            page.lines().any(|line| {
                line.contains("--quiet") && line.contains("Disable reporting on warnings")
            }),
            "{page}"
        );
        let lines = page.lines().collect::<Vec<_>>();
        let severity = lines
            .iter()
            .position(|line| line.contains("--report-unused-disable-directives-severity"))
            .expect("severity flag");
        assert_eq!(
            lines[severity + 1].trim(),
            "Choose the severity for unused directives"
        );
        assert_eq!(
            lines[severity + 2].trim(),
            "while keeping every long explanation inside"
        );
        let multiline = lines
            .iter()
            .position(|line| line.contains("--an-extraordinarily-long-multiline-option-name"))
            .expect("multiline flag");
        assert_eq!(lines[multiline + 1].trim(), "Keep this line.");
        assert_eq!(lines[multiline + 2].trim(), "And this one.");
        if !long {
            assert!(
                page.contains("[env:")
                    && page.contains("COLUMN_CAP_SEVERITY]")
                    && page.contains("(default: warn)"),
                "{page}"
            );
        }
        assert!(
            page.lines()
                .filter(|line| !line.trim_start().starts_with("column-cap-flat run"))
                .all(|line| line.chars().count() <= 80),
            "a flattened help row exceeded 80 columns: {page}"
        );
    }
}

#[test]
fn a_long_command_name_keeps_help_inline_when_useful_room_remains() {
    let spec = CappedCommandColumns::spec();
    let portable: LibSpec = CappedCommandColumns::to_kdl().parse().expect("valid spec");

    for long in [false, true] {
        let page = usage_argv::help::render(spec, spec.root.cmd, long).unwrap();
        assert_eq!(
            page,
            usage::docs::cli::render_help(&portable, &portable.cmd, long)
        );
        assert!(
            page.contains("an-extraordinarily-long-subcommand-name  Explain this unusually named command\n                                  with enough detail to wrap beneath\n                                  its name on a bounded help page"),
            "{page}"
        );
    }
}

#[derive(Args)]
#[usage(deprecated = "use inspect", deprecated_remove_at = "7.0")]
struct Old {}

#[derive(Subcommands)]
enum DeprecatedCommands {
    #[usage(deprecated = "use show", deprecated_warn_at = "6.1")]
    Old(Old),
}

#[derive(Cli)]
#[usage(bin = "deprecated", deprecated = "use the replacement")]
#[allow(dead_code)]
struct DeprecatedCli {
    #[usage(
        long,
        deprecated = "use --new",
        deprecated_warn_at = "6.2",
        deprecated_remove_at = "7.0"
    )]
    old: bool,
    #[usage(subcommand)]
    command: Option<DeprecatedCommands>,
}

#[test]
fn deprecation_metadata_survives_the_typed_spec() {
    let spec: LibSpec = DeprecatedCli::to_kdl().parse().expect("valid spec");
    assert_eq!(spec.cmd.deprecated.as_deref(), Some("use the replacement"));
    let flag = spec
        .cmd
        .flags
        .iter()
        .find(|flag| flag.name == "old")
        .unwrap();
    assert_eq!(flag.deprecated.as_deref(), Some("use --new"));
    assert_eq!(flag.deprecated_warn_at.as_deref(), Some("6.2"));
    assert_eq!(flag.deprecated_remove_at.as_deref(), Some("7.0"));
    let command = spec.cmd.subcommands.get("old").expect("old command");
    assert_eq!(command.deprecated.as_deref(), Some("use show"));
    assert_eq!(command.deprecated_warn_at.as_deref(), Some("6.1"));
    assert_eq!(command.deprecated_remove_at.as_deref(), Some("7.0"));
}

#[test]
fn deprecation_renders_inline_and_in_flattened_command_sections() {
    let spec = DeprecatedCli::spec();
    let page = usage_argv::help::render(spec, spec.root.cmd, false).unwrap();
    // A flag with no description carries its label in the description column, so what
    // separates the two is the column rather than a single space. Read with the layout
    // collapsed: the label reaching the flag's row is the point, not the width of the gap.
    let flattened = page.split_whitespace().collect::<Vec<_>>().join(" ");
    assert!(
        flattened.contains("--old [deprecated: use --new; warns at 6.2; removed at 7.0]"),
        "{page}"
    );

    let mut portable: LibSpec = DeprecatedCli::to_kdl().parse().unwrap();
    portable.cmd.flatten_help = true;
    let page = usage::docs::cli::render_help(&portable, &portable.cmd, false);
    assert!(
        page.contains("old:\n[deprecated: use show; warns at 6.1; removed at 7.0]"),
        "{page}"
    );
}

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
    //
    // Shouted like every other placeholder, so one CLI does not print `-j <jobs>` beside
    // `--jobs <JOBS>`. clap prints `-j <JOBS>`.
    let spec: LibSpec = ShortOnly::to_kdl().parse().expect("valid spec");
    let flag = &spec.cmd.flags[0];
    assert_eq!(
        flag.name, "j",
        "named after the form, as usage-lib names it"
    );
    assert_eq!(
        flag.arg.as_ref().expect("takes a value").name,
        "JOBS",
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
    // so a field called `type_` must not drag its ident into the placeholder and render
    // `--type <TYPE_>`. Shouted, which is what clap prints for all three shapes:
    //
    //       --type <TYPE>
    //       --max-tokens <MAX_TOKENS>
    //   -c, --config <CONFIG>
    let spec: LibSpec = Renamed::to_kdl().parse().expect("valid spec");
    let flag = &spec.cmd.flags[0];
    assert_eq!(flag.name, "type");
    assert_eq!(flag.arg.as_ref().expect("takes a value").name, "TYPE");

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

#[doc = " Ordinary first line\n    indented continuation"]
#[derive(Cli)]
#[usage(bin = "ordinary-comments")]
struct OrdinaryComments {}

#[doc = " Verbatim first line\n    indented continuation"]
#[derive(Cli)]
#[usage(bin = "verbatim-attribute-comments", verbatim_doc_comment)]
struct VerbatimAttributeComments {}

/// First root line
/// second root line
///
///     root example
#[derive(Cli)]
#[usage(bin = "verbatim-comments", verbatim_doc_comment)]
struct VerbatimComments {
    /// First field line
    /// second field line
    ///
    ///     field example
    #[usage(long, verbatim_doc_comment)]
    layout: bool,
    #[usage(subcommand)]
    command: Option<VerbatimCommands>,
}

#[derive(Args)]
struct Paint {}

#[derive(Subcommands)]
enum VerbatimCommands {
    /// First command line
    /// second command line
    #[usage(verbatim_doc_comment)]
    Paint(Paint),
}

#[test]
fn doc_comments_can_preserve_their_layout() {
    let spec: LibSpec = VerbatimComments::to_kdl().parse().expect("valid spec");
    assert_eq!(
        spec.about.as_deref(),
        Some("First root line\nsecond root line")
    );
    assert_eq!(
        spec.about_long.as_deref(),
        Some("First root line\nsecond root line\n\n    root example")
    );

    let layout = spec.cmd.flags.iter().find(|f| f.name == "layout").unwrap();
    assert_eq!(
        layout.help.as_deref(),
        Some("First field line\nsecond field line")
    );
    assert_eq!(
        layout.help_long.as_deref(),
        Some("First field line\nsecond field line\n\n    field example")
    );

    let paint = spec.cmd.subcommands.get("paint").expect("paint");
    assert_eq!(
        paint.help.as_deref(),
        Some("First command line\nsecond command line")
    );
    assert!(paint.help_long.is_none());

    let argv = [
        std::ffi::OsStr::new("--layout"),
        std::ffi::OsStr::new("paint"),
    ];
    let parsed = VerbatimComments::parse_from(&argv).expect("the metadata still parses");
    assert!(parsed.layout);
    assert!(matches!(parsed.command, Some(VerbatimCommands::Paint(_))));
}

#[test]
fn ordinary_multiline_doc_attributes_keep_their_indentation() {
    let spec: LibSpec = OrdinaryComments::to_kdl().parse().expect("valid spec");
    // The continuation is indented, so it is an example line rather than wrapped
    // prose: both forms keep the newline instead of turning it into spaces.
    assert_eq!(
        spec.about.as_deref(),
        Some("Ordinary first line\n    indented continuation")
    );
    assert!(spec.about_long.is_none());
}

#[test]
fn verbatim_multiline_doc_attributes_keep_their_indentation() {
    let spec: LibSpec = VerbatimAttributeComments::to_kdl()
        .parse()
        .expect("valid spec");
    assert_eq!(
        spec.about.as_deref(),
        Some("Verbatim first line\n    indented continuation")
    );
    assert!(spec.about_long.is_none());
}

/// Generates shell code that enables automatic daemon management when changing
/// directories. Required for auto-start/stop features in pitchfork.toml.
///
/// Add to your shell config:
///
///     eval "$(pitchfork activate bash)"
#[derive(Cli)]
#[usage(bin = "flowed-comments")]
struct FlowedComments {
    /// Number of jobs to run.
    ///
    /// More detail continues
    /// onto the next source line.
    #[usage(long)]
    jobs: bool,
}

#[test]
fn ordinary_prose_doc_comments_flow_like_clap() {
    let spec: LibSpec = FlowedComments::to_kdl().parse().expect("valid spec");
    assert_eq!(
        spec.about.as_deref(),
        Some(
            "Generates shell code that enables automatic daemon management when changing directories. Required for auto-start/stop features in pitchfork.toml."
        )
    );
    let long = spec.about_long.as_deref().expect("long help");
    assert_eq!(
        long,
        "Generates shell code that enables automatic daemon management when changing directories. Required for auto-start/stop features in pitchfork.toml.\n\nAdd to your shell config:\n\n    eval \"$(pitchfork activate bash)\""
    );
    assert!(
        spec.about.as_deref().unwrap().ends_with('.'),
        "trailing periods must survive"
    );
    assert!(
        !long.contains("changing\ndirectories"),
        "source wraps must not become markdown line breaks:\n{long}"
    );

    let jobs = spec.cmd.flags.iter().find(|f| f.name == "jobs").unwrap();
    assert_eq!(jobs.help.as_deref(), Some("Number of jobs to run."));
    assert_eq!(
        jobs.help_long.as_deref(),
        Some("Number of jobs to run.\n\nMore detail continues onto the next source line.")
    );

    let parsed = FlowedComments::parse_from(&[std::ffi::OsStr::new("--jobs")])
        .expect("the metadata still parses");
    assert!(parsed.jobs);
}

/// Short summary.
///
/// ~~~
/// first
///
/// second
/// third
/// ```
/// ~~~
#[derive(Cli)]
#[usage(bin = "tilde-fenced-comments")]
struct TildeFencedComments {}

#[test]
fn ordinary_tilde_fenced_blocks_keep_their_line_breaks() {
    let spec: LibSpec = TildeFencedComments::to_kdl().parse().expect("valid spec");
    assert_eq!(spec.about.as_deref(), Some("Short summary."));
    assert!(
        spec.about.as_deref().unwrap().ends_with('.'),
        "trailing periods must survive"
    );
    assert_eq!(
        spec.about_long.as_deref(),
        Some("Short summary.\n\n~~~\nfirst\n\nsecond\nthird\n```\n~~~")
    );
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

/// A command whose short description is on the enum and whose long one is on the struct.
///
/// The shape every generated CLI has, and mise's own: the variant says what the command is for
/// in a line, and the struct's comment carries the detail.
#[derive(Args)]
/// Initializes mise in the current shell session
///
/// This should go into your shell's rc file.
/// Otherwise, it will only take effect in the current session.
///
///     echo 'eval "$(mise activate zsh)"' >> ~/.zshrc
struct ActivateArgs {
    /// Use shims instead of modifying PATH
    #[usage(long)]
    shims: bool,
}

#[derive(Subcommands)]
enum SplitCommands {
    /// Initializes mise in the current shell session
    Activate(Box<ActivateArgs>),
}

#[derive(Cli)]
#[usage(
    bin = "split",
    about = "Dev tools, env vars, and tasks in one CLI",
    long_about = "split prepares your development environment before each command runs."
)]
struct Split {
    #[usage(subcommand)]
    command: Option<SplitCommands>,
}

#[test]
fn a_variants_short_description_does_not_hide_the_structs_long_one() {
    // Each falls back on its own. A variant that gave a short description was suppressing the
    // struct's long one, so the long form went missing from help for every command written the
    // way generated CLIs are written.
    let spec: LibSpec = Split::to_kdl().parse().expect("valid spec");
    let activate = spec.cmd.subcommands.get("activate").expect("activate");
    assert_eq!(
        activate.help.as_deref(),
        Some("Initializes mise in the current shell session"),
        "the variant's line"
    );
    let long = activate.help_long.as_deref().expect("the struct's detail");
    assert!(long.contains("rc file"), "{long}");
}

#[test]
fn an_indented_example_in_help_keeps_its_indentation() {
    // A doc comment's lines were trimmed one by one, which flattened every indented block in a
    // CLI's help — and an indented block is how a spec shows a command to type. mise's help is
    // full of them.
    let spec: LibSpec = Split::to_kdl().parse().expect("valid spec");
    let activate = spec.cmd.subcommands.get("activate").expect("activate");
    let long = activate.help_long.as_deref().expect("long help");
    assert!(
        long.contains("\n    echo 'eval"),
        "the example should still be indented:\n{long}"
    );
}

#[test]
fn a_program_can_describe_itself_twice_over() {
    // A comment's long form always contains its short one, because the short form *is* its
    // first paragraph. A spec keeps the two independent, and mise's differ entirely — so there
    // is no comment that says both.
    let spec: LibSpec = Split::to_kdl().parse().expect("valid spec");
    assert_eq!(
        spec.about.as_deref(),
        Some("Dev tools, env vars, and tasks in one CLI")
    );
    assert_eq!(
        spec.about_long.as_deref(),
        Some("split prepares your development environment before each command runs.")
    );
}

#[test]
fn the_split_description_cli_still_parses() {
    // Reading what the fixture declares, which is also how these structs avoid being dead
    // code: a test CLI nobody parses is a warning, and CI denies warnings.
    use std::ffi::OsStr;

    let argv = [OsStr::new("activate"), OsStr::new("--shims")];
    let Some(SplitCommands::Activate(activate)) =
        Split::parse_from(&argv).expect("should parse").command
    else {
        panic!("expected activate")
    };
    assert!(activate.shims);
}

/// A CLI whose root says something above and below every page.
#[derive(Args)]
struct Inner {
    /// A value
    #[usage(arg, name = "VALUE")]
    value: Option<String>,
}

#[derive(Subcommands)]
enum SurroundedCommands {
    /// Do the thing
    Go(Box<Inner>),
}

#[derive(Cli)]
#[usage(
    bin = "surrounded",
    before_help = "Read this first.",
    after_help = "And this after."
)]
struct Surrounded {
    #[usage(subcommand)]
    command: Option<SurroundedCommands>,
}

#[test]
fn the_roots_surrounding_text_reaches_every_page() {
    // A root has nowhere else to put this: `to_kdl` writes it at the top level, and the
    // reference reads text there as the default for *every* page, not just the root's. Emitted
    // only on the root's metadata, the preamble a CLI declared vanished from every subcommand
    // page — and reappeared when the same CLI was rendered from its own emitted KDL, which is
    // two answers to one question.
    let spec = Surrounded::spec();
    assert_eq!(spec.root.before_help, Some("Read this first."));
    assert_eq!(spec.root.after_help, Some("And this after."));

    let go = spec.root.subcommands[0];
    let page = usage_argv::help::short_help(spec, &["surrounded", "go"], &[spec.root, go]);
    assert!(page.starts_with("Read this first.\n"), "{page}");
    assert!(page.trim_end().ends_with("And this after."), "{page}");

    // The same CLI still parses, so what the pages describe is what it does.
    let argv = [
        std::ffi::OsStr::new("go"),
        std::ffi::OsStr::new("something"),
    ];
    let parsed = Surrounded::parse_from(&argv).expect("a subcommand and its value");
    let Some(SurroundedCommands::Go(inner)) = parsed.command else {
        panic!("expected go")
    };
    assert_eq!(inner.value.as_deref(), Some("something"));

    // And the reference agrees, reading the KDL this CLI writes.
    let kdl = spec.to_kdl();
    let lib: LibSpec = kdl.parse().expect("valid spec");
    let lib_go = lib.cmd.subcommands.get("go").expect("go");
    assert_eq!(page, usage::docs::cli::render_help(&lib, lib_go, false));
}

/// A command whose examples are declared on the type that holds them.
#[derive(Args)]
#[usage(example(
    "worked deploy -e prod",
    header = "Basic deployment",
    help = "Deploy to production"
))]
struct Deploy {
    /// Where to deploy
    #[usage(short, long)]
    environment: Option<String>,
}

/// A command whose examples belong to the variant rather than to the type.
#[derive(Args)]
#[usage(example = "worked shared --from-the-type")]
#[allow(dead_code)]
struct Shared {}

#[derive(Subcommands)]
enum WorkedCommands {
    /// Deploy the application
    Deploy(Deploy),
    /// Do the shared thing
    #[usage(example = "worked shared --from-the-variant")]
    Shared(Shared),
}

/// A tool with worked invocations
#[derive(Cli)]
#[usage(bin = "worked", example = "worked deploy -e prod")]
#[allow(dead_code)]
struct Worked {
    #[usage(subcommand)]
    command: Option<WorkedCommands>,
}

/// Examples were the last thing a KDL spec could say and a typed CLI could not.
///
/// The tables and both help renderers have carried `example` the whole time — only the
/// derive had no vocabulary for one, so a typed CLI's worked invocations had to live as
/// prose inside `after_long_help`, where docs, manpages and `usage lint` cannot read
/// them.
#[test]
fn examples_survive_the_round_trip_from_a_typed_declaration() {
    let spec = Worked::spec();
    let portable: LibSpec = Worked::to_kdl().parse().expect("valid spec");

    assert_eq!(portable.examples.len(), 1);
    assert_eq!(portable.examples[0].code, "worked deploy -e prod");

    let deploy = portable.cmd.subcommands.get("deploy").expect("deploy");
    assert_eq!(deploy.examples.len(), 1);
    assert_eq!(deploy.examples[0].code, "worked deploy -e prod");
    assert_eq!(
        deploy.examples[0].header.as_deref(),
        Some("Basic deployment")
    );
    assert_eq!(
        deploy.examples[0].help.as_deref(),
        Some("Deploy to production")
    );

    // A variant that declares examples speaks for the command; the held type's stand
    // only where it declares none, which is how `after_help` already behaves.
    let shared = portable.cmd.subcommands.get("shared").expect("shared");
    assert_eq!(shared.examples.len(), 1);
    assert_eq!(shared.examples[0].code, "worked shared --from-the-variant");

    // And both renderers show the same page, reading the same declaration.
    let page = usage_argv::help::render(spec, spec.root.cmd, true).unwrap();
    assert!(page.contains("Examples:"), "{page}");
    assert!(page.contains("$ worked deploy -e prod"), "{page}");
    assert_eq!(
        page,
        usage::docs::cli::render_help(&portable, &portable.cmd, true)
    );

    let argv_deploy = spec
        .root
        .subcommands
        .iter()
        .find(|meta| meta.cmd.name == "deploy")
        .expect("deploy");
    let page = usage_argv::help::render(spec, argv_deploy.cmd, true).unwrap();
    assert!(page.contains("  Basic deployment:"), "{page}");
    assert_eq!(page, usage::docs::cli::render_help(&portable, deploy, true));

    // And the example is a command line this CLI accepts, which is the whole claim an
    // example makes. `usage lint` asks the same question of a spec; here it is asked of
    // the declaration the spec came from.
    let argv = [
        std::ffi::OsStr::new("deploy"),
        std::ffi::OsStr::new("-e"),
        std::ffi::OsStr::new("prod"),
    ];
    let parsed = Worked::parse_from(&argv).expect("the example it documents");
    let Some(WorkedCommands::Deploy(deploy)) = parsed.command else {
        panic!("expected deploy")
    };
    assert_eq!(deploy.environment.as_deref(), Some("prod"));
}

#[derive(Args)]
struct Go {
    /// Something to do it to.
    value: Option<String>,
}

#[derive(Subcommands)]
#[allow(dead_code)]
enum LinkedCommands {
    /// Go somewhere.
    Go(Go),
}

#[derive(Cli)]
#[usage(
    bin = "linked",
    repository = "https://github.com/jdx/usage",
    source_code_link_template = r#"{%- set path = path | replace(from='-', to='_') -%}
{%- if cmd.subcommands | length > 0 -%}
{%- set path = path ~ "/mod.rs" -%}
{%- else -%}
{%- set path = path ~ ".rs" -%}
{%- endif -%}
https://github.com/jdx/usage/blob/main/cli/src/cli/{{path}}"#
)]
#[allow(dead_code)]
struct Linked {
    #[usage(subcommand)]
    command: LinkedCommands,
}

#[test]
fn the_source_code_link_template_survives_the_typed_spec() {
    // A fourth. `usage` itself declared this in a KDL fragment appended to its own emitted
    // spec, because the derive had no word for it — the one thing left keeping a hand-written
    // second model beside the declaration. It reaches markdown, where it becomes the "view
    // source" link on every command page, so a derive that drops it silently loses a link on
    // every page of every CLI that wanted one.
    let kdl = Linked::to_kdl();
    let spec: LibSpec = kdl.parse().expect("valid spec");

    // Newlines make it through as a KDL raw multiline string, and the template means
    // nothing if its lines run together.
    let template = spec
        .source_code_link_template
        .as_deref()
        .expect("the template reached the typed spec");
    assert_eq!(template.lines().count(), 7, "{template:?}");
    assert!(template.starts_with("{%- set path"), "{template:?}");
    assert!(template.ends_with("/cli/src/cli/{{path}}"), "{template:?}");
    assert_eq!(
        spec.repository.as_deref(),
        Some("https://github.com/jdx/usage")
    );

    // And it renders, which is the only reason to carry it.
    let go = spec.cmd.subcommands.get("go").expect("go");
    let renderer = usage::docs::markdown::MarkdownRenderer::new(spec.clone());
    let page = renderer.render_cmd(go).expect("a page for go");
    assert!(
        page.contains("https://github.com/jdx/usage/blob/main/cli/src/cli/go.rs"),
        "{page}"
    );
}
