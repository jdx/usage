//! The type a setting was declared with, and reading a raw string as it.
//!
//! A trimmed-down runtime form of the spec's type grammar: enough to coerce and validate,
//! with none of the parsing. `usage-config-build` turns `list<string>` into
//! `Ty::List(&Ty::String)` at build time, so the shape a value must take costs a match
//! rather than a parse.
//!
//! Every layer that reads text — the environment, an `.npmrc`, a git config — hands over a
//! string, and the declared type is the only thing that says whether `"1"` is the number
//! one, the string "1", or a one-element list.

use crate::value::Value;

/// A declared type, as a generated registry holds it.
///
/// Containers borrow so the whole thing is `const`-constructible:
/// `Ty::List(&Ty::String)`.
#[derive(Debug, Copy, Clone, PartialEq)]
pub enum Ty {
    Bool,
    Int,
    /// An integer that may not be negative.
    Uint,
    Float,
    String,
    /// A filesystem path. Read as a string here; what makes it a path is what the CLI does
    /// with it, and refusing one because it does not exist yet would be wrong.
    Path,
    Url,
    /// A span of time, as text — `"30s"`, `"1h"`. Not parsed here: the crate that owns the
    /// duration type owns its spelling, and the generated struct is where it is turned into
    /// one.
    Duration,
    /// A table whose keys the spec does not describe.
    Object,
    List(&'static Ty),
    /// Like a list, but duplicates are dropped on merge.
    Set(&'static Ty),
    /// A table with values of one type.
    Map(&'static Ty),
    /// Absent is a legitimate state. Only meaningful about the setting as a whole, so
    /// coercion looks straight through it.
    Option(&'static Ty),
    /// A union, or a type only the tool understands. Nothing is coerced and nothing is
    /// refused: the spec said usage cannot know what belongs here, so it takes what it is
    /// given.
    Any,
}

/// Why a value could not be read as the type its setting declares.
#[derive(Debug, Clone, PartialEq)]
pub struct TypeError {
    /// The type as a human reads it: "an integer".
    pub expected: &'static str,
    /// What arrived instead, quoted the way it was written.
    pub found: String,
}

impl Ty {
    /// The innermost type, looking through `option`.
    pub fn inner(self) -> Ty {
        match self {
            Self::Option(inner) => inner.inner(),
            other => other,
        }
    }

    /// This type as the spec spells it: `uint`, `list<string>`, `option<path>`.
    ///
    /// Distinct from [`Ty::describe`], which is prose for an error message. An explanation shows
    /// the author's own vocabulary, because that is what a reader will search the docs for —
    /// "type a non-negative integer" sends them looking for something no spec says.
    pub fn name(self) -> String {
        match self {
            Self::Bool => "bool".into(),
            Self::Int => "int".into(),
            Self::Uint => "uint".into(),
            Self::Float => "float".into(),
            Self::String => "string".into(),
            Self::Path => "path".into(),
            Self::Url => "url".into(),
            Self::Duration => "duration".into(),
            Self::Object => "object".into(),
            Self::List(inner) => format!("list<{}>", inner.name()),
            Self::Set(inner) => format!("set<{}>", inner.name()),
            Self::Map(value) => format!("map<string, {}>", value.name()),
            Self::Option(inner) => format!("option<{}>", inner.name()),
            // A union or a type only the tool understands: the registry keeps no spelling for
            // it, and inventing one would be worse than admitting the fact.
            Self::Any => "any".into(),
        }
    }

    /// The name of this type as an error message should say it.
    pub fn describe(self) -> &'static str {
        match self.inner() {
            Self::Bool => "a boolean",
            Self::Int => "an integer",
            Self::Uint => "a non-negative integer",
            Self::Float => "a number",
            Self::String => "a string",
            Self::Path => "a path",
            Self::Url => "a URL",
            Self::Duration => "a duration",
            Self::Object | Self::Map(_) => "a table",
            Self::List(_) | Self::Set(_) => "a list",
            Self::Option(_) | Self::Any => "a value",
        }
    }

    /// `value` read as this type.
    ///
    /// Text arriving from a layer that has no types of its own is converted; a value that
    /// already has the right shape passes through untouched. Anything else is an error
    /// rather than a silent reinterpretation — the whole point of declaring the type.
    pub fn coerce(self, value: Value) -> Result<Value, TypeError> {
        let ty = self.inner();
        // A list-typed setting given one bare value means a list of one. Every registry in
        // the fleet relies on this for `MISE_ENV=production`, and doing it here means no
        // layer has to know.
        if let (
            Self::List(item) | Self::Set(item),
            Value::Bool(_) | Value::Int(_) | Value::Float(_) | Value::String(_),
        ) = (ty, &value)
        {
            // An empty string is no items, not one empty item — the same rule the named
            // parsers follow, and the one `HK_EXCLUDE=` relies on to turn a declared default
            // off. Wrapping it produced a list holding `""`, which cleared nothing and added
            // an item nobody asked for.
            if matches!(&value, Value::String(text) if text.is_empty()) {
                return Ok(Value::List(Vec::new()));
            }
            return Ok(Value::List(vec![item.coerce(value)?]));
        }
        match (ty, value) {
            // Nothing to say about a type nothing was declared for.
            (Self::Any, value) => Ok(value),

            (Self::Bool, Value::Bool(b)) => Ok(Value::Bool(b)),
            (Self::Bool, Value::String(text)) => match text.as_str() {
                // The spellings every one of these registries accepts. Deliberately not
                // "anything non-empty is true": `FOO=false` meaning true is the kind of
                // surprise a config system exists to prevent.
                "true" | "1" | "yes" | "y" | "on" => Ok(Value::Bool(true)),
                "false" | "0" | "no" | "n" | "off" | "" => Ok(Value::Bool(false)),
                _ => Err(TypeError {
                    expected: "a boolean",
                    found: text,
                }),
            },

            (Self::Int | Self::Uint, Value::Int(i)) if ty != Self::Uint || i >= 0 => {
                Ok(Value::Int(i))
            }
            (Self::Int | Self::Uint, Value::String(text)) => match text.trim().parse::<i64>() {
                Ok(i) if ty != Self::Uint || i >= 0 => Ok(Value::Int(i)),
                _ => Err(TypeError {
                    expected: ty.describe(),
                    found: text,
                }),
            },

            (Self::Float, Value::Float(f)) => Ok(Value::Float(f)),
            // A whole number is a perfectly good float, and a spec that says `float` should
            // not reject `1`.
            (Self::Float, Value::Int(i)) => Ok(Value::Float(i as f64)),
            (Self::Float, Value::String(text)) => match text.trim().parse::<f64>() {
                Ok(f) => Ok(Value::Float(f)),
                Err(_) => Err(TypeError {
                    expected: "a number",
                    found: text,
                }),
            },

            (Self::String | Self::Path | Self::Url | Self::Duration, Value::String(s)) => {
                Ok(Value::String(s))
            }
            // A number written where text was expected is text that happens to look like a
            // number — `MISE_PYTHON_VERSION=3` should not fail. A *collection* is not text,
            // though: rendering one gave `"k=v"` or `"a,b"`, which is a value nobody wrote, and
            // for a structured source it turned a table the file really did contain into a
            // string that only looks like one.
            (
                Self::String | Self::Path | Self::Url | Self::Duration,
                found @ (Value::List(_) | Value::Map(_)),
            ) => Err(TypeError {
                expected: ty.describe(),
                found: crate::value::shown(&found),
            }),
            (Self::String | Self::Path | Self::Url | Self::Duration, other) => {
                Ok(Value::String(other.display()))
            }

            (Self::List(item) | Self::Set(item), Value::List(items)) => Ok(Value::List(
                items
                    .into_iter()
                    .map(|value| item.coerce(value))
                    .collect::<Result<Vec<_>, _>>()?,
            )),

            (Self::Object, Value::Map(entries)) => Ok(Value::Map(entries)),
            (Self::Map(item), Value::Map(entries)) => Ok(Value::Map(
                entries
                    .into_iter()
                    .map(|(key, value)| item.coerce(value).map(|value| (key, value)))
                    .collect::<Result<_, _>>()?,
            )),

            (ty, found) => Err(TypeError {
                expected: ty.describe(),
                found: crate::value::shown(&found),
            }),
        }
    }
}

/// A named way of splitting one string into several values.
///
/// Spec vocabulary rather than a Rust callback, so a spec that says `parse="list_by_comma"`
/// means the same thing to a Go or a TypeScript runtime reading the same file. A parser a
/// tool has written itself rides as an `x` extension and never reaches here.
#[derive(Debug, Copy, Clone, PartialEq)]
pub enum Parser {
    ListByComma,
    ListByColon,
    /// `:` or `;`, whichever this platform uses between path entries.
    ListByOsPathSeparator,
    /// Splits on commas and drops repeats, keeping the first of each.
    SetByComma,
}

impl Parser {
    /// The name a spec writes.
    pub fn name(self) -> &'static str {
        match self {
            Self::ListByComma => "list_by_comma",
            Self::ListByColon => "list_by_colon",
            Self::ListByOsPathSeparator => "list_by_os_path_separator",
            Self::SetByComma => "set_by_comma",
        }
    }

    /// This parser by the name a spec writes.
    pub fn from_name(name: &str) -> Option<Self> {
        match name {
            "list_by_comma" => Some(Self::ListByComma),
            "list_by_colon" => Some(Self::ListByColon),
            "list_by_os_path_separator" => Some(Self::ListByOsPathSeparator),
            "set_by_comma" => Some(Self::SetByComma),
            _ => None,
        }
    }

    /// `raw` split into the values it names.
    ///
    /// An empty string is an empty list rather than a list holding nothing — `HK_EXCLUDE=`
    /// means "exclude nothing", which is a thing a user says to override a default.
    pub fn split(self, raw: &str) -> Value {
        let separator = match self {
            Self::ListByComma | Self::SetByComma => ',',
            Self::ListByColon => ':',
            Self::ListByOsPathSeparator => {
                if cfg!(windows) {
                    ';'
                } else {
                    ':'
                }
            }
        };
        if raw.is_empty() {
            return Value::List(Vec::new());
        }
        let mut parts: Vec<&str> = raw.split(separator).map(str::trim).collect();
        if self == Self::SetByComma {
            let mut seen = Vec::new();
            parts.retain(|part| {
                let fresh = !seen.contains(part);
                if fresh {
                    seen.push(*part);
                }
                fresh
            });
        }
        Value::List(parts.into_iter().map(Value::from).collect())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn s(text: &str) -> Value {
        Value::String(text.to_string())
    }

    #[test]
    fn text_from_a_layer_with_no_types_is_read_as_declared() {
        // Every environment variable arrives as a string, so this is the path most values
        // in a real CLI take.
        assert_eq!(Ty::Bool.coerce(s("yes")), Ok(Value::Bool(true)));
        assert_eq!(Ty::Bool.coerce(s("off")), Ok(Value::Bool(false)));
        assert_eq!(Ty::Int.coerce(s("-3")), Ok(Value::Int(-3)));
        assert_eq!(Ty::Float.coerce(s(" 1.5 ")), Ok(Value::Float(1.5)));
        // Whitespace around a number is a typo, not a different number.
        assert_eq!(Ty::Int.coerce(s(" 4 ")), Ok(Value::Int(4)));
    }

    #[test]
    fn a_collection_is_not_text() {
        // A number written where text was expected is text that happens to look like one. A list
        // or a table is not: rendering one produced `a,b` or `k=v`, a value nobody wrote — and
        // for a file, which really can hold a table, it turned that table into a string that only
        // looks like one.
        assert!(Ty::String
            .coerce(Value::List(vec![Value::from("a")]))
            .is_err());
        assert!(Ty::Path
            .coerce(Value::Map(
                [("k".to_string(), Value::from("v"))].into_iter().collect()
            ))
            .is_err());
        // A scalar still converts, which is the rule this is narrowing rather than replacing.
        assert_eq!(Ty::String.coerce(Value::Int(3)), Ok(s("3")));
    }

    #[test]
    fn a_value_that_cannot_be_the_declared_type_is_an_error() {
        // The error carries what arrived, because "expected an integer" without the value is
        // no help when it came from a file three directories up.
        assert_eq!(
            Ty::Int.coerce(s("abc")),
            Err(TypeError {
                expected: "an integer",
                found: "abc".to_string()
            })
        );
        // `FOO=maybe` for a boolean is a mistake worth reporting rather than reading as true.
        assert!(Ty::Bool.coerce(s("maybe")).is_err());
        // A negative number where only positives belong.
        assert!(Ty::Uint.coerce(s("-1")).is_err());
        assert!(Ty::Uint.coerce(Value::Int(-1)).is_err());
        assert_eq!(Ty::Uint.coerce(Value::Int(0)), Ok(Value::Int(0)));
    }

    #[test]
    fn one_value_where_a_list_belongs_is_a_list_of_one() {
        // `MISE_ENV=production`, which every registry in the fleet accepts and no layer
        // should have to know about.
        const ITEM: &Ty = &Ty::String;
        assert_eq!(
            Ty::List(ITEM).coerce(s("production")),
            Ok(Value::List(vec![s("production")]))
        );
        // And the items of a real list are coerced too, so a list of ints from a JSON file
        // full of strings still arrives as ints.
        const INT: &Ty = &Ty::Int;
        assert_eq!(
            Ty::List(INT).coerce(Value::List(vec![s("1"), Value::Int(2)])),
            Ok(Value::List(vec![Value::Int(1), Value::Int(2)]))
        );
        assert!(Ty::List(INT).coerce(Value::List(vec![s("x")])).is_err());
    }

    #[test]
    fn an_empty_string_is_no_items_rather_than_one_empty_one() {
        // What `HK_EXCLUDE=` means, and the rule the named parsers already follow. Wrapping it
        // as a one-element list holding `""` added an item nobody asked for, and — since an
        // empty list is how a higher layer clears a declared default — left the default in
        // place for exactly the setting the user was trying to empty.
        const ITEM: &Ty = &Ty::String;
        assert_eq!(Ty::List(ITEM).coerce(s("")), Ok(Value::List(Vec::new())));
        assert_eq!(Ty::Set(ITEM).coerce(s("")), Ok(Value::List(Vec::new())));
        // A non-empty bare value is still a list of one.
        assert_eq!(
            Ty::List(ITEM).coerce(s("only")),
            Ok(Value::List(vec![s("only")]))
        );
        // And an empty string is still a perfectly good *string*.
        assert_eq!(Ty::String.coerce(s("")), Ok(s("")));
    }

    #[test]
    fn a_type_usage_cannot_know_takes_what_it_is_given() {
        // The escape hatch: a union or a tool-private type. Refusing here would make the
        // spec's own escape hatch unusable.
        assert_eq!(Ty::Any.coerce(s("either")), Ok(s("either")));
        assert_eq!(Ty::Any.coerce(Value::Bool(true)), Ok(Value::Bool(true)));
        // And `option<T>` is coerced as its inner type, since absence is about the setting
        // rather than about the value that did arrive.
        const INNER: &Ty = &Ty::Int;
        assert_eq!(Ty::Option(INNER).coerce(s("7")), Ok(Value::Int(7)));
    }

    #[test]
    fn a_named_parser_splits_one_string_the_way_the_spec_says() {
        assert_eq!(
            Parser::ListByComma.split("a, b,c"),
            Value::List(vec![s("a"), s("b"), s("c")])
        );
        // A set keeps the first of each, so the position of a value is stable.
        assert_eq!(
            Parser::SetByComma.split("a,b,a"),
            Value::List(vec![s("a"), s("b")])
        );
        // Emptying a list is a thing a user does to override a default, so it has to be
        // expressible: `HK_EXCLUDE=` is no items, not one empty one.
        assert_eq!(Parser::ListByComma.split(""), Value::List(Vec::new()));
        // Round-tripping the name is what lets a spec and another language's runtime agree.
        for parser in [
            Parser::ListByComma,
            Parser::ListByColon,
            Parser::ListByOsPathSeparator,
            Parser::SetByComma,
        ] {
            assert_eq!(Parser::from_name(parser.name()), Some(parser));
        }
        assert_eq!(Parser::from_name("list_by_semicolon"), None);
    }
}
