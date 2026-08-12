//! What each `usage` command does to the world.
//!
//! clap has no way to express this, so it is applied to the derived spec on the
//! way out. The same shape mise, hk, pitchfork, aube and communique use.
//!
//! The rule is what the command does to *state the user would miss*, not how
//! much work it does. Reading a spec file and printing a manpage is `read` no
//! matter how much parsing happens in between; writing that manpage to a path
//! the user named is `write`, because something on disk changed.
//!
//! Most commands here print to stdout and take an optional flag to write to a
//! file instead, so they are `read` with the flag raising them — which is the
//! composition rule doing its job: the effect of an invocation is the highest
//! of the command's and those of the flags and args actually supplied.
//!
//! A command that runs code the user supplied is left unset rather than
//! guessed at. See [`UNCLASSIFIED`].

use usage::SpecCommandEffect::{self, Read, Write};

/// Commands whose effect is fixed, keyed by their full path under `usage`.
const EFFECTS: &[(&str, SpecCommandEffect)] = &[
    ("complete-word", Read),
    // Cannot run alone (`subcommand_required`), and every child starts at
    // `read`, so the parent is `read` too.
    ("generate", Read),
    ("generate completion", Read),
    ("generate completion-init", Read),
    ("generate fig", Read),
    ("generate json", Read),
    ("generate json-schema", Read),
    ("generate manpage", Read),
    ("generate markdown", Read),
    // The only generator whose output flag is required: it cannot print an SDK
    // to stdout, so every invocation writes a directory.
    ("generate sdk", Write),
    ("lint", Read),
    // Long-running, but every tool it serves only reads the spec it was given.
    // Unlike `mise mcp`, which is unclassified because it serves a tool that
    // runs tasks, nothing here can act on the CLI it describes.
    ("mcp", Read),
    ("sponsors", Read),
];

/// Flags that raise the effect of the command they are passed to, keyed by
/// `<command path>` and the flag's name.
///
/// All of these redirect output that would otherwise go to stdout.
const FLAG_EFFECTS: &[(&str, &str, SpecCommandEffect)] = &[
    ("generate fig", "out-file", Write),
    ("generate json-schema", "out-file", Write),
    ("generate manpage", "out-file", Write),
    ("generate markdown", "out-dir", Write),
    ("generate markdown", "out-file", Write),
];

/// Commands with no fixed effect, and why.
///
/// These run a script the user supplied, so their effect is whatever that
/// script does. Labeling them would be a lie in whichever direction it was
/// labeled, and `read` in particular would be dangerous.
// Only the coverage test reads this; it exists so the reason a command is left
// unclassified lives next to the decision rather than in a commit message.
#[cfg(test)]
const UNCLASSIFIED: &[(&str, &str)] = &[
    ("bash", "runs a user-supplied script"),
    ("exec", "runs a user-supplied script"),
    ("fish", "runs a user-supplied script"),
    ("powershell", "runs a user-supplied script"),
    ("zsh", "runs a user-supplied script"),
];

/// Apply the tables above to a derived spec.
///
/// A path that no longer exists is skipped rather than panicking — the stale
/// entry is caught by the test below, where a failure is readable, instead of
/// at runtime in a user's shell.
pub(crate) fn apply(spec: &mut usage::Spec) {
    for (path, effect) in EFFECTS {
        if let Some(cmd) = find_mut(spec, path) {
            cmd.effect = Some(*effect);
        }
    }
    for (path, flag_name, effect) in FLAG_EFFECTS {
        if let Some(cmd) = find_mut(spec, path) {
            if let Some(flag) = cmd.flags.iter_mut().find(|f| f.name == *flag_name) {
                flag.effect = Some(*effect);
            }
        }
    }
}

