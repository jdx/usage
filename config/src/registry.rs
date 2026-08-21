//! The settings a CLI has, as a generated table.
//!
//! `usage-config-build` reads the spec's `config` block and emits a `static` of these, so at
//! runtime a registry is a slice — no parsing, no map to build, and a [`PropId`] that indexes
//! it directly. A merge over a hundred settings therefore never hashes a key, which is what
//! makes resolving the whole struct at once cheap enough to do eagerly.
//!
//! Keys are the dotted paths the spec declares. Nesting is a *file's* concern, reconstructed
//! by whatever reads the file; here a key is one string.

use crate::source::SourceKind;
use crate::ty::{Parser, Ty};
use crate::value::{Const, Value};

/// A setting's index in its registry.
///
/// Interned so the merge is array indexing rather than string comparison. `u16` because a
/// registry of 65,000 settings is not a thing that exists — mise, the largest in the fleet,
/// has 280.
#[derive(Debug, Copy, Clone, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct PropId(pub u16);

impl PropId {
    /// This id as a slice index.
    pub fn index(self) -> usize {
        self.0 as usize
    }
}

/// How the values for one setting combine when several layers supply them.
#[derive(Debug, Default, Copy, Clone, PartialEq, Eq)]
pub enum Merge {
    /// The highest-precedence value wins outright.
    #[default]
    Replace,
    /// Every layer contributes; a list is the concatenation, lowest precedence first.
    Union,
    /// Tables merge key by key, the higher precedence winning each key.
    Deep,
}

/// Where a setting will accept a value from.
///
/// Enforced by the merge rather than left to each layer, because mise calls this a security
/// property and a check every layer has to remember is not one.
#[derive(Debug, Default, Copy, Clone, PartialEq, Eq)]
pub enum Scope {
    /// Anywhere.
    #[default]
    Any,
    /// Never from a file a repository can carry — only the user's own configuration, the
    /// machine's, the environment, or the command line.
    Global,
    /// Never from a file at all.
    Env,
}

/// What one setting is.
///
/// Every field is `const`-constructible, so a generated registry is a `static` with no
/// initializer to run.
#[derive(Debug, Copy, Clone)]
pub struct PropMeta {
    /// The dotted key, which is also what a config file and `config set` call it.
    pub key: &'static str,
    pub ty: Ty,
    /// The value when no layer supplies one.
    pub default: Option<Const>,
    pub merge: Merge,
    pub scope: Scope,
    /// How to split a single string into several values, when a layer hands over text.
    pub parse: Option<Parser>,
    /// Environment variables that set it, highest precedence first.
    pub envs: &'static [&'static str],
    /// The flags that set it, as the spec's `cli` node declares them: `["--jobs", "-j"]`.
    ///
    /// Documentation for the most part — an explanation that lists the environment variables and not
    /// the flag is answering half the question a user asked. It is also what a generated test can
    /// hold the *executable* binding against, since a spec that declares `--jobs` and a CLI that
    /// never reads it into the setting is the shape of hk's thirteen dead `sources.cli` lines.
    pub cli: &'static [&'static str],
    /// Its keys in sources usage does not know about: `[("git", "hk.jobs")]`. A custom layer
    /// asks the registry for its own kind and iterates what it finds, which is the whole
    /// mechanism behind hk's git and pkl layers and aube's `.npmrc`.
    pub bindings: &'static [(&'static str, &'static str)],
    /// The only values this setting accepts, when it says.
    ///
    /// Empty means anything the type allows. Declared in the spec as `choice` nodes, where they
    /// already reach the docs, the JSON schema and completions — and, until this, nothing that
    /// *resolved* a value, so a CLI documenting three allowed values accepted a fourth in silence.
    pub choices: &'static [Const],
    /// Kept out of documentation and completions. Still settable.
    pub hide: bool,
    /// Why not to use this any more.
    pub deprecated: Option<&'static str>,
    /// The setting that replaces this one. A value found under the old key is folded into the
    /// new one at the same precedence, with a warning.
    pub renamed_to: Option<&'static str>,
    /// Equivalent keys accepted without a rename warning.
    pub aliases: &'static [&'static str],
    /// An explicit optionality contract, when the declaration does not use inference.
    pub optional: Option<bool>,
    pub help: Option<&'static str>,
}

