//! A `help_template` says what order a page's sections come in, and three implementations have
//! to agree about it.
//!
//! What the rendering corpus cannot ask, because it builds usage-argv's tables out of KDL: does a
//! template written on a Rust type reach the tables at all, and does the page a compiled parser
//! then prints match the one the reference renders from the same CLI's own emitted spec? A field
//! dropped anywhere along `#[usage(help_template = …)]` → codegen → `Spec` → `help::short_help`
//! would leave every page in the default order, which is a plausible-looking page rather than a
//! failure — so the check is against the reference, not against a transcription.
//!
//! The corpus (`corpus/render/04-help-template.json`) pins what the sections contain. This pins
//! the wiring, and the round trip through KDL that carries a template between the two.

use usage::Spec as LibSpec;
use usage_derive::{Args, Cli, Subcommands};

/// A CLI that lays its pages out itself.
///
/// Everything a template can do, in one fixture: `{{flags}}` above `{{args}}`, which inverts the
/// default order; no `{{commands}}`, so a section is missing rather than merely moved; and a line
/// of the author's own at the end, which is what separates a layout from a permutation.
#[derive(Cli)]
#[usage(
    bin = "laid-out",
    about = "An example",
    help_template = "{{about}}\n\n{{usage}}\n\n{{flags}}\n\n{{args}}\n\nSee the docs for more."
)]
struct LaidOut {
    /// Do it anyway
    #[usage(long)]
    force: bool,

    /// Which file
    #[usage(arg, name = "file")]
    file: Option<String>,

    #[usage(subcommand)]
    command: Option<LaidOutCommands>,
}

#[derive(Args)]
struct Run {
    /// Only show changes
    #[usage(long)]
    dry_run: bool,
}

#[derive(Subcommands)]
enum LaidOutCommands {
    /// Run it
    Run(Run),
}

#[test]
fn a_template_declared_on_a_rust_type_reaches_the_tables() {
    // The cold metadata a page is laid out from. Asserted before the page itself, so a field
    // lost in codegen fails as a missing template rather than as a page in the wrong order.
    let spec = LaidOut::spec();
    assert_eq!(
        spec.help_template,
        Some("{{about}}\n\n{{usage}}\n\n{{flags}}\n\n{{args}}\n\nSee the docs for more.")
    );
}

#[test]
fn the_page_a_compiled_parser_prints_is_the_one_the_reference_renders() {
    // The claim this file exists to make. Both pages come from the same CLI — usage-argv's from
    // the derive's tables, usage-lib's from the KDL that same derive emits — so a template that
    // survives one path and not the other fails here rather than in an adopter's terminal.
    let spec = LaidOut::spec();
    let page = usage_argv::help::short_help(spec, &["laid-out"], &[spec.root]);

    let lib: LibSpec = spec
        .to_kdl()
        .parse()
        .expect("the derive emits a valid spec");
    assert_eq!(page, usage::docs::cli::render_help(&lib, &lib.cmd, false));

    // And it is laid out, rather than merely rendered: the flags are above the arguments, the
    // command list the CLI does have is absent, and the author's own line closes the page.
    let flags = page.find("Flags:").expect("a flags section");
    let args = page.find("Arguments:").expect("an arguments section");
    assert!(flags < args, "flags should come first:\n{page}");
    assert!(!page.contains("Commands:"), "{page}");
    assert!(
        page.trim_end().ends_with("See the docs for more."),
        "{page}"
    );

    // The sections themselves are untouched by the reordering — a template moves a page's parts
    // and does not rewrite them.
    assert!(page.contains("      --force  Do it anyway"), "{page}");
    assert!(page.contains("  [file]  Which file"), "{page}");
}

#[test]
fn a_subcommands_page_is_laid_out_by_the_roots_template() {
    // A template belongs to the CLI, not to the command whose page is being written, so one
    // declaration lays out every page. `run` has no arguments of its own, and the gap `{{args}}`
    // would leave closes up rather than pushing the author's line away from the flags.
    let spec = LaidOut::spec();
    let run = spec.root.subcommands[0];
    let page = usage_argv::help::short_help(spec, &["laid-out", "run"], &[spec.root, run]);

    let lib: LibSpec = spec
        .to_kdl()
        .parse()
        .expect("the derive emits a valid spec");
    let lib_run = lib.cmd.subcommands.get("run").expect("run");
    assert_eq!(page, usage::docs::cli::render_help(&lib, lib_run, false));

    assert!(page.contains("--dry-run"), "{page}");
    assert!(!page.contains("Arguments:"), "{page}");
    assert!(
        !page.contains("\n\n\n"),
        "no gap should be left behind:\n{page}"
    );
}

#[test]
fn a_template_survives_the_round_trip_through_kdl() {
    // KDL is the interface to everything downstream — markdown, manpages, the SDK generators —
    // so a template that cannot be written down and read back is a template only the binary that
    // declared it can honour.
    let spec = LaidOut::spec();
    let kdl = spec.to_kdl();
    assert!(kdl.contains("help_template"), "{kdl}");

    let lib: LibSpec = kdl.parse().expect("the derive emits a valid spec");
    assert_eq!(lib.help_template.as_deref(), spec.help_template);

    // And again from the reference's own writer, which is the round trip a spec checked into a
    // repository actually makes.
    let reparsed: LibSpec = lib
        .to_string()
        .parse()
        .expect("the reference writes what it reads");
    assert_eq!(reparsed.help_template, lib.help_template);
    assert_eq!(
        usage::docs::cli::render_help(&reparsed, &reparsed.cmd, false),
        usage::docs::cli::render_help(&lib, &lib.cmd, false)
    );
}

