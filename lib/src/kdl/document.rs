use crate::miette::SourceSpan;
use std::fmt::Display;

use crate::kdl::{FormatConfig, KdlError, KdlNode, KdlValue};

/// Represents a KDL
/// [`Document`](https://github.com/kdl-org/kdl/blob/main/SPEC.md#document).
///
/// This type is also used to manage a [`KdlNode`]'s [`Children
/// Block`](https://github.com/kdl-org/kdl/blob/main/SPEC.md#children-block),
/// when present.
///
/// # Examples
///
/// The easiest way to create a `KdlDocument` is to parse it:
/// ```rust
/// # use usage::kdl::KdlDocument;
/// let kdl: KdlDocument = "foo 1 2 3\nbar 4 5 6".parse().expect("parse failed");
/// ```
#[derive(Debug, Clone, Eq)]
pub struct KdlDocument {
    pub(crate) nodes: Vec<KdlNode>,
    pub(crate) format: Option<KdlDocumentFormat>,
    pub(crate) span: SourceSpan,
}

impl PartialEq for KdlDocument {
    fn eq(&self, other: &Self) -> bool {
        self.nodes == other.nodes && self.format == other.format
        // Intentionally omitted: self.span == other.span
    }
}

impl std::hash::Hash for KdlDocument {
    fn hash<H: std::hash::Hasher>(&self, state: &mut H) {
        self.nodes.hash(state);
        self.format.hash(state);
        // Intentionally omitted: self.span.hash(state)
    }
}

impl Default for KdlDocument {
    fn default() -> Self {
        Self {
            nodes: Default::default(),
            format: Default::default(),
            span: SourceSpan::from(0..0),
        }
    }
}

impl KdlDocument {
    /// Creates a new Document.
    pub fn new() -> Self {
        Default::default()
    }

    /// Gets this document's span.
    ///
    /// This value will be properly initialized when created via [`KdlDocument::parse`]
    /// but may become invalidated if the document is mutated. We do not currently
    /// guarantee this to yield any particularly consistent results at that point.
    pub fn span(&self) -> SourceSpan {
        self.span
    }

    /// Sets this document's span.
    pub fn set_span(&mut self, span: impl Into<SourceSpan>) {
        self.span = span.into();
    }

    /// Gets the first child node with a matching name.
    pub fn get(&self, name: &str) -> Option<&KdlNode> {
        self.nodes.iter().find(move |n| n.name().value() == name)
    }

    /// Gets a reference to the first child node with a matching name.
    pub fn get_mut(&mut self, name: &str) -> Option<&mut KdlNode> {
        self.nodes
            .iter_mut()
            .find(move |n| n.name().value() == name)
    }

    /// Gets the first argument (value) of the first child node with a
    /// matching name. This is a shorthand utility for cases where a document
    /// is being used as a key/value store.
    ///
    /// # Examples
    ///
    /// Given a document like this:
    /// ```kdl
    /// foo 1
    /// bar false
    /// ```
    ///
    /// You can fetch the value of `foo` in a single call like this:
    /// ```rust
    /// # use usage::kdl::{KdlDocument, KdlValue};
    /// # let doc: KdlDocument = "foo 1\nbar #false".parse().unwrap();
    /// assert_eq!(doc.get_arg("foo"), Some(&1.into()));
    /// ```
    pub fn get_arg(&self, name: &str) -> Option<&KdlValue> {
        self.get(name).and_then(|node| node.get(0))
    }

    /// Returns an iterator of the all node arguments (value) of the first child node with a
    /// matching name. This is a shorthand utility for cases where a document
    /// is being used as a key/value store and the value is expected to be
    /// array-ish.
    ///
    /// If a node has no arguments, this will return an empty array.
    ///
    /// # Examples
    ///
    /// Given a document like this:
    /// ```kdl
    /// foo 1 2 3
    /// bar #false
    /// ```
    ///
    /// You can fetch the arguments for `foo` in a single call like this:
    /// ```rust
    /// # use usage::kdl::{KdlDocument, KdlValue};
    /// # let doc: KdlDocument = "foo 1 2 3\nbar #false".parse().unwrap();
    /// assert_eq!(
    ///   doc.iter_args("foo").collect::<Vec<&KdlValue>>(),
    ///   vec![&1.into(), &2.into(), &3.into()]
    /// );
    /// ```
    pub fn iter_args(&self, name: &str) -> impl Iterator<Item = &KdlValue> {
        self.get(name)
            .map(|n| n.entries())
            .unwrap_or_default()
            .iter()
            .filter(|e| e.name().is_none())
            .map(|e| e.value())
    }

    /// Gets a mutable reference to the first argument (value) of the first
    /// child node with a matching name. This is a shorthand utility for cases
    /// where a document is being used as a key/value store.
    pub fn get_arg_mut(&mut self, name: &str) -> Option<&mut KdlValue> {
        self.get_mut(name).and_then(|node| node.get_mut(0))
    }

