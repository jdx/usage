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
    /// The exact canonical key or supported alias a keyed layer matched.
    pub written_key: Option<&'static str>,
}

impl Entry {
    pub fn new(prop: PropId, value: Value, origin: Origin) -> Self {
        Self {
            prop,
            value,
            origin,
            renamed_from: None,
            written_key: None,
        }
    }
}

/// Something a user should know about, which is not bad enough to stop for.
///
/// Returned rather than printed. mise queues these until its logging is up, and a library
/// that writes to stderr on its own cannot be used by anything that has an opinion about
/// output.
///
/// Built through [`Warning::new`] or [`Warning::at`] rather than as a literal: this has gained a
/// field once already, and a warning is something a layer *reports* rather than a shape anything
/// downstream should be pattern-matched against exhaustively. Reading the fields, and matching with
/// `..`, are unaffected.
#[derive(Debug, Clone, PartialEq)]
#[non_exhaustive]
pub struct Warning {
    pub message: String,
    /// Where the value that caused it came from, when there was one.
    pub origin: Option<Origin>,
    /// What sort of thing happened, for a caller that wants to treat them differently.
    pub kind: WarningKind,
}

/// The kinds of thing a resolution has to say.
///
/// The message is for a person and its wording is nobody's contract; this is what a *program* can
/// act on. mise wants its deprecations queued and printed once its logging is up while a bad value
/// goes to stderr immediately; a `--strict` mode wants to exit on anything but a deprecation; the
/// conformance corpus wants to pin what happened without pinning how it was worded, since that is a
/// quality-of-implementation concern and differs between implementations by design.
#[derive(Debug, Default, Clone, Copy, PartialEq, Eq)]
#[non_exhaustive]
pub enum WarningKind {
    /// A key no setting declares. A config file written for a newer binary read by an older one.
    UnknownSetting,
    /// A value the declared type cannot read.
    WrongType,
    /// A value the declared `choice` nodes do not allow.
    NotAllowed,
    /// A place that may not set this setting: a `scope="global"` setting from a checkout.
    OutOfScope,
    /// A setting whose spec says not to use it any more.
    Deprecated,
    /// A value that arrived under an old name and was read as the setting that replaced it.
    Renamed,
    /// A value that was passed over because another name for the same setting won.
    NotRead,
    /// Something a layer of the CLI's own says, which this crate has no name for.
    #[default]
    Other,
}

impl Warning {
    /// A warning of no particular kind, which is what a custom layer's own complaints are.
    pub fn new(message: impl Into<String>) -> Self {
        Self {
            message: message.into(),
            origin: None,
            kind: WarningKind::Other,
        }
    }

    /// The same, about a value that came from somewhere nameable.
    pub fn at(message: impl Into<String>, origin: Origin) -> Self {
        Self {
            message: message.into(),
            origin: Some(origin),
            kind: WarningKind::Other,
        }
    }

    /// This warning, classified.
    ///
    /// Chained rather than an argument so the two constructors keep reading as they did, and so a
    /// layer that has nothing useful to say about the kind is not made to invent one.
    pub fn of(mut self, kind: WarningKind) -> Self {
        self.kind = kind;
        self
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
            Ok(value) => {
                let key = renamed_from.unwrap_or(self.registry.get(id).key);
                if let Some(refused) = self.refused(id, &value, key, &origin) {
                    return Err(refused);
                }
                Ok(Entry {
                    renamed_from,
                    ..Entry::new(id, value, origin)
                })
            }
            Err(err) => {
                // The name that was written, not the one it folded to: a message about a key
                // the user cannot find in their own file is no help.
                let key = renamed_from.unwrap_or(self.registry.get(id).key);
                // Without the origin in the text: the warning carries it, and a renderer that
                // adds it — as `explain::warnings` does for every warning — printed the place
                // twice for exactly the warnings that had bothered to name it.
                Err(Warning::at(
                    format!("{key} expected {} but has `{}`", err.expected, err.found),
                    origin,
                )
                .of(WarningKind::WrongType))
            }
        }
    }
}

impl LayerCtx {
    /// The warning for a value the setting's `choice` nodes do not allow, if it is one.
    ///
    /// Beside the type check and for the same reason: a declared type and a declared set of values
    /// are both the spec saying what may be here, and a value that is neither costs its own key and
    /// nothing else. Until this, choices reached the docs, the JSON schema and completions, and
    /// nothing that *resolved* a value — so a CLI documenting three allowed values took a fourth
    /// without a word, and only failed later, somewhere that could not say why.
    fn refused(&self, id: PropId, value: &Value, key: &str, origin: &Origin) -> Option<Warning> {
        let meta = self.registry.get(id);
        let refused = meta.refuses(value)?;
        Some(
            Warning::at(
                format!(
                    "{key} expected one of {} but has `{}`",
                    meta.allowed(),
                    crate::value::shown(refused)
                ),
                origin.clone(),
            )
            .of(WarningKind::NotAllowed),
        )
    }

