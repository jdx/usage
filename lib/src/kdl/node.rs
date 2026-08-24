use std::{
    fmt::Display,
    ops::{Index, IndexMut},
    str::FromStr,
};

use crate::miette::SourceSpan;

use crate::kdl::{
    v2_parser, FormatConfig, KdlDocument, KdlDocumentFormat, KdlEntry, KdlError, KdlIdentifier,
    KdlValue,
};

/// Represents an individual KDL
/// [`Node`](https://github.com/kdl-org/kdl/blob/main/SPEC.md#node) inside a
/// KDL Document.
#[derive(Debug, Clone, Eq)]
pub struct KdlNode {
    pub(crate) ty: Option<KdlIdentifier>,
    pub(crate) name: KdlIdentifier,
    // TODO: consider using `hashlink` for this instead, later.
    pub(crate) entries: Vec<KdlEntry>,
    pub(crate) children: Option<KdlDocument>,
    pub(crate) format: Option<KdlNodeFormat>,
    pub(crate) span: SourceSpan,
}

impl PartialEq for KdlNode {
    fn eq(&self, other: &Self) -> bool {
        self.ty == other.ty
            && self.name == other.name
            && self.entries == other.entries
            && self.children == other.children
            && self.format == other.format
        // intentionally omitted: self.span == other.span
    }
}

impl std::hash::Hash for KdlNode {
    fn hash<H: std::hash::Hasher>(&self, state: &mut H) {
        self.ty.hash(state);
        self.name.hash(state);
        self.entries.hash(state);
        self.children.hash(state);
        self.format.hash(state);
        // Intentionally omitted: self.span.hash(state);
    }
}

impl KdlNode {
    /// Creates a new KdlNode with a given name.
    pub fn new(name: impl Into<KdlIdentifier>) -> Self {
        Self {
            name: name.into(),
            ty: None,
            entries: Vec::new(),
            children: None,
            format: Some(KdlNodeFormat {
                trailing: "\n".into(),
                ..Default::default()
            }),
            span: SourceSpan::from(0..0),
        }
    }

    /// Gets this node's name.
    pub fn name(&self) -> &KdlIdentifier {
        &self.name
    }

    /// Gets a mutable reference to this node's name.
    pub fn name_mut(&mut self) -> &mut KdlIdentifier {
        &mut self.name
    }

    /// Sets this node's name.
    pub fn set_name(&mut self, name: impl Into<KdlIdentifier>) {
        self.name = name.into();
    }

    /// Gets this node's span.
    ///
    /// This value will be properly initialized when created via [`KdlDocument::parse`]
    /// but may become invalidated if the document is mutated. We do not currently
    /// guarantee this to yield any particularly consistent results at that point.
    pub fn span(&self) -> SourceSpan {
        self.span
    }

    /// Sets this node's span.
    pub fn set_span(&mut self, span: impl Into<SourceSpan>) {
        self.span = span.into();
    }

    /// Gets the node's type identifier, if any.
    pub fn ty(&self) -> Option<&KdlIdentifier> {
        self.ty.as_ref()
    }

    /// Gets a mutable reference to the node's type identifier.
    pub fn ty_mut(&mut self) -> &mut Option<KdlIdentifier> {
        &mut self.ty
    }

    /// Sets the node's type identifier.
    pub fn set_ty(&mut self, ty: impl Into<KdlIdentifier>) {
        self.ty = Some(ty.into());
    }

    /// Returns a reference to this node's entries (arguments and properties).
    pub fn entries(&self) -> &[KdlEntry] {
        &self.entries
    }

    /// Returns a mutable reference to this node's entries (arguments and
    /// properties).
    pub fn entries_mut(&mut self) -> &mut Vec<KdlEntry> {
        &mut self.entries
    }

    /// Clears leading and trailing text (whitespace, comments), as well as
    /// the space before the children block, if any. Individual entries and
    /// their formatting will be preserved.
    ///
    /// If you want to clear formatting on all children and entries as well,
    /// use [`Self::clear_format_recursive`].
    pub fn clear_format(&mut self) {
        self.format = None;
    }

    /// Clears leading and trailing text (whitespace, comments), as well as
    /// the space before the children block, if any. Individual entries and
    /// children formatting will also be cleared.
    pub fn clear_format_recursive(&mut self) {
        self.clear_format();
        self.name.clear_format();
        if let Some(children) = &mut self.children {
            children.clear_format_recursive();
        }
        for entry in self.entries.iter_mut() {
            entry.clear_format();
        }
    }

    /// Fetches an entry by key. Number keys will look up arguments, strings
    /// will look up properties.
    pub fn entry(&self, key: impl Into<NodeKey>) -> Option<&KdlEntry> {
        self.entry_impl(key.into())
    }