    /// This utility makes it easy to interact with a KDL convention where
    /// child nodes named `-` are treated as array-ish values.
    ///
    /// # Examples
    ///
    /// Given a document like this:
    /// ```kdl
    /// foo {
    ///   - 1
    ///   - 2
    ///   - #false
    /// }
    /// ```
    ///
    /// You can fetch the dashed child values of `foo` in a single call like this:
    /// ```rust
    /// # use usage::kdl::{KdlDocument, KdlValue};
    /// # let doc: KdlDocument = "foo {\n - 1\n - 2\n - #false\n}".parse().unwrap();
    /// assert_eq!(
    ///     doc.iter_dash_args("foo").collect::<Vec<&KdlValue>>(),
    ///     vec![&1.into(), &2.into(), &false.into()]
    /// );
    /// ```
    pub fn iter_dash_args(&self, name: &str) -> impl Iterator<Item = &KdlValue> {
        self.get(name)
            .and_then(|n| n.children())
            .map(|doc| doc.nodes())
            .unwrap_or_default()
            .iter()
            .filter(|e| e.name().value() == "-")
            .filter_map(|e| e.get(0))
    }

    /// Returns a reference to this document's child nodes.
    pub fn nodes(&self) -> &[KdlNode] {
        &self.nodes
    }

    /// Returns a mutable reference to this document's child nodes.
    pub fn nodes_mut(&mut self) -> &mut Vec<KdlNode> {
        &mut self.nodes
    }

    /// Gets the formatting details (including whitespace and comments) for this entry.
    pub fn format(&self) -> Option<&KdlDocumentFormat> {
        self.format.as_ref()
    }

    /// Gets a mutable reference to this entry's formatting details.
    pub fn format_mut(&mut self) -> Option<&mut KdlDocumentFormat> {
        self.format.as_mut()
    }

    /// Sets the formatting details for this entry.
    pub fn set_format(&mut self, format: KdlDocumentFormat) {
        self.format = Some(format);
    }

    /// Length of this document when rendered as a string.
    pub fn len(&self) -> usize {
        format!("{self}").len()
    }

    /// Returns true if this document is completely empty (including whitespace)
    pub fn is_empty(&self) -> bool {
        self.len() == 0
    }

    /// Clears leading and trailing text (whitespace, comments). `KdlNode`s in
    /// this document will be unaffected.
    ///
    /// If you need to clear the `KdlNode`s, use [`Self::clear_format_recursive`].
    pub fn clear_format(&mut self) {
        self.format = None;
    }

    /// Clears leading and trailing text (whitespace, comments), also clearing
    /// all the `KdlNode`s in the document.
    pub fn clear_format_recursive(&mut self) {
        self.clear_format();
        for node in self.nodes.iter_mut() {
            node.clear_format_recursive();
        }
    }

    /// Auto-formats this Document, making everything nice while preserving
    /// comments.
    pub fn autoformat(&mut self) {
        self.autoformat_config(&FormatConfig::default());
    }

    /// Formats the document and removes all comments from the document.
    pub fn autoformat_no_comments(&mut self) {
        self.autoformat_config(&FormatConfig {
            no_comments: true,
            ..Default::default()
        });
    }

    /// Formats the document according to `config`.
    pub(crate) fn autoformat_config(&mut self, config: &FormatConfig<'_>) {
        if let Some(KdlDocumentFormat { leading, .. }) = (*self).format_mut() {
            crate::kdl::fmt::autoformat_leading(leading, config);
        }
        let mut has_nodes = false;
        for node in &mut self.nodes {
            has_nodes = true;
            node.autoformat_config(config);
        }
        if let Some(KdlDocumentFormat { trailing, .. }) = (*self).format_mut() {
            crate::kdl::fmt::autoformat_trailing(trailing, config.no_comments);
            if !has_nodes {
                trailing.push('\n');
            }
        };
    }

    /// Parses a KDL v2 string into a document.
    pub fn parse(s: &str) -> Result<Self, KdlError> {
        crate::kdl::v2_parser::try_parse(crate::kdl::v2_parser::document, s)
    }
}

impl std::str::FromStr for KdlDocument {
    type Err = KdlError;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        Self::parse(s)
    }
}

impl Display for KdlDocument {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        self.stringify(f, 0)
    }
}

impl KdlDocument {
    pub(crate) fn stringify(
        &self,
        f: &mut std::fmt::Formatter<'_>,
        indent: usize,
    ) -> std::fmt::Result {
        if let Some(KdlDocumentFormat { leading, .. }) = self.format() {
            write!(f, "{leading}")?;
        }
        for node in &self.nodes {
            node.stringify(f, indent)?;
        }
        if let Some(KdlDocumentFormat { trailing, .. }) = self.format() {
            write!(f, "{trailing}")?;
        }
        Ok(())
    }
}

impl IntoIterator for KdlDocument {
    type Item = KdlNode;
    type IntoIter = std::vec::IntoIter<Self::Item>;

    fn into_iter(self) -> Self::IntoIter {
        self.nodes.into_iter()
    }
}

/// Formatting details for [`KdlDocument`]s.
#[derive(Debug, Clone, Default, Hash, Eq, PartialEq)]
pub struct KdlDocumentFormat {
    /// Whitespace and comments preceding the document's first node.
    pub leading: String,
    /// Whitespace and comments following the document's last node.
    pub trailing: String,
}
