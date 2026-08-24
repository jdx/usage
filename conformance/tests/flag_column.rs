//! Where a flag's name sits in the flags section.
//!
//! Measured from clap 4 rather than remembered. This is the whole rule, in five lines:
//!
//! ```text
//! Options:
//!   -j <JOBS>
//!       --github-release
//!   -n, --dry-run
//!   -o, --output <OUTPUT>
//!   -h, --help             Print help
//! ```
//!
//! A four-character column holds `-x, ` or the blank standing in for it — but *only* where
//! there is a long form to line up with. `-j <JOBS>` has none, so it does not pad; that is
//! clap's behaviour and not an oversight in it.

use usage::docs::markdown::MarkdownRenderer;
use usage_argv::help;
use usage_derive::Cli;

/// A tool with one of each shape
#[derive(Cli)]
#[usage(bin = "ex")]
struct Ex {
    /// Short only, so nothing to align with
    #[usage(short = 'j')]
    jobs: Option<String>,
    /// Long only
    #[usage(long)]
    github_release: bool,
    /// Both
    #[usage(long, short = 'n')]
    dry_run: bool,
    /// Both, and takes a value
    #[usage(long, short)]
    output: Option<String>,
    /// Its description is long enough to need wrapping at any sane width
    #[usage(long)]
    describe: bool,
}

#[derive(Cli)]
#[usage(bin = "repeat")]
#[allow(dead_code)]
struct Repeatable {
    /// A repeatable value
    #[usage(short = 'A', long, value_name = "NAME")]
    allow: Vec<String>,
    /// Repeatable verbosity
    #[usage(long, count)]
    verbose: u8,
}

#[test]
fn repeatable_flags_have_ordinary_spellings_in_every_rendered_format() {
    for long in [false, true] {
        let compiled =
            help::render(Repeatable::spec(), Repeatable::spec().root.cmd, long).expect("a page");
        assert!(
            compiled.contains("-A, --allow <NAME>"),
            "long={long}: {compiled}"
        );
        assert!(compiled.contains("--verbose"), "long={long}: {compiled}");
        assert!(!compiled.contains('…'), "long={long}: {compiled}");

        let spec: usage::Spec = Repeatable::to_kdl().parse().expect("generated spec");
        let allow = spec
            .cmd
            .flags
            .iter()
            .find(|flag| flag.name == "allow")
            .expect("generated spec should contain allow");
        assert!(allow.var, "allow must remain repeatable in generated spec");
        let reference = usage::docs::cli::render_help(&spec, &spec.cmd, long);
        assert_eq!(reference, compiled);

        let markdown = MarkdownRenderer::new(spec.clone())
            .render_cmd(&spec.cmd)
            .expect("markdown page");
        assert!(markdown.contains("-A --allow <NAME>"), "{markdown}");
        assert!(!markdown.contains('…'), "{markdown}");
    }
}

fn page(long: bool) -> String {
    help::render(Ex::spec(), Ex::spec().root.cmd, long).expect("a page")
}

#[test]
fn a_long_form_starts_in_the_same_column_whatever_precedes_it() {
    for long in [false, true] {
        let page = page(long);
        for line in [
            "      --github-release",
            "  -n, --dry-run",
            "  -o, --output <OUTPUT>",
        ] {
            assert!(
                page.lines().any(|l| l.starts_with(line)),
                "long={long}: no line starts `{line}`:\n{page}"
            );
        }
        // A comma, which is the near-universal convention and what clap prints.
        assert!(!page.contains("-n --dry-run"), "long={long}: {page}");
    }
}

#[test]
fn a_flag_with_no_long_form_does_not_pay_for_the_column() {
    // clap writes `-j <JOBS>` at the indent, not padded out to where the long forms begin:
    // there is nothing to line it up with, and the padding would only push it away from its
    // own description.
    for long in [false, true] {
        let page = page(long);
        assert!(
            page.lines().any(|l| l.starts_with("  -j <JOBS>")),
            "long={long}: {page}"
        );
    }
}

