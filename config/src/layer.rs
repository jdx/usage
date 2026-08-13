//! Where values come from, as an interface.
//!
//! usage supplies the command line, the environment and declared defaults. Everything else a
//! CLI reads — a git config, a pkl file, an `.npmrc`, a keyring — is a layer the CLI writes,
//! and it writes it against this trait plus [`Registry::bindings`], which is why hk's git
//! layer is about twenty lines rather than a second resolution system.
//!
//! [`Registry::bindings`]: crate::Registry::bindings

use crate::registry::{PropId, Registry};
use crate::source::{Origin, SourceKind};
use crate::value::Value;

/// One value a layer supplies.
#[derive(Debug, Clone, PartialEq)]
pub struct Entry {
    pub prop: PropId,
    pub value: Value,
    /// The exact place it came from — the variable's name, the file's path.
    pub origin: Origin,
    /// The key the user actually wrote, when it was an old name for `prop`.
    ///
    /// A layer that looks a key up gets the id of the setting that *replaced* it, so without
    /// this the resolver cannot tell that anybody used the old name — and the warning that a
    /// deprecated key is in somebody's config file never fires.
    pub renamed_from: Option<&'static str>,
}

impl Entry {
    pub fn new(prop: PropId, value: Value, origin: Origin) -> Self {
        Self {
            prop,
            value,
            origin,
            renamed_from: None,
        }
    }
}

/// Something a user should know about, which is not bad enough to stop for.
///
/// Returned rather than printed. mise queues these until its logging is up, and a library
/// that writes to stderr on its own cannot be used by anything that has an opinion about
/// output.
#[derive(Debug, Clone, PartialEq)]
pub struct Warning {
    pub message: String,
    /// Where the value that caused it came from, when there was one.
    pub origin: Option<Origin>,
}

impl Warning {
    pub fn new(message: impl Into<String>) -> Self {
        Self {
            message: message.into(),
            origin: None,
        }
    }

    pub fn at(message: impl Into<String>, origin: Origin) -> Self {
        Self {
            message: message.into(),
            origin: Some(origin),
        }
    }
}

/// What a layer found.
#[derive(Debug, Default, Clone, PartialEq)]
pub struct LayerOutput {
    pub entries: Vec<Entry>,
    pub warnings: Vec<Warning>,
}

impl LayerOutput {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn push(&mut self, entry: Entry) {
        self.entries.push(entry);
    }

    pub fn warn(&mut self, warning: Warning) {
        self.warnings.push(warning);
    }
}

/// Anything that can fail while a layer reads.
#[derive(Debug, Clone, PartialEq)]
pub enum LayerError {
    /// The layer could not read its source at all — a malformed file, a subprocess that
    /// failed. Unlike an unknown key, this is not something to degrade past.
    Unreadable { source: String, why: String },
}

impl std::fmt::Display for LayerError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Unreadable { source, why } => write!(f, "could not read {source}: {why}"),
        }
    }
}

impl std::error::Error for LayerError {}

/// What a layer is given while it reads.
///
/// The registry, and the helpers that keep every layer honest about the same two things: a
/// key it does not recognize is a warning rather than an error, and a raw string becomes a
/// value the way the *spec* says rather than the way the layer guesses.
pub struct LayerCtx {
    registry: Registry,
}

impl LayerCtx {
    pub fn new(registry: Registry) -> Self {
        Self { registry }
    }

    pub fn registry(&self) -> Registry {
        self.registry
    }

    /// The setting a dotted key names, following renames.
    pub fn prop(&self, key: &str) -> Option<crate::registry::Lookup> {
        self.registry.lookup(key)
    }

    /// The setting an id ends up on, and the name it was declared under if that differs.
    ///
    /// `Registry::bindings` yields the *pre*-rename id, so a git or pkl layer hands over an
    /// alias — and the alias's own metadata is usually bare. Reading `parse` and `ty` from it
    /// meant a value that should have been split by a declared parser arrived as one unsplit
    /// string on the replacement's list. What governs a value is the setting it lands on.
    fn folded(&self, id: PropId) -> (PropId, Option<&'static str>) {
        let meta = self.registry.get(id);
        match meta.renamed_to.and_then(|key| self.registry.lookup(key)) {
            Some(target) if target.id != id => (target.id, Some(meta.key)),
            _ => (id, None),
        }
    }

    /// A raw string read as the setting's declared type, applying its named parser first.
    ///
    /// Every layer that reads text should come through here. A layer that decides for itself
    /// how to split a list is how two sources of the same setting end up disagreeing about
    /// what a comma means.
    pub fn parse(&self, id: PropId, raw: &str) -> Result<Value, crate::ty::TypeError> {
        let (id, _) = self.folded(id);
        let meta = self.registry.get(id);
        let value = match meta.parse {
            Some(parser) => parser.split(raw),
            None => Value::String(raw.to_string()),
        };
        meta.ty.coerce(value)
    }

    /// An entry for `id`, with `raw` read as the declared type.
    ///
    /// The shape almost every layer wants: on a value that cannot be the declared type, the
    /// entry is dropped and a warning takes its place, naming the origin. A bad value in a
    /// system-wide file must not stop a CLI from starting.
    pub fn entry(&self, id: PropId, raw: &str, origin: Origin) -> Result<Entry, Warning> {
        // Folded here too, and the name that was written kept, so an entry built from a
        // binding carries the same information as one built from a key.
        let (id, renamed_from) = self.folded(id);
        match self.parse(id, raw) {
            Ok(value) => Ok(Entry {
                renamed_from,
                ..Entry::new(id, value, origin)
            }),
            Err(err) => {
                // The name that was written, not the one it folded to: a message about a key
                // the user cannot find in their own file is no help.
                let key = renamed_from.unwrap_or(self.registry.get(id).key);
                Err(Warning::at(
                    format!(
                        "{key} expected {} but {} has `{}`",
                        err.expected,
                        origin.describe(),
                        err.found
                    ),
                    origin,
                ))
            }
        }
    }
}