impl PropMeta {
    /// A setting with nothing but a key and a type, for a generator or a test to build on.
    pub const fn new(key: &'static str, ty: Ty) -> Self {
        Self {
            key,
            ty,
            default: None,
            merge: Merge::Replace,
            scope: Scope::Any,
            parse: None,
            envs: &[],
            cli: &[],
            bindings: &[],
            choices: &[],
            hide: false,
            deprecated: None,
            renamed_to: None,
            aliases: &[],
            optional: None,
            help: None,
        }
    }
}

/// Every setting a CLI has.
#[derive(Debug, Copy, Clone)]
pub struct Registry {
    pub props: &'static [PropMeta],
}

/// What looking a key up found.
#[derive(Debug, Copy, Clone, PartialEq)]
pub struct Lookup {
    pub id: PropId,
    /// The canonical key or alias that matched the lookup, so diagnostics can repeat the key the
    /// user actually wrote without treating a supported alias as a deprecated rename.
    pub written: &'static str,
    /// The key that was asked for, when it is not the key that was found — an old name still
    /// in somebody's config file. Carried so a warning can name it.
    pub renamed_from: Option<&'static str>,
}

impl PropMeta {
    /// The first value here that this setting does not allow, if there is one.
    ///
    /// A collection is checked item by item, because choices on a `list<string>` mean each item is
    /// one of them — the same rule `usage g json-schema` follows, which puts the enum on every value
    /// position rather than on the container. Returning the offender rather than a bool is what lets
    /// the warning quote the item that is wrong instead of the whole list it was in.
    pub fn refuses<'v>(&self, value: &'v Value) -> Option<&'v Value> {
        if self.choices.is_empty() {
            return None;
        }
        self.refuses_as(self.ty, value)
    }

    /// The same, for the type this level of the value is held to.
    ///
    /// Threaded down through a collection because a choice is compared *as the declared type reads
    /// it*, and the declared type of an item is not the declared type of the list it is in.
    fn refuses_as<'v>(&self, ty: Ty, value: &'v Value) -> Option<&'v Value> {
        // On the *declared* type, not on the shape of what arrived. Following the value instead, a
        // scalar-choiced setting the spec left open — `any`, a union — accepted `[1]` for `choice 1`,
        // because the walk went looking for items in something that was never declared to have any.
        match (ty.inner(), value) {
            (Ty::List(item) | Ty::Set(item), Value::List(items)) => {
                items.iter().find_map(|value| self.refuses_as(*item, value))
            }
            (Ty::Map(item), Value::Map(entries)) => entries
                .values()
                .find_map(|value| self.refuses_as(*item, value)),
            // Anything else is compared whole, and a scalar choice is not a list however many items
            // it has: `Const::matches` says as much, and this is where that is asked.
            _ => match self.choices.iter().any(|choice| allows(ty, choice, value)) {
                true => None,
                false => Some(value),
            },
        }
    }

    /// What it allows, written the way the spec declared them, for a message.
    pub fn allowed(&self) -> String {
        self.choices
            .iter()
            .map(|choice| choice.to_value().display())
            .collect::<Vec<_>>()
            .join(", ")
    }
}

/// Whether `choice` is `value`, read the way the declared type reads them both.
///
/// The value arrived here through `Ty::coerce`, and the choice has not: a spec writes `choice "yes"`
/// under `type="bool"` and the value `yes` becomes `Bool(true)`, so comparing them as written refuses
/// a value the spec plainly allows. Reading the choice the same way is what makes the two comparable
/// — it is the same question the coercion already answered.
fn allows(ty: Ty, choice: &Const, value: &Value) -> bool {
    match ty.coerce(choice.to_value()) {
        Ok(coerced) if coerced == *value => true,
        // Coercion did not settle it, so the question falls back to what the two are written as.
        // `any` is where that matters most — a union coerces *nothing*, so a `choice 4` and the
        // string `4` a file supplied stay an integer and a string — and I had this as the `Err` arm,
        // which `any` never takes, since coercing there succeeds by doing nothing at all. It is also
        // the arm for a choice the declared type cannot read, which is one nothing can supply
        // either.
        _ => choice.matches(value),
    }
}

impl Registry {
    pub const fn new(props: &'static [PropMeta]) -> Self {
        Self { props }
    }