    fn entry_impl(&self, key: NodeKey) -> Option<&KdlEntry> {
        match key {
            NodeKey::Key(key) => {
                let mut current = None;
                for entry in &self.entries {
                    if entry.name.is_some()
                        && entry.name.as_ref().map(|i| i.value()) == Some(key.value())
                    {
                        current = Some(entry);
                    }
                }
                current
            }
            NodeKey::Index(idx) => {
                let mut current_idx = 0;
                for entry in &self.entries {
                    if entry.name.is_none() {
                        if current_idx == idx {
                            return Some(entry);
                        }
                        current_idx += 1;
                    }
                }
                None
            }
        }
    }

    /// Fetches a mutable referene to an entry by key. Number keys will look
    /// up arguments, strings will look up properties.
    pub fn entry_mut(&mut self, key: impl Into<NodeKey>) -> Option<&mut KdlEntry> {
        self.entry_mut_impl(key.into())
    }

    fn entry_mut_impl(&mut self, key: NodeKey) -> Option<&mut KdlEntry> {
        match key {
            NodeKey::Key(key) => {
                let mut current = None;
                for entry in &mut self.entries {
                    if entry.name.is_some()
                        && entry.name.as_ref().map(|i| i.value()) == Some(key.value())
                    {
                        current = Some(entry);
                    }
                }
                current
            }
            NodeKey::Index(idx) => {
                let mut current_idx = 0;
                for entry in &mut self.entries {
                    if entry.name.is_none() {
                        if current_idx == idx {
                            return Some(entry);
                        }
                        current_idx += 1;
                    }
                }
                None
            }
        }
    }

    /// Returns a reference to this node's children, if any.
    pub fn children(&self) -> Option<&KdlDocument> {
        self.children.as_ref()
    }

    /// Returns a mutable reference to this node's children, if any.
    pub fn children_mut(&mut self) -> &mut Option<KdlDocument> {
        &mut self.children
    }

    /// Sets the KdlDocument representing this node's children.
    pub fn set_children(&mut self, children: KdlDocument) {
        self.children = Some(children);
    }

    /// Removes this node's children completely.
    pub fn clear_children(&mut self) {
        self.children = None;
    }

    /// Returns a mutable reference to this node's children [`KdlDocument`],
    /// creating one first if one does not already exist.
    pub fn ensure_children(&mut self) -> &mut KdlDocument {
        if self.children.is_none() {
            self.children = Some(KdlDocument::new());
        }
        self.children_mut().as_mut().unwrap()
    }

    /// Gets the formatting details (including whitespace and comments) for this node.
    pub fn format(&self) -> Option<&KdlNodeFormat> {
        self.format.as_ref()
    }

    /// Gets a mutable reference to this node's formatting details.
    pub fn format_mut(&mut self) -> Option<&mut KdlNodeFormat> {
        self.format.as_mut()
    }

    /// Sets the formatting details for this node.
    pub fn set_format(&mut self, format: KdlNodeFormat) {
        self.format = Some(format);
    }
    /// Auto-formats this node and its contents.
    pub fn autoformat(&mut self) {
        self.autoformat_config(&FormatConfig::default());
    }

    /// Auto-formats this node and its contents, stripping comments.
    pub fn autoformat_no_comments(&mut self) {
        self.autoformat_config(&FormatConfig {
            no_comments: true,
            ..Default::default()
        });
    }

    /// Auto-formats this node and its contents according to `config`.
    pub(crate) fn autoformat_config(&mut self, config: &FormatConfig<'_>) {
        if let Some(KdlNodeFormat {
            leading,
            before_terminator,
            terminator,
            trailing,
            before_children,
            ..
        }) = self.format_mut()
        {
            crate::kdl::fmt::autoformat_leading(leading, config);
            crate::kdl::fmt::autoformat_trailing(before_terminator, config.no_comments);
            crate::kdl::fmt::autoformat_trailing(trailing, config.no_comments);
            *trailing = trailing.trim().into();
            if !terminator.starts_with('\n') {
                *terminator = "\n".into();
            }
            if let Some(c) = trailing.chars().next() {
                if !c.is_whitespace() {
                    trailing.insert(0, ' ');
                }
            }

            *before_children = " ".into();
        } else {
            self.set_format(KdlNodeFormat {
                terminator: "\n".into(),
                ..Default::default()
            })
        }
        self.name.clear_format();
        if let Some(ty) = self.ty.as_mut() {
            ty.clear_format()
        }
        for entry in &mut self.entries {
            if config.entry_autoformate_keep {
                entry.keep_format();
            }
            entry.autoformat();
        }
        if let Some(children) = self.children.as_mut() {
            children.autoformat_config(&FormatConfig {
                indent_level: config.indent_level + 1,
                ..*config
            });
            if let Some(KdlDocumentFormat { leading, trailing }) = children.format_mut() {
                *leading = leading.trim().into();
                leading.push('\n');
                for _ in 0..config.indent_level {
                    trailing.push_str(config.indent);
                }
            }
        }
    }

    /// Parses a KDL v2 string into a node.
    pub fn parse(s: &str) -> Result<Self, KdlError> {
        v2_parser::try_parse(v2_parser::padded_node, s)
    }
}

