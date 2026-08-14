//! The command line as a layer.
//!
//! The highest layer there is, and the one the fleet gets least right. hk declares eighteen
//! `sources.cli` bindings and reads five — the flags live in a second, hand-maintained struct and
//! are consumed ad hoc. mise hand-copies thirteen flags into its settings in a forty-nine-line
//! function, and `--jobs` bypasses even that by going through the environment. pitchfork's `--help`
//! documents a CLI layer it does not have.
//!
//! What they are all writing is this: for each setting a flag was given for, one entry at the top of
//! the merge. The part that is easy to get wrong is *given*, which is why this layer takes values
//! rather than a struct — a `bool` field is `false` whether the flag was absent or explicitly
//! negated, and a layer that cannot tell those apart makes `--no-colour` indistinguishable from
//! saying nothing, which silently outranks every file on the machine.
//!
//! ```
//! use usage_config::{resolve, CliLayer, Layers, PropMeta, Registry, Ty, Value};
//!
//! static PROPS: &[PropMeta] = &[PropMeta {
//!     cli: &["--jobs", "-j"],
//!     ..PropMeta::new("jobs", Ty::Uint)
//! }];
//! const REGISTRY: Registry = Registry::new(PROPS);
//!
//! // What a parser produces: the settings a flag was actually given for.
//! let cli = CliLayer::new([("jobs", "8")]);
//! let resolved = resolve(REGISTRY, Layers::new().then(&cli))?;
//!
//! assert_eq!(resolved.get_key("jobs"), Some(&Value::Int(8)));
//! // Named as the flag rather than as "the command line", so an explanation is actionable.
//! assert_eq!(
//!     resolved.origin(REGISTRY.lookup("jobs").unwrap().id).unwrap().describe(),
//!     "--jobs",
//! );
//! # Ok::<(), usage_config::LayerError>(())
//! ```

use crate::layer::{Layer, LayerCtx, LayerError, LayerOutput};
use crate::registry::Registry;
use crate::source::{Origin, SourceKind};
use crate::value::Value;

/// Settings given on the command line.
pub struct CliLayer {
    given: Vec<(String, Given)>,
}

/// One value a flag was given, as text or with a shape of its own.
enum Given {
    /// What a flag's argument is before anything types it, which is what a parser hands over.
    Text(String),
    /// A value a caller has already made: a `bool` from a switch, a count, a list it collected.
    Shaped(Value),
}

impl CliLayer {
    /// The settings a flag was given for, as text.
    ///
    /// Only what was *given*: a flag left off the command line is not an entry here, because the
    /// command line outranks every other layer and an entry it did not earn would silently beat a
    /// file the user did write.
    pub fn new(given: impl IntoIterator<Item = (impl Into<String>, impl Into<String>)>) -> Self {
        Self {
            given: given
                .into_iter()
                .map(|(key, value)| (key.into(), Given::Text(value.into())))
                .collect(),
        }
    }

    /// A setting given as text, added to what this layer already has.
    ///
    /// Chained rather than collected, so a CLI can build the layer with one call per flag it has and
    /// leave out the ones it has not — which is the shape a generated `to_settings_layer` wants.
    pub fn with(mut self, key: impl Into<String>, value: impl Into<String>) -> Self {
        self.given.push((key.into(), Given::Text(value.into())));
        self
    }

    /// A setting given as a value that already has a shape.
    ///
    /// A switch is a `bool`, a `--verbose` count is a number, a repeated flag is a list: a caller
    /// holding those has no reason to render them to text for this to read them back.
    pub fn with_value(mut self, key: impl Into<String>, value: Value) -> Self {
        self.given.push((key.into(), Given::Shaped(value)));
        self
    }

    /// Whether a flag was given for anything at all.
    ///
    /// A CLI with no settings on its command line can leave this layer out of the plan rather than
    /// adding an empty one, though adding one changes nothing.
    pub fn is_empty(&self) -> bool {
        self.given.is_empty()
    }

    /// What a report should call the flag that set `key`.
    ///
    /// The first spelling the setting declares, because that is the long one a spec lists first and
    /// the one worth printing: "set by `--jobs`" is actionable in a way that "set by the command
    /// line" is not. A setting whose registry declares no flag is named by its key, which is the
    /// most that can be said about a CLI that bound something it never documented.
    fn origin(&self, registry: Registry, key: &str) -> Origin {
        let named = registry
            .lookup(key)
            .and_then(|found| registry.get(found.id).cli.first().copied());
        Origin::new(SourceKind::CLI, named.unwrap_or(key))
    }
}

impl Layer for CliLayer {
    fn source(&self) -> SourceKind {
        SourceKind::CLI
    }

