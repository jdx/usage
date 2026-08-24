use std::{
    cmp::Ordering,
    fmt::Display,
    ops::{Index, IndexMut},
    slice::{Iter, IterMut},
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
    pub fn autoformat_config(&mut self, config: &FormatConfig<'_>) {
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

    /// Parses a string into a node.
    ///
    /// If the `v1-fallback` feature is enabled, this method will first try to
    /// parse the string as a KDL v2 node, and, if that fails, it will try
    /// to parse again as a KDL v1 node. If both fail, only the v2 parse
    /// errors will be returned.
    pub fn parse(s: &str) -> Result<Self, KdlError> {
        #[cfg(not(feature = "v1-fallback"))]
        {
            v2_parser::try_parse(v2_parser::padded_node, s)
        }
        #[cfg(feature = "v1-fallback")]
        {
            v2_parser::try_parse(v2_parser::padded_node, s)
                .or_else(|e| KdlNode::parse_v1(s).map_err(|_| e))
        }
    }

    /// Parses a KDL v1 string into a document.
    #[cfg(feature = "v1")]
    pub fn parse_v1(s: &str) -> Result<Self, KdlError> {
        let ret: Result<kdlv1::KdlNode, kdlv1::KdlError> = s.parse();
        ret.map(|x| x.into()).map_err(|e| e.into())
    }

    /// Makes sure this node is in v2 format.
    pub fn ensure_v2(&mut self) {
        self.ty = self.ty.take().map(|ty| ty.value().into());
        let v2_name: KdlIdentifier = self.name.value().into();
        self.name = v2_name;
        for entry in self.iter_mut() {
            entry.ensure_v2();
        }
        self.children = self.children.take().map(|mut doc| {
            doc.ensure_v2();
            doc
        });
    }

    /// Makes sure this node is in v1 format.
    #[cfg(feature = "v1")]
    pub fn ensure_v1(&mut self) {
        self.ty = self.ty.take().map(|ty| {
            let v1_name: kdlv1::KdlIdentifier = ty.value().into();
            v1_name.into()
        });
        let v1_name: kdlv1::KdlIdentifier = self.name.value().into();
        self.name = v1_name.into();
        for entry in self.iter_mut() {
            entry.ensure_v1();
        }
        self.children = self.children.take().map(|mut children| {
            children.ensure_v1();
            children
        });
    }
}

#[cfg(feature = "v1")]
impl From<kdlv1::KdlNode> for KdlNode {
    fn from(value: kdlv1::KdlNode) -> Self {
        let terminator = value
            .trailing()
            .map(|t| if t.contains(';') { ";" } else { "\n" })
            .unwrap_or("\n");
        let trailing = value.trailing().map(|t| {
            if t.contains(';') {
                t.replace(';', "")
            } else {
                let t = t.replace("\r\n", "\n");
                let t = t
                    .chars()
                    .map(|c| {
                        if v2_parser::NEWLINES.iter().any(|nl| nl.contains(c)) {
                            '\n'
                        } else {
                            c
                        }
                    })
                    .collect::<String>();
                if terminator == ";" {
                    t
                } else {
                    t.replacen('\n', "", 1)
                }
            }
        });
        KdlNode {
            ty: value.ty().map(|x| x.clone().into()),
            name: value.name().clone().into(),
            entries: value.entries().iter().map(|x| x.clone().into()).collect(),
            children: value.children().map(|x| x.clone().into()),
            format: Some(KdlNodeFormat {
                leading: value.leading().unwrap_or("").into(),
                before_ty_name: "".into(),
                after_ty_name: "".into(),
                after_ty: "".into(),
                before_children: value.before_children().unwrap_or("").into(),
                before_terminator: "".into(),
                terminator: terminator.into(),
                trailing: trailing.unwrap_or_else(|| "".into()),
            }),
            span: SourceSpan::new(value.span().offset().into(), value.span().len()),
        }
    }
}

// Query language
// impl KdlNode {
// /// Queries this Node according to the KQL
// query language, /// returning an iterator over all matching nodes. pub
// fn query_all( &self, query: impl IntoKdlQuery, ) ->
//     Result<KdlQueryIterator<'_>, KdlDiagnostic> { let q =
//     query.into_query()?; Ok(KdlQueryIterator::new(Some(self), None, q))
// }

// /// Queries this Node according to the KQL query language,
// /// returning the first match, if any.
// pub fn query(&self, query: impl IntoKdlQuery) -> Result<Option<&KdlNode>, KdlDiagnostic> {
//     Ok(self.query_all(query)?.next())
// }

// /// Queries this Node according to the KQL query language,
// /// picking the first match, and calling `.get(key)` on it, if the query
// /// succeeded.
// pub fn query_get(
//     &self,
//     query: impl IntoKdlQuery,
//     key: impl Into<NodeKey>,
// ) -> Result<Option<&KdlValue>, KdlDiagnostic> {
//     Ok(self.query(query)?.and_then(|node| node.get(key)))
// }

// /// Queries this Node according to the KQL query language,
// /// returning an iterator over all matching nodes, returning the requested
// /// field from each of those nodes and filtering out nodes that don't have
// /// it.
// pub fn query_get_all(
//     &self,
//     query: impl IntoKdlQuery,
//     key: impl Into<NodeKey>,
// ) -> Result<impl Iterator<Item = &KdlValue>, KdlDiagnostic> {
//     let key: NodeKey = key.into();
//     Ok(self
//         .query_all(query)?
//         .filter_map(move |node| node.get(key.clone())))
// }
//}

/// Iterator for entries in a node, including properties.
#[derive(Debug)]
pub struct EntryIter<'a>(Iter<'a, KdlEntry>);
impl<'a> Iterator for EntryIter<'a> {
    type Item = &'a KdlEntry;

