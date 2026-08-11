use serde::Serialize;
use strum::{Display as StrumDisplay, EnumString};

/// What to do with a token that looks like a flag but names no declared flag.
///
/// The default is [`UnknownFlags::Value`], which is where this parser parts
/// company with clap, argparse, commander, oclif v2+, and POSIX `getopt` — all of
/// which reject the token. The reason is that those parse *their own* argv, where
/// a dash-word can only be a flag or a typo, while a usage spec is also used to
/// parse things whose flags it does not own:
///
/// - a shell script run through `usage exec`, forwarding options to a tool it wraps
/// - a task's arguments, where the task script is the authority on what it accepts
/// - a completion, asked about a command line that is still being typed
///
/// In all three, a dash-word the spec has not heard of is far more likely to be
/// data in transit than a mistake, and rejecting it would break the wrapper for
/// everyone who did not enumerate the flags of the program behind it.
///
/// The cost is real and worth stating: a misspelled `--hekp` becomes an argument
/// instead of an error, and whether it does depends on whether a positional is
/// free to take it. A CLI that owns all of its flags — as opposed to forwarding
/// them — should say [`UnknownFlags::Error`] and get the stricter reading.
#[derive(Debug, Default, Copy, Clone, PartialEq, Eq, EnumString, StrumDisplay, Serialize)]
#[strum(serialize_all = "snake_case")]
#[serde(rename_all = "snake_case")]
pub enum UnknownFlags {
    /// Offer the token to the positional arguments, like any other word. If none
    /// can take it, it is an unexpected argument — the same error an extra word
    /// would produce.
    #[default]
    Value,
    /// Reject the token. A CLI whose flags are all its own gets typo detection
    /// this way, at the price of needing `--` to pass a value that begins with a
    /// dash.
    Error,
}

impl UnknownFlags {
    pub fn as_str(&self) -> &'static str {
        match self {
            UnknownFlags::Value => "value",
            UnknownFlags::Error => "error",
        }
    }
}

/// The values a spec may use, for error messages.
pub(crate) const UNKNOWN_FLAGS_VALUES: &str = "value, error";