impl LayerCtx {
    /// An entry for a dotted key, which is what a layer reading a file has in hand.
    ///
    /// The path worth taking: it looks the key up, follows a rename while remembering the name
    /// that was written, reads the value as the declared type, and turns an unknown key into a
    /// warning rather than an error — everything a layer would otherwise have to remember to
    /// do, and the reason a deprecated key in somebody's config file gets reported at all.
    pub fn entry_for_key(&self, key: &str, raw: &str, origin: Origin) -> Result<Entry, Warning> {
        let Some(found) = self.prop(key) else {
            return Err(Warning::at(format!("unknown setting `{key}`"), origin));
        };
        let mut entry = self.entry(found.id, raw, origin)?;
        // `lookup` already folded, so `entry` had nothing left to fold and nothing to report;
        // the name the *user* wrote is the one to keep.
        entry.renamed_from = found.renamed_from.or(entry.renamed_from);
        Ok(entry)
    }
}

/// A source of configuration values.
pub trait Layer {
    /// Which kind of place this reads. Used by the scope check and reported by `explain`.
    fn source(&self) -> SourceKind;

    /// Everything this layer has to say, in one pass.
    fn load(&self, ctx: &LayerCtx) -> Result<LayerOutput, LayerError>;
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::registry::PropMeta;
    use crate::ty::{Parser, Ty};

    static PROPS: &[PropMeta] = &[
        PropMeta::new("jobs", Ty::Uint),
        PropMeta {
            parse: Some(Parser::ListByComma),
            ..PropMeta::new("exclude", Ty::List(&Ty::String))
        },
    ];
    const REGISTRY: Registry = Registry::new(PROPS);

    #[test]
    fn a_raw_string_is_read_the_way_the_spec_says() {
        let ctx = LayerCtx::new(REGISTRY);
        let jobs = ctx.prop("jobs").expect("declared").id;
        assert_eq!(ctx.parse(jobs, "4"), Ok(Value::Int(4)));

        // The declared parser runs before the type does, so one string becomes a list and
        // no layer has to know that this setting is comma-separated.
        let exclude = ctx.prop("exclude").expect("declared").id;
        assert_eq!(
            ctx.parse(exclude, "target,node_modules"),
            Ok(Value::List(vec![
                Value::from("target"),
                Value::from("node_modules")
            ]))
        );
    }

    #[test]
    fn an_alias_is_read_with_the_metadata_of_the_setting_it_became() {
        // `Registry::bindings` yields the pre-rename id, so a git or pkl layer hands over an
        // alias — whose own metadata is usually bare. Reading `parse` and `ty` from it meant a
        // comma-separated value arrived as one unsplit string on the replacement's list.
        static PROPS: &[PropMeta] = &[
            PropMeta {
                parse: Some(Parser::ListByComma),
                ..PropMeta::new("exclude", Ty::List(&Ty::String))
            },
            // The alias: no parser, no list type of its own.
            PropMeta {
                renamed_to: Some("exclude"),
                ..PropMeta::new("excludes", Ty::String)
            },
        ];
        const REGISTRY: Registry = Registry::new(PROPS);
        let ctx = LayerCtx::new(REGISTRY);
        let alias = PropId(1);

        assert_eq!(
            ctx.parse(alias, "target,vendor"),
            Ok(Value::List(vec![
                Value::from("target"),
                Value::from("vendor")
            ]))
        );
        // And the entry lands on the replacement while remembering the name it came in under,
        // so the deprecation warning still has something to say.
        let entry = ctx
            .entry(
                alias,
                "target",
                Origin::new(SourceKind::new("git"), "hk.excludes"),
            )
            .expect("should parse");
        assert_eq!(entry.prop, PropId(0));
        assert_eq!(entry.renamed_from, Some("excludes"));
    }

    #[test]
    fn a_bad_value_becomes_a_warning_that_names_where_it_came_from() {
        // A CLI has to start even when a file it does not own has nonsense in it, and the
        // warning has to say which file, because otherwise the user cannot find it.
        let ctx = LayerCtx::new(REGISTRY);
        let jobs = ctx.prop("jobs").expect("declared").id;
        let origin = Origin::new(SourceKind::ENV, "HK_JOBS");
        let warning = ctx
            .entry(jobs, "lots", origin.clone())
            .expect_err("should not be an entry");
        assert_eq!(
            warning.message,
            "jobs expected a positive integer but HK_JOBS has `lots`"
        );
        assert_eq!(warning.origin, Some(origin));

        // And under an old name, the message says the name that was written — a complaint about
        // a key the user cannot find in their own file is no help at all.
        static RENAMED: &[PropMeta] = &[
            PropMeta::new("jobs", Ty::Uint),
            PropMeta {
                renamed_to: Some("jobs"),
                ..PropMeta::new("concurrency", Ty::Uint)
            },
        ];
        const WITH_ALIAS: Registry = Registry::new(RENAMED);
        let ctx = LayerCtx::new(WITH_ALIAS);
        let warning = ctx
            .entry(
                PropId(1),
                "lots",
                Origin::new(SourceKind::ENV, "HK_CONCURRENCY"),
            )
            .expect_err("should not be an entry");
        assert!(
            warning.message.starts_with("concurrency expected"),
            "{}",
            warning.message
        );
    }
}