    fn load(&self, ctx: &LayerCtx) -> Result<LayerOutput, LayerError> {
        let mut out = LayerOutput::new();
        for (key, given) in &self.given {
            let origin = self.origin(ctx.registry(), key);
            let entry = match given {
                Given::Text(raw) => ctx.entry_for_key(key, raw, origin),
                Given::Shaped(value) => ctx.entry_from_value(key, value.clone(), origin),
            };
            match entry {
                Ok(entry) => out.push(entry),
                // A flag the CLI bound to a setting nothing declares, or a value the declared type
                // cannot read: reported like any other layer's, rather than a panic in the one layer
                // whose contents are the CLI author's own doing. They will see it on their first run.
                Err(warning) => out.warn(warning),
            }
        }
        Ok(out)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::registry::{PropMeta, Scope};
    use crate::resolve::{resolve, Layers};
    use crate::ty::{Parser, Ty};
    use crate::value::Const;

    static PROPS: &[PropMeta] = &[
        PropMeta {
            default: Some(Const::Int(4)),
            envs: &["HK_JOBS"],
            cli: &["--jobs", "-j"],
            ..PropMeta::new("jobs", Ty::Uint)
        },
        PropMeta {
            cli: &["--colour", "--no-colour"],
            ..PropMeta::new("colour", Ty::Bool)
        },
        PropMeta {
            parse: Some(Parser::ListByComma),
            cli: &["--exclude"],
            ..PropMeta::new("exclude", Ty::List(&Ty::String))
        },
        // Settable from a checkout's own file is exactly what this is not, and the command line is
        // the user's own — so a flag may set it.
        PropMeta {
            scope: Scope::Global,
            cli: &["--trusted"],
            ..PropMeta::new("trusted", Ty::Bool)
        },
        // No flag at all, which is most settings.
        PropMeta::new("stash", Ty::String),
    ];
    const REGISTRY: Registry = Registry::new(PROPS);

    #[test]
    fn a_flag_that_was_given_sets_its_setting() {
        let cli = CliLayer::new([("jobs", "8")]);
        let resolved = resolve(REGISTRY, Layers::new().then(&cli)).expect("resolves");
        assert_eq!(resolved.get_key("jobs"), Some(&Value::Int(8)));
        // Named as the flag, not as "the command line": a user who does not like the answer needs to
        // know what to stop passing.
        assert_eq!(
            resolved
                .origin(REGISTRY.lookup("jobs").expect("declared").id)
                .map(|o| o.describe()),
            Some("--jobs")
        );
    }

    #[test]
    fn the_command_line_outranks_everything() {
        let cli = CliLayer::new([("jobs", "8")]);
        let env = crate::env::EnvLayer::new([("HK_JOBS".to_string(), "6".to_string())]);
        let resolved = resolve(REGISTRY, Layers::new().then(&cli).then(&env)).expect("resolves");
        assert_eq!(resolved.get_key("jobs"), Some(&Value::Int(8)));
    }

    #[test]
    fn a_flag_that_was_not_given_is_not_an_entry() {
        // The whole point of taking given values rather than a struct. A `bool` field is `false`
        // whether the flag was absent or explicitly negated, so a layer built from the struct sets
        // every switch on the command line — silently outranking every file on the machine.
        let cli = CliLayer::new(Vec::<(String, String)>::new());
        assert!(cli.is_empty());
        let env = crate::env::EnvLayer::new([("HK_JOBS".to_string(), "6".to_string())]);
        let resolved = resolve(REGISTRY, Layers::new().then(&cli).then(&env)).expect("resolves");
        assert_eq!(
            resolved.get_key("jobs"),
            Some(&Value::Int(6)),
            "the environment should still be what set it"
        );
        assert_eq!(resolved.get_key("colour"), None, "no flag, no value");
    }

    #[test]
    fn a_value_that_already_has_a_shape_is_taken_as_it_is() {
        // A switch is a `bool` and a repeated flag is a list. A caller holding those has no reason to
        // render them to text for this to read them back.
        let cli = CliLayer::new(Vec::<(String, String)>::new())
            .with_value("colour", Value::Bool(false))
            .with_value(
                "exclude",
                Value::List(vec![Value::from("target"), Value::from("dist")]),
            );
        let resolved = resolve(REGISTRY, Layers::new().then(&cli)).expect("resolves");
        assert_eq!(resolved.get_key("colour"), Some(&Value::Bool(false)));
        assert_eq!(
            resolved.get_key("exclude"),
            Some(&Value::List(vec![
                Value::from("target"),
                Value::from("dist")
            ]))
        );
    }

    #[test]
    fn a_flag_may_set_a_setting_no_file_can() {
        // `scope="global"` is about what a *checkout* can carry. The command line is the user's own.
        let cli = CliLayer::new([("trusted", "true")]);
        let resolved = resolve(REGISTRY, Layers::new().then(&cli)).expect("resolves");
        assert_eq!(resolved.get_key("trusted"), Some(&Value::Bool(true)));
        assert!(resolved.warnings.is_empty(), "{:?}", resolved.warnings);
    }

    #[test]
    fn a_setting_with_no_declared_flag_is_named_by_its_key() {
        // A CLI can bind a flag to a setting whose spec never mentioned one. That is worth reporting
        // as best as it can be — the key — rather than refusing to resolve it.
        let cli = CliLayer::new([("stash", "none")]);
        let resolved = resolve(REGISTRY, Layers::new().then(&cli)).expect("resolves");
        assert_eq!(resolved.get_key("stash"), Some(&Value::from("none")));
        assert_eq!(
            resolved
                .origin(REGISTRY.lookup("stash").expect("declared").id)
                .map(|o| o.describe()),
            Some("stash")
        );
    }

    #[test]
    fn a_flag_bound_to_nothing_is_a_warning_rather_than_a_crash() {
        // The CLI author's own mistake, found on their first run: a flag bound to a setting that does
        // not exist, or a value the declared type cannot read. Reported like any other layer's,
        // because a panic in the one layer whose contents are the author's doing is no more useful
        // and much harder to see past.
        let cli = CliLayer::new([("nonesuch", "1"), ("jobs", "lots")]);
        let resolved = resolve(REGISTRY, Layers::new().then(&cli)).expect("resolves");
        assert_eq!(
            resolved.get_key("jobs"),
            Some(&Value::Int(4)),
            "the default"
        );
        let kinds: Vec<_> = resolved.warnings.iter().map(|w| w.kind).collect();
        assert_eq!(
            kinds,
            vec![
                crate::layer::WarningKind::UnknownSetting,
                crate::layer::WarningKind::WrongType
            ]
        );
    }
}
