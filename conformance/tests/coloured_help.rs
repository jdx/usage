//! Coloured help, pinned byte for byte on the pages the plain corpus cannot speak for.
//!
//! The rendering corpus compares plain pages with the reference; colour is this crate's alone,
//! and is written by the section writers as they write the structure — headings, the synopsis,
//! the usage or command name that starts each row. These fixtures reach the places that has to
//! cover and a CLI rarely combines: global flags, a flattened page, next-line help, a logo in
//! the margin, a help template that repeats and reorders sections, and author prose that happens
//! to read like a heading or a row, which stays prose.

use usage_argv::help::{render_styled, Style};
use usage_argv::spec::CommandMeta;

/// Everything but the template: globals, groups, a flattened child, next-line help, a logo.
const TOOL: &str = r#"
name "tool"
bin "tool"
version "1.2.3"
about "A **tool** with `code` and _emphasis_"
before_help "Flags:\n  -f, --force  mentioned in prose\n  build  Build it"
after_help "Examples:\n  -f, --force  again\nArguments:\n$ not styled *x*"
logo "  /\\\n /  \\\n/____\\" style="cyan"
author "Someone"
license "MIT"
flag "-f --force" help="Do it *anyway*" global=#true
flag "--config <path>" help="Config file" env="TOOL_CONFIG" default="a.toml" global=#true
flag "--mode <mode>" help="Mode" help_heading="Options" {
  choices "fast" "slow_mode"
}
flag "--old" deprecated="use --new"
arg "[file]" help="The __file__ to use" env="TOOL_FILE"
arg "[rest]..." help="More" help_heading="Extra"
cmd "build" help="Build it" help_heading="Main" {
  flag "--release" help="Optimised ~~build~~"
  arg "<target>" help="What"
}
cmd "flat" help="Flattened" flatten_help=#true {
  cmd "one" help="First" {
    flag "--alpha <a>" help="Alpha"
    arg "<x>" help="X"
  }
  cmd "two" help="Second" next_line_help=#true {
    flag "-b --beta" help="Beta"
  }
}
cmd "nl" help="Next line" next_line_help=#true {
  flag "--gamma <g>" help="Gamma" env="G" default="1"
  arg "[y]" help="Y"
}
"#;

/// A template that places every section, puts the grouped flags and the commands a second time
/// after the author's prose, and wraps it all in styled text of its own.
const LAID: &str = r#"
name "laid"
bin "laid"
version "0.1.0"
help_template "{$bold}Top{/$}\n{{about}}\n\n{{usage}}\n\n{{ungrouped_flags}}\n\n{{grouped_flags}}\n\n{{grouped_args}}\n\n{{ungrouped_args}}\n\n{{commands}}\n\n{{after_help}}\n\n{{grouped_flags}}\n\n{{commands}}\n{$heading}End{/$}"
after_help "Flags:\n  --force  prose"
flag "--force" help="Force *it*" global=#true
flag "--name <n>" help="Name" help_heading="Naming"
arg "<in>" help="Input"
arg "[out]" help="Output" help_heading="Outputs"
cmd "go" help="Go" {
  flag "--fast"
}
cmd "flatp" flatten_help=#true help="Flat" {
  cmd "x" help="X" {
    flag "--xx"
  }
}
"#;

fn built(kdl: &str) -> &'static usage_argv::spec::Spec<'static> {
    let mut spec: usage::Spec = kdl.parse().expect("the fixture parses");
    // A fixed width, so the page (and whether the logo fits beside it) does not depend on the
    // terminal the tests run in.
    fn pin(cmd: &mut usage::SpecCommand) {
        cmd.term_width = Some(80);
        for sub in cmd.subcommands.values_mut() {
            pin(sub);
        }
    }
    pin(&mut spec.cmd);
    usage_conformance::tables::build_spec(&spec)
}

