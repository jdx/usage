use std::fmt::{self, Display};

use serde::Serialize;
use strum::{Display as StrumDisplay, EnumString};

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
#[derive(Debug, Copy, Clone, PartialEq, Eq, EnumString, StrumDisplay, Serialize)]
#[strum(serialize_all = "snake_case")]
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

#[derive(Debug)]
pub struct ParseEffectError(pub String);

impl Display for ParseEffectError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(
            f,
            "unknown effect {:?}, expected one of: {EFFECT_VALUES}",
            self.0
        )
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
