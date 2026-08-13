//! Does the rendered usage line match usage-lib's, at mise's scale?
//!
//! usage-lib builds this from a spec, through a tera template over a runtime model.
//! usage-argv has no spec at run time — only `&'static` tables — so the rules are
//! reimplemented there, and reimplemented rules drift. The check is to run both over mise's
//! real spec and compare all 211 lines, because an adopter's help text changing is a visible
//! regression even when the change is one bracket.
//!
//! The shadow is generated from the same `mise.usage.kdl` that usage-lib is given here, so
//! the two sides describe the same CLI by construction rather than by a fixture kept in step
//! by hand.

use usage::{Spec as LibSpec, SpecCommand};
use usage_argv::help::{long_help, short_help, usage_line};
use usage_argv::spec::CommandMeta;

/// mise's committed spec, which the shadow was generated from.
fn mise_spec() -> LibSpec {
    let kdl = include_str!("../../mise.usage.kdl");
    kdl.parse().expect("mise's spec should parse")
}

/// Every command in the tree, as (path, metadata) — the path being what a user types.
fn walk<'a>(
    path: Vec<&'a str>,
    meta: &'a CommandMeta<'a>,
    out: &mut Vec<(Vec<&'a str>, &'a CommandMeta<'a>)>,
) {
    out.push((path.clone(), meta));
    for sub in meta.subcommands {
        // Aliases live in the parse table beside the canonical name; help names the command.
        let mut child = path.clone();
        child.push(sub.cmd.name);
        walk(child, sub, out);
    }
}

/// usage-lib's line for the same command, found by following the same path.
fn lib_command<'a>(spec: &'a LibSpec, path: &[&str]) -> Option<&'a SpecCommand> {
    let mut cmd = &spec.cmd;
    for name in path {
        cmd = cmd.subcommands.get(*name)?;
    }
    Some(cmd)
}

#[test]
fn every_usage_line_matches_the_reference() {
    let spec = mise_spec();
    let root = shadow_mise::Cli::spec().root;

    let mut commands = Vec::new();
    walk(vec!["mise"], root, &mut commands);
    assert!(
        commands.len() > 200,
        "the shadow should cover mise's whole tree, found {}",
        commands.len()
    );

    let mut differences = Vec::new();
    for (path, meta) in &commands {
        let ours = usage_line(path, meta);
        // usage-lib's `usage()` omits the binary and starts at the command path, so the
        // comparison puts it back — the same string the template writes after `Usage: `.
        let theirs = match lib_command(&spec, &path[1..]) {
            Some(cmd) => format!("mise {}", cmd.usage()).trim().to_string(),
            None => {
                differences.push(format!("{}: not found in the spec at all", path.join(" ")));
                continue;
            }
        };
        if ours != theirs {
            differences.push(format!(
                "{}\n     ours: {ours}\n      lib: {theirs}",
                path.join(" ")
            ));
        }
    }

    assert!(
        differences.is_empty(),
        "{} of {} usage lines differ from usage-lib:\n  - {}",
        differences.len(),
        commands.len(),
        differences.join("\n  - ")
    );
}

#[test]
fn the_root_line_is_what_a_user_would_recognise() {
    // One case spelled out, so a reader can see what the parity test is asserting 211 times.
    let root = shadow_mise::Cli::spec().root;
    let line = usage_line(&["mise"], root);
    assert!(
        line.starts_with("mise "),
        "the line should start with the binary: {line}"
    );
    assert!(
        line.ends_with("<SUBCOMMAND>"),
        "mise has subcommands, so the line should end by saying so: {line}"
    );
}

/// The first line that differs, with a little context — a whole help page twice over is not
/// something anyone reads.
fn first_diff(ours: &str, theirs: &str) -> String {
    let mine: Vec<&str> = ours.lines().collect();
    let ref_: Vec<&str> = theirs.lines().collect();
    for (i, (a, b)) in mine.iter().zip(ref_.iter()).enumerate() {
        if a != b {
            return format!("  line {}:\n    ours: {a:?}\n     lib: {b:?}", i + 1);
        }
    }
    format!(
        "  same for {} lines, then ours has {} and the reference {}",
        mine.len().min(ref_.len()),
        mine.len(),
        ref_.len()
    )
}

