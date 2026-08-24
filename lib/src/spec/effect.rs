use serde::Serialize;

/// What running a command does to the world.
///
/// This is a coarse, three-way classification rather than a permission model.
/// It exists so a spec can distinguish "safe to run to find something out" from
/// "changes state" from "may destroy something", which is the distinction
/// consumers keep reinventing:
///
/// - documentation and `--help` can mark destructive commands
/// - a shell wrapper can require confirmation
/// - an AI coding agent can be given an allowlist of read-only commands rather
///   than asking about every invocation
///
/// It is deliberately not inherited by subcommands. `git remote` and
/// `git remote remove` do different things, and silently inheriting an effect
/// from a parent would make the strictest reading of a spec the wrong one.
///
/// Flags and arguments may carry an effect too, for commands whose danger
/// depends on how they are invoked — `pitchfork logs` reads, `pitchfork logs
/// --clear` deletes. A flag or argument can only ever *raise* the effect, never
/// lower it, so a consumer that cannot parse the invocation may fall back to
/// the maximum over the command and all of its flags and arguments and still be
/// safe. `--dry-run` is the tempting counterexample; lowering is the dangerous
/// direction, because a bug in the dry-run path would then be a spec that
/// claims a command is safe when it is not.
/// Ordered least to most dangerous, so `Ord` gives the combining rule: the
/// effect of an invocation is the maximum of the command's effect and the
/// effect of every flag and argument actually supplied.
#[derive(Debug, Copy, Clone, PartialEq, Eq, PartialOrd, Ord, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum SpecCommandEffect {
    /// Only inspects state. Running it twice is the same as running it once,
    /// and not running it changes nothing.
    Read,
    /// Creates or modifies state, but does not remove anything the user cannot
    /// recreate by running another command.
    Write,
    /// May delete or irreversibly overwrite something. Deserves a confirmation
    /// prompt.
    Destructive,
}

impl_string_enum!(SpecCommandEffect {
    SpecCommandEffect::Read => "read",
    SpecCommandEffect::Write => "write",
    SpecCommandEffect::Destructive => "destructive",
});

impl SpecCommandEffect {
    pub fn as_str(&self) -> &'static str {
        match self {
            Self::Read => "read",
            Self::Write => "write",
            Self::Destructive => "destructive",
        }
    }

    /// Human-readable label used in generated documentation.
    pub fn label(&self) -> &'static str {
        match self {
            Self::Read => "read-only",
            Self::Write => "modifies state",
            Self::Destructive => "destructive",
        }
    }
}

/// The set of values accepted by `effect=`, for error messages.
pub(crate) const EFFECT_VALUES: &str = "read, write, destructive";

