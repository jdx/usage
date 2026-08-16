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
