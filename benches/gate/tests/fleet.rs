//! Does every jdx CLI's help match usage-lib's, not just mise's?
//!
//! `help.rs` beside this asks the same question of mise alone, at length and in detail. This
//! asks it of the fleet, and exists because one CLI cannot ask it properly: a gate reading a
//! single spec is blind to everything that spec's vocabulary happens not to use, and mise's
//! misses more than it looks like it would.
//!
//! Three bugs were living in that blind spot when this was written, and none of them is exotic:
//!
//! | | what mise does | what hid the bug |
//! |---|---|---|
//! | version banner | declares no `version` | nothing to print, so both sides agreed |
//! | optional flag value | has none | `<X>` vs `[X]` never came up |
//! | description ending in a break | none in `Commands:` | the stray blank never appeared |
//!
//! Between them they changed the front page of five of the seven CLIs. So the fixture is the
//! fleet, and adding a CLI here is how a new adopter's shape gets covered.
//!
//! See `benches/fleet/README.md` for where the specs come from and how to refresh one.

use std::collections::BTreeSet;

use usage::{Spec as LibSpec, SpecCommand};
use usage_argv::help::{long_help, short_help};
use usage_argv::spec::{CommandMeta, Spec};

/// Every command in a tree, as the path a user types and the chain that leads to it.
///
/// The chain and not just the command: a page lists what it inherits, which only the ancestors
/// know.
fn walk<'a>(
    path: Vec<&'a str>,
    chain: Vec<&'a CommandMeta<'a>>,
    meta: &'a CommandMeta<'a>,
    out: &mut Vec<(Vec<&'a str>, Vec<&'a CommandMeta<'a>>)>,
) {
    let mut chain = chain;
    chain.push(meta);
    out.push((path.clone(), chain.clone()));
    for sub in meta.subcommands {
        let mut child = path.clone();
        child.push(sub.cmd.name);
        walk(child, chain.clone(), sub, out);
    }
}

fn lib_command<'a>(spec: &'a LibSpec, path: &[&str]) -> Option<&'a SpecCommand> {
    let mut cmd = &spec.cmd;
    for name in path {
        cmd = cmd.subcommands.get(*name)?;
    }
    Some(cmd)
}

fn walk_lib(path: Vec<String>, cmd: &SpecCommand, out: &mut BTreeSet<String>) {
    out.insert(path.join(" "));
    for (name, sub) in &cmd.subcommands {
        let mut child = path.clone();
        child.push(name.clone());
        walk_lib(child, sub, out);
    }
}

/// The first line where two pages part company, with a little of each side.
fn first_diff(ours: &str, theirs: &str) -> String {
    let (o, t): (Vec<&str>, Vec<&str>) = (ours.lines().collect(), theirs.lines().collect());
    let at = o
        .iter()
        .zip(t.iter())
        .position(|(a, b)| a != b)
        .unwrap_or(o.len().min(t.len()));
    format!(
        "    line {}\n      argv: {:?}\n      lib : {:?}",
        at + 1,
        o.get(at),
        t.get(at)
    )
}

/// One CLI: every command, both help forms, compared against the reference.
fn agrees(name: &str, bin: &str, kdl: &str, root: &'static Spec<'static>) -> Vec<String> {
    let spec: LibSpec = kdl
        .parse()
        .unwrap_or_else(|e| panic!("{name}'s spec should parse: {e}"));

    let mut commands = Vec::new();
    walk(vec![bin], Vec::new(), root.root, &mut commands);
    assert!(
        !commands.is_empty(),
        "{name}: the shadow has no commands, so this would pass by measuring nothing"
    );

    let mut differences = Vec::new();
    let shadow_paths: BTreeSet<String> = commands.iter().map(|(path, _)| path.join(" ")).collect();
    let mut lib_paths = BTreeSet::new();
    walk_lib(vec![bin.to_string()], &spec.cmd, &mut lib_paths);
    for path in lib_paths.difference(&shadow_paths) {
        differences.push(format!("  {path}: missing from the shadow"));
    }
    for path in shadow_paths.difference(&lib_paths) {
        differences.push(format!("  {path}: not in the spec"));
    }

    for (path, chain) in &commands {
        let Some(cmd) = lib_command(&spec, &path[1..]) else {
            continue;
        };
        // Both forms: `-h` and `--help` share a column calculation and diverge in layout, and a
        // bug has sat in one while the other agreed.
        for long in [false, true] {
            let ours = if long {
                long_help(root, path, chain)
            } else {
                short_help(root, path, chain)
            };
            let theirs = usage::docs::cli::render_help(&spec, cmd, long);
            if ours != theirs {
                differences.push(format!(
                    "  {} ({})\n{}",
                    path.join(" "),
                    if long { "--help" } else { "-h" },
                    first_diff(&ours, &theirs)
                ));
            }
        }
    }
    differences
}

/// The fleet, one entry per CLI.
///
/// A macro rather than a table, because each arm names a different crate: the shadows are
/// separate crates so that a spec's own `Cli` type is a real `&'static` table rather than
/// something built at run time, which is the thing being measured.
macro_rules! fleet {
    ($($name:literal => $shadow:ident from $spec:literal),* $(,)?) => {
        #[test]
        fn every_jdx_cli_matches_the_reference() {
            let mut report: Vec<String> = Vec::new();
            let mut counted = 0usize;
            $(
                let differences = agrees(
                    $name,
                    $name,
                    include_str!($spec),
                    $shadow::Cli::spec(),
                );
                counted += 1;
                if !differences.is_empty() {
                    report.push(format!(
                        "{} — {} page(s) differ:\n{}",
                        $name,
                        differences.len(),
                        // Two is enough to work from; the whole set buries the count.
                        differences.iter().take(2).cloned().collect::<Vec<_>>().join("\n"),
                    ));
                }
            )*
            assert!(counted >= 7, "the fleet should have every CLI in it, got {counted}");
            assert!(report.is_empty(), "{}", report.join("\n\n"));
        }
    };
}

fleet! {
    "mise" => shadow_mise from "../../mise.usage.kdl",
    "hk" => shadow_hk from "../../fleet/hk.usage.kdl",
    "fnox" => shadow_fnox from "../../fleet/fnox.usage.kdl",
    "pitchfork" => shadow_pitchfork from "../../fleet/pitchfork.usage.kdl",
    "aube" => shadow_aube from "../../fleet/aube.usage.kdl",
    "tak" => shadow_tak from "../../fleet/tak.usage.kdl",
    "communique" => shadow_communique from "../../fleet/communique.usage.kdl",
}