    pub fn get(&self, id: PropId) -> &'static PropMeta {
        &self.props[id.index()]
    }

    /// The id of a dotted key, following a rename to the setting that replaced it.
    ///
    /// Linear, because a registry is small and a lookup happens once per key a layer
    /// supplies — not once per key that exists. A binary search over a sorted table would be
    /// a fine optimization and is not yet worth the invariant it demands of the generator.
    pub fn lookup(&self, key: &str) -> Option<Lookup> {
        let (index, written, matched_alias) =
            self.props.iter().enumerate().find_map(|(index, meta)| {
                if meta.key == key {
                    Some((index, meta.key, false))
                } else {
                    meta.aliases
                        .iter()
                        .copied()
                        .find(|alias| *alias == key)
                        .map(|alias| (index, alias, true))
                }
            })?;
        let mut id = PropId(index as u16);
        // A chain of renames resolves to its end, so two releases of renaming do not leave the
        // second one unreachable — walked rather than recursed, and bounded by the number of
        // settings there are. A cycle, which is one mistyped field away in a registry somebody
        // wrote by hand, overflowed the stack: an abort with no message rather than a lookup
        // that fails. A chain longer than the registry is a cycle by definition.
        for _ in 0..self.props.len() {
            let Some(new_key) = self.props[id.index()].renamed_to else {
                return Some(Lookup {
                    id,
                    written,
                    renamed_from: (id.index() != index && !matched_alias).then_some(written),
                });
            };
            id = self.lookup_exact(new_key)?;
        }
        None
    }

    /// The id of a dotted key, *without* following a rename.
    ///
    /// [`Registry::lookup`] answers "which setting does this key mean", which is what a reader
    /// wants. This answers "which declaration is this key", which is what a warning wants: the
    /// deprecation message lives on the old name's own declaration.
    pub fn lookup_exact(&self, key: &str) -> Option<PropId> {
        self.props
            .iter()
            .position(|meta| meta.key == key)
            .map(|index| PropId(index as u16))
    }

    /// Whether a table at `key` is itself a setting value rather than a path to
    /// more-specific settings.
    ///
    /// Aliases participate in file lookup, but an alias that is also the prefix
    /// of a declared dotted key must not swallow that nested value. A leaf alias
    /// for a map or object still names the whole table.
    pub fn names_file_value(&self, key: &str) -> bool {
        let Some(found) = self.lookup(key) else {
            return false;
        };
        let meta = self.get(found.id);
        // A canonical table setting owns its value. Another declaration's alias below
        // that key cannot turn the canonical map into a namespace and make the whole
        // value disappear during flattening.
        if meta.key == key && matches!(meta.ty.inner(), Ty::Map(_) | Ty::Object | Ty::Any) {
            return true;
        }
        let prefix = format!("{key}.");
        !self.props.iter().any(|meta| {
            meta.key.starts_with(&prefix)
                || meta.aliases.iter().any(|alias| alias.starts_with(&prefix))
        })
    }

    /// The first deprecation notice along the rename chain that starts at `key`.
    ///
    /// The chain, not the declaration named: `a` renamed to `b`, and `b` the one carrying the notice
    /// that says to use `c`. A user who wrote `a` is being told the same thing either way, and which
    /// release the notice was attached in is not something they can see.
    ///
    /// Bounded by the number of settings there are, so a registry whose renames form a cycle stops
    /// rather than following them forever — the same guard [`Registry::lookup`] uses, and for the
    /// same reason: this is an authoring mistake, and hanging is a worse way to report one than
    /// nothing at all. `usage-config-build` refuses such a registry outright.
    pub fn deprecation(&self, key: &str) -> Option<&'static str> {
        let mut current = self
            .props
            .iter()
            .position(|meta| meta.key == key || meta.aliases.contains(&key))
            .map(|index| PropId(index as u16))?;
        for _ in 0..self.props.len() {
            let meta = self.get(current);
            if let Some(why) = meta.deprecated {
                return Some(why);
            }
            current = meta.renamed_to.and_then(|next| self.lookup_exact(next))?;
        }
        None
    }

    /// The settings an environment variable sets, and the variable that set them.
    ///
    /// Several names per setting are aliases in descending precedence, which the env layer
    /// honours by taking the first one that is present.
    pub fn ids(&self) -> impl Iterator<Item = PropId> {
        (0..self.props.len()).map(|i| PropId(i as u16))
    }

    /// Every way the flags a spec *declares* and the flags a CLI *binds* disagree.
    ///
    /// Empty means they agree. This is the check hk needed and did not have: it declares eighteen
    /// `sources.cli` bindings and reads five, because the declaration lives in a spec and the
    /// reading lives in a hand-written struct, and nothing has ever compared the two. A spec that
    /// documents `--jobs` and a CLI that never puts it anywhere is a promise to a user that no test
    /// could catch.
    ///
    /// `bound` is what the CLI actually does: pairs of a flag and the setting it sets. A CLI whose
    /// flags come from `usage::Cli` can generate that list; one that binds by hand writes it out,
    /// which is still one list rather than two behaviours.
    ///
    /// Both directions are reported, because they are different mistakes. A declared flag nothing
    /// binds is documentation for something that does not happen. A bound flag the setting does not
    /// declare happens without being documented — the user cannot discover it, and `explain` cannot
    /// name it.
    pub fn drift(&self, bound: &[(&str, &str)]) -> Vec<String> {
        let mut problems = Vec::new();

        for (flag, key) in bound {
            let Some(found) = self.lookup(key) else {
                problems.push(format!(
                    "`{flag}` is bound to `{key}`, which is not a setting"
                ));
                continue;
            };
            if !self.declares(found.id, flag) {
                problems.push(format!(
                    "`{flag}` is bound to `{key}`, which does not declare it: add `cli \"{flag}\"` to the spec"
                ));
            }
        }

        for id in self.ids() {
            let meta = self.get(id);
            // Asked once, and refused when it is `None`: two keys that resolve to nothing are not
            // two keys that mean the same setting, and comparing the answers directly made a flag
            // on a dangling `renamed_to` look bound by any flag bound to any other broken key.
            let means = self.means(meta.key);
            for flag in meta.cli {
                let Some(means) = means else {
                    problems.push(format!(
                        "`{}` says `{flag}` sets it, and it is not a setting anything can reach: \
                         its `renamed_to` names nothing, or the chain it starts loops",
                        meta.key
                    ));
                    continue;
                };
                // Compared against the *setting* the binding names rather than the flag alone: a CLI
                // may bind `--jobs` to something, and binding it to the wrong setting is not the same
                // as binding it.
                let bound_here = bound
                    .iter()
                    .any(|(bound_flag, key)| bound_flag == flag && self.means(key) == Some(means));
                if !bound_here {
                    problems.push(format!(
                        "`{}` says `{flag}` sets it, and nothing does",
                        meta.key
                    ));
                }
            }
        }
        problems
    }

    /// Whether any declaration of the setting `id` lists `flag`.
    ///
    /// Any, because a rename leaves two declarations of one setting and either may be the one
    /// carrying the flag: a CLI that has not dropped `--concurrency` yet is bound to a key that
    /// *means* `jobs`, and the flag is declared where the old name is. Asking only the replacement
    /// called a live binding drift; asking only the declaration named would miss a flag added to the
    /// new name and still bound through the old one.
    fn declares(&self, id: PropId, flag: &str) -> bool {
        self.ids().any(|candidate| {
            self.means(self.get(candidate).key) == Some(id)
                && self.get(candidate).cli.contains(&flag)
        })
    }

    /// Which setting a key means, following renames — `None` for a key that is not one.
    fn means(&self, key: &str) -> Option<PropId> {
        self.lookup(key).map(|found| found.id)
    }

    /// Every setting bound to `kind`, with its key in that source.
    ///
    /// The generic mechanism a custom layer is written against: a git layer asks for `"git"`
    /// and reads the keys it gets back, without usage knowing anything about git.
    pub fn bindings(
        &self,
        kind: SourceKind,
    ) -> impl Iterator<Item = (PropId, &'static str)> + use<'_> {
        self.ids().flat_map(move |id| {
            self.get(id)
                .bindings
                .iter()
                .filter(move |(k, _)| *k == kind.name())
                .map(move |(_, key)| (id, *key))
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    static PROPS: &[PropMeta] = &[
        PropMeta {
            key: "jobs",
            aliases: &["parallelism"],
            envs: &["HK_JOBS", "HK_JOB"],
            cli: &["--jobs", "-j"],
            bindings: &[("git", "hk.jobs"), ("pkl", "jobs")],
            ..PropMeta::new("jobs", Ty::Uint)
        },
        PropMeta {
            renamed_to: Some("jobs"),
            deprecated: Some("Use jobs instead."),
            ..PropMeta::new("concurrency", Ty::Uint)
        },
        PropMeta {
            // Two renames deep: this is what a second release of renaming looks like.
            renamed_to: Some("concurrency"),
            ..PropMeta::new("threads", Ty::Uint)
        },
        PropMeta {
            // Declares a flag, which is what hk's dead `sources.cli` lines look like from here.
            cli: &["--check"],
            bindings: &[("git", "hk.check")],
            ..PropMeta::new("check", Ty::Bool)
        },
    ];
    const REGISTRY: Registry = Registry::new(PROPS);

    #[test]
    fn aliases_resolve_without_becoming_renames() {
        let found = REGISTRY.lookup("parallelism").expect("alias");
        assert_eq!(found.id, REGISTRY.lookup("jobs").unwrap().id);
        assert_eq!(found.written, "parallelism");
        assert_eq!(found.renamed_from, None);
        assert_eq!(REGISTRY.deprecation("parallelism"), None);
    }

    #[test]
    fn an_alias_on_a_renamed_prop_stays_warning_free_but_keeps_deprecation() {
        static RENAMED: &[PropMeta] = &[
            PropMeta::new("jobs", Ty::Uint),
            PropMeta {
                aliases: &["parallelism"],
                renamed_to: Some("jobs"),
                deprecated: Some("Use jobs instead."),
                ..PropMeta::new("concurrency", Ty::Uint)
            },
        ];
        let registry = Registry::new(RENAMED);
        let found = registry.lookup("parallelism").unwrap();
        assert_eq!(found.id, PropId(0));
        assert_eq!(found.renamed_from, None);
        assert_eq!(
            registry.deprecation("parallelism"),
            Some("Use jobs instead.")
        );
    }

    #[test]
    fn an_alias_prefix_does_not_swallow_a_nested_file_key() {
        const PREFIXED: &[PropMeta] = &[
            PropMeta {
                aliases: &["old.key"],
                ..PropMeta::new("replacement", Ty::Map(&Ty::String))
            },
            PropMeta::new("old.key.extra", Ty::String),
            PropMeta {
                aliases: &["whole.table"],
                ..PropMeta::new("map", Ty::Map(&Ty::String))
            },
        ];
        let registry = Registry::new(PREFIXED);
        assert!(!registry.names_file_value("old.key"));
        assert!(registry.names_file_value("old.key.extra"));
        assert!(registry.names_file_value("whole.table"));
    }

    #[test]
    fn a_canonical_table_value_is_not_turned_into_a_namespace_by_an_alias() {
        const PROPS: &[PropMeta] = &[
            PropMeta::new("providers", Ty::Map(&Ty::String)),
            PropMeta::new("optional", Ty::Option(&Ty::Map(&Ty::String))),
            PropMeta::new("union", Ty::Any),
            PropMeta {
                aliases: &["providers.legacy"],
                ..PropMeta::new("legacy_provider", Ty::String)
            },
            PropMeta {
                aliases: &["optional.legacy"],
                ..PropMeta::new("optional_legacy", Ty::String)
            },
            PropMeta {
                aliases: &["union.legacy"],
                ..PropMeta::new("union_legacy", Ty::String)
            },
        ];
        let registry = Registry::new(PROPS);
        assert!(registry.names_file_value("providers"));
        assert!(registry.names_file_value("optional"));
        assert!(registry.names_file_value("union"));
    }

    #[test]
    fn a_cli_that_binds_what_the_spec_declares_has_no_drift() {
        let bound = [("--jobs", "jobs"), ("-j", "jobs"), ("--check", "check")];
        assert_eq!(REGISTRY.drift(&bound), Vec::<String>::new());
    }

    #[test]
    fn a_declared_flag_nothing_binds_is_reported() {
        // hk's thirteen dead `sources.cli` lines, and the reason they lasted: the declaration is in a
        // spec, the reading is in a hand-written struct, and nothing compared them. A user reads
        // `--check` in the documentation and it does nothing at all.
        let bound = [("--jobs", "jobs"), ("-j", "jobs")];
        assert_eq!(
            REGISTRY.drift(&bound),
            vec!["`check` says `--check` sets it, and nothing does"]
        );

        // Every spelling counts. `-j` is as much a promise as `--jobs` is.
        let bound = [("--jobs", "jobs"), ("--check", "check")];
        assert_eq!(
            REGISTRY.drift(&bound),
            vec!["`jobs` says `-j` sets it, and nothing does"]
        );
    }

    #[test]
    fn a_flag_bound_to_the_wrong_setting_is_not_a_flag_that_is_bound() {
        // The failure a flag-only comparison would miss: `--check` is bound, so a check that only
        // asked "is this flag bound anywhere" would pass — while `check` is still set by nothing and
        // `jobs` is now set by a flag it never declared.
        let bound = [("--jobs", "jobs"), ("-j", "jobs"), ("--check", "jobs")];
        assert_eq!(
            REGISTRY.drift(&bound),
            vec![
                "`--check` is bound to `jobs`, which does not declare it: add `cli \"--check\"` to the spec",
                "`check` says `--check` sets it, and nothing does"
            ]
        );
    }

    #[test]
    fn a_flag_bound_to_a_setting_that_does_not_exist_is_reported() {
        let bound = [
            ("--jobs", "jobs"),
            ("-j", "jobs"),
            ("--check", "check"),
            ("--nonesuch", "nonesuch"),
        ];
        assert_eq!(
            REGISTRY.drift(&bound),
            vec!["`--nonesuch` is bound to `nonesuch`, which is not a setting"]
        );
    }

    #[test]
    fn a_flag_bound_through_an_old_name_is_bound() {
        // A CLI written before a rename binds the name it knew. The value lands on the setting that
        // replaced it, so the binding is real — reporting it as drift would make living through a
        // rename impossible.
        let bound = [("--jobs", "jobs"), ("-j", "jobs"), ("--check", "check")];
        assert_eq!(REGISTRY.drift(&bound), Vec::<String>::new());
        let through_old_name = [
            ("--jobs", "concurrency"),
            ("-j", "jobs"),
            ("--check", "check"),
        ];
        assert_eq!(
            REGISTRY.drift(&through_old_name),
            Vec::<String>::new(),
            "`concurrency` is `jobs`, and `jobs` declares `--jobs`"
        );
    }

    // An old name that kept the flag it was documented with — what a rename looks like for a CLI
    // that has not dropped the old spelling yet. Its own registry, because a declared flag is a
    // promise: adding it to the shared one would make every other test's bindings incomplete.
    static RENAMED_PROPS: &[PropMeta] = &[
        PropMeta {
            cli: &["--jobs"],
            ..PropMeta::new("jobs", Ty::Uint)
        },
        PropMeta {
            cli: &["--concurrency"],
            renamed_to: Some("jobs"),
            ..PropMeta::new("concurrency", Ty::Uint)
        },
    ];
    const RENAMED: Registry = Registry::new(RENAMED_PROPS);

    #[test]
    fn an_old_name_that_kept_its_flag_is_not_drift() {
        // The two questions a rename separates. `--concurrency` is declared on the old name and
        // binds the old key, so both sides are talking about `jobs` — but one asked `lookup` and the
        // other did not, and the disagreement was reported twice: the flag "is not declared" by the
        // replacement, and the declaration's flag "nothing does". A CLI would have deleted a live
        // binding to satisfy it.
        let bound = [("--jobs", "jobs"), ("--concurrency", "concurrency")];
        assert_eq!(RENAMED.drift(&bound), Vec::<String>::new());

        // And bound through the name that replaced it, which is the same setting and the same flag.
        let by_new_name = [("--jobs", "jobs"), ("--concurrency", "jobs")];
        assert_eq!(RENAMED.drift(&by_new_name), Vec::<String>::new());
    }

    #[test]
    fn a_flag_on_a_rename_that_leads_nowhere_is_not_satisfied_by_another_broken_one() {
        // `means` is `None` for a key whose rename names nothing or loops. Compared to each other,
        // two of those were equal — so a declared flag counted as bound because some *other* dead
        // key happened to be bound to the same spelling, and the dead declaration went unreported.
        // The generator refuses a registry like this, but `drift` is also what a hand-written one
        // is held to.
        static BROKEN_PROPS: &[PropMeta] = &[
            PropMeta {
                cli: &["--gone"],
                renamed_to: Some("nowhere"),
                ..PropMeta::new("gone", Ty::Uint)
            },
            PropMeta {
                cli: &["--gone"],
                renamed_to: Some("also-nowhere"),
                ..PropMeta::new("other", Ty::Uint)
            },
        ];
        const BROKEN: Registry = Registry::new(BROKEN_PROPS);

        let bound = [("--gone", "other")];
        let problems = BROKEN.drift(&bound);
        assert_eq!(
            problems,
            vec![
                "`--gone` is bound to `other`, which is not a setting",
                "`gone` says `--gone` sets it, and it is not a setting anything can reach: its \
                 `renamed_to` names nothing, or the chain it starts loops",
                "`other` says `--gone` sets it, and it is not a setting anything can reach: its \
                 `renamed_to` names nothing, or the chain it starts loops",
            ]
        );
    }

    #[test]
    fn a_key_resolves_to_its_own_index() {
        let found = REGISTRY.lookup("jobs").expect("declared");
        assert_eq!(found.id, PropId(0));
        assert_eq!(found.renamed_from, None);
        assert_eq!(REGISTRY.get(found.id).key, "jobs");
        assert_eq!(REGISTRY.lookup("nonesuch"), None);
    }

    #[test]
    fn an_old_name_resolves_to_the_setting_that_replaced_it() {
        // What a config file written a year ago needs, and the reason a rename does not have
        // to be a breaking change.
        let found = REGISTRY.lookup("concurrency").expect("declared");
        assert_eq!(found.id, PropId(0), "should land on jobs");
        assert_eq!(found.renamed_from, Some("concurrency"));

        // Through two renames, reporting the name the user actually wrote rather than the
        // intermediate one they have never heard of.
        let chained = REGISTRY.lookup("threads").expect("declared");
        assert_eq!(chained.id, PropId(0));
        assert_eq!(chained.renamed_from, Some("threads"));
    }

    #[test]
    fn a_rename_cycle_fails_the_lookup_rather_than_the_process() {
        // One mistyped field in a registry somebody wrote by hand. Recursing on `renamed_to`
        // overflowed the stack, which is an abort with no message — a lookup that cannot answer
        // should return `None` and let the caller warn about an unknown key.
        static CYCLE: &[PropMeta] = &[
            PropMeta {
                renamed_to: Some("b"),
                ..PropMeta::new("a", Ty::Bool)
            },
            PropMeta {
                renamed_to: Some("a"),
                ..PropMeta::new("b", Ty::Bool)
            },
            // The simplest form: a setting renamed to itself.
            PropMeta {
                renamed_to: Some("self"),
                ..PropMeta::new("self", Ty::Bool)
            },
        ];
        const REGISTRY: Registry = Registry::new(CYCLE);
        assert_eq!(REGISTRY.lookup("a"), None);
        assert_eq!(REGISTRY.lookup("b"), None);
        assert_eq!(REGISTRY.lookup("self"), None);
        // And a chain that does end still resolves, so the bound is not simply refusing chains.
        static CHAIN: &[PropMeta] = &[
            PropMeta::new("new", Ty::Bool),
            PropMeta {
                renamed_to: Some("new"),
                ..PropMeta::new("middle", Ty::Bool)
            },
            PropMeta {
                renamed_to: Some("middle"),
                ..PropMeta::new("old", Ty::Bool)
            },
        ];
        const CHAINED: Registry = Registry::new(CHAIN);
        let found = CHAINED.lookup("old").expect("declared");
        assert_eq!(found.id, PropId(0));
        assert_eq!(found.renamed_from, Some("old"));
    }

    #[test]
    fn a_custom_layer_finds_its_own_keys() {
        // The whole interface a git or pkl or npmrc layer is written against.
        let git: Vec<_> = REGISTRY.bindings(SourceKind::new("git")).collect();
        assert_eq!(git, vec![(PropId(0), "hk.jobs"), (PropId(3), "hk.check")]);
        let pkl: Vec<_> = REGISTRY.bindings(SourceKind::new("pkl")).collect();
        assert_eq!(pkl, vec![(PropId(0), "jobs")]);
        // A kind nothing is bound to yields nothing rather than everything.
        assert_eq!(REGISTRY.bindings(SourceKind::new("npmrc")).count(), 0);
    }
}