impl crate::SpecCommand {
    /// The effect of running this command with `flags` and `args` supplied,
    /// as the maximum of the command's own effect and theirs.
    ///
    /// A flag contributes both its own effect and that of its value argument,
    /// so `--output <file>` can declare the danger on either.
    ///
    /// Pass only what the command line actually supplied. Feeding in values
    /// that came from defaults or the environment is safe — the result can
    /// only be too high, never too low — but it will over-report.
    ///
    /// Returns `None` when nothing involved declares an effect, which means
    /// "unknown" — consumers should treat that as "ask", not as safe.
    pub fn effect_of<'a>(
        &self,
        flags: impl IntoIterator<Item = &'a crate::SpecFlag>,
        args: impl IntoIterator<Item = &'a crate::SpecArg>,
    ) -> Option<SpecCommandEffect> {
        flags
            .into_iter()
            .flat_map(|f| [f.effect, f.arg.as_ref().and_then(|a| a.effect)])
            .flatten()
            .chain(args.into_iter().filter_map(|a| a.effect))
            .chain(self.effect)
            .max()
    }

    /// The worst effect any invocation of this command could have: its own
    /// effect combined with *every* flag and argument it declares.
    ///
    /// This is the safe fallback for a consumer that has a spec but not a
    /// parsed command line.
    pub fn max_effect(&self) -> Option<SpecCommandEffect> {
        self.effect_of(&self.flags, &self.args)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::str::FromStr;

    #[test]
    fn test_parse() {
        assert_eq!(
            SpecCommandEffect::from_str("read").unwrap(),
            SpecCommandEffect::Read
        );
        assert_eq!(
            SpecCommandEffect::from_str("destructive").unwrap(),
            SpecCommandEffect::Destructive
        );
        assert!(SpecCommandEffect::from_str("readonly").is_err());
    }

    #[test]
    fn test_ordering_is_least_to_most_dangerous() {
        assert!(SpecCommandEffect::Read < SpecCommandEffect::Write);
        assert!(SpecCommandEffect::Write < SpecCommandEffect::Destructive);
    }

    #[test]
    fn test_effect_of_takes_the_maximum() {
        use crate::Spec;
        let spec: Spec = r#"
bin "pitchfork"
cmd "logs" effect="read" {
    flag "--clear" effect="destructive"
    flag "--follow"
    arg "[daemon]"
}
cmd "quiet" {
    flag "--verbose"
}
        "#
        .parse()
        .unwrap();

        let logs = &spec.cmd.subcommands["logs"];
        let clear = logs.flags.iter().find(|f| f.name == "clear").unwrap();
        let follow = logs.flags.iter().find(|f| f.name == "follow").unwrap();

        // No flags supplied: just the command's own effect.
        assert_eq!(
            logs.effect_of(vec![], vec![]),
            Some(SpecCommandEffect::Read)
        );
        // A flag with no effect of its own does not change anything.
        assert_eq!(
            logs.effect_of(vec![follow], vec![]),
            Some(SpecCommandEffect::Read)
        );
        // A dangerous flag raises it, even though the command reads.
        assert_eq!(
            logs.effect_of(vec![clear], vec![]),
            Some(SpecCommandEffect::Destructive)
        );
        // The pessimistic fallback assumes every flag was supplied.
        assert_eq!(logs.max_effect(), Some(SpecCommandEffect::Destructive));

        // Nothing declared anywhere stays unknown rather than becoming `read`.
        assert_eq!(spec.cmd.subcommands["quiet"].max_effect(), None);
    }

    /// A flag's value argument can carry the effect instead of the flag, and
    /// the pessimistic bound has to see it or it under-reports danger.
    #[test]
    fn test_effect_on_a_flag_value_arg_counts() {
        use crate::Spec;
        let spec: Spec = r#"
bin "x"
cmd "write-to" effect="read" {
    flag "--output <file>" {
        arg "<file>" effect="destructive"
    }
}
        "#
        .parse()
        .unwrap();
        let cmd = &spec.cmd.subcommands["write-to"];
        let output = cmd.flags.iter().find(|f| f.name == "output").unwrap();
        assert_eq!(
            cmd.effect_of(vec![output], vec![]),
            Some(SpecCommandEffect::Destructive)
        );
        assert_eq!(cmd.max_effect(), Some(SpecCommandEffect::Destructive));
    }

    #[test]
    fn test_effect_on_an_arg_raises_too() {
        use crate::Spec;
        // `mise settings foo` reads; `mise settings foo=bar` writes, and the
        // discriminator is a positional rather than a flag.
        let spec: Spec = r#"
bin "mise"
cmd "settings" effect="read" {
    arg "[setting]"
    arg "[value]" effect="write"
}
        "#
        .parse()
        .unwrap();
        let cmd = &spec.cmd.subcommands["settings"];
        let value = cmd.args.iter().find(|a| a.name == "value").unwrap();
        assert_eq!(cmd.effect_of(vec![], vec![]), Some(SpecCommandEffect::Read));
        assert_eq!(
            cmd.effect_of(vec![], vec![value]),
            Some(SpecCommandEffect::Write)
        );
    }

    #[test]
    fn test_unknown_effect_on_a_flag_is_an_error() {
        use crate::Spec;
        let err = r#"
bin "x"
cmd "y" {
    flag "--z" effect="readonly"
}
        "#
        .parse::<Spec>()
        .unwrap_err();
        assert!(err.to_string().contains("Invalid usage config"));
    }

    #[test]
    fn test_display_roundtrips() {
        for effect in [
            SpecCommandEffect::Read,
            SpecCommandEffect::Write,
            SpecCommandEffect::Destructive,
        ] {
            assert_eq!(
                SpecCommandEffect::from_str(&effect.to_string()).unwrap(),
                effect
            );
            assert_eq!(effect.to_string(), effect.as_str());
        }
    }
}
