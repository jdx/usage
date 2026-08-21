//! What a parse has to say about declarations that still work but should not be used.
//!
//! A `deprecated` flag, command, or environment alias reaches help and completion descriptions
//! already. This is the other half: telling the person who just used one, which is the only thing
//! that actually moves anybody off an old spelling.
//!
//! Nothing here prints. A library that writes to stderr cannot be used by anything with an
//! opinion about output, and mise queues its deprecations until its logging is up — the same rule
//! [`usage_config`](https://docs.rs/usage-config) resolution follows. `parse()`, which *is* the
//! process, renders them; every other entry point hands them back.
//!
//! Every field borrows the static metadata tables, so a warning allocates nothing. The `Vec` a
//! caller passes in only allocates if something was actually deprecated, and an entry point that
//! was not asked for warnings never collects any.

use core::cmp::Ordering;

/// One thing a command line used that its own spec says not to use any more.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct Warning<'t> {
    /// What sort of declaration it was, for a caller that treats them differently.
    pub kind: WarningKind,
    /// What the user typed or set: `--old-flag`, `old-cmd`, `OLD_ENV`.
    pub name: &'t str,
    /// The author's reason, when the declaration carries one.
    pub message: Option<&'t str>,
    /// The release warnings start at. A warning that is here has already passed it.
    pub warn_at: Option<&'t str>,
    /// The release the declaration goes away in, when the author has named one.
    pub remove_at: Option<&'t str>,
    /// What to use instead, when the declaration implies one — the current variable, for a
    /// deprecated environment alias.
    pub replacement: Option<&'t str>,
}

/// The kinds of deprecation a parse can run into.
///
/// The message is for a person and its wording is nobody's contract; this is what a *program* can
/// act on. A CLI that queues its warnings wants to sort them; a `--strict` mode wants to refuse
/// one kind and not another; a conformance vector wants to pin what happened without pinning how
/// it was worded.
#[derive(Clone, Copy, Debug, Default, Eq, PartialEq)]
#[non_exhaustive]
pub enum WarningKind {
    /// A flag whose declaration says not to use it any more.
    DeprecatedFlag,
    /// A command whose declaration says not to use it any more. Every deprecated command on the
    /// selected path reports, not only the last one.
    DeprecatedCommand,
    /// A value that arrived through a `deprecated_env` alias rather than a current name.
    DeprecatedEnv,
    /// Something a CLI's own layer says, which this crate has no name for.
    #[default]
    Other,
}

impl<'t> Warning<'t> {
    /// A deprecated flag that was given.
    pub const fn flag(
        name: &'t str,
        message: Option<&'t str>,
        warn_at: Option<&'t str>,
        remove_at: Option<&'t str>,
    ) -> Self {
        Self {
            kind: WarningKind::DeprecatedFlag,
            name,
            message,
            warn_at,
            remove_at,
            replacement: None,
        }
    }

    /// A deprecated command that was selected.
    pub const fn command(
        name: &'t str,
        message: Option<&'t str>,
        warn_at: Option<&'t str>,
        remove_at: Option<&'t str>,
    ) -> Self {
        Self {
            kind: WarningKind::DeprecatedCommand,
            name,
            message,
            warn_at,
            remove_at,
            replacement: None,
        }
    }

    /// A value read from a deprecated environment alias, and the current name for it.
    ///
    /// An alias carries no milestones of its own: the declaration that named it is what says it
    /// is deprecated, so this always warns.
    pub const fn env(name: &'t str, replacement: Option<&'t str>) -> Self {
        Self {
            kind: WarningKind::DeprecatedEnv,
            name,
            message: None,
            warn_at: None,
            remove_at: None,
            replacement,
        }
    }
}

/// Whether a CLI at `current` has reached the release a deprecation starts warning at.
///
/// The gate exists because `deprecated_warn_at` is how an author says *not yet*: a declaration
/// deprecated in next year's release should not be shouting about it in this one.
///
/// Every uncertain case warns. A missing milestone means it is deprecated now; a version this
/// cannot read is a spec or build problem, and answering it with silence would hide a deprecation
/// nobody then hears about. Noise is recoverable, silence is not.
pub fn version_reaches(current: Option<&str>, warn_at: Option<&str>) -> bool {
    let Some(warn_at) = warn_at else {
        return true;
    };
    let Some(current) = current else {
        return true;
    };
    match compare(current, warn_at) {
        Some(Ordering::Less) => false,
        Some(_) => true,
        None => true,
    }
}

