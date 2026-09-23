//! Coloured help, pinned byte for byte on the pages the plain corpus cannot speak for.
//!
//! The rendering corpus compares plain pages with the reference; colour is this crate's alone,
//! and is laid over a page by recognising what the page wrote — its headings, the usage that
//! starts each row, the synopsis. These fixtures reach the places that recognition has to cover
//! and a CLI rarely combines: global flags, a flattened page, next-line help, a logo in the
//! margin, a help template that repeats and reorders sections, and author prose that happens to
//! read like a heading or a row. The last is pinned as it is today rather than as it ideally
//! would be: a change to it should be a decision, not a side effect.

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