fn find<'a>(meta: &'a CommandMeta<'a>, path: &[&str]) -> &'a CommandMeta<'a> {
    path.iter().fold(meta, |meta, name| {
        meta.subcommands
            .iter()
            .find(|sub| sub.cmd.name == *name)
            .expect("the fixture has the command")
    })
}

/// The page, with its escapes spelled out so a snapshot can be read.
fn page(kdl: &str, path: &[&str], long: bool) -> String {
    let spec = built(kdl);
    let meta = find(spec.root, path);
    render_styled(spec, meta.cmd, long, Style::COLOURED)
        .expect("the command is in its spec")
        .replace('\u{1b}', "␛")
}

#[test]
fn root_pages() {
    insta::assert_snapshot!("tool_short", page(TOOL, &[], false));
    insta::assert_snapshot!("tool_long", page(TOOL, &[], true));
}

#[test]
fn a_flattened_page_with_global_flags() {
    insta::assert_snapshot!("flat_short", page(TOOL, &["flat"], false));
    insta::assert_snapshot!("flat_long", page(TOOL, &["flat"], true));
}

#[test]
fn next_line_help() {
    insta::assert_snapshot!("nl_short", page(TOOL, &["nl"], false));
    insta::assert_snapshot!("nl_long", page(TOOL, &["nl"], true));
}

#[test]
fn a_template_page() {
    insta::assert_snapshot!("laid_short", page(LAID, &[], false));
    insta::assert_snapshot!("laid_flat_long", page(LAID, &["flatp"], true));
}

/// Author text that reads like structure is not coloured as structure.
///
/// Colour used to be laid over a finished page by matching its lines against the headings and
/// usages it contained, so a `Flags:` line in `after_help` came out as a heading, and section
/// prose starting with a flag's usage came out as that flag's row. Only what the renderer
/// itself writes is structure now; the author's text keeps its emphasis and nothing more.
#[test]
fn author_text_that_looks_like_structure_stays_prose() {
    const LOOKALIKE: &str = r#"
name "ex"
bin "ex"
heading "Options" help="-f, --force overrides the *lock*"
after_help "Flags:\n  -f, --force  mentioned, not listed"
flag "-f --force" help="Do it anyway"
flag "--mode <mode>" help="Mode" help_heading="Options"
"#;
    let page = page(LOOKALIKE, &[], true);
    let lines: Vec<&str> = page.lines().collect();

    // The real headings and rows are coloured.
    assert!(lines.contains(&"␛[1;33mFlags:␛[0m"), "{page}");
    assert!(lines.contains(&"␛[1;33mOptions:␛[0m"), "{page}");
    assert!(
        lines.contains(&"  ␛[1;32m-f␛[0m, ␛[1;32m--force␛[0m        Do it anyway"),
        "{page}"
    );
    assert!(
        lines.contains(&"      ␛[1;32m--mode␛[0m ␛[1;35m<mode>␛[0m  Mode"),
        "{page}"
    );

    // The author's look-alikes are not, though their emphasis still is.
    assert!(
        lines.contains(&"  -f, --force overrides the ␛[3mlock␛[23m"),
        "{page}"
    );
    assert!(
        lines.contains(&"-f, --force mentioned, not listed"),
        "{page}"
    );

    // Each `Flags:` once, and in its own place: the coloured one is the section heading ahead
    // of the `Options` section, the plain one is the author's, in `after_help` at the end. Only
    // checking that both forms exist would pass with the two swapped.
    let at = |line: &str| {
        let found: Vec<usize> = (0..lines.len()).filter(|&i| lines[i] == line).collect();
        assert_eq!(found.len(), 1, "{line:?} once in:\n{page}");
        found[0]
    };
    let heading = at("␛[1;33mFlags:␛[0m");
    let options = at("␛[1;33mOptions:␛[0m");
    let authored = at("Flags:");
    assert!(heading < options && options < authored, "{page}");
    assert_eq!(
        lines[authored + 1],
        "-f, --force mentioned, not listed",
        "{page}"
    );
}