/// Order two versions, or `None` if either is not a version this can read.
///
/// Dotted integers, compared left to right, with a missing segment reading as zero so `2026.12`
/// and `2026.12.0` are one release. A `-suffix` sorts before the same numbers without one, which
/// is semver's rule and the one a `-rc` needs; build metadata after `+` is ignored, also semver's
/// rule. Two pre-releases on the same numbers compare as text, which is coarser than semver's
/// per-identifier rule and enough for a gate: the answer only has to be stable.
///
/// This is deliberately the same rule usage-lib implements separately. Neither crate can depend on
/// the other — this one has no dependencies at all — so the rule is written down at
/// <https://usage.jdx.dev/spec/argv> and a parity test holds the two to it.
pub fn compare(a: &str, b: &str) -> Option<Ordering> {
    let (a_core, a_pre) = split(a);
    let (b_core, b_pre) = split(b);
    let mut a_segments = a_core.split('.');
    let mut b_segments = b_core.split('.');
    loop {
        let (a_next, b_next) = (a_segments.next(), b_segments.next());
        if a_next.is_none() && b_next.is_none() {
            break;
        }
        match segment(a_next)?.cmp(&segment(b_next)?) {
            Ordering::Equal => continue,
            ordering => return Some(ordering),
        }
    }
    Some(match (a_pre, b_pre) {
        (None, None) => Ordering::Equal,
        (Some(_), None) => Ordering::Less,
        (None, Some(_)) => Ordering::Greater,
        (Some(a), Some(b)) => a.cmp(b),
    })
}

/// The numeric core of a version, and its pre-release suffix if it has one.
fn split(version: &str) -> (&str, Option<&str>) {
    let version = version.split('+').next().unwrap_or(version);
    match version.split_once('-') {
        Some((core, pre)) => (core, Some(pre)),
        None => (version, None),
    }
}

/// One dotted segment as a number. Absent is zero; anything not a number is unreadable.
fn segment(segment: Option<&str>) -> Option<u64> {
    match segment {
        None => Some(0),
        Some(text) => text.parse().ok(),
    }
}

/// Drop the warnings a CLI at `version` has not reached yet.
///
/// Applied once, by the entry point that knows the CLI's version — which the command whose
/// metadata carries the milestone does not: a nested command's tables say nothing about the root's
/// version, and a CLI with a computed `runtime_version` only settles it at run time.
pub fn retain_reached(warnings: &mut Vec<Warning<'_>>, version: Option<&str>) {
    warnings.retain(|warning| version_reaches(version, warning.warn_at));
}

/// What a caller should print for one warning, without the renderer that colours it.
///
/// See [`crate::render_warnings`] for the entry point generated code reaches for; this is the
/// wording both halves share.
pub fn render_warning(warning: &Warning<'_>) -> String {
    let mut out = String::new();
    out.push_str("warning: ");
    out.push_str(&subject(warning));
    out.push_str(" is deprecated");
    if let Some(at) = warning.remove_at {
        out.push_str(", removed at ");
        out.push_str(at);
    }
    match tail(warning) {
        Some(Tail::Message(message)) => {
            out.push_str(": ");
            out.push_str(message);
        }
        Some(Tail::Replacement(replacement)) => {
            out.push_str(": use ");
            out.push_str(replacement);
        }
        None => {}
    }
    out.push('\n');
    out
}