/// A CLI naming every section there is, in an order no default page uses.
///
/// The fixture that holds the vocabularies together. The derive keeps its own copy of the six
/// names — a proc-macro crate cannot depend on the crate its output calls into — so a name
/// missing from that copy refuses this struct at compile time, and one missing from
/// `usage_argv::help` renders here as the braces somebody typed.
#[derive(Cli)]
#[usage(
    bin = "every-section",
    version = "1.2.3",
    about = "An example",
    after_help = "Read the docs.",
    help_template = "{{commands}}\n\n{{args}}\n\n{{flags}}\n\n{{usage}}\n\n{{after_help}}\n\n{{about}}"
)]
struct EverySection {
    /// Do it anyway
    #[usage(long)]
    force: bool,

    /// Which file
    #[usage(arg, name = "file")]
    file: Option<String>,

    #[usage(subcommand)]
    command: Option<LaidOutCommands>,
}

#[test]
fn every_section_the_vocabulary_holds_can_be_placed() {
    let spec = EverySection::spec();
    let page = usage_argv::help::long_help(spec, &["every-section"], &[spec.root]);

    // Nothing was left as a placeholder: a name this renderer does not know would survive
    // substitution as literal braces rather than fail, so the braces are what to look for.
    assert!(!page.contains("{{"), "a section went unfilled:\n{page}");

    // Each of the six put its own content where the template asked for it.
    let at = |needle: &str| {
        page.find(needle)
            .unwrap_or_else(|| panic!("{needle}:\n{page}"))
    };
    let order = [
        at("Commands:"),
        at("Arguments:"),
        at("Flags:"),
        at("Usage:"),
        at("Read the docs."),
        at("every-section 1.2.3"),
    ];
    assert!(
        order.windows(2).all(|pair| pair[0] < pair[1]),
        "the sections are not in the template's order:\n{page}"
    );

    // And the reference agrees, from the KDL this CLI writes.
    let lib: LibSpec = spec
        .to_kdl()
        .parse()
        .expect("the derive emits a valid spec");
    assert_eq!(page, usage::docs::cli::render_help(&lib, &lib.cmd, true));
}

#[test]
fn the_two_rust_vocabularies_are_the_same_six_words() {
    // One list, three Rust copies: this one, usage-argv's, and the derive's. The derive's cannot
    // be reached from here, which is what `every_section_the_vocabulary_holds_can_be_placed` is
    // for; these two can be compared outright.
    assert_eq!(
        usage::help_template::SECTIONS,
        usage_argv::help::SECTIONS,
        "the reference and the compiled renderer name different sections"
    );
}

#[test]
fn a_template_naming_no_section_is_refused_where_the_spec_is_read() {
    // The vocabulary is closed, and a page cannot be assembled from a section nothing renders.
    // KDL is checked at parse; the derive is checked at compile time, which is asserted by
    // `derive/tests/ui` rather than here because a compile failure cannot be caught at run time.
    let err = "bin \"ex\"\nhelp_template \"{{about}}{{options}}\"\n"
        .parse::<LibSpec>()
        .expect_err("no section is called options");
    let usage::error::UsageErr::InvalidInput(message, ..) = err else {
        panic!("a template is refused as invalid input, not as {err:?}")
    };
    assert!(message.contains("options"), "{message}");
    // And says what to write instead, since a ported clap template is what hits this.
    assert!(message.contains("flags"), "{message}");
}

#[test]
fn an_empty_template_is_the_default_page() {
    // Accepted, because it names no unknown section, and stored as unset, because it
    // also names no layout. Rust and Go then assemble the same default page.
    let spec: LibSpec = "bin \"ex\"\nabout \"An example\"\nhelp_template \"\"\n"
        .parse()
        .expect("empty is valid");
    assert_eq!(spec.help_template, None);
    let with = usage::docs::cli::render_help(&spec, &spec.cmd, false);
    let without: LibSpec = "bin \"ex\"\nabout \"An example\"\n".parse().unwrap();
    assert_eq!(
        with,
        usage::docs::cli::render_help(&without, &without.cmd, false)
    );
}

#[test]
fn the_cli_a_template_describes_still_parses() {
    // A page describing something the parser does not do is worse than no page. Reading the
    // fixture also keeps its fields from being dead code, which CI denies.
    use std::ffi::OsStr;

    let parsed = LaidOut::parse_from(&[OsStr::new("--force"), OsStr::new("notes.txt")])
        .expect("a flag and an argument");
    assert!(parsed.force);
    assert_eq!(parsed.file.as_deref(), Some("notes.txt"));

    let sub = LaidOut::parse_from(&[OsStr::new("run"), OsStr::new("--dry-run")])
        .expect("a subcommand and its flag");
    let Some(LaidOutCommands::Run(run)) = sub.command else {
        panic!("expected run")
    };
    assert!(run.dry_run);

    // The all-sections fixture too, whose page the vocabulary test reads.
    let every = EverySection::parse_from(&[OsStr::new("--force"), OsStr::new("notes.txt")])
        .expect("a flag and an argument");
    assert!(every.force);
    assert_eq!(every.file.as_deref(), Some("notes.txt"));
    assert!(every.command.is_none());
}
