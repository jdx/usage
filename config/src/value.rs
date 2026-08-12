//! What a setting holds, at runtime and as a declared default.
//!
//! Two types rather than one, because they answer to different masters. [`Value`] is owned:
//! it comes from a file or an environment variable while the process runs, so it has to be
//! allocated. [`Const`] is what a *declared* default is, and every field of it is
//! `const`-constructible so a generated registry costs nothing to load — no parsing, no
//! allocation, nothing done per process start for a setting nobody reads.

use std::collections::BTreeMap;
use std::fmt;

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
