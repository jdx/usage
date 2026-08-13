//! A resolution read as the types a settings struct holds.
//!
//! The merge already validates every value against the type its setting declares, so most of
//! this cannot fail. Three things keep it from being a formality, and they are the reason the
//! errors here carry provenance rather than panicking:
//!
//! - A **post-merge hook** writes with [`Resolved::coerced`], which is deliberately unchecked:
//!   the hook is where a CLI puts the rules only it knows, and making it validate would make it
//!   a second merge. A hook that writes `-1` to a `uint` is a bug in the CLI, and the error has
//!   to say which setting and which hook — `mise`'s post-merge coercions touch a dozen settings.
//! - A type **only the tool understands** (`any`, a union) is not coerced by the merge at all,
//!   by declaration. The field that holds it is still concrete.
//! - The field type **narrows further** than the declared one: `uint` is an `i64` in the merge
//!   and a `u64` in the struct, and `int` is an `i64` that a field may hold as something
//!   smaller.
//!
//! Every failure is collected rather than returned at the first one. A user fixing a config file
//! wants the whole list — the fleet's hand-written folds return the first problem, so a file with
//! three bad values takes three runs to fix.

use std::collections::BTreeMap;
use std::fmt;
use std::path::PathBuf;

use crate::registry::PropId;
use crate::resolve::Resolved;
use crate::source::Origin;
use crate::ty::TypeError;
use crate::value::{one_line, Value};

/// A [`Value`] read as the type a field holds.
///
/// Strict: nothing here converts. Coercion happens once, in the merge, against the type the
/// spec declared — a reader that converted as well would be a second set of rules for what a
/// value means, which is the drift this crate exists to remove.
pub trait FromValue: Sized {
    /// Read `value`, or say what was expected and what arrived.
    fn from_value(value: &Value) -> Result<Self, TypeError>;
}

/// Why a setting could not be read as the type its field holds.
#[derive(Debug, Clone, PartialEq)]
pub struct ReadError {
    /// The setting's key, as the spec declares it.
    pub key: &'static str,
    /// Where the offending value came from. Absent when there is no value at all — nothing to
    /// have an origin.
    pub origin: Option<Origin>,
    pub kind: ReadErrorKind,
}

/// The two ways reading a setting fails.
#[derive(Debug, Clone, PartialEq)]
pub enum ReadErrorKind {
    /// A value of the wrong shape for the field.
    Type(TypeError),
    /// Nothing supplied a value, and the spec declares no default.
    Missing,
}

impl fmt::Display for ReadError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match &self.kind {
            // The same shape a layer's warning uses, so the two read alike when a CLI prints
            // both, plus the place to go and edit: "expected a non-negative integer" with no source
            // sends a user looking through every file in the chain.
            //
            // Through `one_line` for the same reason the explanation is: these are joined with
            // newlines below, and the two things interpolated here are the ones that can contain
            // one — a value out of a file, where a multi-line string is perfectly ordinary, and a
            // path. One failure spilling onto three lines hides the failures after it.
            ReadErrorKind::Type(err) => write!(
                f,
                "{} expected {} but has `{}`",
                self.key,
                err.expected,
                one_line(&err.found)
            )?,
            ReadErrorKind::Missing => write!(f, "{} has no value and no default", self.key)?,
        }
        if let Some(origin) = &self.origin {
            write!(f, " (set by {})", one_line(origin.describe()))?;
        }
        Ok(())
    }
}

/// Every setting that could not be read, in the order the registry declares them.
#[derive(Debug, Clone, PartialEq)]
pub struct ReadErrors(pub Vec<ReadError>);

impl fmt::Display for ReadErrors {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        for (i, error) in self.0.iter().enumerate() {
            if i > 0 {
                f.write_str("\n")?;
            }
            write!(f, "{error}")?;
        }
        Ok(())
    }
}

impl std::error::Error for ReadErrors {}

/// Reading a whole settings struct out of one resolution.
///
/// Generated code reads each field in turn and calls [`Fold::finish`] once, which is what makes
/// the errors a list rather than the first thing that went wrong.
pub struct Fold<'a> {
    resolved: &'a Resolved,
    errors: Vec<ReadError>,
}

