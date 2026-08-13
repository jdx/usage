//! The environment as a layer.
//!
//! The one layer every CLI in the fleet has, and the one every CLI writes again: mise reads 33 of
//! its settings through `parse_env` functions, hk's generator emits a match arm per variable, fnox
//! has about 48 `FNOX_*` variables that live outside its registry entirely. All of them are doing
//! the same thing — look up the names a setting declares, take the first one that is set — and the
//! registry already knows those names.
//!
//! The environment is *injected* rather than read at the point of use, so a test does not have to
//! touch the process to describe one, and two tests can describe different ones at the same time.
//! `EnvLayer::from_process` is the one place `std::env` is read.

use std::collections::BTreeMap;
use std::ffi::OsStr;

use crate::layer::{Layer, LayerCtx, LayerError, LayerOutput, Warning};
use crate::registry::PropId;
use crate::source::{Origin, SourceKind};

/// Settings read from environment variables.
pub struct EnvLayer {
    /// Keyed by the comparable form of each name, holding the name as it was actually set and its
    /// value. Both are kept because the comparison and the report want different things: one needs
    /// the names to match, the other needs to print what the user typed.
    vars: BTreeMap<String, (String, String)>,
}

impl EnvLayer {
    /// The variables this process was started with.
    ///
    /// `std::env::vars` *panics* on a name or value that is not UTF-8, which would take the CLI down
    /// before it resolved anything — over a variable that in all likelihood belongs to something
    /// else entirely. Read as OS strings and skipped when they will not convert: a setting whose
    /// variable holds bytes this cannot read has no value it could have been given anyway.
    pub fn from_process() -> Self {
        Self::new(std::env::vars_os().filter_map(|(name, value)| readable(&name, &value)))
    }

    /// A named environment, for a test or for a CLI that has its own idea of one.
    pub fn new(vars: impl IntoIterator<Item = (String, String)>) -> Self {
        Self {
            vars: vars
                .into_iter()
                .map(|(name, value)| (normalize(&name), (name, value)))
                .collect(),
        }
    }

    /// What this layer would read for `name`, if anything.
    pub fn get(&self, name: &str) -> Option<&str> {
        self.vars
            .get(&normalize(name))
            .map(|(_, value)| value.as_str())
    }

    /// The name as it is really set, which is what a report should print.
    fn set_as(&self, name: &str) -> Option<&str> {
        self.vars
            .get(&normalize(name))
            .map(|(set_as, _)| set_as.as_str())
    }
}

/// One variable, if it is text at all.
///
/// A name or value that is not UTF-8 is skipped. A setting whose variable holds bytes that cannot be
/// read has no value it could have been given, and the alternative is what `std::env::vars` does:
/// panic, taking the CLI down before it resolves anything, over a variable that in all likelihood
/// belongs to something else entirely.
fn readable(name: &OsStr, value: &OsStr) -> Option<(String, String)> {
    Some((name.to_str()?.to_string(), value.to_str()?.to_string()))
}

/// A variable's name in the form this layer compares.
///
/// Windows environment variable names are case-insensitive — `std::env::var("PATH")` finds `Path` —
/// so a lookup that is case-sensitive there would miss a variable the user has plainly set, and
/// would do it only on Windows, which is the worst place for a difference like this to live.
/// Everywhere else the name is the name.
fn normalize(name: &str) -> String {
    if cfg!(windows) {
        name.to_uppercase()
    } else {
        name.to_string()
    }
}

impl Layer for EnvLayer {
    fn source(&self) -> SourceKind {
        SourceKind::ENV
    }

