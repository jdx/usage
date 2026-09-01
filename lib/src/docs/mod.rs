use regex::Regex;
use std::borrow::Cow;
use std::sync::LazyLock;

#[cfg(feature = "cli-help")]
pub mod cli;
#[cfg(feature = "cli-help")]
mod layout;
#[cfg(feature = "manpage")]
pub mod manpage;
#[cfg(feature = "markdown")]
pub mod markdown;
#[cfg(feature = "cli-help")]
pub(crate) mod models;

/// An ANSI control-sequence introducer escape, including the SGR sequences commonly embedded by
/// `color_print::cstr!`.
static ANSI_CSI: LazyLock<Regex> =
    LazyLock::new(|| Regex::new(r"\x1b\[[0-?]*[ -/]*[@-~]").unwrap());

pub(crate) fn strip_ansi(value: &str) -> Cow<'_, str> {
    ANSI_CSI.replace_all(value, "")
}