fn find_mut<'a>(spec: &'a mut usage::Spec, path: &str) -> Option<&'a mut usage::SpecCommand> {
    let mut cmd = &mut spec.cmd;
    for segment in path.split(' ') {
        cmd = cmd.subcommands.get_mut(segment)?;
    }
    Some(cmd)
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::collections::HashSet;

    fn spec() -> usage::Spec {
        let mut cli = <crate::cli::Cli as clap::CommandFactory>::command();
        let mut spec = clap_usage::spec(&mut cli, "usage");
        apply(&mut spec);
        spec
    }

    /// Every command path in the spec, deepest-first order irrelevant.
    fn walk(cmd: &usage::SpecCommand, path: &mut Vec<String>, out: &mut Vec<(String, bool)>) {
        for (name, sub) in &cmd.subcommands {
            path.push(name.clone());
            out.push((path.join(" "), sub.effect.is_some()));
            walk(sub, path, out);
            path.pop();
        }
    }

    fn commands() -> Vec<(String, bool)> {
        let spec = spec();
        let mut out = vec![];
        walk(&spec.cmd, &mut vec![], &mut out);
        out
    }

    #[test]
    fn no_entry_points_at_a_command_that_does_not_exist() {
        // `apply` skips a stale path silently so a rename cannot break the CLI.
        // This is where that shows up instead.
        let real: HashSet<_> = commands().into_iter().map(|(path, _)| path).collect();
        let stale: Vec<_> = EFFECTS
            .iter()
            .map(|(path, _)| *path)
            .chain(FLAG_EFFECTS.iter().map(|(path, _, _)| *path))
            .chain(UNCLASSIFIED.iter().map(|(path, _)| *path))
            .filter(|path| !real.contains(*path))
            .collect();
        assert!(stale.is_empty(), "no such commands: {stale:?}");
    }

    #[test]
    fn no_flag_entry_points_at_a_flag_that_does_not_exist() {
        let spec = spec();
        let missing: Vec<_> = FLAG_EFFECTS
            .iter()
            .filter(|(path, flag_name, _)| {
                find(&spec, path).is_none_or(|cmd| !cmd.flags.iter().any(|f| f.name == *flag_name))
            })
            .map(|(path, flag_name, _)| format!("{path} --{flag_name}"))
            .collect();
        assert!(missing.is_empty(), "no such flags: {missing:?}");
    }

    #[test]
    fn nothing_is_unclassified_by_accident() {
        // An unset effect means "unknown, ask", which is right for the shell
        // commands and a silent gap for anything else.
        let deliberate: HashSet<_> = UNCLASSIFIED.iter().map(|(path, _)| *path).collect();
        let accidental: Vec<_> = commands()
            .into_iter()
            .filter(|(path, classified)| !classified && !deliberate.contains(path.as_str()))
            .map(|(path, _)| path)
            .collect();
        assert!(
            accidental.is_empty(),
            "unclassified with no entry in UNCLASSIFIED: {accidental:?}"
        );
    }

    #[test]
    fn an_output_flag_raises_a_read_command_to_write() {
        // The composition rule is the reason flags carry effects at all:
        // `usage g markdown -f x.kdl` only reads, the same command with
        // `--out-file` writes.
        let spec = spec();
        let md = find(&spec, "generate markdown").unwrap();
        assert_eq!(md.effect, Some(Read));
        let out_file = md.flags.iter().find(|f| f.name == "out-file").unwrap();
        assert_eq!(out_file.effect, Some(Write));
        // A flag that changes only the rendering stays unset.
        let html = md.flags.iter().find(|f| f.name == "html-encode").unwrap();
        assert_eq!(html.effect, None);
    }

    #[test]
    fn a_required_output_flag_makes_the_command_write() {
        // `generate sdk` cannot print to stdout, so there is no read-only way
        // to invoke it and the effect belongs on the command.
        let spec = spec();
        let sdk = find(&spec, "generate sdk").unwrap();
        assert_eq!(sdk.effect, Some(Write));
        assert!(sdk.flags.iter().any(|f| f.name == "output" && f.required));
    }

    fn find<'a>(spec: &'a usage::Spec, path: &str) -> Option<&'a usage::SpecCommand> {
        let mut cmd = &spec.cmd;
        for segment in path.split(' ') {
            cmd = cmd.subcommands.get(segment)?;
        }
        Some(cmd)
    }
}
