use std::fmt::Display;

/// A specific [KDL Value](https://github.com/kdl-org/kdl/blob/main/SPEC.md#value).
#[derive(Debug, Clone, PartialOrd)]
pub enum KdlValue {
    /// A [KDL String](https://github.com/kdl-org/kdl/blob/main/SPEC.md#string).
    String(String),

    /// A non-float [KDL
    /// Number](https://github.com/kdl-org/kdl/blob/main/SPEC.md#number)
    Integer(i128),

    /// A floating point [KDL
    /// Number](https://github.com/kdl-org/kdl/blob/main/SPEC.md#number)
    Float(f64),

    /// A [KDL Boolean](https://github.com/kdl-org/kdl/blob/main/SPEC.md#boolean).
    Bool(bool),

    /// The [KDL Null Value](https://github.com/kdl-org/kdl/blob/main/SPEC.md#null).
    Null,
}

impl Eq for KdlValue {}

fn normalize_float(f: &f64) -> f64 {
    match f {
        _ if f == &f64::INFINITY => f64::MAX,
        _ if f == &f64::NEG_INFINITY => -f64::MAX,
        // We collapse NaN to 0.0 because we're evil like that.
        _ if f.is_nan() => 0.0,
        _ => *f,
    }
}

impl PartialEq for KdlValue {
    fn eq(&self, other: &Self) -> bool {
        match (self, other) {
            (Self::String(l0), Self::String(r0)) => l0 == r0,
            (Self::Integer(l0), Self::Integer(r0)) => l0 == r0,
            (Self::Float(l0), Self::Float(r0)) => normalize_float(l0) == normalize_float(r0),
            (Self::Bool(l0), Self::Bool(r0)) => l0 == r0,
            _ => core::mem::discriminant(self) == core::mem::discriminant(other),
        }
    }
}

// NOTE: I know, I know. This is terrible and I shouldn't do it, but it's
// better than not being able to hash KdlValue at all.
impl std::hash::Hash for KdlValue {
    fn hash<H: std::hash::Hasher>(&self, state: &mut H) {
        match self {
            Self::String(val) => val.hash(state),
            Self::Integer(val) => val.hash(state),
            Self::Float(val) => {
                let val = normalize_float(val);
                // Good enough to be close-ish for our purposes.
                (val.trunc() as i128).hash(state);
                (val.fract() as i128).hash(state);
            }
            Self::Bool(val) => val.hash(state),
            Self::Null => core::mem::discriminant(self).hash(state),
        }
    }
}

impl KdlValue {
    /// Returns `true` if the value is a [`KdlValue::String`].
    pub fn is_string(&self) -> bool {
        matches!(self, Self::String(..))
    }

    /// Returns `true` if the value is a [`KdlValue::Integer`].
    pub fn is_integer(&self) -> bool {
        matches!(self, Self::Integer(..))
    }

    /// Returns `true` if the value is a [`KdlValue::Float`].
    pub fn is_float(&self) -> bool {
        matches!(self, Self::Float(..))
    }

    /// Returns `true` if the value is a [`KdlValue::Bool`].
    pub fn is_bool(&self) -> bool {
        matches!(self, Self::Bool(..))
    }

    /// Returns `true` if the value is a [`KdlValue::Null`].
    pub fn is_null(&self) -> bool {
        matches!(self, Self::Null)
    }

    /// Returns `Some(&str)` if the `KdlValue` is a [`KdlValue::String`],
    /// otherwise returns `None`.
    pub fn as_string(&self) -> Option<&str> {
        use KdlValue::*;
        match self {
            String(s) => Some(s),
            _ => None,
        }
    }

    /// Returns `Some(i128)` if the `KdlValue` is a [`KdlValue::Integer`],
    /// otherwise returns `None`.
    pub fn as_integer(&self) -> Option<i128> {
        use KdlValue::*;
        match self {
            Integer(i) => Some(*i),
            _ => None,
        }
    }

    /// Returns `Some(f64)` if the `KdlValue` is a [`KdlValue::Float`],
    /// otherwise returns `None`.
    pub fn as_float(&self) -> Option<f64> {
        match self {
            Self::Float(i) => Some(*i),
            _ => None,
        }
    }

    /// Returns `Some(bool)` if the `KdlValue` is a [`KdlValue::Bool`], otherwise returns `None`.
    pub fn as_bool(&self) -> Option<bool> {
        if let Self::Bool(v) = self {
            Some(*v)
        } else {
            None
        }
    }
}

impl Display for KdlValue {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::String(_) => self.write_string(f),
            Self::Integer(value) => write!(f, "{value:?}"),
            Self::Float(value) => write!(
                f,
                "{}",
                if value == &f64::INFINITY {
                    "#inf".into()
                } else if value == &f64::NEG_INFINITY {
                    "#-inf".into()
                } else if value.is_nan() {
                    "#nan".into()
                } else {
                    format!("{:?}", *value)
                }
            ),
            Self::Bool(value) => write!(f, "#{value}"),
            Self::Null => write!(f, "#null"),
        }
    }
}

pub(crate) fn is_plain_ident(ident: &str) -> bool {
    let ident_bytes = ident.as_bytes();
    ident
        .find(crate::kdl::v2_parser::is_disallowed_ident_char)
        .is_none()
        && ident_bytes.first().map(|c| c.is_ascii_digit()) != Some(true)
        && !(ident.chars().next().map(|c| matches!(c, '.' | '-' | '+')) == Some(true)
            && ident_bytes.get(1).map(|c| c.is_ascii_digit()) == Some(true))
        && ident != "inf"
        && ident != "-inf"
        && ident != "nan"
        && ident != "true"
        && ident != "false"
        && ident != "null"
}
impl KdlValue {
    fn write_string(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        let string = self.as_string().unwrap();
        if !string.is_empty() && is_plain_ident(string) {
            write!(f, "{string}")?;
        } else {
            write!(f, "\"")?;
            for char in string.chars() {
                match char {
                    '\\' | '"' => write!(f, "\\{char}")?,
                    '\n' => write!(f, "\\n")?,
                    '\r' => write!(f, "\\r")?,
                    '\t' => write!(f, "\\t")?,
                    '\u{08}' => write!(f, "\\b")?,
                    '\u{0C}' => write!(f, "\\f")?,
                    _ => write!(f, "{char}")?,
                }
            }
            write!(f, "\"")?;
        }
        Ok(())
    }
}

impl From<i128> for KdlValue {
    fn from(value: i128) -> Self {
        Self::Integer(value)
    }
}

impl From<f64> for KdlValue {
    fn from(value: f64) -> Self {
        Self::Float(value)
    }
}

impl From<&str> for KdlValue {
    fn from(value: &str) -> Self {
        Self::String(value.to_string())
    }
}

impl From<String> for KdlValue {
    fn from(value: String) -> Self {
        Self::String(value)
    }
}

impl From<bool> for KdlValue {
    fn from(value: bool) -> Self {
        Self::Bool(value)
    }
}

impl<T> From<Option<T>> for KdlValue
where
    T: Into<Self>,
{
    fn from(value: Option<T>) -> Self {
        match value {
            Some(value) => value.into(),
            None => Self::Null,
        }
    }
}