impl Resolved {
    /// Start reading this resolution as typed values.
    pub fn fold(&self) -> Fold<'_> {
        Fold {
            resolved: self,
            errors: Vec::new(),
        }
    }

    /// One setting, as the type `T` holds, with the error naming where the value came from.
    ///
    /// For a CLI reading a handful of settings by hand. Generated code uses [`Resolved::fold`],
    /// which reports every bad value instead of this one.
    pub fn read<T: FromValue>(&self, id: PropId) -> Result<Option<T>, ReadError> {
        let mut fold = self.fold();
        let value = fold.optional(id);
        match fold.errors.pop() {
            Some(error) => Err(error),
            None => Ok(value),
        }
    }
}

impl Fold<'_> {
    /// A setting that may have no value, as `Option<T>`.
    ///
    /// Absence is not an error here: a setting with no default and nothing set is a field that
    /// holds `None`, which is what `option<T>` in the spec means.
    pub fn optional<T: FromValue>(&mut self, id: PropId) -> Option<T> {
        let value = self.resolved.get(id)?;
        match T::from_value(value) {
            Ok(value) => Some(value),
            Err(err) => {
                self.errors.push(ReadError {
                    key: self.resolved.registry().get(id).key,
                    origin: self.resolved.origin(id).cloned(),
                    kind: ReadErrorKind::Type(err),
                });
                None
            }
        }
    }

    /// A setting that must have a value, for a field that is not an `Option`.
    ///
    /// Returns `None` only when it has recorded an error, so code that has already called
    /// [`Fold::finish`] and found it `Ok` may unwrap what this returned. That is the contract
    /// generated code is written against: the fold reports, and the struct is built afterwards.
    pub fn required<T: FromValue>(&mut self, id: PropId) -> Option<T> {
        if self.resolved.get(id).is_none() {
            self.errors.push(ReadError {
                key: self.resolved.registry().get(id).key,
                origin: None,
                kind: ReadErrorKind::Missing,
            });
            return None;
        }
        self.optional(id)
    }

    /// What has gone wrong so far, for a caller that wants to add to the list.
    pub fn errors(&self) -> &[ReadError] {
        &self.errors
    }

    /// Every setting read, or every reason one could not be.
    pub fn finish(self) -> Result<(), ReadErrors> {
        if self.errors.is_empty() {
            Ok(())
        } else {
            Err(ReadErrors(self.errors))
        }
    }
}

/// The error for a value of the wrong shape, quoted the way it was written.
fn mismatch(expected: &'static str, value: &Value) -> TypeError {
    TypeError {
        expected,
        // Through the same renderer the merge's own warnings use, so an empty value is reported as
        // `[]` or `""` rather than as nothing at all: "expected a list but has ``" names no value.
        found: crate::value::shown(value),
    }
}

impl FromValue for bool {
    fn from_value(value: &Value) -> Result<Self, TypeError> {
        match value {
            Value::Bool(b) => Ok(*b),
            other => Err(mismatch("a boolean", other)),
        }
    }
}

impl FromValue for i64 {
    fn from_value(value: &Value) -> Result<Self, TypeError> {
        match value {
            Value::Int(i) => Ok(*i),
            other => Err(mismatch("an integer", other)),
        }
    }
}

impl FromValue for u64 {
    fn from_value(value: &Value) -> Result<Self, TypeError> {
        match value {
            // Not `as`: a negative one becomes an enormous positive one, which is the shape of
            // this bug in every codebase that has it. The merge refuses a negative `uint`, so
            // getting here means a post-merge hook wrote one.
            Value::Int(i) => {
                Self::try_from(*i).map_err(|_| mismatch("a non-negative integer", value))
            }
            other => Err(mismatch("a non-negative integer", other)),
        }
    }
}

impl FromValue for f64 {
    fn from_value(value: &Value) -> Result<Self, TypeError> {
        match value {
            Value::Float(f) => Ok(*f),
            // A whole number is a perfectly good float, which is the rule the merge follows
            // too: a spec that says `float` should not refuse `1`.
            Value::Int(i) => Ok(*i as Self),
            other => Err(mismatch("a number", other)),
        }
    }
}

impl FromValue for String {
    fn from_value(value: &Value) -> Result<Self, TypeError> {
        match value {
            Value::String(s) => Ok(s.clone()),
            other => Err(mismatch("a string", other)),
        }
    }
}

impl FromValue for PathBuf {
    fn from_value(value: &Value) -> Result<Self, TypeError> {
        match value {
            Value::String(s) => Ok(Self::from(s)),
            other => Err(mismatch("a path", other)),
        }
    }
}