/// Every warning, in the order they were collected.
pub fn render_warnings(warnings: &[Warning<'_>]) -> String {
    warnings.iter().map(render_warning).collect()
}

/// What the warning is about, named the way the user named it.
pub(crate) fn subject(warning: &Warning<'_>) -> String {
    match warning.kind {
        WarningKind::DeprecatedCommand => format!("the {} command", warning.name),
        _ => warning.name.to_string(),
    }
}

/// What to do about a deprecation: the author's own words, or the name that replaced this one.
///
/// Which of the two a warning has is decided here and nowhere else, so the plain renderer and the
/// coloured one can differ in how they show it without differing in what they show.
pub(crate) enum Tail<'t> {
    /// The author's message, which is prose.
    Message(&'t str),
    /// A name to type instead.
    Replacement(&'t str),
}

pub(crate) fn tail<'t>(warning: &Warning<'t>) -> Option<Tail<'t>> {
    match (warning.message, warning.replacement) {
        (Some(message), _) => Some(Tail::Message(message)),
        (None, Some(replacement)) => Some(Tail::Replacement(replacement)),
        (None, None) => None,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_missing_milestone_warns_now() {
        assert!(version_reaches(Some("1.0.0"), None));
        assert!(version_reaches(None, None));
    }

    #[test]
    fn a_version_the_cli_has_not_reached_stays_quiet() {
        assert!(!version_reaches(Some("1.0.0"), Some("2.0.0")));
        assert!(!version_reaches(Some("2026.11.0"), Some("2026.12.0")));
        assert!(!version_reaches(Some("1.9.9"), Some("1.10.0")));
    }

    #[test]
    fn the_release_itself_warns() {
        assert!(version_reaches(Some("2.0.0"), Some("2.0.0")));
        assert!(version_reaches(Some("2.0.1"), Some("2.0.0")));
        assert!(version_reaches(Some("2026.12.0"), Some("2026.11.0")));
    }

    #[test]
    fn a_missing_segment_is_zero() {
        assert_eq!(compare("2026.12", "2026.12.0"), Some(Ordering::Equal));
        assert_eq!(compare("1", "1.0.0"), Some(Ordering::Equal));
        assert_eq!(compare("1.0.1", "1"), Some(Ordering::Greater));
    }

    #[test]
    fn a_pre_release_comes_before_the_release() {
        assert_eq!(compare("1.0.0-rc.1", "1.0.0"), Some(Ordering::Less));
        assert!(!version_reaches(Some("2.0.0-rc.1"), Some("2.0.0")));
        assert!(version_reaches(Some("2.0.0"), Some("2.0.0-rc.1")));
        assert_eq!(compare("1.0.0-rc.1", "1.0.0-rc.2"), Some(Ordering::Less));
    }

    #[test]
    fn build_metadata_is_ignored() {
        assert_eq!(compare("1.0.0+abc", "1.0.0+def"), Some(Ordering::Equal));
        assert!(version_reaches(Some("2.0.0+build.7"), Some("2.0.0")));
    }

    #[test]
    fn a_version_that_cannot_be_read_warns_rather_than_hides() {
        assert_eq!(compare("nightly", "2.0.0"), None);
        assert_eq!(compare("", "2.0.0"), None);
        assert!(version_reaches(Some("nightly"), Some("2.0.0")));
        assert!(version_reaches(Some(""), Some("2.0.0")));
        assert!(version_reaches(Some("1.0.0"), Some("whenever")));
    }

    #[test]
    fn a_warning_says_what_to_do_about_it() {
        assert_eq!(
            render_warning(&Warning::flag("--old", Some("use --new"), None, None)),
            "warning: --old is deprecated: use --new\n",
        );
        assert_eq!(
            render_warning(&Warning::flag("--old", None, None, Some("2.0.0"))),
            "warning: --old is deprecated, removed at 2.0.0\n",
        );
        assert_eq!(
            render_warning(&Warning::command("old", None, None, None)),
            "warning: the old command is deprecated\n",
        );
        assert_eq!(
            render_warning(&Warning::env("OLD_TOKEN", Some("APP_TOKEN"))),
            "warning: OLD_TOKEN is deprecated: use APP_TOKEN\n",
        );
    }

    #[test]
    fn retaining_drops_only_what_has_not_arrived() {
        let mut warnings = vec![
            Warning::flag("--soon", None, Some("9.0.0"), None),
            Warning::flag("--now", None, Some("1.0.0"), None),
            Warning::env("OLD_TOKEN", None),
        ];
        retain_reached(&mut warnings, Some("1.2.3"));
        assert_eq!(
            warnings.iter().map(|w| w.name).collect::<Vec<_>>(),
            ["--now", "OLD_TOKEN"],
        );
    }
}
