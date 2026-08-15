//! One merge, and the provenance is its output.
//!
//! Precedence is fixed and universal: the command line beats the environment, which beats
//! files, nearest first, which beat the user's own configuration, which beats the machine's,
//! which beat the declared defaults. *Which* layers a CLI has is its own business; their
//! relative order is not negotiable, because a fleet where two CLIs disagree about whether
//! `--jobs` beats `JOBS` is the thing this crate exists to end.
//!
//! Layers are given highest precedence first — the order they read in the builder and the
//! order `--help` describes them — and folded lowest first, so the last writer wins.

use std::collections::BTreeMap;

use crate::layer::{Layer, LayerCtx, LayerError, Warning, WarningKind};
use crate::registry::{Merge, PropId, Registry, Scope};
use crate::source::{Origin, SourceKind, Trust};
use crate::value::Value;

/// Everything a resolution produced.
#[derive(Debug, Clone)]
pub struct Resolved {
    /// Indexed by [`PropId`]: the winning value, or `None` where nothing supplied one and no
    /// default was declared.
    values: Vec<Option<Value>>,
    /// Indexed by [`PropId`], alongside the values so the two cannot come apart.
    provenance: Vec<Option<Origin>>,
    /// Contributors, in the order they were merged, for a setting that took several.
    contributors: BTreeMap<PropId, Vec<Origin>>,
    /// Everything a user should be told, in the order it was found.
    pub warnings: Vec<Warning>,
    registry: Registry,
}

impl Resolved {
    /// The winning value for a setting.
    pub fn get(&self, id: PropId) -> Option<&Value> {
        self.values.get(id.index()).and_then(Option::as_ref)
    }

    /// The winning value for a dotted key, following renames.
    pub fn get_key(&self, key: &str) -> Option<&Value> {
        self.get(self.registry.lookup(key)?.id)
    }

    /// Where the winning value came from.
    pub fn origin(&self, id: PropId) -> Option<&Origin> {
        self.provenance.get(id.index()).and_then(Option::as_ref)
    }

    /// Every place that contributed to this setting, in merge order.
    ///
    /// One entry for a `replace` setting, several for a `union` or `deep` one — which is
    /// what makes per-item provenance possible for a list assembled from four files.
    pub fn contributors(&self, id: PropId) -> &[Origin] {
        self.contributors
            .get(&id)
            .map(Vec::as_slice)
            .unwrap_or_default()
    }

    pub fn registry(&self) -> Registry {
        self.registry
    }

    /// Record that the CLI rewrote a value after merging.
    ///
    /// The typed post-merge hook is where a CLI's own rules live — mise's `raw` implying
    /// `jobs = 1`, its `ci` implying `yes`. Going through here rather than assigning to the
    /// struct keeps `explain` honest: the origin becomes [`SourceKind::COERCED`] with the
    /// reason, instead of continuing to name a file that never said it.
    pub fn coerced(&mut self, id: PropId, value: Value, why: impl Into<String>) {
        let index = id.index();
        if index >= self.values.len() {
            return;
        }
        let origin = Origin::new(SourceKind::COERCED, why);
        self.values[index] = Some(value);
        self.provenance[index] = Some(origin.clone());
        // On the contributor list too, or `origin()` would name the rewrite while
        // `contributors().last()` still named whatever the rewrite replaced — the same split
        // between the two that the merge itself is written to avoid.
        self.contributors.entry(id).or_default().push(origin);
    }
}

/// The layers to resolve, highest precedence first.
///
/// Ordered by the caller because only the caller knows which layers it has; the order they
/// are added in is the order `--help` and the docs describe, so a builder that read
/// bottom-up would invite exactly the kind of quiet disagreement this replaces.
#[derive(Default)]
pub struct Layers<'a> {
    layers: Vec<&'a dyn Layer>,
}

impl<'a> Layers<'a> {
    pub fn new() -> Self {
        Self::default()
    }

    /// Add a layer below every layer added so far.
    pub fn then(mut self, layer: &'a dyn Layer) -> Self {
        self.layers.push(layer);
        self
    }

    pub fn len(&self) -> usize {
        self.layers.len()
    }

    pub fn is_empty(&self) -> bool {
        self.layers.is_empty()
    }
}

