//! Explicit, generator-only Go field types. The portable grammar remains textual.

use std::collections::BTreeMap;
use std::str::FromStr;

use super::Emitted;

/// An opt-in Go type for a value-taking flag or positional argument.
/// Variadic entries become slices of this type. Absent entries keep their zero value.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ValueType {
    /// Go's machine-sized signed integer.
    Int,
    /// A signed 64-bit integer.
    Int64,
    /// An unsigned 64-bit integer.
    Uint64,
    /// A 64-bit floating point value.
    Float64,
    /// A boolean text value (not a valueless boolean flag).
    Bool,
    /// A duration in Go's notation, such as `1m30s`.
    Duration,
}

impl FromStr for ValueType {
    type Err = String;

    /// Parse the stable command-line spelling for a supported generated Go type.
    fn from_str(value: &str) -> Result<Self, Self::Err> {
        match value {
            "int" => Ok(Self::Int),
            "int64" => Ok(Self::Int64),
            "uint64" => Ok(Self::Uint64),
            "float64" => Ok(Self::Float64),
            "bool" => Ok(Self::Bool),
            "duration" => Ok(Self::Duration),
            _ => Err(format!("unknown Go field type {value:?}; expected int, int64, uint64, float64, bool, or duration")),
        }
    }
}

impl ValueType {
    /// Spell the scalar or slice type in a generated struct.
    pub(super) fn field_type(self, many: bool) -> String {
        let scalar = match self {
            Self::Int => "int",
            Self::Int64 => "int64",
            Self::Uint64 => "uint64",
            Self::Float64 => "float64",
            Self::Bool => "bool",
            Self::Duration => "time.Duration",
        };
        if many {
            format!("[]{scalar}")
        } else {
            scalar.to_string()
        }
    }

    /// Reuse the runtime converter rather than generating a second conversion policy.
    pub(super) fn converter(self) -> &'static str {
        match self {
            Self::Int => "NativeInt",
            Self::Int64 => "Int",
            Self::Uint64 => "Uint",
            Self::Float64 => "Float",
            Self::Bool => "Bool",
            Self::Duration => "Duration",
        }
    }
}

/// Reject misspelled keys and entries that have no textual values to convert.
pub(super) fn validate(
    commands: &[Emitted],
    types: &BTreeMap<String, ValueType>,
) -> Result<(), String> {
    for key in types.keys() {
        let flag = commands
            .iter()
            .flat_map(|e| &e.flags)
            .find(|(_, named)| &named.key == key);
        if let Some((flag, _)) = flag {
            if flag.count || flag.arg.is_none() {
                return Err(format!("{key} is not a value-taking flag"));
            }
            continue;
        }
        if commands
            .iter()
            .flat_map(|e| &e.args)
            .any(|(_, named)| &named.key == key)
        {
            continue;
        }
        return Err(format!("{key} is not a generated command-level flag or argument key; clause fields are not supported yet"));
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::go::{generate, generate_with_types, GoOptions};
    use crate::Spec;

    /// Typed generation is opt-in and invalid bindings fail before source is emitted.
    #[test]
    fn explicit_types_are_checked() {
        let spec: Spec = "name \"ex\"\nflag \"--timeout <value>\"\nflag \"-v\" count=#true\nflag \"--force\"\narg \"[file]\"".parse().unwrap();
        let opts = GoOptions::default();
        assert_eq!(
            generate(&spec, &opts),
            generate_with_types(&spec, &opts, &BTreeMap::new()).unwrap()
        );
        for key in ["FlagMissing", "FlagV", "FlagForce", "CmdRoot"] {
            assert!(
                generate_with_types(
                    &spec,
                    &opts,
                    &BTreeMap::from([(key.to_string(), ValueType::Duration)])
                )
                .is_err(),
                "{key}"
            );
        }
        let out = generate_with_types(
            &spec,
            &opts,
            &BTreeMap::from([("FlagTimeout".to_string(), ValueType::Duration)]),
        )
        .unwrap();
        assert!(out.contains("time.Duration"));
        assert!(out.contains("argv.Duration(\"--timeout\", values[len(values)-1])"));
        assert!(!out.contains("out.Timeout = ev.Value"));
        assert!("typo".parse::<ValueType>().is_err());
    }
}
