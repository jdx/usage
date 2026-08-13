//! Where a value came from.
//!
//! Provenance is not an extra pass here: it is the output of the only merge there is. hk grew
//! a second parallel merge function purely to answer "where did this come from", and the two
//! could disagree — so `hk config explain` could describe a resolution that never happened.
//! Recording the origin as the value is chosen makes that class of bug unreachable.

/// A kind of place a value can come from.
///
/// Deliberately open: usage knows about the command line, the environment, files and declared
/// defaults, and every CLI in the fleet has at least one kind it reads itself — a git config,
/// a pkl file, an `.npmrc`. Those declare a `source` in the spec and pass their own kind here.
#[derive(Debug, Copy, Clone, PartialEq, Eq)]
pub struct SourceKind(&'static str);

impl SourceKind {
    /// The command line.
    pub const CLI: Self = Self("cli");
    /// The environment.
    pub const ENV: Self = Self("env");
    /// A configuration file usage read itself.
    pub const FILE: Self = Self("file");
    /// The default the spec declares.
    pub const DEFAULTS: Self = Self("defaults");
    /// A value the CLI rewrote after merging — mise's `raw` implying `jobs = 1`.
    ///
    /// Its own kind so `explain` never claims a file said something it did not. A rewrite
    /// that looked like it came from wherever the original value did is how a user ends up
    /// editing a file that has nothing to do with the value they are seeing.
    pub const COERCED: Self = Self("coerced");

    pub const fn new(name: &'static str) -> Self {
        Self(name)
    }

    pub const fn name(self) -> &'static str {
        self.0
    }
}

/// Which class of file a value came from, when it came from one.
///
/// Mirrors `scope=` on a spec's `file` node, and decides the origin's [`Trust`].
#[derive(Debug, Copy, Clone, PartialEq, Eq)]
pub enum FileScope {
    /// Somewhere a repository can carry — the least trusted.
    Project,
    /// The user's own configuration.
    Global,
    /// Installed by whoever administers the machine.
    System,
}

/// How much a place is trusted, which is what a setting's scope is about.
///
/// The distinction is not "was it a file": a pkl file, a git config or an `.npmrc` inside a
/// repository is every bit as much a thing a checkout can carry as `hk.toml` is. Asking about
/// files let every custom source — the natural use of [`Origin::new`] — walk straight past a
/// check the spec calls a security property.
///
/// So the question is trust, every origin carries an answer, and the default for a kind usage
/// does not recognize is the *least* trusting one. A layer that knows better says so with
/// [`Origin::trusted_as`]; a layer that says nothing cannot accidentally be believed.
#[derive(Debug, Copy, Clone, PartialEq, Eq, PartialOrd, Ord)]
pub enum Trust {
    /// Somewhere a repository can carry — a project file, a git config in the checkout.
    Project,
    /// The user's own configuration, or the machine's.
    Operator,
    /// This invocation itself: the command line, the environment, a declared default.
    Invocation,
}

/// The exact place a value came from.
///
/// Not just the kind: the *identifier*, because "from the environment" is not an answer a
/// user can act on and `HK_JOBS` is. This is what makes `config explain` worth having.
#[derive(Debug, Clone, PartialEq)]
pub struct Origin {
    pub kind: SourceKind,
    /// The environment variable's name, the file's path, the git key — whatever a user would
    /// have to go and edit.
    pub identifier: String,
    /// How much this place is trusted, which is what the scope check reads.
    pub trust: Trust,
}

impl Origin {
    /// An origin of the given kind.
    ///
    /// The trust follows the kind: this invocation for the command line, the environment and
    /// the built-ins, and [`Trust::Project`] for anything else — because a kind usage does not
    /// recognize is one it cannot vouch for, and a check that has to be remembered by each new
    /// layer is one a new layer will forget. Say otherwise with [`Origin::trusted_as`].
    pub fn new(kind: SourceKind, identifier: impl Into<String>) -> Self {
        let trust = match kind {
            SourceKind::CLI | SourceKind::ENV | SourceKind::DEFAULTS | SourceKind::COERCED => {
                Trust::Invocation
            }
            _ => Trust::Project,
        };
        Self {
            kind,
            identifier: identifier.into(),
            trust,
        }
    }

    /// The same origin, trusted as stated.
    ///
    /// For a custom layer that knows where it read from: a git config in `$HOME` is the
    /// user's own, while one in the checkout is not.
    pub fn trusted_as(mut self, trust: Trust) -> Self {
        self.trust = trust;
        self
    }

    /// An origin in a config file of the given class.
    pub fn file(identifier: impl Into<String>, scope: FileScope) -> Self {
        Self {
            kind: SourceKind::FILE,
            identifier: identifier.into(),
            trust: match scope {
                FileScope::Project => Trust::Project,
                FileScope::Global | FileScope::System => Trust::Operator,
            },
        }
    }

    /// The declared default.
    ///
    /// Named for what it *is* rather than spelled `Default::default`, because an `Origin` has
    /// no sensible zero — every one of them names a real place.
    pub fn declared_default() -> Self {
        Self::new(SourceKind::DEFAULTS, "the default")
    }

    /// How to describe this in one phrase: `HK_JOBS`, `hk.toml`, `the default`.
    pub fn describe(&self) -> &str {
        &self.identifier
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_kind_usage_does_not_know_is_still_a_kind() {
        // hk's git config, aube's .npmrc: the reason this is not a closed enum.
        let git = SourceKind::new("git");
        assert_eq!(git.name(), "git");
        assert_ne!(git, SourceKind::FILE);
        // And the built-ins are distinguishable from each other, which the scope check and
        // `explain` both depend on.
        assert_ne!(SourceKind::CLI, SourceKind::ENV);
        assert_ne!(SourceKind::DEFAULTS, SourceKind::COERCED);
    }

    #[test]
    fn a_kind_usage_cannot_vouch_for_is_trusted_least() {
        // The hole this closes: the scope check used to ask whether an origin was a *file*, so
        // every custom source — a pkl file, a git config, an `.npmrc`, all built with
        // `Origin::new` — walked straight past it. A pkl file in a checkout is exactly as much
        // a thing a repository can carry as `hk.toml` is.
        assert_eq!(
            Origin::new(SourceKind::new("pkl"), "jobs").trust,
            Trust::Project
        );
        assert_eq!(
            Origin::new(SourceKind::new("git"), "hk.jobs").trust,
            Trust::Project
        );
        // The kinds usage does know are the invocation itself.
        for kind in [SourceKind::CLI, SourceKind::ENV, SourceKind::COERCED] {
            assert_eq!(Origin::new(kind, "x").trust, Trust::Invocation, "{kind:?}");
        }
        assert_eq!(Origin::declared_default().trust, Trust::Invocation);
        // A layer that knows better says so, rather than being believed by default.
        assert_eq!(
            Origin::new(SourceKind::new("git"), "hk.jobs")
                .trusted_as(Trust::Operator)
                .trust,
            Trust::Operator
        );
        // And a file's class decides its trust.
        assert_eq!(
            Origin::file("hk.toml", FileScope::Project).trust,
            Trust::Project
        );
        for scope in [FileScope::Global, FileScope::System] {
            assert_eq!(Origin::file("x", scope).trust, Trust::Operator, "{scope:?}");
        }
    }
}