    fn load(&self, ctx: &LayerCtx) -> Result<LayerOutput, LayerError> {
        let registry = ctx.registry();
        let mut out = LayerOutput::new();

        // Everything the environment has to say, gathered by the setting it will end up in — which
        // is what an old name shares with the name that replaced it.
        let mut found: BTreeMap<PropId, Vec<(PropId, &str, &str)>> = BTreeMap::new();
        for id in registry.ids() {
            let meta = registry.get(id);
            // The *declared* order, because a setting's variables are listed highest first: mise's
            // `MISE_JOBS` beside an older `MISE_JOB`. First one set wins and the rest are not looked
            // at, which is what makes an alias an alias rather than a second setting.
            for name in meta.envs {
                let Some(raw) = self.get(name) else {
                    continue;
                };
                // The name the *user* set, not the setting's canonical one — an explanation that said
                // "set by the environment" would send them looking through all of them — and their
                // spelling of it, which on Windows need not be the declared one.
                let set_as = self.set_as(name).unwrap_or(name);
                let target = meta
                    .renamed_to
                    .and_then(|to| registry.lookup(to))
                    .map_or(id, |found| found.id);
                found.entry(target).or_default().push((id, set_as, raw));
                break;
            }
        }

        for (target, mut candidates) in found {
            // One entry per setting, chosen here rather than left to the merge. Pushing every one of
            // them and relying on the last writer only works for `replace`: a `union` list took the
            // items from a deprecated variable *as well*, and an emptied one cleared the default
            // before the current name was merged.
            //
            // The setting's own name wins; among old names, the first the registry declares. Which
            // old name should beat another is a question nothing declares an answer to, so the answer
            // is stable and the layer says what it did.
            candidates.sort_by_key(|(id, ..)| *id != target);
            let mut read_by: Option<&str> = None;
            for (id, set_as, raw) in candidates {
                let origin = Origin::new(SourceKind::ENV, set_as);
                if let Some(first) = read_by {
                    out.warn(Warning::at(
                        format!(
                            "{set_as} was not read: {first} also sets {}",
                            registry.get(target).key
                        ),
                        origin,
                    ));
                    continue;
                }
                match ctx.entry(id, raw, origin) {
                    // Only a value that *reads* speaks for its setting: a typo in one name would
                    // otherwise discard a perfectly good value in another.
                    Ok(entry) => {
                        out.push(entry);
                        read_by = Some(set_as);
                    }
                    // And a value of the wrong type costs that variable and nothing else. Refusing to
                    // start because one variable in a shell profile is a typo would be worse than the
                    // typo.
                    Err(warning) => out.warn(warning),
                }
            }
        }
        Ok(out)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::registry::{PropMeta, Registry, Scope};
    use crate::resolve::{resolve, Layers};
    use crate::ty::{Parser, Ty};
    use crate::value::{Const, Value};

    static PROPS: &[PropMeta] = &[
        PropMeta {
            default: Some(Const::Int(4)),
            // Highest first, which is what makes the second one an alias.
            envs: &["HK_JOBS", "HK_JOB"],
            ..PropMeta::new("jobs", Ty::Uint)
        },
        PropMeta {
            parse: Some(Parser::ListByComma),
            envs: &["HK_EXCLUDE"],
            ..PropMeta::new("exclude", Ty::List(&Ty::String))
        },
        PropMeta {
            scope: Scope::Global,
            envs: &["HK_TRUSTED"],
            ..PropMeta::new("trusted", Ty::Bool)
        },
        // No variables at all: plenty of settings are file-only.
        PropMeta::new("stash", Ty::String),
        PropMeta {
            envs: &["HK_CONCURRENCY"],
            deprecated: Some("Use jobs instead."),
            renamed_to: Some("jobs"),
            ..PropMeta::new("concurrency", Ty::Uint)
        },
        // A `union` list and an old name for it: this is the pair that "last writer wins" could not
        // settle, since a union takes from *every* contributor rather than the last.
        PropMeta {
            merge: crate::registry::Merge::Union,
            parse: Some(Parser::ListByComma),
            envs: &["HK_SKIP"],
            ..PropMeta::new("skip", Ty::List(&Ty::String))
        },
        PropMeta {
            merge: crate::registry::Merge::Union,
            parse: Some(Parser::ListByComma),
            envs: &["HK_SKIP_STEPS"],
            deprecated: Some("Use skip instead."),
            renamed_to: Some("skip"),
            ..PropMeta::new("skip_steps", Ty::List(&Ty::String))
        },
        // A second old name for the same setting, which is what a registry looks like after two
        // renames. Sorts after `concurrency`, so it is the one that used to win by accident.
        PropMeta {
            envs: &["HK_THREADS"],
            deprecated: Some("Use jobs instead."),
            renamed_to: Some("jobs"),
            ..PropMeta::new("threads", Ty::Uint)
        },
    ];
    const REGISTRY: Registry = Registry::new(PROPS);

    fn env(vars: &[(&str, &str)]) -> EnvLayer {
        EnvLayer::new(
            vars.iter()
                .map(|(k, v)| ((*k).to_string(), (*v).to_string())),
        )
    }

    #[test]
    fn a_setting_is_read_from_the_variables_it_declares() {
        let layer = env(&[
            ("HK_JOBS", "8"),
            ("HK_EXCLUDE", "target,dist"),
            ("PATH", "/bin"),
        ]);
        let resolved = resolve(REGISTRY, Layers::new().then(&layer)).expect("resolves");

        assert_eq!(resolved.get_key("jobs"), Some(&Value::Int(8)));
        assert_eq!(
            resolved.get_key("exclude"),
            Some(&Value::List(vec![
                Value::from("target"),
                Value::from("dist")
            ]))
        );
        // A variable no setting declares is not this layer's business, and certainly not a warning:
        // the environment of a running process has hundreds of them in it.
        assert!(resolved.warnings.is_empty(), "{:?}", resolved.warnings);
        // The name the user set is what an explanation names.
        assert_eq!(
            resolved
                .origin(REGISTRY.lookup("jobs").expect("declared").id)
                .map(|o| o.describe()),
            Some("HK_JOBS")
        );
    }

    #[test]
    fn the_first_name_a_setting_declares_is_the_one_that_wins() {
        // Both set, which happens while a rename is being lived through. The declared order is the
        // answer, and the loser is not read at all — an alias is a second name for one setting, not
        // a second setting.
        let layer = env(&[("HK_JOB", "2"), ("HK_JOBS", "8")]);
        let resolved = resolve(REGISTRY, Layers::new().then(&layer)).expect("resolves");
        assert_eq!(resolved.get_key("jobs"), Some(&Value::Int(8)));
        let id = REGISTRY.lookup("jobs").expect("declared").id;
        assert_eq!(
            resolved.contributors(id).len(),
            2,
            "the default and one variable, not both variables: {:?}",
            resolved.contributors(id)
        );

        // And with only the older one set, it is read.
        let layer = env(&[("HK_JOB", "2")]);
        let resolved = resolve(REGISTRY, Layers::new().then(&layer)).expect("resolves");
        assert_eq!(resolved.get_key("jobs"), Some(&Value::Int(2)));
        assert_eq!(
            resolved.origin(id).map(|o| o.describe()),
            Some("HK_JOB"),
            "named as the user set it"
        );
    }

    #[test]
    fn a_variable_that_is_not_set_leaves_the_default_alone() {
        let layer = env(&[]);
        let resolved = resolve(REGISTRY, Layers::new().then(&layer)).expect("resolves");
        assert_eq!(resolved.get_key("jobs"), Some(&Value::Int(4)));
        assert_eq!(resolved.get_key("exclude"), None);
        assert!(resolved.warnings.is_empty());
    }

    #[test]
    fn a_value_of_the_wrong_type_costs_its_own_variable_and_nothing_else() {
        // A typo in a shell profile. Refusing to start would be worse than the typo, and every
        // other setting is perfectly readable.
        let layer = env(&[("HK_JOBS", "lots"), ("HK_EXCLUDE", "target")]);
        let resolved = resolve(REGISTRY, Layers::new().then(&layer)).expect("resolves");
        assert_eq!(
            resolved.get_key("jobs"),
            Some(&Value::Int(4)),
            "the default"
        );
        assert_eq!(
            resolved.get_key("exclude"),
            Some(&Value::List(vec![Value::from("target")]))
        );
        let warnings = crate::explain::warnings(&resolved);
        assert_eq!(warnings.len(), 1, "{warnings:?}");
        assert!(warnings[0].contains("HK_JOBS"), "{warnings:?}");
        assert!(warnings[0].contains("but has `lots`"), "{warnings:?}");
    }

    #[test]
    fn an_environment_variable_may_set_a_global_scoped_setting() {
        // The point of `scope="global"` is that a *checkout* cannot set it. The environment is the
        // user's own, so it can.
        let layer = env(&[("HK_TRUSTED", "yes")]);
        let resolved = resolve(REGISTRY, Layers::new().then(&layer)).expect("resolves");
        assert_eq!(resolved.get_key("trusted"), Some(&Value::Bool(true)));
        assert!(resolved.warnings.is_empty(), "{:?}", resolved.warnings);
    }

    #[test]
    fn an_old_name_in_the_environment_is_folded_and_reported() {
        // A variable for a setting that has been renamed. The value still applies — an upgrade must
        // not silently change what a machine's environment means — and something is said about it.
        let layer = env(&[("HK_CONCURRENCY", "6")]);
        let resolved = resolve(REGISTRY, Layers::new().then(&layer)).expect("resolves");
        assert_eq!(resolved.get_key("jobs"), Some(&Value::Int(6)));
        // Two things worth saying, and the merge says both: why not to use the old name, and where
        // the value ended up. Both name the variable the user set rather than the setting.
        let warnings = crate::explain::warnings(&resolved);
        assert_eq!(warnings.len(), 2, "{warnings:?}");
        assert!(
            warnings[0].starts_with("concurrency is deprecated: Use jobs instead."),
            "{warnings:?}"
        );
        assert!(
            warnings[1].starts_with("concurrency was read as jobs"),
            "{warnings:?}"
        );
        assert!(
            warnings.iter().all(|w| w.contains("HK_CONCURRENCY")),
            "{warnings:?}"
        );
    }

    #[test]
    fn a_setting_own_variable_beats_the_one_it_replaced() {
        // Both set, which is exactly what living through a rename looks like. Pushed and left to the
        // merge, which of them won came down to which key sorted later — and for a `union` setting
        // both applied. The setting's own name is chosen here, and the old one is reported.
        let layer = env(&[("HK_CONCURRENCY", "6"), ("HK_JOBS", "8")]);
        let resolved = resolve(REGISTRY, Layers::new().then(&layer)).expect("resolves");
        assert_eq!(resolved.get_key("jobs"), Some(&Value::Int(8)));
        let id = REGISTRY.lookup("jobs").expect("declared").id;
        assert_eq!(
            resolved.origin(id).map(|o| o.describe()),
            Some("HK_JOBS"),
            "the name that is not deprecated"
        );
        // One warning, and it is the one to act on: the old variable does nothing at all. Its
        // deprecation notice is about a value that was *used*, and none was — if the user removes
        // `HK_JOBS` believing the old name still works, the notice arrives then, which is when it
        // says something they need.
        let warnings = crate::explain::warnings(&resolved);
        assert_eq!(
            warnings,
            vec!["HK_CONCURRENCY was not read: HK_JOBS also sets jobs (HK_CONCURRENCY)"]
        );
    }

    #[test]
    fn one_of_two_old_names_is_chosen_and_the_other_is_reported() {
        // Two deprecated names for one setting, both set, and the current name not set at all.
        // Nothing declares which old name should win, so the answer is the first in registry order —
        // and taking the last one silently made it a matter of which key sorted later.
        let layer = env(&[("HK_CONCURRENCY", "6"), ("HK_THREADS", "9")]);
        let resolved = resolve(REGISTRY, Layers::new().then(&layer)).expect("resolves");
        assert_eq!(resolved.get_key("jobs"), Some(&Value::Int(6)));
        let warnings = crate::explain::warnings(&resolved);
        assert!(
            warnings
                .iter()
                .any(|w| w.starts_with("HK_THREADS was not read: HK_CONCURRENCY also sets jobs")),
            "{warnings:?}"
        );

        // And the current name still beats both of them.
        let layer = env(&[
            ("HK_CONCURRENCY", "6"),
            ("HK_THREADS", "9"),
            ("HK_JOBS", "8"),
        ]);
        let resolved = resolve(REGISTRY, Layers::new().then(&layer)).expect("resolves");
        assert_eq!(resolved.get_key("jobs"), Some(&Value::Int(8)));
    }

    #[test]
    fn an_old_name_that_does_not_read_does_not_speak_for_the_setting() {
        // The rule this layer already had — a bad value costs its own variable and nothing else —
        // applied to the rule this layer just gained. Recorded on presence rather than on reading,
        // a typo in the first old name discarded a good value in the second, and the warning said
        // the failed variable had set the setting.
        let layer = env(&[("HK_CONCURRENCY", "lots"), ("HK_THREADS", "9")]);
        let resolved = resolve(REGISTRY, Layers::new().then(&layer)).expect("resolves");
        assert_eq!(resolved.get_key("jobs"), Some(&Value::Int(9)));

        let warnings = crate::explain::warnings(&resolved);
        assert!(
            warnings
                .iter()
                .any(|w| w.contains("HK_CONCURRENCY") && w.contains("but has `lots`")),
            "{warnings:?}"
        );
        assert!(
            !warnings.iter().any(|w| w.contains("was not read")),
            "nothing was passed over: {warnings:?}"
        );
    }

    #[test]
    fn an_old_name_does_not_contribute_to_a_union_beside_the_new_one() {
        // The case last-writer-wins could not settle. A `union` takes from every contributor, so a
        // deprecated variable's items ended up in the list *as well* as the current one's — and an
        // emptied old variable cleared the declared default on its way past.
        let layer = env(&[("HK_SKIP_STEPS", "lint"), ("HK_SKIP", "test")]);
        let resolved = resolve(REGISTRY, Layers::new().then(&layer)).expect("resolves");
        assert_eq!(
            resolved.get_key("skip"),
            Some(&Value::List(vec![Value::from("test")])),
            "only the name that is not deprecated"
        );
        let warnings = crate::explain::warnings(&resolved);
        assert_eq!(
            warnings,
            vec!["HK_SKIP_STEPS was not read: HK_SKIP also sets skip (HK_SKIP_STEPS)"]
        );

        // With only the old name set it is read, because then it is the only thing that can be.
        let layer = env(&[("HK_SKIP_STEPS", "lint")]);
        let resolved = resolve(REGISTRY, Layers::new().then(&layer)).expect("resolves");
        assert_eq!(
            resolved.get_key("skip"),
            Some(&Value::List(vec![Value::from("lint")]))
        );
    }

    #[test]
    fn a_variable_that_is_not_text_is_skipped_rather_than_fatal() {
        // `std::env::vars` panics on a name or value that is not UTF-8, which would take a CLI down
        // before it resolved anything — over a variable that probably belongs to something else.
        // Nothing here can construct one portably, so this is the property that matters: reading the
        // process environment does not panic, whatever is in it.
        let layer = EnvLayer::from_process();
        let resolved = resolve(REGISTRY, Layers::new().then(&layer)).expect("resolves");
        // Whatever the machine's environment holds, a setting nothing declares a variable for is
        // untouched by it.
        assert_eq!(resolved.get_key("stash"), None);
    }

    #[cfg(unix)]
    #[test]
    fn bytes_that_are_not_text_are_not_a_variable() {
        use std::os::unix::ffi::OsStrExt;
        let ok = readable(OsStr::new("HK_JOBS"), OsStr::new("8"));
        assert_eq!(ok, Some(("HK_JOBS".to_string(), "8".to_string())));
        // Either half being unreadable is enough to skip it, and neither is a reason to stop.
        let bad_value = readable(OsStr::new("HK_JOBS"), OsStr::from_bytes(&[0xff, 0xfe]));
        assert_eq!(bad_value, None);
        let bad_name = readable(OsStr::from_bytes(&[0xff, 0xfe]), OsStr::new("8"));
        assert_eq!(bad_name, None);
    }

    #[test]
    fn the_environment_is_described_rather_than_reached_for() {
        // Injection, which is what lets these tests exist at all: two of them describing different
        // environments at once, and none of them touching the process.
        let layer = env(&[("HK_JOBS", "8")]);
        assert_eq!(layer.get("HK_JOBS"), Some("8"));
        assert_eq!(layer.get("HK_NOTHING"), None);
        // And the process is still readable, for the CLI that wants it.
        let _ = EnvLayer::from_process();
    }

    #[cfg(windows)]
    #[test]
    fn a_name_that_windows_spells_differently_is_still_the_same_name() {
        // `std::env::var("HK_JOBS")` finds `Hk_Jobs` on Windows, so a case-sensitive lookup would
        // miss a variable the user has plainly set — and only there.
        let layer = env(&[("Hk_Jobs", "8")]);
        let resolved = resolve(REGISTRY, Layers::new().then(&layer)).expect("resolves");
        assert_eq!(resolved.get_key("jobs"), Some(&Value::Int(8)));
        // And reported as the user spelled it, not as the spec declares it: this is the only
        // platform where those can differ, so it is the only place the difference can be asserted.
        assert_eq!(
            resolved
                .origin(REGISTRY.lookup("jobs").expect("declared").id)
                .map(|o| o.describe()),
            Some("Hk_Jobs")
        );
    }
}
