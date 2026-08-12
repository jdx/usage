//! The type a config property's values take.
//!
//! A small expression grammar rather than an enum of names, because the fleet's registries
//! need composition — `list<string>`, `map<string,string>`, `option<path>`, and the
//! occasional union like `bool|string`. Anything unrecognized parses as
//! [`Base::Custom`] and is preserved verbatim: a spec written for a newer usage, or one
//! naming a type only its own tool understands, keeps working rather than failing to load.

use std::fmt;
use std::str::FromStr;

use serde::Serialize;

use crate::error::UsageErr;

/// A named, non-composite type.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum Base {
    Bool,
    String,
    Int,
    Uint,
    Float,
    Path,
    Url,
    Duration,
    /// A free-form table: keys and values the spec does not describe.
    Object,
    /// A name this version does not know.
    ///
    /// Not an error: `data_type` used to be five values and a spec that names a sixth
    /// should load. Consumers that need a type they understand treat it as a string, which
    /// is what a schema generator can always do.
    Custom(String),
}

impl fmt::Display for Base {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        let name = match self {
            Self::Bool => "bool",
            Self::String => "string",
            Self::Int => "int",
            Self::Uint => "uint",
            Self::Float => "float",
            Self::Path => "path",
            Self::Url => "url",
            Self::Duration => "duration",
            Self::Object => "object",
            Self::Custom(name) => name,
        };
        f.write_str(name)
    }
}

impl From<&str> for Base {
    fn from(name: &str) -> Self {
        match name {
            "bool" | "boolean" => Self::Bool,
            "string" | "str" => Self::String,
            "int" | "integer" => Self::Int,
            "uint" | "usize" => Self::Uint,
            "float" | "number" => Self::Float,
            "path" => Self::Path,
            "url" => Self::Url,
            "duration" => Self::Duration,
            "object" | "table" => Self::Object,
            other => Self::Custom(other.to_string()),
        }
    }
}

/// A config property's type, as written.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[serde(rename_all = "snake_case", tag = "kind", content = "of")]
pub enum SpecConfigType {
    Base(Base),
    /// `list<T>` — ordered, duplicates kept.
    List(Box<SpecConfigType>),
    /// `set<T>` — deduplicated.
    Set(Box<SpecConfigType>),
    /// `map<K, V>` — the key is always a base type.
    Map(Base, Box<SpecConfigType>),
    /// `option<T>` — may be absent, with no default standing in.
    Option(Box<SpecConfigType>),
    /// `a|b` — one of several. Only what a spec can express; nothing validates which.
    Union(Vec<SpecConfigType>),
}

impl Default for SpecConfigType {
    fn default() -> Self {
        Self::Base(Base::String)
    }
}

impl SpecConfigType {
    /// The type this collapses to for a consumer that only handles one shape.
    ///
    /// A union's first member, an option's inner type: what a schema generator writes when
    /// it has to pick. Kept here rather than in each generator so they agree.
    pub fn simplified(&self) -> &SpecConfigType {
        match self {
            Self::Option(inner) => inner.simplified(),
            Self::Union(members) => members.first().map_or(self, |m| m.simplified()),
            other => other,
        }
    }

    /// Whether a value may be absent with nothing standing in for it.
    pub fn is_optional(&self) -> bool {
        matches!(self, Self::Option(_))
    }
}

impl fmt::Display for SpecConfigType {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Base(base) => write!(f, "{base}"),
            Self::List(inner) => write!(f, "list<{inner}>"),
            Self::Set(inner) => write!(f, "set<{inner}>"),
            Self::Map(key, value) => write!(f, "map<{key}, {value}>"),
            Self::Option(inner) => write!(f, "option<{inner}>"),
            Self::Union(members) => {
                let rendered: Vec<String> = members.iter().map(|m| m.to_string()).collect();
                f.write_str(&rendered.join("|"))
            }
        }
    }
}

impl FromStr for SpecConfigType {
    type Err = UsageErr;

    fn from_str(text: &str) -> Result<Self, Self::Err> {
        parse_union(text.trim())
    }
}

fn invalid(text: &str, why: &str) -> UsageErr {
    UsageErr::InvalidInput(
        format!("`{text}` is not a config type: {why}"),
        (0, 0).into(),
        miette::NamedSource::new("", String::new()),
    )
}

/// `a|b|c`, splitting only on bars outside angle brackets.
fn parse_union(text: &str) -> Result<SpecConfigType, UsageErr> {
    let members = split_top_level(text, '|');
    match members.as_slice() {
        [] => Err(invalid(text, "it is empty")),
        [one] => parse_single(one),
        many => Ok(SpecConfigType::Union(
            many.iter()
                .map(|m| parse_single(m))
                .collect::<Result<Vec<_>, _>>()?,
        )),
    }
}

