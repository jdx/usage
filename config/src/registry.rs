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
            bindings: &[],
            choices: &[],
            hide: false,
            deprecated: None,
            renamed_to: None,
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
        let (index, meta) = self
            .props
            .iter()
            .enumerate()
            .find(|(_, meta)| meta.key == key)?;
        let mut id = PropId(index as u16);
        let written = meta.key;
        // A chain of renames resolves to its end, so two releases of renaming do not leave the
        // second one unreachable — walked rather than recursed, and bounded by the number of
        // settings there are. A cycle, which is one mistyped field away in a registry somebody
        // wrote by hand, overflowed the stack: an abort with no message rather than a lookup
        // that fails. A chain longer than the registry is a cycle by definition.
        for _ in 0..self.props.len() {
            let Some(new_key) = self.props[id.index()].renamed_to else {
                return Some(Lookup {
                    id,
                    renamed_from: (id.index() != index).then_some(written),
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

    /// The settings an environment variable sets, and the variable that set them.
    ///
    /// Several names per setting are aliases in descending precedence, which the env layer
    /// honours by taking the first one that is present.
    pub fn ids(&self) -> impl Iterator<Item = PropId> {
        (0..self.props.len()).map(|i| PropId(i as u16))
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
            envs: &["HK_JOBS", "HK_JOB"],
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
            bindings: &[("git", "hk.check")],
            ..PropMeta::new("check", Ty::Bool)
        },
    ];
    const REGISTRY: Registry = Registry::new(PROPS);

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
