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
use usage_argv::help::usage_line;
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
