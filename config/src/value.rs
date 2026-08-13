//! What a setting holds, at runtime and as a declared default.
//!
//! Two types rather than one, because they answer to different masters. [`Value`] is owned:
//! it comes from a file or an environment variable while the process runs, so it has to be
//! allocated. [`Const`] is what a *declared* default is, and every field of it is
//! `const`-constructible so a generated registry costs nothing to load — no parsing, no
//! allocation, nothing done per process start for a setting nobody reads.

use std::collections::BTreeMap;
use std::fmt;

/// Text on one line, whatever it contains.
///
/// Everything this crate prints is line-oriented — one setting per line for a listing, one fact
/// per line for an explanation, one warning or failure per line — so anything interpolated into a
/// line has to stay on it. Three kinds of thing can carry a newline and all three did: a value (a
/// multi-line string is perfectly ordinary in TOML), an origin (a path may contain one), and a
/// message that quotes either of them. Lives here beside [`Value::display`] because that is what
/// it is usually wrapped around, and one rule shared by every renderer is one rule.
pub(crate) fn one_line(text: &str) -> String {
    // Newlines only. Escaping backslashes as well made `C:\Users\me\hk.toml` render with
    // doubled separators — a path a reader would copy and find nothing at, which is a worse
    // failure than the ambiguity it bought: this output is for a human, and the one thing it
    // needs is that a record stays on its line.
    text.replace('\n', "\\n").replace('\r', "\\r")
}

/// A value as one line of output, with its shape shown when there is nothing in it.
///
/// [`Value::display`] writes a value the way a user would type it, and three values are typed as
/// nothing at all: the empty string, the empty list, the empty map. Interpolated into `key = {}`
/// that produced a line ending after the `=` — a trailing space and a truncated look for a value
/// that is perfectly ordinary, since clearing a list is how a declared default is turned off
/// (`HK_EXCLUDE=`). Emptiness is a fact about a value and worth a word, so it gets the spelling
/// its own format would use.
///
/// Not folded into `display` itself, which is also what `config get` prints: there, an empty
/// setting printing nothing is exactly right, and `[]` would be a value nobody wrote.
pub(crate) fn shown(value: &Value) -> String {
    match value {
        // Asked of the *value*, not of its text. Read from the text, a one-item list holding the
        // empty string looked exactly like a cleared one — `[]` for a list that has something in
        // it, which says the opposite of what is true.
        Value::List(items) if items.is_empty() => "[]".to_string(),
        Value::Map(entries) if entries.is_empty() => "{}".to_string(),
        Value::String(text) if text.is_empty() => "\"\"".to_string(),
        // A list is joined with commas, which hides any item whose own text is empty: one empty
        // string came out as nothing (a cleared list), and two came out as `,`. So when an item
        // would disappear, the whole list is written out item by item instead — `[""]` is one item
        // and reads as one, `[a,""]` is two.
        Value::List(items) if items.iter().any(|item| item.display().is_empty()) => {
            let items: Vec<String> = items.iter().map(shown).collect();
            format!("[{}]", items.join(","))
        }
        // Everything else writes something: a number and a boolean always do, and a map's keys are
        // in its text even when its values are empty.
        other => one_line(&other.display()),
    }
}

/// A resolved configuration value.
#[derive(Debug, Clone, PartialEq)]
pub enum Value {
    Bool(bool),
    Int(i64),
    Float(f64),
    String(String),
    List(Vec<Value>),
    /// A table. Ordered by key so a resolution is reproducible and two runs of `config
    /// explain` cannot disagree about what came first.
    Map(BTreeMap<String, Value>),
}

impl Value {
    /// The name of this shape, for an error a human has to read.
    pub fn type_name(&self) -> &'static str {
        match self {
            Self::Bool(_) => "a boolean",
            Self::Int(_) => "an integer",
            Self::Float(_) => "a number",
            Self::String(_) => "a string",
            Self::List(_) => "a list",
            Self::Map(_) => "a table",
        }
    }

    /// This value written the way a user would type it.
    pub fn display(&self) -> String {
        match self {
            Self::Bool(b) => b.to_string(),
            Self::Int(i) => i.to_string(),
            Self::Float(f) => f.to_string(),
            Self::String(s) => s.clone(),
            Self::List(items) => items
                .iter()
                .map(Self::display)
                .collect::<Vec<_>>()
                .join(","),
            Self::Map(entries) => entries
                .iter()
                .map(|(key, value)| format!("{key}={}", value.display()))
                .collect::<Vec<_>>()
                .join(","),
        }
    }
}

impl fmt::Display for Value {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(&self.display())
    }
}

