//! What each `usage` command does to the world.
//!
//! Declared where the command is, as `#[usage(effect = "…")]`. It used to be a table applied
//! to the spec on the way out, because clap had no way to express it; the derive does, so the
//! table is gone and what is left here is the coverage that kept it honest.
//!
//! The rule is what the command does to *state the user would miss*, not how much work it
//! does. Reading a spec file and printing a manpage is `read` no matter how much parsing
//! happens in between; writing that manpage to a path the user named is `write`, because
//! something on disk changed.
//!
//! Most commands print to stdout and take an optional flag to write to a file instead, so
//! they are `read` with the flag raising them — which is the composition rule doing its job:
//! the effect of an invocation is the highest of the command's and those of the flags and
//! args actually supplied.
//!
//! A command that runs code the user supplied is left unset rather than guessed at. See
//! [`UNCLASSIFIED`].

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

#[cfg(test)]
mod tests {
    use super::*;
    use std::collections::HashSet;
    use usage_rs::spec::{CommandMeta, Effect};

    use crate::cli::Cli;

    /// The static metadata the derive compiled, which is what `--usage-spec` prints.
    fn root() -> &'static CommandMeta<'static> {
        Cli::spec().root
    }

    /// Every command path under the root, with whether it declares an effect.
    fn walk(cmd: &CommandMeta, path: &mut Vec<String>, out: &mut Vec<(String, bool)>) {
        for sub in cmd.subcommands {
            path.push(sub.cmd.name.to_string());
            out.push((path.join(" "), sub.effect.is_some()));
            walk(sub, path, out);
            path.pop();
        }
    }

    fn commands() -> Vec<(String, bool)> {
        let mut out = vec![];
        walk(root(), &mut vec![], &mut out);
        out
    }

    fn find(path: &str) -> Option<&'static CommandMeta<'static>> {
        let mut cmd = root();
        for segment in path.split(' ') {
            cmd = cmd.subcommands.iter().find(|s| s.cmd.name == segment)?;
        }
        Some(cmd)
    }

    #[test]
    fn nothing_names_a_command_that_does_not_exist() {
        // The effects themselves can no longer go stale — a `#[usage(effect)]` on a command
        // that was renamed moves with it, and one on a command that was deleted is deleted
        // too. `UNCLASSIFIED` is a list of names again, so this is where it reports itself.
        let real: HashSet<_> = commands().into_iter().map(|(path, _)| path).collect();
        let stale: Vec<_> = UNCLASSIFIED
            .iter()
            .map(|(path, _)| *path)
            .filter(|path| !real.contains(*path))
            .collect();
        assert!(stale.is_empty(), "no such commands: {stale:?}");
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
        let md = find("generate markdown").unwrap();
        assert_eq!(md.effect, Some(Effect::Read));
        let flag = |name: &str| md.flags.iter().find(|f| f.flag.name == name).unwrap();
        assert_eq!(flag("out-file").effect, Some(Effect::Write));
        assert_eq!(flag("out-dir").effect, Some(Effect::Write));
        // A flag that changes only the rendering stays unset.
        assert_eq!(flag("html-encode").effect, None);
    }

    #[test]
    fn every_generator_that_can_redirect_its_output_says_so() {
        // The four generators that print to stdout unless told otherwise. Kept as one list
        // because the risk is a new `--out-file` arriving without an effect, which reads to
        // an agent as a command that only ever prints.
        for path in [
            "generate fig",
            "generate go",
            "generate json-schema",
            "generate manpage",
        ] {
            let cmd = find(path).unwrap_or_else(|| panic!("no such command: {path}"));
            let out_file = cmd
                .flags
                .iter()
                .find(|f| f.flag.name == "out-file")
                .unwrap_or_else(|| panic!("{path} has no --out-file"));
            assert_eq!(out_file.effect, Some(Effect::Write), "{path} --out-file");
        }
    }

    #[test]
    fn a_required_output_flag_makes_the_command_write() {
        // `generate sdk` cannot print to stdout, so there is no read-only way
        // to invoke it and the effect belongs on the command.
        let sdk = find("generate sdk").unwrap();
        assert_eq!(sdk.effect, Some(Effect::Write));
        assert!(sdk
            .flags
            .iter()
            .any(|f| f.flag.name == "output" && f.required));
    }
}
