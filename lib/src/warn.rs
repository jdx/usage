//! What a parse has to say about declarations that still work but should not be used.
//!
//! The reference implementation's half of what `usage-argv` reports from its static tables. Both
//! answer the same question — which deprecated declarations did this command line use — and the
//! rule they answer it by is written down at <https://usage.jdx.dev/spec/argv>, not in either
//! crate. Neither can depend on the other: usage-argv has no dependencies at all, on purpose.
//!
//! Nothing here prints. A resolution reports, as configuration resolution does, so a CLI that
//! queues its deprecations until its logging is up can have them as values.

use std::cmp::Ordering;

/// One thing a command line used that its own spec says not to use any more.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct Warning {
    /// What sort of declaration it was, for a caller that treats them differently.
    pub kind: WarningKind,
    /// What the user typed or set: `--old-flag`, `old-cmd`, `OLD_ENV`.
    pub name: String,
    /// The author's reason, when the declaration carries one.
    pub message: Option<String>,
    /// The release warnings start at. A warning that is here has already passed it.
    pub warn_at: Option<String>,
    /// The release the declaration goes away in, when the author has named one.
    pub remove_at: Option<String>,
    /// What to use instead, when the declaration implies one.
    pub replacement: Option<String>,
}

/// The kinds of deprecation a parse can run into.
///
/// The wording of a message is nobody's contract; this is what a program can act on.
#[derive(Clone, Copy, Debug, Default, Eq, PartialEq)]
#[non_exhaustive]
pub enum WarningKind {
    /// A flag whose declaration says not to use it any more.
    DeprecatedFlag,
    /// A command whose declaration says not to use it any more.
    DeprecatedCommand,
    /// A value that arrived through a `deprecated_env` alias rather than a current name.
    DeprecatedEnv,
    /// Something a CLI's own layer says, which this crate has no name for.
    #[default]
    Other,
}

impl Warning {
    /// A deprecated flag that was given, named as the user names it.
    pub fn flag(
        name: impl Into<String>,
        message: Option<String>,
        warn_at: Option<String>,
        remove_at: Option<String>,
    ) -> Self {
        Self {
            kind: WarningKind::DeprecatedFlag,
            name: name.into(),
            message,
            warn_at,
            remove_at,
            replacement: None,
        }
    }

    /// A deprecated command that was selected.
    pub fn command(
        name: impl Into<String>,
        message: Option<String>,
        warn_at: Option<String>,
        remove_at: Option<String>,
    ) -> Self {
        Self {
            kind: WarningKind::DeprecatedCommand,
            name: name.into(),
            message,
            warn_at,
            remove_at,
            replacement: None,
        }
    }

    /// A value read from a deprecated environment alias, and the current name for it.
    pub fn env(name: impl Into<String>, replacement: Option<String>) -> Self {
        Self {
            kind: WarningKind::DeprecatedEnv,
            name: name.into(),
            message: None,
            warn_at: None,
            remove_at: None,
            replacement,
        }
    }

    /// What a caller should print for this warning.
    pub fn render(&self) -> String {
        let subject = match self.kind {
            WarningKind::DeprecatedCommand => format!("the {} command", self.name),
            _ => self.name.clone(),
        };
        let mut out = format!("warning: {subject} is deprecated");
        if let Some(at) = &self.remove_at {
            out.push_str(&format!(", removed at {at}"));
        }
        match (&self.message, &self.replacement) {
            (Some(message), _) => out.push_str(&format!(": {message}")),
            (None, Some(replacement)) => out.push_str(&format!(": use {replacement}")),
            (None, None) => {}
        }
        out.push('\n');
        out
    }
}

/// Whether a CLI at `current` has reached the release a deprecation starts warning at.
///
/// `deprecated_warn_at` is how an author says *not yet*. Every uncertain case warns: a missing
/// milestone means deprecated now, and a version this cannot read is a spec or build problem that
/// should be noisy rather than silent.
pub fn version_reaches(current: Option<&str>, warn_at: Option<&str>) -> bool {
    let (Some(warn_at), Some(current)) = (warn_at, current) else {
        return true;
    };
    !matches!(compare(current, warn_at), Some(Ordering::Less))
}

/// Order two versions, or `None` if either is not a version this can read.
///
/// Dotted integers compared left to right, a missing segment reading as zero, a `-suffix` sorting
/// before the same numbers without one, and `+build` ignored. The same rule `usage_argv::warn`
/// implements, held to it by a parity test.
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

fn split(version: &str) -> (&str, Option<&str>) {
    let version = version.split('+').next().unwrap_or(version);
    match version.split_once('-') {
        Some((core, pre)) => (core, Some(pre)),
        None => (version, None),
    }
}

fn segment(segment: Option<&str>) -> Option<u64> {
    match segment {
        None => Some(0),
        Some(text) => text.parse().ok(),
    }
}

/// Drop the warnings a CLI at `version` has not reached yet.
pub fn retain_reached(warnings: &mut Vec<Warning>, version: Option<&str>) {
    warnings.retain(|warning| version_reaches(version, warning.warn_at.as_deref()));
}

/// Everything, one line each, in the order they were collected.
pub fn render(warnings: &[Warning]) -> String {
    warnings.iter().map(Warning::render).collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_gate_matches_the_rule() {
        assert!(version_reaches(Some("1.0.0"), None));
        assert!(version_reaches(None, Some("2.0.0")));
        assert!(!version_reaches(Some("1.0.0"), Some("2.0.0")));
        assert!(version_reaches(Some("2.0.0"), Some("2.0.0")));
        assert!(!version_reaches(Some("2.0.0-rc.1"), Some("2.0.0")));
        assert!(version_reaches(Some("nightly"), Some("2.0.0")));
        assert_eq!(compare("2026.12", "2026.12.0"), Some(Ordering::Equal));
        assert_eq!(compare("1.0.0+abc", "1.0.0+def"), Some(Ordering::Equal));
        assert_eq!(compare("nightly", "1.0.0"), None);
    }

    #[test]
    fn a_warning_says_what_to_do_about_it() {
        assert_eq!(
            Warning::flag(
                "--old",
                Some("use --new".into()),
                None,
                Some("2.0.0".into())
            )
            .render(),
            "warning: --old is deprecated, removed at 2.0.0: use --new\n",
        );
        assert_eq!(
            Warning::command("old", None, None, None).render(),
            "warning: the old command is deprecated\n",
        );
        assert_eq!(
            Warning::env("OLD_TOKEN", Some("APP_TOKEN".into())).render(),
            "warning: OLD_TOKEN is deprecated: use APP_TOKEN\n",
        );
    }
}