impl<T: FromValue> FromValue for Vec<T> {
    fn from_value(value: &Value) -> Result<Self, TypeError> {
        match value {
            // A `set` is a list here: the merge has already dropped the duplicates, and a list
            // keeps the order the files were read in — which a sorted set would throw away for
            // settings like a `PATH` where the order is the meaning.
            Value::List(items) => items.iter().map(T::from_value).collect(),
            other => Err(mismatch("a list", other)),
        }
    }
}

impl<T: FromValue> FromValue for BTreeMap<String, T> {
    fn from_value(value: &Value) -> Result<Self, TypeError> {
        match value {
            Value::Map(entries) => entries
                .iter()
                .map(|(key, value)| T::from_value(value).map(|value| (key.clone(), value)))
                .collect(),
            other => Err(mismatch("a table", other)),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::layer::{Layer, LayerCtx, LayerError, LayerOutput};
    use crate::registry::{PropMeta, Registry};
    use crate::resolve::{resolve, Layers};
    use crate::source::SourceKind;
    use crate::ty::{Parser, Ty};
    use crate::value::Const;

    static PROPS: &[PropMeta] = &[
        PropMeta {
            default: Some(Const::Int(4)),
            envs: &["MYCLI_JOBS"],
            ..PropMeta::new("jobs", Ty::Uint)
        },
        PropMeta {
            default: Some(Const::Bool(false)),
            envs: &["MYCLI_RAW"],
            ..PropMeta::new("raw", Ty::Bool)
        },
        PropMeta {
            envs: &["MYCLI_CACHE_DIR"],
            ..PropMeta::new("cache_dir", Ty::Option(&Ty::Path))
        },
        // Text splitting is the named parser's job, not the reader's: these arrive from an
        // environment variable as one string.
        PropMeta {
            envs: &["MYCLI_EXCLUDE"],
            parse: Some(Parser::ListByComma),
            ..PropMeta::new("exclude", Ty::List(&Ty::String))
        },
        PropMeta {
            envs: &["MYCLI_PORTS"],
            parse: Some(Parser::ListByComma),
            ..PropMeta::new("ports", Ty::List(&Ty::Uint))
        },
        PropMeta {
            envs: &["MYCLI_ALIASES"],
            ..PropMeta::new("aliases", Ty::Map(&Ty::String))
        },
        PropMeta {
            envs: &["MYCLI_RATIO"],
            ..PropMeta::new("ratio", Ty::Float)
        },
        // No default and not an option: a setting the CLI must have and nobody has supplied.
        PropMeta::new("profile", Ty::String),
    ];
    const REGISTRY: Registry = Registry::new(PROPS);

    fn id(key: &str) -> PropId {
        REGISTRY.lookup(key).expect("declared").id
    }

    /// Values as text, the way an environment variable arrives.
    struct Text(&'static [(&'static str, &'static str)]);

    impl Layer for Text {
        fn source(&self) -> SourceKind {
            SourceKind::ENV
        }
        fn load(&self, ctx: &LayerCtx) -> Result<LayerOutput, LayerError> {
            let mut out = LayerOutput::new();
            for (key, raw) in self.0 {
                let origin = Origin::new(SourceKind::ENV, format!("MYCLI_{}", key.to_uppercase()));
                match ctx.entry_for_key(key, raw, origin) {
                    Ok(entry) => out.push(entry),
                    Err(warning) => out.warn(warning),
                }
            }
            Ok(out)
        }
    }

    #[test]
    fn a_resolution_reads_as_the_types_a_struct_holds() {
        let layer = Text(&[
            ("jobs", "8"),
            ("raw", "yes"),
            ("cache_dir", "/tmp/cache"),
            ("exclude", "target,dist"),
            ("ports", "80,443"),
            ("ratio", "0.5"),
            ("profile", "release"),
        ]);
        let resolved = resolve(REGISTRY, Layers::new().then(&layer)).expect("resolves");

        let mut fold = resolved.fold();
        let jobs: Option<u64> = fold.required(id("jobs"));
        let raw: Option<bool> = fold.required(id("raw"));
        let cache_dir: Option<PathBuf> = fold.optional(id("cache_dir"));
        let exclude: Option<Vec<String>> = fold.required(id("exclude"));
        let ports: Option<Vec<u64>> = fold.required(id("ports"));
        let ratio: Option<f64> = fold.required(id("ratio"));
        let profile: Option<String> = fold.required(id("profile"));
        fold.finish().expect("every value fits its field");

        assert_eq!(jobs, Some(8));
        assert_eq!(raw, Some(true));
        assert_eq!(cache_dir, Some(PathBuf::from("/tmp/cache")));
        assert_eq!(
            exclude,
            Some(vec!["target".to_string(), "dist".to_string()]),
            "a list-typed setting keeps the order the file gave it"
        );
        assert_eq!(
            ports,
            Some(vec![80, 443]),
            "and reads its items as the type"
        );
        assert_eq!(ratio, Some(0.5));
        assert_eq!(profile, Some("release".to_string()));
    }

    #[test]
    fn a_declared_default_is_read_like_any_other_value() {
        // Nothing supplied a thing, so this reads the seeded defaults — the case where a
        // generated struct is built from the registry alone.
        let resolved = resolve(REGISTRY, Layers::new()).expect("resolves");
        let mut fold = resolved.fold();
        let jobs: Option<u64> = fold.required(id("jobs"));
        let cache_dir: Option<PathBuf> = fold.optional(id("cache_dir"));
        let exclude: Option<Vec<String>> = fold.optional(id("exclude"));
        assert_eq!(jobs, Some(4));
        assert_eq!(cache_dir, None, "no default, and absence is not an error");
        assert_eq!(exclude, None);
    }

    #[test]
    fn a_setting_with_no_value_and_no_default_says_which_one() {
        // A field that is not an `Option` and has nothing to hold. Reported rather than
        // unwrapped, because the CLI's own registry is what is wrong and the author needs the
        // key to fix it.
        let resolved = resolve(REGISTRY, Layers::new()).expect("resolves");
        let mut fold = resolved.fold();
        let profile: Option<String> = fold.required(id("profile"));
        assert_eq!(profile, None);
        let err = fold.finish().expect_err("should not read");
        assert_eq!(err.to_string(), "profile has no value and no default");
    }

    #[test]
    fn a_value_the_field_cannot_hold_names_where_it_came_from() {
        // The reachable failure: a hook writing past the declared type. `Resolved::coerced` is
        // unchecked by design — it is where a CLI puts the rules only it knows — so this is
        // where writing `-1` to a `uint` is caught, and the message has to name the hook rather
        // than leave the author guessing which of a dozen coercions did it.
        let mut resolved = resolve(REGISTRY, Layers::new()).expect("resolves");
        resolved.coerced(id("jobs"), Value::Int(-1), "one job when raw");

        let mut fold = resolved.fold();
        let jobs: Option<u64> = fold.required(id("jobs"));
        assert_eq!(jobs, None);
        let err = fold.finish().expect_err("should not read");
        assert_eq!(
            err.to_string(),
            "jobs expected a non-negative integer but has `-1` (set by one job when raw)"
        );
    }

    #[test]
    fn every_bad_value_is_reported_and_not_only_the_first() {
        // Three settings the fields cannot hold. The fleet's hand-written folds return the
        // first, so a config file with three mistakes in it takes three runs to fix.
        let mut resolved = resolve(REGISTRY, Layers::new()).expect("resolves");
        resolved.coerced(id("jobs"), Value::Int(-1), "a hook");
        resolved.coerced(id("raw"), Value::String("sometimes".into()), "a hook");
        resolved.coerced(
            id("ports"),
            Value::List(vec![Value::Int(80), Value::Int(-443)]),
            "a hook",
        );

        let mut fold = resolved.fold();
        let _: Option<u64> = fold.required(id("jobs"));
        let _: Option<bool> = fold.required(id("raw"));
        let _: Option<Vec<u64>> = fold.required(id("ports"));
        let _: Option<String> = fold.required(id("profile"));
        let err = fold.finish().expect_err("should not read");

        let message = err.to_string();
        let lines: Vec<&str> = message.lines().collect();
        assert_eq!(lines.len(), 4, "{err}");
        assert!(
            lines[0].starts_with("jobs expected a non-negative integer"),
            "{err}"
        );
        assert!(
            lines[1].starts_with("raw expected a boolean but has `sometimes`"),
            "{err}"
        );
        // The item is what is wrong, and the item is what the message quotes.
        assert!(
            lines[2].starts_with("ports expected a non-negative integer but has `-443`"),
            "{err}"
        );
        assert!(lines[3].starts_with("profile has no value"), "{err}");
    }

    #[test]
    fn a_failure_stays_on_its_own_line_whatever_the_value_holds() {
        // A multi-line string is perfectly ordinary in TOML, and a path may contain a newline.
        // Interpolated as they are, one failure spilled across three lines and the failures listed
        // after it read as part of it.
        let mut resolved = resolve(REGISTRY, Layers::new()).expect("resolves");
        resolved.coerced(
            id("jobs"),
            Value::String("two\nor three".into()),
            "a hook\nover two lines",
        );
        let mut fold = resolved.fold();
        let _: Option<u64> = fold.required(id("jobs"));
        let _: Option<String> = fold.required(id("profile"));
        let err = fold.finish().expect_err("should not read");

        let message = err.to_string();
        assert_eq!(message.lines().count(), 2, "{message}");
        assert!(
            message.starts_with(
                "jobs expected a non-negative integer but has `two\\nor three` \
                 (set by a hook\\nover two lines)"
            ),
            "{message}"
        );
    }

    #[test]
    fn a_failure_about_an_empty_value_still_names_one() {
        // An emptied list is a value, and one a user can perfectly well have arrived at:
        // `MYCLI_EXCLUDE=` clears a declared default. Quoted as its own text it named nothing —
        // "expected a non-negative integer but has ``" — so it is reported as its shape, the same
        // way the merge's warnings and `explain` report it.
        let mut resolved = resolve(REGISTRY, Layers::new()).expect("resolves");
        resolved.coerced(id("jobs"), Value::List(Vec::new()), "a hook");
        let mut fold = resolved.fold();
        let jobs: Option<u64> = fold.required(id("jobs"));
        assert_eq!(jobs, None);
        let err = fold.finish().expect_err("a list is not an integer");
        assert_eq!(
            err.to_string(),
            "jobs expected a non-negative integer but has `[]` (set by a hook)"
        );
    }

    #[test]
    fn a_type_only_the_tool_understands_is_read_as_whatever_the_field_says() {
        // `any` is not coerced by the merge, by declaration — so the field type is the only
        // thing that says what belongs, and it is also the only place a mismatch can be caught.
        static ANY: &[PropMeta] = &[PropMeta::new("either", Ty::Any)];
        const ANY_REGISTRY: Registry = Registry::new(ANY);
        let layer = Text(&[]);
        let mut resolved = resolve(ANY_REGISTRY, Layers::new().then(&layer)).expect("resolves");
        let id = ANY_REGISTRY.lookup("either").expect("declared").id;
        resolved.coerced(id, Value::Map(BTreeMap::new()), "a hook");

        let mut fold = resolved.fold();
        let text: Option<String> = fold.optional(id);
        assert_eq!(text, None);
        let err = fold.finish().expect_err("a table is not a string");
        assert!(
            err.to_string().starts_with("either expected a string"),
            "{err}"
        );
    }

    #[test]
    fn a_table_setting_reads_as_a_map_of_the_declared_type() {
        let layer = Text(&[("aliases", "lts")]);
        let mut resolved = resolve(REGISTRY, Layers::new().then(&layer)).expect("resolves");
        resolved.coerced(
            id("aliases"),
            Value::Map(
                [("node".to_string(), Value::from("20"))]
                    .into_iter()
                    .collect(),
            ),
            "a hook",
        );
        let mut fold = resolved.fold();
        let aliases: Option<BTreeMap<String, String>> = fold.optional(id("aliases"));
        fold.finish().expect("reads");
        assert_eq!(
            aliases,
            Some(
                [("node".to_string(), "20".to_string())]
                    .into_iter()
                    .collect()
            )
        );
    }

    #[test]
    fn one_setting_can_be_read_without_a_fold() {
        // For a CLI reading a couple of settings by hand rather than generating a struct.
        let layer = Text(&[("jobs", "12")]);
        let resolved = resolve(REGISTRY, Layers::new().then(&layer)).expect("resolves");
        assert_eq!(resolved.read::<u64>(id("jobs")), Ok(Some(12)));
        assert_eq!(resolved.read::<PathBuf>(id("cache_dir")), Ok(None));
        let err = resolved
            .read::<bool>(id("jobs"))
            .expect_err("not a boolean");
        assert_eq!(
            err.to_string(),
            "jobs expected a boolean but has `12` (set by MYCLI_JOBS)"
        );
    }
}
