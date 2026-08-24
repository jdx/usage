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

    /// Keeps the general entry formatting, though v1 entries will still be
    /// updated to v2 while preserving as much as possible.
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
            #[cfg(feature = "v1")]
            self.ensure_v2();
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

    /// Parses a string into a entry.
    ///
    /// If the `v1-fallback` feature is enabled, this method will first try to
    /// parse the string as a KDL v2 entry, and, if that fails, it will try
    /// to parse again as a KDL v1 entry. If both fail, only the v2 parse
    /// errors will be returned.
    pub fn parse(s: &str) -> Result<Self, KdlError> {
        #[cfg(not(feature = "v1-fallback"))]
        {
            v2_parser::try_parse(v2_parser::padded_node_entry, s)
        }
        #[cfg(feature = "v1-fallback")]
        {
            v2_parser::try_parse(v2_parser::padded_node_entry, s)
                .or_else(|e| KdlEntry::parse_v1(s).map_err(|_| e))
        }
    }

    /// Parses a KDL v1 string into an entry.
    #[cfg(feature = "v1")]
    pub fn parse_v1(s: &str) -> Result<Self, KdlError> {
        let ret: Result<kdlv1::KdlEntry, kdlv1::KdlError> = s.parse();
        ret.map(|x| x.into()).map_err(|e| e.into())
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

    /// Makes sure this entry is in v1 format.
    #[cfg(feature = "v1")]
    pub fn ensure_v1(&mut self) {
        let value_repr = self.format.as_ref().map(|x| {
            match &self.value {
                KdlValue::String(val) => {
                    // cleanup. I don't _think_ this should have any whitespace,
                    // but just in case.
                    let s = x.value_repr.trim();
                    // convert raw strings to v1 format
                    let s = if s.starts_with('#') {
                        format!("r{s}")
                    } else {
                        s.to_string()
                    };

                    if crate::kdl::value::is_plain_ident(val)
                        && !s.starts_with('\"')
                        && !s.starts_with("r#")
                    {
                        format!("\"{val}\"")
                    } else if s
                        .find(|c| v2_parser::NEWLINES.iter().any(|nl| nl.contains(c)))
                        .is_some()
                    {
                        // Multiline string. Let's make sure it's v1.
                        if s.contains("\"\"\"") {
                            let prefix = s
                                .chars()
                                .rev()
                                .skip_while(|c| c == &'"')
                                .take_while(|c| {
                                    v2_parser::NEWLINES.iter().any(|nl| nl.contains(*c))
                                })
                                .collect::<String>();
                            let prefix = prefix.chars().rev().collect::<String>();
                            // Sigh. Yeah. I didn't promise this would be _efficient_.
                            let mut s = s;
                            for nl in v2_parser::NEWLINES {
                                s = s.replace(&format!("{nl}{prefix}"), nl);
                            }
                            // And now we strips the beginning and ending newlines.
                            // Finally, replace `"""` with `"`.
                            s
                        } else {
                            // It's already a v1 string
                            s
                        }
                    } else if !s.starts_with("r#") {
                        // `/` is an escaped char in v2
                        let s = s.replace("\\/", "/"); // Maneuvering. Will fix in a sec.
                        s.replace('/', "\\/")
                    } else {
                        // We're all good! Let's move on.
                        s.to_string()
                    }
                }
                // No more # prefix for these
                KdlValue::Bool(b) => b.to_string(),
                KdlValue::Null => "null".to_string(),
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
        } else {
            let v1_val = match self.value() {
                KdlValue::String(s) => kdlv1::KdlValue::String(s.clone()),
                KdlValue::Integer(i) => kdlv1::KdlValue::Base10(*i as i64),
                KdlValue::Float(f) => kdlv1::KdlValue::Base10Float(*f),
                KdlValue::Bool(b) => kdlv1::KdlValue::Bool(*b),
                KdlValue::Null => kdlv1::KdlValue::Null,
            };
            self.format = Some(KdlEntryFormat {
                value_repr: v1_val.to_string(),
                leading: " ".into(),
                ..Default::default()
            })
        }
    }
}

#[cfg(feature = "v1")]
impl From<kdlv1::KdlEntry> for KdlEntry {
    fn from(value: kdlv1::KdlEntry) -> Self {
        Self {
            ty: value.ty().map(|x| x.clone().into()),
            value: value.value().clone().into(),
            name: value.name().map(|x| x.clone().into()),
            format: Some(KdlEntryFormat {
                value_repr: value.value_repr().unwrap_or("").into(),
                leading: value.leading().unwrap_or("").into(),
                trailing: value.trailing().unwrap_or("").into(),
                ..Default::default()
            }),
            span: SourceSpan::new(value.span().offset().into(), value.span().len()),
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

#[cfg(all(test, feature = "kdl-upstream-tests"))]
mod test {
    use super::*;

    #[test]
    fn reset_value_repr() -> miette::Result<()> {
        let mut left_entry: KdlEntry = "   name=1.03e2".parse()?;
        let mut right_entry: KdlEntry = "   name=103.0".parse()?;
        assert_ne!(left_entry, right_entry);
        left_entry.clear_format();
        right_entry.clear_format();
        assert_eq!(left_entry, right_entry);
        Ok(())
    }

    #[test]
    fn new() {
        let entry = KdlEntry::new(42);
        assert_eq!(
            entry,
            KdlEntry {
                ty: None,
                value: KdlValue::Integer(42),
                name: None,
                format: None,
                span: SourceSpan::from(0..0),
            }
        );

        let entry = KdlEntry::new_prop("name", 42);
        assert_eq!(
            entry,
            KdlEntry {
                ty: None,
                value: KdlValue::Integer(42),
                name: Some("name".into()),
                format: None,
                span: SourceSpan::from(0..0),
            }
        );
    }

    #[test]
    fn parsing() -> miette::Result<()> {
        let entry: KdlEntry = "foo".parse()?;
        assert_eq!(
            entry,
            KdlEntry {
                ty: None,
                value: KdlValue::from("foo"),
                name: None,
                format: Some(KdlEntryFormat {
                    value_repr: "foo".into(),
                    ..Default::default()
                }),
                span: SourceSpan::from(0..3),
            }
        );

        let entry: KdlEntry = "foo=bar".parse()?;
        assert_eq!(
            entry,
            KdlEntry {
                ty: None,
                value: KdlValue::from("bar"),
                name: Some("foo".parse()?),
                format: Some(KdlEntryFormat {
                    value_repr: "bar".into(),
                    ..Default::default()
                }),
                span: SourceSpan::from(0..7),
            }
        );

        let entry: KdlEntry = " \\\n (\"m\\\"eh\")0xDEADbeef\t\\\n".parse()?;
        #[allow(unused_mut)]
        {
            let mut ty: KdlIdentifier = "\"m\\\"eh\"".parse()?;
            {
                ty.span = (5..12).into();
            }
            assert_eq!(
                entry,
                KdlEntry {
                    ty: Some(ty),
                    value: KdlValue::Integer(0xdeadbeef),
                    name: None,
                    format: Some(KdlEntryFormat {
                        leading: " \\\n ".into(),
                        trailing: "\t\\\n".into(),
                        value_repr: "0xDEADbeef".into(),
                        ..Default::default()
                    }),
                    span: SourceSpan::from(0..26),
                }
            );
        }

        let entry: KdlEntry = " \\\n \"foo\"=(\"m\\\"eh\")0xDEADbeef\t\\\n".parse()?;
        assert_eq!(
            entry,
            KdlEntry {
                format: Some(KdlEntryFormat {
                    leading: " \\\n ".into(),
                    trailing: "\t\\\n".into(),
                    value_repr: "0xDEADbeef".into(),
                    before_ty_name: "".into(),
                    after_ty_name: "".into(),
                    after_ty: "".into(),
                    after_key: "".into(),
                    after_eq: "".into(),
                    autoformat_keep: false
                }),
                ty: Some("\"m\\\"eh\"".parse()?),
                value: KdlValue::Integer(0xdeadbeef),
                name: Some("\"foo\"".parse()?),
                span: SourceSpan::from(0..0),
            }
        );

        Ok(())
    }

    #[test]
    fn display() {
        let entry = KdlEntry::new(KdlValue::Integer(42));
        assert_eq!(format!("{entry}"), "42");

        let entry = KdlEntry::new_prop("name", KdlValue::Integer(42));
        assert_eq!(format!("{entry}"), "name=42");
    }

    #[cfg(feature = "v1")]
    #[test]
    fn v1_to_v2_format() -> miette::Result<()> {
        let mut entry = KdlEntry::parse_v1(r##"r#"hello, world!"#"##)?;
        entry.keep_format();
        entry.autoformat();
        assert_eq!(format!("{}", entry), r##" #"hello, world!"#"##);

        let mut entry = KdlEntry::parse_v1(r#""hello, \" world!""#)?;
        entry.keep_format();
        entry.autoformat();
        assert_eq!(format!("{}", entry), r#" "hello, \" world!""#);

        let mut entry = KdlEntry::parse_v1("\"foo!`~.,<>\"")?;
        entry.keep_format();
        entry.autoformat();
        assert_eq!(format!("{}", entry), " foo!`~.,<>");

        let mut entry = KdlEntry::parse_v1("\"\nhello, world!\"")?;
        entry.keep_format();
        entry.autoformat();
        assert_eq!(format!("{}", entry), " \"\"\"\n\nhello, world!\n\"\"\"");

        let mut entry = KdlEntry::parse_v1("r#\"\nhello, world!\"#")?;
        entry.keep_format();
        entry.autoformat();
        assert_eq!(format!("{}", entry), " #\"\"\"\n\nhello, world!\n\"\"\"#");

        let mut entry = KdlEntry::parse_v1("true")?;
        entry.keep_format();
        entry.autoformat();
        assert_eq!(format!("{}", entry), " #true");

        let mut entry = KdlEntry::parse_v1("false")?;
        entry.keep_format();
        entry.autoformat();
        assert_eq!(format!("{}", entry), " #false");

        let mut entry = KdlEntry::parse_v1("null")?;
        entry.keep_format();
        entry.autoformat();
        assert_eq!(format!("{}", entry), " #null");

        let mut entry = KdlEntry::parse_v1("1_234_567")?;
        entry.keep_format();
        entry.autoformat();
        assert_eq!(format!("{}", entry), " 1_234_567");

        let mut entry = KdlEntry::parse_v1("1_234_567E-10")?;
        entry.keep_format();
        entry.autoformat();
        assert_eq!(format!("{}", entry), " 1_234_567E-10");
        Ok(())
    }
}