    /// An entry for a dotted key, which is what a layer reading a file has in hand.
    ///
    /// The path worth taking: it looks the key up, follows a rename while remembering the name
    /// that was written, reads the value as the declared type, and turns an unknown key into a
    /// warning rather than an error — everything a layer would otherwise have to remember to
    /// do, and the reason a deprecated key in somebody's config file gets reported at all.
    pub fn entry_for_key(&self, key: &str, raw: &str, origin: Origin) -> Result<Entry, Warning> {
        let Some(found) = self.prop(key) else {
            return Err(Warning::at(format!("unknown setting `{key}`"), origin)
                .of(WarningKind::UnknownSetting));
        };
        match self.parse(found.id, raw) {
            Ok(value) => {
                if let Some(refused) = self.refused(found.id, &value, found.written, &origin) {
                    return Err(refused);
                }
                Ok(Entry {
                    renamed_from: found.renamed_from,
                    written_key: Some(found.written),
                    ..Entry::new(found.id, value, origin)
                })
            }
            Err(err) => Err(Warning::at(
                format!(
                    "{} expected {} but has `{}`",
                    found.written, err.expected, err.found
                ),
                origin,
            )
            .of(WarningKind::WrongType)),
        }
    }

    /// An entry for a dotted key whose value already has a shape.
    ///
    /// A file has structure of its own — an array is an array, a table is a table — and there is
    /// no text a named parser could turn into one, so a layer reading a structured format hands
    /// the value over as it found it. It still goes through the declared type, which is what
    /// keeps the promise that a value of the wrong type costs a warning and not a wrong value:
    /// a `map<string, string>` given a number inside it says so, rather than storing it.
    pub fn entry_from_value(
        &self,
        key: &str,
        value: Value,
        origin: Origin,
    ) -> Result<Entry, Warning> {
        let Some(found) = self.prop(key) else {
            return Err(Warning::at(format!("unknown setting `{key}`"), origin)
                .of(WarningKind::UnknownSetting));
        };
        let meta = self.registry.get(found.id);
        match meta.ty.coerce(value) {
            Ok(value) => {
                if let Some(refused) = self.refused(found.id, &value, found.written, &origin) {
                    return Err(refused);
                }
                Ok(Entry {
                    renamed_from: found.renamed_from,
                    written_key: Some(found.written),
                    ..Entry::new(found.id, value, origin)
                })
            }
            Err(err) => Err(Warning::at(
                format!(
                    "{} expected {} but has `{}`",
                    found.written, err.expected, err.found
                ),
                origin,
            )
            .of(WarningKind::WrongType)),
        }
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
    use super::WarningKind;
    use super::*;
    use crate::registry::PropMeta;
    use crate::ty::{Parser, Ty};
    use crate::value::Const;

    static PROPS: &[PropMeta] = &[
        PropMeta {
            aliases: &["parallelism"],
            ..PropMeta::new("jobs", Ty::Uint)
        },
        PropMeta {
            parse: Some(Parser::ListByComma),
            ..PropMeta::new("exclude", Ty::List(&Ty::String))
        },
        // A `bool` whose choices are written the way a person says them, which the coercion turns
        // into the same two values.
        PropMeta {
            choices: &[Const::Str("yes"), Const::Str("no")],
            ..PropMeta::new("colour", Ty::Bool)
        },
        // A type usage cannot know — a union — with its choices written as numbers. Nothing is
        // coerced here by declaration, so a value out of a file stays a string and the choice stays
        // an integer, and only their written forms can answer whether they are the same.
        PropMeta {
            choices: &[Const::Int(1), Const::Int(2)],
            ..PropMeta::new("level", Ty::Any)
        },
        // A setting the spec limits to three values, which is hk's `stash` and mise's nine
        // enum-valued settings.
        PropMeta {
            aliases: &["storage"],
            choices: &[
                Const::Str("git"),
                Const::Str("patch-file"),
                Const::Str("none"),
            ],
            ..PropMeta::new("stash", Ty::String)
        },
        // A list whose *items* are booleans, with the choices written as words. Unusual, and the one
        // shape where the item's own type is the only thing that makes the comparison work: read as
        // the list it is in, `yes` becomes a one-item list and matches nothing, and read as text it
        // is `yes` against `true`.
        PropMeta {
            parse: Some(Parser::ListByComma),
            choices: &[Const::Str("yes"), Const::Str("no")],
            ..PropMeta::new("flags", Ty::List(&Ty::Bool))
        },
        // Choices on a list: each *item* is one of them, the way the JSON schema reads it.
        PropMeta {
            parse: Some(Parser::ListByComma),
            choices: &[Const::Str("lint"), Const::Str("test")],
            ..PropMeta::new("skip", Ty::List(&Ty::String))
        },
    ];
    const REGISTRY: Registry = Registry::new(PROPS);

    #[test]
    fn a_value_the_spec_does_not_allow_is_refused_with_the_list_of_what_is() {
        // Choices reached the docs, the JSON schema and completions, and nothing that *resolved* a
        // value: a CLI documenting three allowed values took a fourth in silence, and failed later
        // somewhere that could not say why.
        let ctx = LayerCtx::new(REGISTRY);
        let origin = Origin::new(SourceKind::ENV, "HK_STASH");
        let warning = ctx
            .entry_for_key("stash", "svn", origin.clone())
            .expect_err("not one of the three");
        assert_eq!(
            warning.message,
            "stash expected one of git, patch-file, none but has `svn`"
        );
        // Which is a different sort of thing from a value of the wrong *type*, and a caller that
        // treats them differently needs to be able to tell.
        assert_eq!(warning.kind, WarningKind::NotAllowed);
        let alias_warning = ctx
            .entry_for_key("storage", "svn", origin.clone())
            .expect_err("alias should use the same choices");
        assert_eq!(
            alias_warning.message,
            "storage expected one of git, patch-file, none but has `svn`"
        );
        assert_eq!(
            ctx.entry_for_key("jobs", "lots", origin.clone())
                .expect_err("not a number")
                .kind,
            WarningKind::WrongType
        );
        // And a value that is one of them is just a value.
        assert_eq!(
            ctx.entry_for_key("stash", "git", origin.clone())
                .map(|entry| entry.value),
            Ok(Value::from("git"))
        );
        let alias = ctx.entry_for_key("storage", "git", origin.clone()).unwrap();
        assert_eq!(alias.written_key, Some("storage"));
        assert_eq!(alias.renamed_from, None);

        // A list is checked item by item, and the *item* is what the message quotes — naming the
        // whole list would leave the user to work out which of five items was the problem.
        let warning = ctx
            .entry_for_key("skip", "lint,fmt", origin.clone())
            .expect_err("`fmt` is not one of them");
        assert_eq!(
            warning.message,
            "skip expected one of lint, test but has `fmt`"
        );
        assert!(ctx.entry_for_key("skip", "lint,test", origin).is_ok());
    }

    #[test]
    fn a_choice_is_read_the_way_the_declared_type_reads_it() {
        // `choice "yes"` under `type="bool"`: the value `yes` is coerced to `true` on its way in, so
        // comparing the two as written refused a value the spec plainly allows. The choice is read
        // the same way the value was, which is the same question the coercion already answered.
        let ctx = LayerCtx::new(REGISTRY);
        let origin = Origin::new(SourceKind::ENV, "HK_COLOUR");
        assert_eq!(
            ctx.entry_for_key("colour", "yes", origin.clone())
                .map(|entry| entry.value),
            Ok(Value::Bool(true))
        );
        // `no` is the other declared choice, and `true` is neither of them written down — but it is
        // what `yes` reads as, so it is allowed for the same reason.
        assert_eq!(
            ctx.entry_for_key("colour", "no", origin.clone())
                .map(|entry| entry.value),
            Ok(Value::Bool(false))
        );
        assert_eq!(
            ctx.entry_for_key("colour", "true", origin.clone())
                .map(|entry| entry.value),
            Ok(Value::Bool(true))
        );

        // And inside a collection it is the *item's* type that reads the choice, not the list's:
        // read as the list, `yes` becomes a one-item list and matches nothing at all.
        assert_eq!(
            ctx.entry_for_key("flags", "yes,no", origin.clone())
                .map(|entry| entry.value),
            Ok(Value::List(vec![Value::Bool(true), Value::Bool(false)]))
        );
        // The type is asked first, which is why this says what it says: `maybe` is not a boolean, and
        // there is nothing useful to say about which *choice* it is not. With both spellings of a
        // boolean declared as choices, every boolean is one of them — so for this setting the choices
        // can only refuse what the type has already refused.
        let warning = ctx
            .entry_for_key("flags", "yes,maybe", origin)
            .expect_err("`maybe` is not a boolean");
        assert_eq!(warning.message, "flags expected a boolean but has `maybe`");
    }

    #[test]
    fn a_float_choice_is_the_number_the_spec_wrote() {
        // Rendered without its point, a whole-number float was the same text as an integer — so a
        // `choice 1.0` under a *string* type accepted `1` and refused the `1.0` the spec had written.
        static PROPS: &[PropMeta] = &[
            PropMeta {
                choices: &[Const::Float(1.0), Const::Float(1.5)],
                ..PropMeta::new("scale", Ty::String)
            },
            // And where the type *is* a float, the coercion settles it before any text is compared.
            PropMeta {
                choices: &[Const::Int(1), Const::Float(1.5)],
                ..PropMeta::new("ratio", Ty::Float)
            },
        ];
        const REGISTRY: Registry = Registry::new(PROPS);
        let ctx = LayerCtx::new(REGISTRY);
        let origin = Origin::new(SourceKind::ENV, "HK_SCALE");

        assert_eq!(
            ctx.entry_for_key("scale", "1.0", origin.clone())
                .map(|entry| entry.value),
            Ok(Value::from("1.0"))
        );
        let warning = ctx
            .entry_for_key("scale", "1", origin.clone())
            .expect_err("`1` is not `1.0`");
        assert_eq!(
            warning.message,
            "scale expected one of 1.0, 1.5 but has `1`"
        );

        // `choice 1` under `type="float"` is the float one, because that is what the type reads it
        // as — the case the point-less rendering used to settle by accident.
        assert_eq!(
            ctx.entry_for_key("ratio", "1", origin)
                .map(|entry| entry.value),
            Ok(Value::Float(1.0))
        );
    }

    #[test]
    fn a_type_nothing_coerces_compares_its_choices_as_written() {
        // `any` coerces nothing, by declaration — so a value arrives as the string a file or an
        // environment variable wrote, the choice stays the integer the spec wrote, and comparing them
        // after a coercion that did nothing refuses a value the spec allows.
        let ctx = LayerCtx::new(REGISTRY);
        let origin = Origin::new(SourceKind::ENV, "HK_LEVEL");
        assert_eq!(
            ctx.entry_for_key("level", "2", origin.clone())
                .map(|entry| entry.value),
            Ok(Value::from("2"))
        );
        let warning = ctx
            .entry_for_key("level", "3", origin.clone())
            .expect_err("not one of them");
        assert_eq!(warning.message, "level expected one of 1, 2 but has `3`");

        // And a *list* of them is not one of them. Nothing declared this setting to have items, so
        // there is nothing to walk into: following the value's shape instead, `[1]` was accepted for
        // `choice 1` — and for a type the spec left open, a value's shape is whatever a file wrote.
        let warning = ctx
            .entry_from_value("level", Value::List(vec![Value::Int(1)]), origin)
            .expect_err("a list of one choice is not that choice");
        assert_eq!(warning.message, "level expected one of 1, 2 but has `1`");
    }

    #[test]
    fn a_setting_with_no_choices_takes_what_its_type_takes() {
        // Most settings say nothing about their values, and the check has to cost them nothing and
        // refuse them nothing.
        let ctx = LayerCtx::new(REGISTRY);
        let origin = Origin::new(SourceKind::ENV, "HK_JOBS");
        assert_eq!(
            ctx.entry_for_key("jobs", "8", origin).map(|e| e.value),
            Ok(Value::Int(8))
        );
    }

    #[test]
    fn a_structured_value_is_held_to_the_same_choices() {
        // The other way a value arrives — a table or a list out of a file, which never passes
        // through a parser. Checking one path and not the other is how a rule ends up applying to
        // the environment and not to the file beside it.
        let ctx = LayerCtx::new(REGISTRY);
        let origin = Origin::new(SourceKind::FILE, "hk.toml");
        let warning = ctx
            .entry_from_value(
                "skip",
                Value::List(vec![Value::from("test"), Value::from("deploy")]),
                origin.clone(),
            )
            .expect_err("`deploy` is not one of them");
        assert_eq!(
            warning.message,
            "skip expected one of lint, test but has `deploy`"
        );
        assert!(ctx
            .entry_from_value("skip", Value::List(vec![Value::from("test")]), origin)
            .is_ok());
    }

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
        // The message says what is wrong; the `origin` says where. Naming the place in both
        // meant every renderer that shows the origin — as `explain::warnings` does — printed it
        // twice, for exactly the warnings that had bothered to be specific.
        assert_eq!(
            warning.message,
            "jobs expected a non-negative integer but has `lots`"
        );
        assert_eq!(warning.origin, Some(origin));

        let alias_warning = ctx
            .entry_for_key(
                "parallelism",
                "lots",
                Origin::new(SourceKind::FILE, "config.toml"),
            )
            .expect_err("alias value should still be checked");
        assert_eq!(
            alias_warning.message,
            "parallelism expected a non-negative integer but has `lots`"
        );

        let structured_alias_warning = ctx
            .entry_from_value(
                "parallelism",
                Value::from("lots"),
                Origin::new(SourceKind::FILE, "config.toml"),
            )
            .expect_err("structured alias value should still be checked");
        assert_eq!(structured_alias_warning.message, alias_warning.message);

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