/// Resolve every setting in `registry` from `layers`.
///
/// Declared defaults are the bottom layer always, and are not a [`Layer`]: they cost one
/// `const` conversion per setting that needs one and cannot fail, so making them an
/// implementation would be ceremony that could also be forgotten.
pub fn resolve(registry: Registry, layers: Layers<'_>) -> Result<Resolved, LayerError> {
    let ctx = LayerCtx::new(registry);
    let count = registry.props.len();
    let mut resolved = Resolved {
        values: vec![None; count],
        provenance: vec![None; count],
        contributors: BTreeMap::new(),
        warnings: Vec::new(),
        registry,
    };

    // Declared defaults are the bottom layer, seeded before anything else rather than applied
    // afterwards as a floor. As a floor they could not take part in a merge at all: a `union`
    // list with a declared default and any layer at all lost the default's items, because the
    // floor only filled in what nothing had set. Being the lowest contributor is also what a
    // default *is*, so `explain` now says so.
    for id in registry.ids() {
        // An old name is an alias, not a setting: seeding its default under its own id put the
        // value somewhere no reader looks, since every lookup folds to the replacement. The
        // setting that replaced it declares its own default.
        if registry.get(id).renamed_to.is_some() {
            continue;
        }
        if let Some(default) = registry.get(id).default {
            let index = id.index();
            resolved.values[index] = Some(default.to_value());
            resolved.provenance[index] = Some(Origin::declared_default());
            resolved
                .contributors
                .entry(id)
                .or_default()
                .push(Origin::declared_default());
        }
    }

    // Lowest precedence first, so a higher layer overwrites what a lower one put there.
    // Loaded in this order too, which means a layer's warnings arrive in the order a reader
    // would look for them.
    let mut outputs = Vec::with_capacity(layers.len());
    for layer in layers.layers.iter().rev() {
        outputs.push(layer.load(&ctx)?);
    }

    for output in outputs {
        resolved.warnings.extend(output.warnings);
        for entry in output.entries {
            let written = registry.get(entry.prop);
            // Follow a rename here, not only in `LayerCtx::prop`: a layer that took its ids
            // from `Registry::bindings` or `ids` supplies the old prop's own id, and storing
            // the value there left it somewhere `get_key` — which follows the rename — would
            // never look, so the value was silently dropped.
            let (prop, meta) = match written
                .renamed_to
                .and_then(|new_key| registry.lookup(new_key))
            {
                Some(target) => (target.id, registry.get(target.id)),
                None => (entry.prop, written),
            };
            // Whichever way the old name arrived: on the entry, because the layer looked the
            // key up and `LayerCtx` folded it, or as a raw id this loop folded just now.
            // Keyed on the fold alone, a file layer's deprecated key was folded in silence.
            let written_key = entry.renamed_from.unwrap_or(written.key);
            if let Some(refusal) = refuse(meta.scope, &entry.origin) {
                // `written_key`, like the two warnings below it: after `LayerCtx` folds a
                // rename, `written.key` is the *replacement's* name, so a refused value was
                // reported under a key that does not appear in the file the user would go and
                // edit.
                resolved.warnings.push(
                    Warning::at(format!("{written_key} {refusal}"), entry.origin)
                        .of(WarningKind::OutOfScope),
                );
                continue;
            }
            // Along the chain rather than off the declaration written, which is what `explain` has
            // always done: a notice can sit on a name further along, and reading only the one the
            // user wrote meant `config explain` told them to stop using a key that running the CLI
            // said nothing about.
            if let Some(why) = registry.deprecation(written_key) {
                resolved.warnings.push(
                    Warning::at(
                        format!("{written_key} is deprecated: {why}"),
                        entry.origin.clone(),
                    )
                    .of(WarningKind::Deprecated),
                );
            }
            if written_key != meta.key {
                // Both names: the key the user wrote, and the one it was read as.
                resolved.warnings.push(
                    Warning::at(
                        format!("{written_key} was read as {}", meta.key),
                        entry.origin.clone(),
                    )
                    .of(WarningKind::Renamed),
                );
            }
            let index = prop.index();
            let merged = match meta.merge {
                Merge::Replace => entry.value,
                // Through `union` even for the first contribution, so a set's deduplication
                // applies to one layer's list as well as across two — a single `TAGS=a,b,a`
                // kept its repeat, because dedup lived only on the merge-two path.
                Merge::Union => union(
                    resolved.values[index]
                        .take()
                        .unwrap_or(Value::List(Vec::new())),
                    entry.value,
                    meta.ty,
                ),
                Merge::Deep => match resolved.values[index].take() {
                    Some(existing) => deep(existing, entry.value),
                    None => entry.value,
                },
            };
            resolved.values[index] = Some(merged);
            // The winner is whatever came last, which after the reverse above is the
            // highest-precedence contributor.
            resolved.provenance[index] = Some(entry.origin.clone());
            // Keyed by the folded id, like the value and the winning origin beside it. Keyed
            // by the id the layer supplied, a renamed setting's contributors ended up on a
            // prop nothing reads while its value and origin were on another — the provenance
            // split this crate exists to make unreachable, reintroduced by two lines.
            resolved
                .contributors
                .entry(prop)
                .or_default()
                .push(entry.origin);
        }
    }

    Ok(resolved)
}

/// Why this scope will not take a value from this origin, if it will not.
///
/// Enforced here rather than in each layer: mise calls this a security property, and a check
/// that every layer has to remember to make is one a new layer will forget.
fn refuse(scope: Scope, origin: &Origin) -> Option<&'static str> {
    match scope {
        Scope::Any => None,
        // Anything a repository can carry, whatever kind of place it is. Asking whether the
        // origin was a *file* let a pkl file, a git config or an `.npmrc` in the checkout walk
        // past a check the spec calls a security property.
        // Not "config file": since the check became one about trust, this refuses a git
        // config, a pkl file or an `.npmrc` in the checkout too, and telling that user their
        // *config file* is at fault points them at a file that never held the value. The
        // warning carries the origin, so whoever renders it can name the place exactly.
        Scope::Global if origin.trust < Trust::Operator => {
            Some("cannot be set by anything a project can carry")
        }
        Scope::Env if origin.trust < Trust::Invocation => {
            Some("can only be set in the environment or on the command line")
        }
        _ => None,
    }
}