fn parse_single(text: &str) -> Result<SpecConfigType, UsageErr> {
    let text = text.trim();
    if text.is_empty() {
        return Err(invalid(text, "it is empty"));
    }
    let Some(open) = text.find('<') else {
        return Ok(SpecConfigType::Base(Base::from(text)));
    };
    if !text.ends_with('>') {
        return Err(invalid(text, "a `<` without a closing `>`"));
    }
    let name = text[..open].trim();
    let inner = &text[open + 1..text.len() - 1];
    match name {
        "list" | "array" => Ok(SpecConfigType::List(Box::new(parse_union(inner)?))),
        "set" => Ok(SpecConfigType::Set(Box::new(parse_union(inner)?))),
        "option" | "optional" => Ok(SpecConfigType::Option(Box::new(parse_union(inner)?))),
        "map" | "table" => {
            let parts = split_top_level(inner, ',');
            match parts.as_slice() {
                // `map<string>` is a map from strings to that type, which is what every
                // registry in the fleet means by a one-argument map.
                [value] => Ok(SpecConfigType::Map(
                    Base::String,
                    Box::new(parse_union(value)?),
                )),
                [key, value] => {
                    let key = key.trim();
                    if key.contains('<') {
                        return Err(invalid(text, "a map's key is a plain type"));
                    }
                    Ok(SpecConfigType::Map(
                        Base::from(key),
                        Box::new(parse_union(value)?),
                    ))
                }
                _ => Err(invalid(text, "a map takes a key and a value")),
            }
        }
        other => Err(invalid(
            text,
            &format!("`{other}` does not take a type argument"),
        )),
    }
}

/// Split on `sep`, ignoring separators nested inside `<…>`.
fn split_top_level(text: &str, sep: char) -> Vec<&str> {
    let mut parts = Vec::new();
    let mut depth = 0usize;
    let mut start = 0usize;
    for (i, c) in text.char_indices() {
        match c {
            '<' => depth += 1,
            '>' => depth = depth.saturating_sub(1),
            c if c == sep && depth == 0 => {
                parts.push(text[start..i].trim());
                start = i + c.len_utf8();
            }
            _ => {}
        }
    }
    let last = text[start..].trim();
    if !last.is_empty() || !parts.is_empty() {
        parts.push(last);
    }
    parts.retain(|p| !p.is_empty());
    parts
}

#[cfg(test)]
mod tests {
    use super::*;

    fn parsed(text: &str) -> SpecConfigType {
        text.parse().unwrap_or_else(|e| panic!("{text}: {e}"))
    }

    #[test]
    fn every_shape_round_trips_through_its_own_spelling() {
        // The written form is the interchange, so parsing and displaying have to agree —
        // otherwise a spec changes meaning by being saved.
        for text in [
            "bool",
            "string",
            "uint",
            "path",
            "duration",
            "object",
            "list<string>",
            "set<path>",
            "map<string, string>",
            "option<int>",
            "list<map<string, list<string>>>",
            "bool|string",
            "option<list<string>>",
        ] {
            assert_eq!(parsed(text).to_string(), text, "{text}");
        }
    }

    #[test]
    fn familiar_spellings_are_accepted() {
        // The registries being migrated say `boolean`, `ListString`-ish `array<…>`,
        // `usize`, `number`. Accepting the synonyms costs nothing and normalizes them.
        assert_eq!(parsed("boolean"), SpecConfigType::Base(Base::Bool));
        assert_eq!(parsed("usize"), SpecConfigType::Base(Base::Uint));
        assert_eq!(parsed("number"), SpecConfigType::Base(Base::Float));
        assert_eq!(
            parsed("array<string>"),
            SpecConfigType::List(Box::new(SpecConfigType::Base(Base::String)))
        );
        assert_eq!(
            parsed("optional<path>"),
            SpecConfigType::Option(Box::new(SpecConfigType::Base(Base::Path)))
        );
        // A one-argument map is keyed by strings, which is what the fleet means by it.
        assert_eq!(
            parsed("map<string>"),
            SpecConfigType::Map(Base::String, Box::new(SpecConfigType::Base(Base::String)))
        );
    }

    #[test]
    fn an_unknown_name_is_kept_rather_than_refused() {
        // The escape hatch: a tool may name a type only it understands, and a spec written
        // for a newer usage must still load.
        let ty = parsed("crate::PythonUvVenvAuto");
        assert_eq!(
            ty,
            SpecConfigType::Base(Base::Custom("crate::PythonUvVenvAuto".into()))
        );
        assert_eq!(ty.to_string(), "crate::PythonUvVenvAuto");
        // Including inside a composite.
        assert_eq!(parsed("list<Weird>").to_string(), "list<Weird>");
    }

    #[test]
    fn a_malformed_type_is_an_error() {
        for text in ["list<string", "list<>", "map<string, int, extra>", "int<x>"] {
            assert!(
                text.parse::<SpecConfigType>().is_err(),
                "`{text}` should not parse"
            );
        }
    }

    #[test]
    fn simplified_picks_what_a_generator_can_write() {
        assert_eq!(
            parsed("option<list<string>>").simplified(),
            &parsed("list<string>")
        );
        assert_eq!(parsed("bool|string").simplified(), &parsed("bool"));
        assert!(parsed("option<int>").is_optional());
        assert!(!parsed("int").is_optional());
    }
}