#[test]
fn the_short_page_lines_its_descriptions_up_too() {
    // It did not. Every description began directly after the name it belonged to, so nothing
    // in `-h` lined up with anything — and `-h` is the form most people type. One column per
    // section, which is the rule the long page already followed.
    let page = page(false);
    // Where each description begins, for three flags whose names are three different lengths.
    // If the column is real they are equal; before this they were 2 + the length of the name.
    let column_of = |needle: &str, help: &str| -> usize {
        let line = page
            .lines()
            .find(|l| l.contains(needle))
            .unwrap_or_else(|| panic!("no line for {needle}:\n{page}"));
        line.find(help)
            .unwrap_or_else(|| panic!("no help on {line:?}"))
    };
    let a = column_of("--github-release", "Long only");
    let b = column_of("--dry-run", "Both");
    let c = column_of("--describe", "Its description");
    assert!(
        a == b && b == c,
        "descriptions should start in one column, got {a}, {b}, {c}:\n{page}"
    );
}

#[test]
fn the_usage_line_is_not_padded() {
    // `column_usage` is separate from the usage line's own rendering for this reason: a
    // `Usage: ex [    --describe]` would be absurd.
    let page = page(true);
    let usage = page
        .lines()
        .find(|l| l.starts_with("Usage:"))
        .expect("a usage line");
    assert!(!usage.contains("  --"), "padded usage line: {usage}");
}

#[test]
fn the_fields_are_bound() {
    use std::ffi::OsStr;
    let argv = [
        "-j",
        "4",
        "--github-release",
        "-n",
        "--output",
        "o",
        "--describe",
    ]
    .map(OsStr::new);
    let ex = Ex::parse_from(&argv).expect("should parse");
    assert_eq!(ex.jobs.as_deref(), Some("4"));
    assert!(ex.github_release && ex.dry_run && ex.describe);
    assert_eq!(ex.output.as_deref(), Some("o"));
}

/// A flag whose declared name the forms do not imply, and one whose help says nothing
#[derive(Cli)]
#[usage(bin = "odd")]
struct Odd {
    /// How many at once
    #[usage(name = "jobs", long = "parallel", short = 'j')]
    parallel: Option<String>,
    /// A description made only of spaces is no description
    #[usage(long, help = "   ")]
    blank: bool,
}

#[test]
fn a_declared_name_is_not_mistaken_for_a_short_form() {
    // `jobs: -j --parallel` — the prefix is the flag's *name*, not something to line a comma up
    // after. Gluing one on lost the space entirely and rendered `jobs: -j,--parallel`, because
    // the joined string is already wider than the column it was being padded to.
    let page = usage_argv::help::render(Odd::spec(), Odd::spec().root.cmd, false).expect("a page");
    assert!(page.contains("jobs: -j --parallel"), "{page}");
    assert!(!page.contains("-j,--parallel"), "{page}");
}

#[test]
fn a_description_of_only_spaces_is_no_description() {
    // Filtered wherever a description is read, so it does not buy a column of padding and a
    // line of trailing spaces. usage-lib normalises it in the docs model for the same reason —
    // one blank spec, two renderings, was a parity break waiting to be found.
    let page = usage_argv::help::render(Odd::spec(), Odd::spec().root.cmd, false).expect("a page");
    let line = page
        .lines()
        .find(|l| l.contains("--blank"))
        .unwrap_or_else(|| panic!("{page}"));
    assert_eq!(line, line.trim_end(), "trailing space on {line:?}");
}

#[test]
fn the_odd_fields_are_bound() {
    use std::ffi::OsStr;
    let argv = ["-j", "4", "--blank"].map(OsStr::new);
    let odd = Odd::parse_from(&argv).expect("should parse");
    assert_eq!(odd.parallel.as_deref(), Some("4"));
    assert!(odd.blank);
}
