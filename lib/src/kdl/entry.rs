use crate::miette::SourceSpan;
use std::{fmt::Display, str::FromStr};

use crate::kdl::{v2_parser, KdlError, KdlIdentifier, KdlValue};

/// KDL Entries are the "arguments" to KDL nodes: either a (positional)
/// [`Argument`](https://github.com/kdl-org/kdl/blob/main/SPEC.md#argument) or
/// a (key/value)
/// [`Property`](https://github.com/kdl-org/kdl/blob/main/SPEC.md#property)
#[derive(Debug, Clone, Eq)]
pub struct KdlEntry {
    pub(crate) ty: Option<KdlIdentifier>,
    pub(crate) value: KdlValue,
    pub(crate) name: Option<KdlIdentifier>,
    pub(crate) format: Option<KdlEntryFormat>,
    pub(crate) span: SourceSpan,
}

impl PartialEq for KdlEntry {
    fn eq(&self, other: &Self) -> bool {
        self.ty == other.ty
            && self.value == other.value
            && self.name == other.name
            && self.format == other.format
        // intentionally omitted: self.span == other.span
    }
}

impl std::hash::Hash for KdlEntry {
    fn hash<H: std::hash::Hasher>(&self, state: &mut H) {
        self.ty.hash(state);
        self.value.hash(state);
        self.name.hash(state);
        self.format.hash(state);
        // intentionally omitted: self.span.hash(state)
    }
}

impl KdlEntry {
    /// Creates a new Argument (positional) KdlEntry.
    pub fn new(value: impl Into<KdlValue>) -> Self {
        Self {
            ty: None,
            value: value.into(),
            name: None,
            format: None,
            span: (0..0).into(),
        }
    }

    /// Gets a reference to this entry's name, if it's a property entry.
    pub fn name(&self) -> Option<&KdlIdentifier> {
        self.name.as_ref()
    }

    /// Gets a mutable reference to this node's name.
    pub fn name_mut(&mut self) -> Option<&mut KdlIdentifier> {
        self.name.as_mut()
    }

    /// Sets this node's name.
    pub fn set_name(&mut self, name: Option<impl Into<KdlIdentifier>>) {
        self.name = name.map(|x| x.into());
    }

    /// Gets the entry's value.
    pub fn value(&self) -> &KdlValue {
        &self.value
    }

    /// Gets a mutable reference to this entry's value.
    pub fn value_mut(&mut self) -> &mut KdlValue {
        &mut self.value
    }

    /// Sets the entry's value.
    pub fn set_value(&mut self, value: impl Into<KdlValue>) {
        self.value = value.into();
    }

    /// Gets this entry's span.
    ///
    /// This value will be properly initialized when created via [`crate::kdl::KdlDocument::parse`]
    /// but may become invalidated if the document is mutated. We do not currently
    /// guarantee this to yield any particularly consistent results at that point.
    pub fn span(&self) -> SourceSpan {
        self.span
    }

    /// Sets this entry's span.
    pub fn set_span(&mut self, span: impl Into<SourceSpan>) {
        self.span = span.into();
    }

    /// Gets the entry's type.
    pub fn ty(&self) -> Option<&KdlIdentifier> {
        self.ty.as_ref()
    }

    /// Gets a mutable reference to this entry's type.
    pub fn ty_mut(&mut self) -> Option<&mut KdlIdentifier> {
        self.ty.as_mut()
    }

    /// Sets the entry's type.
    pub fn set_ty(&mut self, ty: impl Into<KdlIdentifier>) {
        self.ty = Some(ty.into());
    }

    /// Gets the formatting details (including whitespace and comments) for this entry.
    pub fn format(&self) -> Option<&KdlEntryFormat> {
        self.format.as_ref()
    }

    /// Gets a mutable reference to this entry's formatting details.
    pub fn format_mut(&mut self) -> Option<&mut KdlEntryFormat> {
        self.format.as_mut()
    }

    /// Sets the formatting details for this entry.
    pub fn set_format(&mut self, format: KdlEntryFormat) {
        self.format = Some(format);
    }

    /// Creates a new Property (key/value) KdlEntry.
    pub fn new_prop(key: impl Into<KdlIdentifier>, value: impl Into<KdlValue>) -> Self {
        Self {
            ty: None,
            value: value.into(),
            name: Some(key.into()),
            format: None,
            span: SourceSpan::from(0..0),
        }
    }

    /// Clears leading and trailing text (whitespace, comments), as well as
    /// resetting this entry's value to its default representation.
    pub fn clear_format(&mut self) {
        self.format = None;
        if let Some(ty) = &mut self.ty {
            ty.clear_format();
        }
        if let Some(name) = &mut self.name {
            name.clear_format();
        }
    }

    /// Length of this entry when rendered as a string.
    pub fn len(&self) -> usize {
        format!("{self}").len()
    }

    /// Returns true if this entry is completely empty (including whitespace).
    pub fn is_empty(&self) -> bool {
        self.len() == 0
    }

    /// Keeps the general entry formatting during automatic formatting.
    pub fn keep_format(&mut self) {
        if let Some(fmt) = self.format_mut() {
            fmt.autoformat_keep = true;
        }
    }

