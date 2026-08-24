use usage::docs::markdown::{MarkdownRenderer, MarkdownTheme};
use usage_argv::help;
use usage_derive::{Args, Cli};

const FILTERS: &str = "Filters accumulate from left to right on the command line.\n\
For example: `-D correctness -A no-debugger`.";

#[derive(Args)]
#[allow(dead_code)]
struct Filters {
    /// Allow the rule or category.
    #[usage(short = 'A', long)]
    allow: Vec<String>,

    /// Deny the rule or category.
    #[usage(short = 'D', long)]
    deny: Vec<String>,
}

#[derive(Cli)]
#[usage(
    bin = "ex",
    heading("Filters", help = FILTERS),
    heading("Ignore Files", help = "Paths are matched the way `.gitignore` matches them.")
)]
#[allow(dead_code)]
struct Ex {
    #[usage(flatten, next_help_heading = "Filters")]
    filters: Filters,

    /// Ignore file to read.
    #[usage(long, help_heading = "Ignore Files")]
    ignore_path: Option<String>,

    /// File to inspect.
    file: Option<String>,
}

/// Prose declared on the flattened type itself, for the section it contributes.
#[derive(Args)]
#[usage(heading("Output", help = "Formats are stable; parse the JSON one."))]
#[allow(dead_code)]
struct Output {
    /// Output format.
    #[usage(long, help_heading = "Output")]
    format: Option<String>,
}

#[derive(Cli)]
#[usage(bin = "flat")]
#[allow(dead_code)]
struct FlattenedCli {
    #[usage(flatten)]
    output: Output,
}

#[test]
fn section_prose_introduces_its_heading_on_the_long_page() {
    let long = help::render(Ex::spec(), Ex::spec().root.cmd, true).expect("long help");
    assert!(
        long.contains(
            "Filters:\n  Filters accumulate from left to right on the command line.\n  For example: `-D correctness -A no-debugger`.\n"
        ),
        "{long}"
    );
    assert!(
        long.contains("Ignore Files:\n  Paths are matched the way `.gitignore` matches them.\n"),
        "{long}"
    );
    // The prose introduces the section rather than replacing it.
    let filters = long.find("Filters:").expect("filters heading");
    let allow = long.find("--allow").expect("the section's own entries");
    assert!(filters < allow, "{long}");

    // A short page stays a summary, as it does for admonitions.
    let short = help::render(Ex::spec(), Ex::spec().root.cmd, false).expect("short help");
    assert!(short.contains("Filters:"), "{short}");
    assert!(!short.contains("accumulate from left to right"), "{short}");
}

#[test]
fn a_topic_opens_with_its_sections_prose() {
    let topic = usage_argv::help::render_topic(Ex::spec(), Ex::command(), "filters", true)
        .expect("the heading is a topic");
    assert!(
        topic.starts_with(
            "Filters:\n  Filters accumulate from left to right on the command line.\n"
        ),
        "{topic}"
    );
}

#[test]
fn section_prose_survives_the_round_trip_through_the_spec() {
    let kdl = Ex::to_kdl();
    assert!(kdl.contains("heading \"Ignore Files\""), "{kdl}");
    assert!(kdl.contains("heading Filters"), "{kdl}");

    let spec: usage::Spec = kdl.parse().expect("generated spec");
    assert_eq!(spec.cmd.headings.len(), 2);
    let filters = spec
        .cmd
        .headings
        .iter()
        .find(|heading| heading.title == "Filters")
        .expect("filters prose");
    assert_eq!(filters.help, FILTERS);

    // The reference renderer is held byte-identical to the compiled one.
    let reference = usage::docs::cli::render_help(&spec, &spec.cmd, true);
    let compiled = help::render(Ex::spec(), Ex::spec().root.cmd, true).expect("long help");
    assert_eq!(reference, compiled);
}

#[test]
fn section_prose_reaches_generated_markdown() {
    let spec: usage::Spec = Ex::to_kdl().parse().expect("generated spec");

    let compact = MarkdownRenderer::new(spec.clone())
        .render_cmd(&spec.cmd)
        .expect("markdown page");
    assert!(
        compact.contains("Filters accumulate from left to right on the command line."),
        "{compact}"
    );
    assert!(
        compact.contains("Paths are matched the way `.gitignore` matches them."),
        "{compact}"
    );

    let detailed = MarkdownRenderer::new(spec.clone())
        .with_theme(MarkdownTheme::Detailed)
        .render_cmd(&spec.cmd)
        .expect("detailed markdown page");
    assert!(
        detailed.contains("Filters accumulate from left to right on the command line."),
        "{detailed}"
    );
}

/// Prose named for a default section title, which is not a declared heading.
#[derive(Cli)]
#[usage(
    bin = "dflt",
    heading("Flags", help = "Should not appear on any page.")
)]
#[allow(dead_code)]
struct DefaultTitled {
    /// Do it anyway.
    #[usage(long)]
    force: bool,
}

#[test]
fn only_a_declared_heading_takes_prose() {
    let long = help::render(DefaultTitled::spec(), DefaultTitled::spec().root.cmd, true)
        .expect("long help");
    assert!(long.contains("Flags:"), "{long}");
    assert!(!long.contains("Should not appear"), "{long}");

    // The unheaded group is the one that exists because nothing asked for a section, and
    // it renders under a different default title per page, so keying prose to it would
    // mean different things in each renderer. Both must agree that it takes none.
    let spec: usage::Spec = DefaultTitled::to_kdl().parse().expect("generated spec");
    let reference = usage::docs::cli::render_help(&spec, &spec.cmd, true);
    assert_eq!(reference, long);

    let markdown = MarkdownRenderer::new(spec.clone())
        .render_cmd(&spec.cmd)
        .expect("markdown page");
    assert!(!markdown.contains("Should not appear"), "{markdown}");
}

#[test]
fn a_flattened_type_speaks_for_the_section_it_contributes() {
    let long =
        help::render(FlattenedCli::spec(), FlattenedCli::spec().root.cmd, true).expect("long help");
    assert!(
        long.contains("Output:\n  Formats are stable; parse the JSON one.\n"),
        "{long}"
    );

    let kdl = FlattenedCli::to_kdl();
    assert!(kdl.contains("heading Output"), "{kdl}");
}