impl From<bool> for Value {
    fn from(value: bool) -> Self {
        Self::Bool(value)
    }
}

impl From<i64> for Value {
    fn from(value: i64) -> Self {
        Self::Int(value)
    }
}

impl From<f64> for Value {
    fn from(value: f64) -> Self {
        Self::Float(value)
    }
}

impl From<&str> for Value {
    fn from(value: &str) -> Self {
        Self::String(value.to_string())
    }
}

impl From<String> for Value {
    fn from(value: String) -> Self {
        Self::String(value)
    }
}

/// A declared default, in the form a generated registry can hold as a `const`.
///
/// The same shapes as [`Value`], with borrowed strings and slices so nothing is allocated
/// until somebody actually asks for the default of a setting no layer supplied.
#[derive(Debug, Copy, Clone, PartialEq)]
pub enum Const {
    Bool(bool),
    Int(i64),
    Float(f64),
    Str(&'static str),
    List(&'static [Const]),
    /// Key-value pairs, ordered by the generator so the `Value` it becomes is too.
    Map(&'static [(&'static str, Const)]),
}

impl Const {
    /// The owned value this stands for.
    pub fn to_value(self) -> Value {
        match self {
            Self::Bool(b) => Value::Bool(b),
            Self::Int(i) => Value::Int(i),
            Self::Float(f) => Value::Float(f),
            Self::Str(s) => Value::String(s.to_string()),
            Self::List(items) => Value::List(items.iter().map(|item| item.to_value()).collect()),
            Self::Map(entries) => Value::Map(
                entries
                    .iter()
                    .map(|(key, value)| ((*key).to_string(), value.to_value()))
                    .collect(),
            ),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_declared_default_becomes_the_value_it_names() {
        // The registry holds these as consts, so this conversion is the only cost a default
        // ever has — and only for a setting whose default is actually reached for.
        const NESTED: &[Const] = &[Const::Int(80), Const::Int(443)];
        const PAIRS: &[(&str, Const)] = &[("a", Const::Bool(true))];
        assert_eq!(Const::Bool(true).to_value(), Value::Bool(true));
        assert_eq!(Const::Str("x").to_value(), Value::String("x".into()));
        assert_eq!(
            Const::List(NESTED).to_value(),
            Value::List(vec![Value::Int(80), Value::Int(443)])
        );
        assert_eq!(
            Const::Map(PAIRS).to_value(),
            Value::Map([("a".to_string(), Value::Bool(true))].into_iter().collect())
        );
    }

    #[test]
    fn an_empty_value_is_told_apart_from_a_value_that_writes_as_nothing() {
        // The distinction `explain`, `list` and every type error rest on. A cleared list is a
        // supported state — `HK_EXCLUDE=` turns a declared default off — and a list holding one
        // empty string is a different one, though their text is identical.
        assert_eq!(shown(&Value::List(Vec::new())), "[]");
        assert_eq!(shown(&Value::List(vec![Value::from("")])), "[\"\"]");
        assert_eq!(
            shown(&Value::List(vec![Value::from(""), Value::from("")])),
            "[\"\",\"\"]"
        );
        assert_eq!(shown(&Value::from("")), "\"\"");
        assert_eq!(shown(&Value::Map(BTreeMap::new())), "{}");

        // And anything with text of its own is that text, unquoted and unbracketed: this is output
        // a person reads, not a serializer.
        assert_eq!(shown(&Value::from("git")), "git");
        assert_eq!(shown(&Value::Int(0)), "0");
        assert_eq!(shown(&Value::Bool(false)), "false");
        assert_eq!(
            shown(&Value::List(vec![Value::from("a"), Value::from("b")])),
            "a,b"
        );
        // A list with one empty item among several is written out too, since that item is the one
        // the comma-joined form would lose.
        assert_eq!(
            shown(&Value::List(vec![Value::from("a"), Value::from("")])),
            "[a,\"\"]"
        );
        // A map whose value is empty still has its key in the text, which is enough to read.
        assert_eq!(
            shown(&Value::Map(
                [("k".to_string(), Value::from(""))].into_iter().collect()
            )),
            "k="
        );
    }

    #[test]
    fn a_value_can_be_written_the_way_it_was_typed() {
        // What `config get` prints and what an error quotes back, so a list has to read as
        // one rather than as its debug form.
        assert_eq!(Value::Bool(false).display(), "false");
        assert_eq!(
            Value::List(vec![Value::String("a".into()), Value::Int(2)]).display(),
            "a,2"
        );
        assert_eq!(
            Value::Map([("k".to_string(), Value::Int(1))].into_iter().collect()).display(),
            "k=1"
        );
    }
}