// Vec-style APIs
impl KdlNode {
    /// Gets a value by key. Number keys will look up arguments, strings will
    /// look up properties.
    pub fn get(&self, key: impl Into<NodeKey>) -> Option<&KdlValue> {
        self.entry_impl(key.into()).map(|e| &e.value)
    }

    /// Fetches a mutable referene to an value by key. Number keys will look
    /// up arguments, strings will look up properties.
    pub fn get_mut(&mut self, key: impl Into<NodeKey>) -> Option<&mut KdlValue> {
        self.entry_mut_impl(key.into()).map(|e| &mut e.value)
    }

    /// Number of entries in this node.
    pub fn len(&self) -> usize {
        self.entries.len()
    }

    /// Returns true if this node is completely empty (including whitespace).
    pub fn is_empty(&self) -> bool {
        self.entries.is_empty()
    }

    /// Shorthand for `self.entries_mut().push(entry)`.
    pub fn push(&mut self, entry: impl Into<KdlEntry>) {
        self.entries.push(entry.into());
    }
}

/// Represents a [`KdlNode`]'s entry key.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum NodeKey {
    /// Key for a node property entry.
    Key(KdlIdentifier),
    /// Index for a node argument entry (positional value).
    Index(usize),
}

impl From<&str> for NodeKey {
    fn from(key: &str) -> Self {
        Self::Key(key.into())
    }
}

impl From<String> for NodeKey {
    fn from(key: String) -> Self {
        Self::Key(key.into())
    }
}

impl From<usize> for NodeKey {
    fn from(key: usize) -> Self {
        Self::Index(key)
    }
}

impl Index<usize> for KdlNode {
    type Output = KdlValue;

    fn index(&self, index: usize) -> &Self::Output {
        self.get(index).expect("Argument out of range.")
    }
}

impl IndexMut<usize> for KdlNode {
    fn index_mut(&mut self, index: usize) -> &mut Self::Output {
        self.get_mut(index).expect("Argument out of range.")
    }
}

impl Index<&str> for KdlNode {
    type Output = KdlValue;

    fn index(&self, key: &str) -> &Self::Output {
        self.get(key).expect("No such property.")
    }
}

impl IndexMut<&str> for KdlNode {
    fn index_mut(&mut self, key: &str) -> &mut Self::Output {
        if self.get(key).is_none() {
            self.push((key, KdlValue::Null));
        }
        self.get_mut(key).expect("Something went wrong.")
    }
}

impl FromStr for KdlNode {
    type Err = KdlError;

    fn from_str(input: &str) -> Result<Self, Self::Err> {
        v2_parser::try_parse(v2_parser::padded_node, input)
    }
}

impl Display for KdlNode {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        self.stringify(f, 0)
    }
}

impl KdlNode {
    pub(crate) fn stringify(
        &self,
        f: &mut std::fmt::Formatter<'_>,
        indent: usize,
    ) -> std::fmt::Result {
        if let Some(KdlNodeFormat { leading, .. }) = self.format() {
            write!(f, "{leading}")?;
        } else {
            write!(f, "{:indent$}", "", indent = indent)?;
        }
        if let Some(ty) = &self.ty {
            write!(f, "({ty})")?;
        }
        write!(f, "{}", self.name)?;
        let mut space_before_children = true;
        for entry in &self.entries {
            if entry.format().is_none() {
                write!(f, " ")?;
            }
            write!(f, "{entry}")?;
            space_before_children = entry.format().is_none();
        }
        if let Some(children) = &self.children {
            if let Some(KdlNodeFormat {
                before_children, ..
            }) = self.format()
            {
                write!(f, "{before_children}")?;
            } else if space_before_children {
                write!(f, " ")?;
            }
            write!(f, "{{")?;
            if children.format().is_none() {
                writeln!(f)?;
            }
            children.stringify(f, indent + 4)?;
            if children.format().is_none() {
                write!(f, "{:indent$}", "", indent = indent)?;
            }
            write!(f, "}}")?;
        }
        if let Some(KdlNodeFormat {
            before_terminator,
            terminator,
            trailing,
            ..
        }) = self.format()
        {
            write!(f, "{before_terminator}{terminator}{trailing}")?;
        }
        Ok(())
    }
}

/// Formatting details for [`KdlNode`].
#[derive(Debug, Clone, Default, Hash, Eq, PartialEq)]
pub struct KdlNodeFormat {
    /// Whitespace and comments preceding the node itself.
    pub leading: String,
    /// Whitespace and comments between the opening `(` of a type annotation and the actual annotation name.
    pub before_ty_name: String,
    /// Whitespace and comments between the annotation name and the closing `)`.
    pub after_ty_name: String,
    /// Whitespace and comments after a node's type annotation.
    pub after_ty: String,
    /// Whitespace and comments preceding the node's children block.
    pub before_children: String,
    /// Whitespace and comments right before the node's terminator.
    pub before_terminator: String,
    /// The terminator for the node.
    pub terminator: String,
    /// Whitespace and comments following the node itself, after the terminator.
    pub trailing: String,
}