/// Lower-precedence values first, higher appended, repeats dropped for a set.
fn union(existing: Value, incoming: Value, ty: crate::ty::Ty) -> Value {
    // An explicit empty list means "none", and is how a user turns a declared default off:
    // `HK_EXCLUDE=` parses to an empty list for exactly that reason. Concatenating with it
    // left every default item in place, so a `union` setting with a default could not be
    // cleared at all.
    if matches!(&incoming, Value::List(items) if items.is_empty()) {
        return Value::List(Vec::new());
    }
    let mut items = match existing {
        Value::List(items) => items,
        single => vec![single],
    };
    match incoming {
        Value::List(more) => items.extend(more),
        single => items.push(single),
    }
    if matches!(ty.inner(), crate::ty::Ty::Set(_)) {
        // First occurrence keeps its position, so the order of a set is the order it was
        // first mentioned rather than something that shifts when a lower layer changes.
        let mut seen: Vec<Value> = Vec::with_capacity(items.len());
        items.retain(|item| {
            let fresh = !seen.contains(item);
            if fresh {
                seen.push(item.clone());
            }
            fresh
        });
    }
    Value::List(items)
}

/// Tables merged key by key, the incoming (higher-precedence) side winning each key.
fn deep(existing: Value, incoming: Value) -> Value {
    match (existing, incoming) {
        (Value::Map(mut base), Value::Map(overlay)) => {
            for (key, value) in overlay {
                let merged = match base.remove(&key) {
                    // Nested tables merge too, so a `deep` setting is deep all the way down
                    // rather than only at the top.
                    Some(existing @ Value::Map(_)) => deep(existing, value),
                    _ => value,
                };
                base.insert(key, merged);
            }
            Value::Map(base)
        }
        // A `deep` setting given something that is not a table on either side has nothing to
        // merge; the higher-precedence value stands, as `replace` would have it.
        (_, incoming) => incoming,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::layer::{Entry, LayerOutput};
    use crate::registry::PropMeta;
    use crate::source::{FileScope, Trust};
    use crate::ty::Ty;
    use crate::value::Const;

    static PROPS: &[PropMeta] = &[
        PropMeta {
            default: Some(Const::Int(4)),
            ..PropMeta::new("jobs", Ty::Uint)
        },
        PropMeta {
            merge: Merge::Union,
            ..PropMeta::new("exclude", Ty::List(&Ty::String))
        },
        PropMeta {
            merge: Merge::Union,
            ..PropMeta::new("tags", Ty::Set(&Ty::String))
        },
        PropMeta {
            merge: Merge::Union,
            default: Some(Const::List(&[Const::Str("target")])),
            ..PropMeta::new("excluded", Ty::List(&Ty::String))
        },
        PropMeta {
            merge: Merge::Deep,
            ..PropMeta::new("urls", Ty::Map(&Ty::String))
        },
        PropMeta {
            scope: Scope::Global,
            ..PropMeta::new("trusted", Ty::Bool)
        },
        // An old name for a scope-restricted setting, which is how a refusal comes to be
        // reported under a key the user never wrote.
        PropMeta {
            renamed_to: Some("trusted"),
            ..PropMeta::new("old_trusted", Ty::Bool)
        },
        PropMeta {
            scope: Scope::Env,
            ..PropMeta::new("config_file", Ty::Path)
        },
        PropMeta {
            deprecated: Some("Use jobs instead."),
            ..PropMeta::new("old_jobs", Ty::Uint)
        },
        // Deprecated *and* replaced, which is the pair a rename actually comes as.
        PropMeta {
            deprecated: Some("Use jobs instead."),
            renamed_to: Some("jobs"),
            // A default on an alias, which is a thing a registry ends up with after a rename
            // and which must not be seeded anywhere.
            default: Some(Const::Int(7)),
            ..PropMeta::new("renamed_jobs", Ty::Uint)
        },
        PropMeta::new("undeclared_default", Ty::String),
    ];
    const REGISTRY: Registry = Registry::new(PROPS);

    #[test]
    fn every_warning_says_what_sort_of_thing_it_is() {
        // The message is for a person and its wording is nobody's contract. This is what a program
        // acts on: mise queues its deprecations until logging is up while a bad value goes to stderr
        // at once, a `--strict` mode exits on everything but a deprecation, and the conformance
        // corpus pins what happened without pinning how it was said.
        let ctx = LayerCtx::new(REGISTRY);
        let file = Origin::file("hk.toml", FileScope::Project);

        // Every kind this crate produces, from the place that produces it.
        let unknown = ctx
            .entry_for_key("nonesuch", "1", file.clone())
            .expect_err("no such setting");
        assert_eq!(unknown.kind, WarningKind::UnknownSetting);
        let wrong = ctx
            .entry_for_key("jobs", "lots", file.clone())
            .expect_err("not a number");
        assert_eq!(wrong.kind, WarningKind::WrongType);
        let shaped = ctx
            .entry_from_value("jobs", Value::from("lots"), file.clone())
            .expect_err("still not a number");
        assert_eq!(shaped.kind, WarningKind::WrongType);

        // And the three the *merge* adds, which no layer can know about on its own: whether a place
        // is allowed to set a setting, and what its name turned out to mean.
        let layer = Fixed {
            kind: SourceKind::FILE,
            entries: vec![
                Entry::new(id("trusted"), Value::Bool(true), file.clone()),
                // The *unfolded* id, which is what a layer reading `Registry::ids` hands over —
                // `lookup` folds a rename, so going through it could not reproduce the case.
                Entry::new(raw_id("renamed_jobs"), Value::Int(8), file),
            ],
        };
        let resolved = resolve(REGISTRY, Layers::new().then(&layer)).expect("resolves");
        let kinds: Vec<WarningKind> = resolved.warnings.iter().map(|w| w.kind).collect();
        assert_eq!(
            kinds,
            vec![
                WarningKind::OutOfScope,
                WarningKind::Deprecated,
                WarningKind::Renamed
            ],
            "{:?}",
            resolved.warnings
        );

        // A layer of the CLI's own says whatever it likes, and is not made to invent a kind for it.
        assert_eq!(
            Warning::new("the git config could not be read").kind,
            WarningKind::Other
        );
    }

    /// A layer holding whatever a test hands it.
    struct Fixed {
        kind: SourceKind,
        entries: Vec<Entry>,
    }

    impl Layer for Fixed {
        fn source(&self) -> SourceKind {
            self.kind
        }

        fn load(&self, _ctx: &LayerCtx) -> Result<LayerOutput, LayerError> {
            Ok(LayerOutput {
                entries: self.entries.clone(),
                warnings: Vec::new(),
            })
        }
    }

    fn id(key: &str) -> PropId {
        REGISTRY.lookup(key).expect("declared").id
    }

    /// The id of a key *without* following its rename.
    ///
    /// What `Registry::bindings` and `Registry::ids` hand a layer — `lookup` folds renames, so
    /// a test that went through it could not reproduce the case at all.
    fn raw_id(key: &str) -> PropId {
        let index = PROPS
            .iter()
            .position(|meta| meta.key == key)
            .expect("declared");
        PropId(index as u16)
    }

    fn layer(kind: SourceKind, entries: Vec<(&str, Value, Origin)>) -> Fixed {
        Fixed {
            kind,
            entries: entries
                .into_iter()
                .map(|(key, value, origin)| Entry::new(id(key), value, origin))
                .collect(),
        }
    }

    #[test]
    fn the_highest_layer_wins_and_says_so() {
        let cli = layer(
            SourceKind::CLI,
            vec![(
                "jobs",
                Value::Int(1),
                Origin::new(SourceKind::CLI, "--jobs"),
            )],
        );
        let env = layer(
            SourceKind::ENV,
            vec![(
                "jobs",
                Value::Int(2),
                Origin::new(SourceKind::ENV, "HK_JOBS"),
            )],
        );
        let file = layer(
            SourceKind::FILE,
            vec![(
                "jobs",
                Value::Int(3),
                Origin::file("hk.toml", FileScope::Project),
            )],
        );

        let resolved = resolve(REGISTRY, Layers::new().then(&cli).then(&env).then(&file))
            .expect("should resolve");

        assert_eq!(resolved.get_key("jobs"), Some(&Value::Int(1)));
        // The identifier, not just the kind: "from the environment" is not something a user
        // can act on and `--jobs` is.
        assert_eq!(resolved.origin(id("jobs")).unwrap().describe(), "--jobs");
        // Every contributor is kept, lowest precedence first, even for a `replace` setting —
        // and the declared default is the lowest of all, because that is what a default is.
        let contributors: Vec<_> = resolved
            .contributors(id("jobs"))
            .iter()
            .map(|o| o.describe().to_string())
            .collect();
        assert_eq!(
            contributors,
            ["the default", "hk.toml", "HK_JOBS", "--jobs"]
        );
    }

    #[test]
    fn a_declared_default_is_the_floor_and_is_marked_as_one() {
        let resolved = resolve(REGISTRY, Layers::new()).expect("should resolve");
        assert_eq!(resolved.get_key("jobs"), Some(&Value::Int(4)));
        assert_eq!(
            resolved.origin(id("jobs")).unwrap().kind,
            SourceKind::DEFAULTS
        );
        // A setting with no default and no value is absent rather than guessed at, which is
        // what makes `option<T>` expressible.
        assert_eq!(resolved.get_key("undeclared_default"), None);
        assert_eq!(resolved.origin(id("undeclared_default")), None);
    }

    #[test]
    fn a_union_setting_takes_from_every_layer_lowest_first() {
        let env = layer(
            SourceKind::ENV,
            vec![(
                "exclude",
                Value::List(vec![Value::from("target")]),
                Origin::new(SourceKind::ENV, "HK_EXCLUDE"),
            )],
        );
        let file = layer(
            SourceKind::FILE,
            vec![(
                "exclude",
                Value::List(vec![Value::from("vendor")]),
                Origin::file("hk.toml", FileScope::Project),
            )],
        );
        let resolved =
            resolve(REGISTRY, Layers::new().then(&env).then(&file)).expect("should resolve");
        assert_eq!(
            resolved.get_key("exclude"),
            Some(&Value::List(vec![
                Value::from("vendor"),
                Value::from("target")
            ])),
            "lower precedence first, so the most specific reads last"
        );
        // Both places are recorded, which is what per-item provenance is built on.
        assert_eq!(resolved.contributors(id("exclude")).len(), 2);
    }

    #[test]
    fn a_set_keeps_the_first_of_each() {
        let a = layer(
            SourceKind::ENV,
            vec![(
                "tags",
                Value::List(vec![Value::from("x"), Value::from("y")]),
                Origin::new(SourceKind::ENV, "TAGS"),
            )],
        );
        let b = layer(
            SourceKind::FILE,
            vec![(
                "tags",
                Value::List(vec![Value::from("y"), Value::from("z")]),
                Origin::file("hk.toml", FileScope::Project),
            )],
        );
        let resolved = resolve(REGISTRY, Layers::new().then(&a).then(&b)).expect("should resolve");
        assert_eq!(
            resolved.get_key("tags"),
            Some(&Value::List(vec![
                Value::from("y"),
                Value::from("z"),
                Value::from("x")
            ])),
            "y was first mentioned by the file, so it stays where it was"
        );
    }

    #[test]
    fn a_deep_setting_merges_tables_key_by_key() {
        let map = |pairs: &[(&str, &str)]| {
            Value::Map(
                pairs
                    .iter()
                    .map(|(k, v)| ((*k).to_string(), Value::from(*v)))
                    .collect(),
            )
        };
        let env = layer(
            SourceKind::ENV,
            vec![(
                "urls",
                map(&[("a", "from-env")]),
                Origin::new(SourceKind::ENV, "URLS"),
            )],
        );
        let file = layer(
            SourceKind::FILE,
            vec![(
                "urls",
                map(&[("a", "from-file"), ("b", "only-in-file")]),
                Origin::file("hk.toml", FileScope::Project),
            )],
        );
        let resolved =
            resolve(REGISTRY, Layers::new().then(&env).then(&file)).expect("should resolve");
        assert_eq!(
            resolved.get_key("urls"),
            Some(&map(&[("a", "from-env"), ("b", "only-in-file")])),
            "the higher layer wins its own key without dropping the other's"
        );
    }

    #[test]
    fn a_scope_refuses_what_it_says_it_refuses() {
        // The security property: a repository can carry a project file, so a setting that
        // must not be changeable by a checkout says so and the merge enforces it — not each
        // layer, which is how a new layer forgets.
        let project = layer(
            SourceKind::FILE,
            vec![
                (
                    "trusted",
                    Value::Bool(true),
                    Origin::file("hk.toml", FileScope::Project),
                ),
                (
                    "config_file",
                    Value::from("/tmp/x"),
                    Origin::file("hk.toml", FileScope::Project),
                ),
            ],
        );
        let resolved = resolve(REGISTRY, Layers::new().then(&project)).expect("should resolve");
        assert_eq!(resolved.get_key("trusted"), None);
        assert_eq!(resolved.get_key("config_file"), None);
        // Refused out loud: silently ignoring what somebody wrote is how they conclude the
        // setting does not work.
        let messages: Vec<_> = resolved
            .warnings
            .iter()
            .map(|w| w.message.clone())
            .collect();
        assert_eq!(
            messages,
            [
                "trusted cannot be set by anything a project can carry",
                "config_file can only be set in the environment or on the command line",
            ]
        );

        // The same settings from the places they *do* accept.
        let global = layer(
            SourceKind::FILE,
            vec![(
                "trusted",
                Value::Bool(true),
                Origin::file("~/.config/hk.toml", FileScope::Global),
            )],
        );
        let env = layer(
            SourceKind::ENV,
            vec![(
                "config_file",
                Value::from("/tmp/x"),
                Origin::new(SourceKind::ENV, "HK_CONFIG_FILE"),
            )],
        );
        let resolved =
            resolve(REGISTRY, Layers::new().then(&env).then(&global)).expect("should resolve");
        assert_eq!(resolved.get_key("trusted"), Some(&Value::Bool(true)));
        assert_eq!(
            resolved.get_key("config_file"),
            Some(&Value::from("/tmp/x"))
        );
        assert!(resolved.warnings.is_empty(), "{:?}", resolved.warnings);
    }

    #[test]
    fn using_a_deprecated_setting_says_so_once_per_place_it_was_set() {
        let file = layer(
            SourceKind::FILE,
            vec![(
                "old_jobs",
                Value::Int(2),
                Origin::file("hk.toml", FileScope::Project),
            )],
        );
        let resolved = resolve(REGISTRY, Layers::new().then(&file)).expect("should resolve");
        // Still honoured — a warning is not a refusal.
        assert_eq!(resolved.get_key("old_jobs"), Some(&Value::Int(2)));
        assert_eq!(
            resolved.warnings[0].message,
            "old_jobs is deprecated: Use jobs instead."
        );
        assert_eq!(
            resolved.warnings[0].origin.as_ref().unwrap().describe(),
            "hk.toml"
        );
    }

    #[test]
    fn a_value_the_cli_rewrote_says_it_was_rewritten() {
        // mise's `raw` implying `jobs = 1`. Recording this as coming from wherever the
        // original value came from is how a user ends up editing a file that has nothing to
        // do with what they are seeing.
        let env = layer(
            SourceKind::ENV,
            vec![(
                "jobs",
                Value::Int(8),
                Origin::new(SourceKind::ENV, "HK_JOBS"),
            )],
        );
        let mut resolved = resolve(REGISTRY, Layers::new().then(&env)).expect("should resolve");
        resolved.coerced(id("jobs"), Value::Int(1), "raw implies one job");
        assert_eq!(resolved.get_key("jobs"), Some(&Value::Int(1)));
        let origin = resolved.origin(id("jobs")).unwrap();
        assert_eq!(origin.kind, SourceKind::COERCED);
        assert_eq!(origin.describe(), "raw implies one job");
    }

    #[test]
    fn a_refusal_names_the_key_that_was_written() {
        // The deprecation and rename warnings already said the name the user wrote; the refusal
        // still said the folded one, so a refused value was reported under a key that does not
        // appear anywhere in the file they would go and edit.
        struct FileLike;
        impl Layer for FileLike {
            fn source(&self) -> SourceKind {
                SourceKind::FILE
            }
            fn load(&self, ctx: &LayerCtx) -> Result<LayerOutput, LayerError> {
                let mut out = LayerOutput::new();
                let origin = Origin::file("hk.toml", FileScope::Project);
                match ctx.entry_for_key("old_trusted", "true", origin) {
                    Ok(entry) => out.push(entry),
                    Err(warning) => out.warn(warning),
                }
                Ok(out)
            }
        }
        let file = FileLike;
        let resolved = resolve(REGISTRY, Layers::new().then(&file)).expect("should resolve");
        assert_eq!(resolved.get_key("trusted"), None);
        let messages: Vec<_> = resolved
            .warnings
            .iter()
            .map(|w| w.message.clone())
            .collect();
        assert!(
            messages
                .iter()
                .any(|m| m.starts_with("old_trusted cannot be set")),
            "the refusal should name the key in the file: {messages:?}"
        );
    }

    #[test]
    fn a_refused_custom_source_is_not_called_a_config_file() {
        // The check is about trust now, so it refuses a pkl file or a git config in the checkout
        // too — and telling that user their *config file* is at fault points them at a file that
        // never held the value.
        let pkl = layer(
            SourceKind::new("pkl"),
            vec![(
                "trusted",
                Value::Bool(true),
                Origin::new(SourceKind::new("pkl"), "hk.pkl"),
            )],
        );
        let resolved = resolve(REGISTRY, Layers::new().then(&pkl)).expect("should resolve");
        assert_eq!(
            resolved.warnings[0].message,
            "trusted cannot be set by anything a project can carry"
        );
        // And the origin travels with it, so a renderer can name the place exactly.
        assert_eq!(
            resolved.warnings[0].origin.as_ref().unwrap().describe(),
            "hk.pkl"
        );
    }

    #[test]
    fn a_custom_source_is_held_to_the_same_scope_as_a_file() {
        // The hole: `refuse` asked whether the origin was a *file*, and every custom source is
        // built with `Origin::new`, so a pkl file or a git config in the checkout could set a
        // setting the spec says a project must not touch. A pkl file in a repository is as much
        // a thing a checkout carries as `hk.toml` is.
        let pkl = layer(
            SourceKind::new("pkl"),
            vec![
                (
                    "trusted",
                    Value::Bool(true),
                    Origin::new(SourceKind::new("pkl"), "hk.pkl"),
                ),
                (
                    "config_file",
                    Value::from("/tmp/x"),
                    Origin::new(SourceKind::new("pkl"), "hk.pkl"),
                ),
            ],
        );
        let resolved = resolve(REGISTRY, Layers::new().then(&pkl)).expect("should resolve");
        assert_eq!(resolved.get_key("trusted"), None);
        assert_eq!(resolved.get_key("config_file"), None);
        assert_eq!(resolved.warnings.len(), 2, "{:?}", resolved.warnings);

        // And a layer that knows it read from the user's own configuration says so, at which
        // point a `global` setting will take it — while an `env` one still will not.
        let global_pkl = layer(
            SourceKind::new("pkl"),
            vec![
                (
                    "trusted",
                    Value::Bool(true),
                    Origin::new(SourceKind::new("pkl"), "~/.config/hk.pkl")
                        .trusted_as(Trust::Operator),
                ),
                (
                    "config_file",
                    Value::from("/tmp/x"),
                    Origin::new(SourceKind::new("pkl"), "~/.config/hk.pkl")
                        .trusted_as(Trust::Operator),
                ),
            ],
        );
        let resolved = resolve(REGISTRY, Layers::new().then(&global_pkl)).expect("should resolve");
        assert_eq!(resolved.get_key("trusted"), Some(&Value::Bool(true)));
        assert_eq!(resolved.get_key("config_file"), None);
    }

    #[test]
    fn a_value_written_under_an_old_key_lands_on_the_new_one() {
        // `LayerCtx::prop` follows a rename, but a layer that took its ids from
        // `Registry::bindings` or `ids` hands over the *old* prop's id — and storing the value
        // there put it somewhere `get_key`, which follows the rename, would never look. The
        // value was honoured nowhere and reported nowhere.
        let git = Fixed {
            kind: SourceKind::new("git"),
            entries: vec![Entry::new(
                raw_id("renamed_jobs"),
                Value::Int(3),
                Origin::new(SourceKind::new("git"), "hk.renamedJobs"),
            )],
        };
        let resolved = resolve(REGISTRY, Layers::new().then(&git)).expect("should resolve");
        assert_eq!(
            resolved.get_key("jobs"),
            Some(&Value::Int(3)),
            "the old key's value should land on the setting that replaced it"
        );
        // Said out loud, in both names: the one written and the one it was read as.
        let messages: Vec<_> = resolved
            .warnings
            .iter()
            .map(|w| w.message.clone())
            .collect();
        assert!(
            messages.contains(&"renamed_jobs was read as jobs".to_string()),
            "{messages:?}"
        );
        assert!(
            messages.iter().any(|m| m.contains("is deprecated")),
            "{messages:?}"
        );
    }

    #[test]
    fn a_set_drops_a_repeat_from_one_source_too() {
        // Deduplication lived on the merge-two path, so a single `TAGS=a,b,a` kept its repeat —
        // a set that is only a set once two layers disagree is not a set.
        let one = layer(
            SourceKind::ENV,
            vec![(
                "tags",
                Value::List(vec![Value::from("a"), Value::from("b"), Value::from("a")]),
                Origin::new(SourceKind::ENV, "TAGS"),
            )],
        );
        let resolved = resolve(REGISTRY, Layers::new().then(&one)).expect("should resolve");
        assert_eq!(
            resolved.get_key("tags"),
            Some(&Value::List(vec![Value::from("a"), Value::from("b")]))
        );
    }

    #[test]
    fn a_collection_default_takes_part_in_the_merge() {
        // As a floor rather than a layer, a default only applied where nothing had been set —
        // so a `union` list with a declared default lost every one of the default's items the
        // moment any layer supplied anything at all.
        let env = layer(
            SourceKind::ENV,
            vec![(
                "excluded",
                Value::List(vec![Value::from("from-env")]),
                Origin::new(SourceKind::ENV, "EXCLUDED"),
            )],
        );
        let resolved = resolve(REGISTRY, Layers::new().then(&env)).expect("should resolve");
        assert_eq!(
            resolved.get_key("excluded"),
            Some(&Value::List(vec![
                Value::from("target"),
                Value::from("from-env")
            ])),
            "the default's items are the lowest-precedence contribution, not a fallback"
        );
        // And the default is recorded as the contributor it is.
        assert_eq!(
            resolved.contributors(id("excluded"))[0].describe(),
            "the default"
        );
    }

    #[test]
    fn the_winning_origin_is_always_the_last_contributor() {
        // The invariant behind `explain`, asserted as an invariant rather than field by field.
        // A rename put the value and the winning origin on the folded prop and its contributors
        // on the one the layer named, and every per-field assertion I had still passed.
        let cli = layer(
            SourceKind::CLI,
            vec![(
                "jobs",
                Value::Int(1),
                Origin::new(SourceKind::CLI, "--jobs"),
            )],
        );
        let env = layer(
            SourceKind::ENV,
            vec![
                (
                    "jobs",
                    Value::Int(2),
                    Origin::new(SourceKind::ENV, "HK_JOBS"),
                ),
                (
                    "excluded",
                    Value::List(vec![Value::from("from-env")]),
                    Origin::new(SourceKind::ENV, "EXCLUDED"),
                ),
                (
                    "tags",
                    Value::List(vec![Value::from("a"), Value::from("a")]),
                    Origin::new(SourceKind::ENV, "TAGS"),
                ),
            ],
        );
        let renamed = Fixed {
            kind: SourceKind::new("git"),
            entries: vec![Entry::new(
                raw_id("renamed_jobs"),
                Value::Int(9),
                Origin::new(SourceKind::new("git"), "hk.renamedJobs"),
            )],
        };
        let resolved = resolve(REGISTRY, Layers::new().then(&cli).then(&env).then(&renamed))
            .expect("should resolve");

        for id in REGISTRY.ids() {
            let key = REGISTRY.get(id).key;
            match (resolved.origin(id), resolved.contributors(id).last()) {
                (Some(winner), Some(last)) => assert_eq!(
                    winner, last,
                    "{key}: the winning origin is not the last contributor"
                ),
                (None, None) => {}
                (winner, last) => panic!("{key}: origin {winner:?} but contributors end {last:?}"),
            }
        }
        // And specifically for the renamed one, whose contributors used to live elsewhere.
        let contributors: Vec<_> = resolved
            .contributors(id("jobs"))
            .iter()
            .map(|o| o.describe().to_string())
            .collect();
        assert!(
            contributors.contains(&"hk.renamedJobs".to_string()),
            "a value read through a rename contributed and should say so: {contributors:?}"
        );
    }

    #[test]
    fn a_deprecated_key_is_reported_however_the_layer_found_it() {
        // A layer reading a file looks keys up, and `LayerCtx` folds a rename on the way — so
        // the entry arrives already carrying the *new* id and the resolver could not tell that
        // anybody had written the old name. The deprecated key in somebody's config file was
        // honoured in complete silence.
        struct FileLike;
        impl Layer for FileLike {
            fn source(&self) -> SourceKind {
                SourceKind::FILE
            }
            fn load(&self, ctx: &LayerCtx) -> Result<LayerOutput, LayerError> {
                let mut out = LayerOutput::new();
                let origin = Origin::file("hk.toml", FileScope::Project);
                match ctx.entry_for_key("renamed_jobs", "5", origin) {
                    Ok(entry) => out.push(entry),
                    Err(warning) => out.warn(warning),
                }
                Ok(out)
            }
        }
        let file = FileLike;
        let resolved = resolve(REGISTRY, Layers::new().then(&file)).expect("should resolve");
        assert_eq!(resolved.get_key("jobs"), Some(&Value::Int(5)));
        let messages: Vec<_> = resolved
            .warnings
            .iter()
            .map(|w| w.message.clone())
            .collect();
        assert!(
            messages.contains(&"renamed_jobs is deprecated: Use jobs instead.".to_string()),
            "{messages:?}"
        );
        assert!(
            messages.contains(&"renamed_jobs was read as jobs".to_string()),
            "{messages:?}"
        );
    }

    #[test]
    fn a_notice_further_along_a_chain_of_renames_is_still_given() {
        // Two releases of renaming: `threads` became `concurrency`, which became `jobs` and carries
        // the notice. `explain` walked the chain for it and the merge read only the declaration
        // written, so `config explain threads` told a user to stop using a key that running the CLI
        // said nothing about — one rule, two implementations, and the quieter one was the one a CLI
        // actually surfaces.
        static PROPS: &[PropMeta] = &[
            PropMeta {
                default: Some(Const::Int(1)),
                ..PropMeta::new("jobs", Ty::Uint)
            },
            PropMeta {
                renamed_to: Some("jobs"),
                deprecated: Some("Use jobs instead."),
                ..PropMeta::new("concurrency", Ty::Uint)
            },
            PropMeta {
                renamed_to: Some("concurrency"),
                ..PropMeta::new("threads", Ty::Uint)
            },
        ];
        const CHAINED: Registry = Registry::new(PROPS);

        struct Wrote;
        impl Layer for Wrote {
            fn source(&self) -> SourceKind {
                SourceKind::FILE
            }
            fn load(&self, ctx: &LayerCtx) -> Result<LayerOutput, LayerError> {
                let mut out = LayerOutput::new();
                let origin = Origin::file("hk.toml", FileScope::Project);
                match ctx.entry_for_key("threads", "8", origin) {
                    Ok(entry) => out.push(entry),
                    Err(warning) => out.warn(warning),
                }
                Ok(out)
            }
        }
        let resolved = resolve(CHAINED, Layers::new().then(&Wrote)).expect("should resolve");
        assert_eq!(resolved.get_key("jobs"), Some(&Value::Int(8)));
        let kinds: Vec<_> = resolved.warnings.iter().map(|w| w.kind).collect();
        assert_eq!(kinds, vec![WarningKind::Deprecated, WarningKind::Renamed]);
        // Named by what the user wrote, since that is the line in the file they would go and edit.
        assert!(
            resolved.warnings[0].message == "threads is deprecated: Use jobs instead.",
            "{:?}",
            resolved.warnings[0].message
        );
    }

    #[test]
    fn an_unknown_key_is_a_warning_rather_than_a_failure() {
        // Newer config read by an older binary: the key it does not know is reported and the
        // rest of the file still applies.
        struct Stray;
        impl Layer for Stray {
            fn source(&self) -> SourceKind {
                SourceKind::FILE
            }
            fn load(&self, ctx: &LayerCtx) -> Result<LayerOutput, LayerError> {
                let mut out = LayerOutput::new();
                let origin = Origin::file("hk.toml", FileScope::Project);
                match ctx.entry_for_key("from_the_future", "1", origin) {
                    Ok(entry) => out.push(entry),
                    Err(warning) => out.warn(warning),
                }
                Ok(out)
            }
        }
        let stray = Stray;
        let resolved = resolve(REGISTRY, Layers::new().then(&stray)).expect("should resolve");
        assert_eq!(
            resolved.warnings[0].message,
            "unknown setting `from_the_future`"
        );
    }

    #[test]
    fn an_alias_does_not_carry_a_default_of_its_own() {
        // Seeded under its own id, a renamed prop's default landed where no reader looks: every
        // lookup folds to the replacement. The setting that replaced it declares its own.
        let resolved = resolve(REGISTRY, Layers::new()).expect("should resolve");
        assert_eq!(
            resolved.get(raw_id("renamed_jobs")),
            None,
            "an alias should hold nothing at all"
        );
        assert_eq!(resolved.get_key("jobs"), Some(&Value::Int(4)));
    }

    #[test]
    fn an_explicit_empty_list_clears_a_union_default() {
        // How a user turns a declared default off. `HK_EXCLUDE=` parses to an empty list for
        // exactly this reason, and with defaults now merging rather than filling in, an empty
        // list that concatenated left every default item in place.
        let env = layer(
            SourceKind::ENV,
            vec![(
                "excluded",
                Value::List(Vec::new()),
                Origin::new(SourceKind::ENV, "EXCLUDED"),
            )],
        );
        let resolved = resolve(REGISTRY, Layers::new().then(&env)).expect("should resolve");
        assert_eq!(resolved.get_key("excluded"), Some(&Value::List(Vec::new())));
    }

    #[test]
    fn a_rewrite_stays_the_last_contributor() {
        // The invariant `explain` rests on has to survive the post-merge hook too: rewriting
        // the value and the winning origin without touching the contributor list left
        // `origin()` naming the rewrite and `contributors().last()` naming what it replaced.
        let env = layer(
            SourceKind::ENV,
            vec![(
                "jobs",
                Value::Int(8),
                Origin::new(SourceKind::ENV, "HK_JOBS"),
            )],
        );
        let mut resolved = resolve(REGISTRY, Layers::new().then(&env)).expect("should resolve");
        resolved.coerced(id("jobs"), Value::Int(1), "raw implies one job");
        assert_eq!(
            resolved.origin(id("jobs")),
            resolved.contributors(id("jobs")).last()
        );
    }

    #[test]
    fn a_layer_that_cannot_read_its_source_stops_the_resolution() {
        // Unlike an unknown key, which degrades to a warning: a file that exists and cannot
        // be parsed means the values a user believes are in effect are not, and carrying on
        // as though they had never written it is worse than saying so.
        struct Broken;
        impl Layer for Broken {
            fn source(&self) -> SourceKind {
                SourceKind::FILE
            }
            fn load(&self, _ctx: &LayerCtx) -> Result<LayerOutput, LayerError> {
                Err(LayerError::Unreadable {
                    source: "hk.toml".to_string(),
                    why: "expected a value at line 3".to_string(),
                })
            }
        }
        let broken = Broken;
        let err = resolve(REGISTRY, Layers::new().then(&broken)).expect_err("should fail");
        assert_eq!(
            err.to_string(),
            "could not read hk.toml: expected a value at line 3"
        );
    }
}