#[test]
fn every_short_help_matches_the_reference() {
    // The same standard as the usage line, over the whole document: `-h` for all 211 of
    // mise's commands, byte for byte against usage-lib's. This is the test that decides
    // whether an adopter's help output changes, so it compares the text rather than a
    // summary of it.
    let spec = mise_spec();
    let root = shadow_mise::Cli::spec();

    let mut commands = Vec::new();
    walk(vec!["mise"], root.root, &mut commands);

    let mut differences = Vec::new();
    for (path, meta) in &commands {
        let ours = short_help(root, path, meta);
        let Some(cmd) = lib_command(&spec, &path[1..]) else {
            differences.push(format!("{}: not in the spec", path.join(" ")));
            continue;
        };
        let theirs = usage::docs::cli::render_help(&spec, cmd, false);
        if ours != theirs {
            differences.push(format!(
                "{}\n{}",
                path.join(" "),
                first_diff(&ours, &theirs)
            ));
        }
    }

    assert!(
        differences.is_empty(),
        "{} of {} help pages differ:\n{}",
        differences.len(),
        commands.len(),
        // Two is enough to work from, and the whole set would bury the count.
        differences
            .iter()
            .take(2)
            .cloned()
            .collect::<Vec<_>>()
            .join("\n")
    );
}

#[test]
fn every_long_help_matches_the_reference() {
    // `--help`: the same content through the wider layout — help aligned into a column and
    // wrapped, long descriptions preferred, annotations on their own lines. Both sides read
    // `COLUMNS` the same way and fall back to the same 80, so they agree about where a line
    // ends whatever the environment says.
    let spec = mise_spec();
    let root = shadow_mise::Cli::spec();

    let mut commands = Vec::new();
    walk(vec!["mise"], root.root, &mut commands);

    let mut differences = Vec::new();
    for (path, meta) in &commands {
        let ours = long_help(root, path, meta);
        let Some(cmd) = lib_command(&spec, &path[1..]) else {
            continue;
        };
        let theirs = usage::docs::cli::render_help(&spec, cmd, true);
        if ours != theirs {
            differences.push(format!(
                "{}\n{}",
                path.join(" "),
                first_diff(&ours, &theirs)
            ));
        }
    }

    assert!(
        differences.is_empty(),
        "{} of {} long help pages differ:\n{}",
        differences.len(),
        commands.len(),
        differences
            .iter()
            .take(2)
            .cloned()
            .collect::<Vec<_>>()
            .join("\n")
    );
}

/// A command declaring everything mise's spec does not: an example with a description, and the
/// text that goes above and below the page.
///
/// mise carries its Examples in `after_long_help` and declares no `example` nodes at all, so the
/// 211-page comparison never reaches this code. Three bugs hid there — a missing preamble in both
/// forms and an example's description printed after its command — which is what a fixture built
/// from one real CLI cannot catch on its own.
fn surrounded() -> LibSpec {
    "name \"ex\"\nbin \"ex\"\ncmd go help=\"Go somewhere\" {\n  \
     before_help \"Read this first.\"\n  \
     before_long_help \"Read this first, at length.\"\n  \
     after_help \"And this after.\"\n  \
     after_long_help \"And this after, at length.\"\n  \
     example \"ex go --fast\" help=\"the quick way\"\n}\n"
        .parse()
        .expect("valid spec")
}

#[test]
fn the_text_around_a_page_is_rendered_where_the_reference_puts_it() {
    let spec = surrounded();
    let go = spec.cmd.subcommands.get("go").expect("go");

    // Built by hand rather than derived: the point is to compare the renderer against the
    // reference for a shape the shadow does not have.
    static GO: usage_argv::Command = usage_argv::Command {
        name: "go",
        ..usage_argv::Command::EMPTY
    };
    static GO_META: CommandMeta = CommandMeta {
        cmd: &GO,
        about: Some("Go somewhere"),
        before_help: Some("Read this first."),
        before_long_help: Some("Read this first, at length."),
        after_help: Some("And this after."),
        after_long_help: Some("And this after, at length."),
        examples: &[usage_argv::spec::Example {
            code: "ex go --fast",
            header: None,
            help: Some("the quick way"),
        }],
        ..CommandMeta::EMPTY
    };
    static SPEC: usage_argv::spec::Spec = usage_argv::spec::Spec {
        name: "ex",
        bin: Some("ex"),
        version: None,
        about: None,
        long_about: None,
        default_subcommand: None,
        root: &GO_META,
    };

    assert_eq!(
        short_help(&SPEC, &["ex", "go"], &GO_META),
        usage::docs::cli::render_help(&spec, go, false),
        "short form"
    );
    assert_eq!(
        long_help(&SPEC, &["ex", "go"], &GO_META),
        usage::docs::cli::render_help(&spec, go, true),
        "long form"
    );
}