    fn next(&mut self) -> Option<Self::Item> {
        self.0.next()
    }
}

/// Mutable iterator for entries in a node, including properties.
#[derive(Debug)]
pub struct EntryIterMut<'a>(IterMut<'a, KdlEntry>);
impl<'a> Iterator for EntryIterMut<'a> {
    type Item = &'a mut KdlEntry;

    fn next(&mut self) -> Option<Self::Item> {
        self.0.next()
    }
}

/// Iterator for child nodes for this node. Empty if there is no children block.
#[derive(Debug)]
pub struct ChildrenIter<'a>(Option<Iter<'a, KdlNode>>);
impl<'a> Iterator for ChildrenIter<'a> {
    type Item = &'a KdlNode;

    fn next(&mut self) -> Option<Self::Item> {
        self.0.as_mut().and_then(|x| x.next())
    }
}

/// Iterator for child nodes for this node. Empty if there is no children block.
#[derive(Debug)]
pub struct ChildrenIterMut<'a>(Option<IterMut<'a, KdlNode>>);
impl<'a> Iterator for ChildrenIterMut<'a> {
    type Item = &'a mut KdlNode;

    fn next(&mut self) -> Option<Self::Item> {
        self.0.as_mut().and_then(|x| x.next())
    }
}

// Vec-style APIs
impl KdlNode {
    /// Returns an iterator over the node's entries, including properties.
    pub fn iter(&self) -> EntryIter<'_> {
        EntryIter(self.entries.iter())
    }

    /// Returns a mutable iterator over the node's entries, including properties.
    pub fn iter_mut(&mut self) -> EntryIterMut<'_> {
        EntryIterMut(self.entries.iter_mut())
    }

    /// Returns an iterator over the node's children, if any. Nodes without
    /// children will return an empty iterator.
    pub fn iter_children(&self) -> ChildrenIter<'_> {
        ChildrenIter(self.children.as_ref().map(|x| x.nodes.iter()))
    }

    /// Returns a mutable iterator over the node's children, if any. Nodes
    /// without children will return an empty iterator.
    pub fn iter_children_mut(&mut self) -> ChildrenIterMut<'_> {
        ChildrenIterMut(self.children.as_mut().map(|x| x.nodes.iter_mut()))
    }

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

    /// Shorthand for `self.entries_mut().clear()`
    pub fn clear(&mut self) {
        self.entries.clear();
    }

    /// Shorthand for `self.entries_mut().push(entry)`.
    pub fn push(&mut self, entry: impl Into<KdlEntry>) {
        self.entries.push(entry.into());
    }

    /// Inserts an entry into this node. If an entry already exists with the
    /// same string key, it will be replaced and the previous entry will be
    /// returned.
    ///
    /// Numerical keys will insert arguments, string keys will insert
    /// properties.
    pub fn insert(
        &mut self,
        key: impl Into<NodeKey>,
        entry: impl Into<KdlEntry>,
    ) -> Option<KdlEntry> {
        self.insert_impl(key.into(), entry.into())
    }

    fn insert_impl(&mut self, key: NodeKey, mut entry: KdlEntry) -> Option<KdlEntry> {
        match key {
            NodeKey::Key(ref key_val) => {
                if entry.name.is_none() {
                    entry.name = Some(key_val.clone());
                }
                if entry.name.as_ref().map(|i| i.value()) != Some(key_val.value()) {
                    panic!("Property name mismatch");
                }
                if let Some(existing) = self.entry_mut(key) {
                    std::mem::swap(existing, &mut entry);
                    Some(entry)
                } else {
                    self.entries.push(entry);
                    None
                }
            }
            NodeKey::Index(idx) => {
                if entry.name.is_some() {
                    panic!("Cannot insert property with name under a numerical key");
                }
                let mut current_idx = 0;
                for (idx_existing, existing) in self.entries.iter().enumerate() {
                    if existing.name.is_none() {
                        if current_idx == idx {
                            self.entries.insert(idx_existing, entry);
                            return None;
                        }
                        current_idx += 1;
                    }
                }
                if idx > current_idx {
                    panic!("Insertion index (is {idx}) should be <= len (is {current_idx})");
                } else {
                    self.entries.push(entry);
                    None
                }
            }
        }
    }

    // pub fn replace(
    //     &mut self,
    //     key: impl Into<NodeKey>,
    //     entry: impl Into<KdlEntry>,
    // ) -> Option<KdlEntry> {
    //     self.replace_impl(key.into(), entry.into())
    // }

    // fn replace_impl(&mut self, key: NodeKey, mut entry: KdlEntry) -> Option<KdlEntry> {
    //     todo!();
    //     // match key {
    //     //     NodeKey::Key(ref key_val) => {
    //     //         if entry.name.is_none() {
    //     //             entry.name = Some(key_val.clone());
    //     //         }
    //     //         if entry.name.as_ref().map(|i| i.value()) != Some(key_val.value()) {
    //     //             panic!("Property name mismatch");
    //     //         }
    //     //         if let Some(existing) = self.entry_mut(key) {
    //     //             std::mem::swap(existing, &mut entry);
    //     //             Some(entry)
    //     //         } else {
    //     //             self.entries.push(entry);
    //     //             None
    //     //         }
    //     //     }
    //     //     NodeKey::Index(idx) => {
    //     //         if entry.name.is_some() {
    //     //             panic!("Cannot insert property with name under a numerical key");
    //     //         }
    //     //         let mut current_idx = 0;
    //     //         for (idx_existing, existing) in self.entries.iter().enumerate() {
    //     //             if existing.name.is_none() {
    //     //                 if current_idx == idx {
    //     //                     self.entries.replace(idx_existing, entry);
    //     //                     return None;
    //     //                 }
    //     //                 current_idx += 1;
    //     //             }
    //     //         }
    //     //         if idx > current_idx {
    //     //             panic!(
    //     //                 "Insertion index (is {}) should be <= len (is {})",
    //     //                 idx, current_idx
    //     //             );
    //     //         } else {
    //     //             self.entries.push(entry);
    //     //             None
    //     //         }
    //     //     }
    //     // }
    // }

    /// Removes an entry from this node. If an entry already exists with the
    /// same key, it will be returned.
    ///
    /// Numerical keys will remove arguments, string keys will remove
    /// properties.
    pub fn remove(&mut self, key: impl Into<NodeKey>) -> Option<KdlEntry> {
        self.remove_impl(key.into())
    }

    fn remove_impl(&mut self, key: NodeKey) -> Option<KdlEntry> {
        match key {
            NodeKey::Key(key) => {
                for (idx, entry) in self.entries.iter().enumerate() {
                    if entry.name.is_some() && entry.name.as_ref() == Some(&key) {
                        return Some(self.entries.remove(idx));
                    }
                }
                None
            }
            NodeKey::Index(idx) => {
                let mut current_idx = 0;
                for (idx_entry, entry) in self.entries.iter().enumerate() {
                    if entry.name.is_none() {
                        if current_idx == idx {
                            return Some(self.entries.remove(idx_entry));
                        }
                        current_idx += 1;
                    }
                }
                panic!(
                    "removal index (is {idx}) should be < number of index entries (is {current_idx})"
                );
            }
        }
    }

    /// Retains only the elements specified by the predicate.
    pub fn retain<F>(&mut self, keep: F)
    where
        F: FnMut(&KdlEntry) -> bool,
    {
        self.entries.retain(keep)
    }

    /// Sorts the slice with a comparison function, preserving initial order of
    /// equal elements.
    pub fn sort_by<F>(&mut self, compare: F)
    where
        F: FnMut(&KdlEntry, &KdlEntry) -> Ordering,
    {
        self.entries.sort_by(compare)
    }

    /// Sorts the slice with a key extraction function, preserving initial order
    /// of equal elements.
    pub fn sort_by_key<K, F>(&mut self, f: F)
    where
        F: FnMut(&KdlEntry) -> K,
        K: Ord,
    {
        self.entries.sort_by_key(f)
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

#[cfg(all(test, feature = "kdl-upstream-tests"))]
mod test {
    use super::*;

    #[test]
    fn canonical_clear_fmt() -> miette::Result<()> {
        let mut left_node: KdlNode = r#"node /-"commented" param_name=103.000 {
            // This is a nested node
            nested 1 2 3
        }"#
        .parse()?;
        let mut right_node: KdlNode = "node param_name=103.0 { nested 1 2 3; }".parse()?;
        assert_ne!(left_node, right_node);
        left_node.clear_format_recursive();
        right_node.clear_format_recursive();
        assert_eq!(left_node.to_string(), right_node.to_string());
        Ok(())
    }

    #[test]
    fn parsing() -> miette::Result<()> {
        let node: KdlNode = "\n\t  (\"ty\")\"node\" 0xDEADbeef;\n".parse()?;
        assert_eq!(node.ty(), Some(&"\"ty\"".parse()?));
        assert_eq!(node.name(), &"\"node\"".parse()?);
        assert_eq!(node.entry(0), Some(&" 0xDEADbeef".parse()?));
        assert_eq!(
            node.format(),
            Some(&KdlNodeFormat {
                leading: "\n\t  ".into(),
                before_terminator: "".into(),
                terminator: ";".into(),
                trailing: "\n".into(),
                before_ty_name: "".into(),
                after_ty_name: "".into(),
                after_ty: "".into(),
                before_children: "".into(),
            })
        );

        let node: KdlNode = r#"node test {
    link "blah" anything=self
}
"#
        .parse::<KdlNode>()?;
        assert_eq!(node.entry(0), Some(&" test".parse()?));
        assert_eq!(node.children().unwrap().nodes().len(), 1);

        Ok(())
    }

    #[test]
    fn indexing() {
        let mut node = KdlNode::new("foo");
        node.push("bar");
        node["foo"] = 1.into();

        assert_eq!(node[0], "bar".into());
        assert_eq!(node["foo"], 1.into());

        node[0] = false.into();
        node["foo"] = KdlValue::Null;

        assert_eq!(node[0], false.into());
        assert_eq!(node["foo"], KdlValue::Null);

        node.entries_mut().push(KdlEntry::new_prop("x", 1));
        node.entries_mut().push(KdlEntry::new_prop("x", 2));
        assert_eq!(&node["x"], &2.into())
    }

    #[test]
    fn insertion() {
        let mut node = KdlNode::new("foo");
        node.push("pos0");
        node.insert("keyword", 6.0);
        node.push("pos1");
        assert_eq!(node.entries().len(), 3);

        node.insert(0, "inserted0");
        node.insert(2, "inserted1");
        assert_eq!(node.entries().len(), 5);
        assert_eq!(node[0], "inserted0".into());
        assert_eq!(node[1], "pos0".into());
        assert_eq!(node[2], "inserted1".into());
        assert_eq!(node[3], "pos1".into());
    }

    #[test]
    fn removal() {
        let mut node = KdlNode::new("foo");
        node.push("pos0");
        node.insert("keyword", 6.0);
        node.push("pos1");
        assert_eq!(node.entries().len(), 3);

        node.remove(1);
        assert_eq!(node.entries().len(), 2, "index removal should succeed");
        assert!(
            node.get("keyword").is_some(),
            "keyword property should not be removed by index removal"
        );
        node.remove("not an existing keyword");
        assert_eq!(node.entries().len(), 2, "key removal should not succeed");
        node.remove("keyword");
        assert_eq!(node.entries().len(), 1, "key removal should succeed");
        node.remove(0);
        assert_eq!(node.entries().len(), 0, "index removal should suceed");
    }

    #[test]
    #[should_panic(expected = "removal index (is 0) should be < number of index entries (is 0)")]
    fn remove_panic() {
        let mut node = KdlNode::new("foo");
        node.push("pos0");
        node.insert("keyword", 6.0);
        node.remove(0);
        assert_eq!(node.entries().len(), 1, "key removal should succeed");
        node.remove(0); // should panic here
    }
}