    /// Auto-formats this entry.
    pub fn autoformat(&mut self) {
        // TODO once MSRV allows (1.80.0):
        //self.format.take_if(|f| !f.autoformat_keep);
        if !self
            .format
            .as_ref()
            .map(|f| f.autoformat_keep)
            .unwrap_or(false)
        {
            self.format = None;
        } else {
            self.format = self.format.take().map(|f| KdlEntryFormat {
                value_repr: f.value_repr,
                leading: f.leading,
                ..Default::default()
            });
        }

        if let Some(name) = &mut self.name {
            name.autoformat();
        }
    }

    /// Parses a KDL v2 string into an entry.
    pub fn parse(s: &str) -> Result<Self, KdlError> {
        v2_parser::try_parse(v2_parser::padded_node_entry, s)
    }
    /// Makes sure this entry is in v2 format.
    pub fn ensure_v2(&mut self) {
        let value_repr = self.format.as_ref().map(|x| {
            match &self.value {
                KdlValue::String(val) => {
                    // cleanup. I don't _think_ this should have any whitespace,
                    // but just in case.
                    let s = x.value_repr.trim();
                    // convert raw strings to new format
                    let s = s.strip_prefix('r').unwrap_or(s);

                    if crate::kdl::value::is_plain_ident(val) {
                        val.into()
                    } else if s
                        .find(|c| v2_parser::NEWLINES.iter().any(|nl| nl.contains(c)))
                        .is_some()
                    {
                        // Multiline string. Need triple quotes if they're not there already.
                        if s.contains("\"\"\"") {
                            // We're probably good. This could be more precise, but close enough.
                            s.to_string()
                        } else {
                            // `"` -> `"""` but also extra newlines need to be
                            // added because v2 strips the first and last ones.
                            let s = s.replacen('\"', "\"\"\"\n", 1);
                            s.chars()
                                .rev()
                                .collect::<String>()
                                .replacen('\"', "\"\"\"\n", 1)
                                .chars()
                                .rev()
                                .collect::<String>()
                        }
                    } else if !s.starts_with('#') {
                        // `/` is no longer an escaped char in v2.
                        s.replace("\\/", "/")
                    } else {
                        // We're all good! Let's move on.
                        s.to_string()
                    }
                }
                // These have `#` prefixes now. The regular Display impl will
                // take care of that.
                KdlValue::Bool(_) | KdlValue::Null => format!("{}", self.value),
                // These should be fine as-is?
                KdlValue::Integer(_) | KdlValue::Float(_) => x.value_repr.clone(),
            }
        });

        if let Some(value_repr) = value_repr.as_ref() {
            self.format = Some(
                self.format
                    .clone()
                    .map(|mut x| {
                        x.value_repr = value_repr.into();
                        x
                    })
                    .unwrap_or_else(|| KdlEntryFormat {
                        value_repr: value_repr.into(),
                        leading: " ".into(),
                        ..Default::default()
                    }),
            )
        }
    }
}
impl Display for KdlEntry {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        if let Some(KdlEntryFormat { leading, .. }) = &self.format {
            write!(f, "{leading}")?;
        }
        if let Some(name) = &self.name {
            write!(f, "{name}")?;
            if let Some(KdlEntryFormat {
                after_key,
                after_eq,
                ..
            }) = &self.format
            {
                write!(f, "{after_key}={after_eq}")?;
            } else {
                write!(f, "=")?;
            }
        }
        if let Some(ty) = &self.ty {
            write!(f, "(")?;
            if let Some(KdlEntryFormat { before_ty_name, .. }) = &self.format {
                write!(f, "{before_ty_name}")?;
            }
            write!(f, "{ty}")?;
            if let Some(KdlEntryFormat { after_ty_name, .. }) = &self.format {
                write!(f, "{after_ty_name}")?;
            }
            write!(f, ")")?;
        }
        if let Some(KdlEntryFormat {
            after_ty,
            value_repr,
            ..
        }) = &self.format
        {
            write!(f, "{after_ty}{value_repr}")?;
        } else {
            write!(f, "{}", self.value)?;
        }
        if let Some(KdlEntryFormat { trailing, .. }) = &self.format {
            write!(f, "{trailing}")?;
        }
        Ok(())
    }
}

impl<T> From<T> for KdlEntry
where
    T: Into<KdlValue>,
{
    fn from(value: T) -> Self {
        Self::new(value)
    }
}

impl<K, V> From<(K, V)> for KdlEntry
where
    K: Into<KdlIdentifier>,
    V: Into<KdlValue>,
{
    fn from((key, value): (K, V)) -> Self {
        Self::new_prop(key, value)
    }
}

impl FromStr for KdlEntry {
    type Err = KdlError;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        Self::parse(s)
    }
}

/// Formatting details for [`KdlEntry`]s.
#[derive(Debug, Default, Clone, Eq, PartialEq, Hash)]
pub struct KdlEntryFormat {
    /// The actual text representation of the entry's value.
    pub value_repr: String,
    /// Whitespace and comments preceding the entry itself.
    pub leading: String,
    /// Whitespace and comments following the entry itself.
    pub trailing: String,
    /// Whitespace and comments after the entry's type annotation's closing
    /// `)`, before its value.
    pub after_ty: String,
    /// Whitespace and comments between the opening `(` of an entry's type
    /// annotation and its actual type name.
    pub before_ty_name: String,
    /// Whitespace and comments between the actual type name and the closing
    /// `)` in an entry's type annotation.
    pub after_ty_name: String,
    /// Whitespace and comments between an entry's key name and its equals sign.
    pub after_key: String,
    /// Whitespace and comments between an entry's equals sign and its value.
    pub after_eq: String,
    /// Do not clobber this format during autoformat
    pub autoformat_keep: bool,
}
