use indexmap::IndexMap;
use kdl::{KdlEntry, KdlNode, KdlValue};
use miette::SourceSpan;
use std::fmt::Debug;
use std::ops::RangeBounds;

use crate::error::UsageErr;
use crate::spec::context::ParsingContext;

#[derive(Debug)]
pub struct NodeHelper<'a> {
    pub(crate) node: &'a KdlNode,
    pub(crate) ctx: &'a ParsingContext,
}

impl<'a> NodeHelper<'a> {
    pub(crate) fn new(ctx: &'a ParsingContext, node: &'a KdlNode) -> Self {
        Self { node, ctx }
    }

    pub(crate) fn name(&self) -> &str {
        self.node.name().value()
    }
    pub(crate) fn span(&self) -> SourceSpan {
        (self.node.span().offset(), self.node.span().len()).into()
    }
    pub(crate) fn ensure_arg_len<R>(&self, range: R) -> Result<&Self, UsageErr>
    where
        R: RangeBounds<usize> + Debug,
    {
        let count = self.args().count();
        if !range.contains(&count) {
            let ctx = self.ctx;
            let span = self.span();
            bail_parse!(ctx, span, "expected {range:?} arguments, got {count}",)
        }
        Ok(self)
    }
    pub(crate) fn get(&self, key: &str) -> Option<ParseEntry<'_>> {
        self.node.entry(key).map(|e| ParseEntry::new(self.ctx, e))
    }
    pub(crate) fn arg(&self, i: usize) -> Result<ParseEntry<'_>, UsageErr> {
        if let Some(entry) = self.args().nth(i) {
            return Ok(entry);
        }
        bail_parse!(self.ctx, self.span(), "missing argument")
    }
    pub(crate) fn args(&self) -> impl Iterator<Item = ParseEntry<'_>> + '_ {
        self.node
            .entries()
            .iter()
            .filter(|e| e.name().is_none())
            .map(|e| ParseEntry::new(self.ctx, e))
    }
    pub(crate) fn props(&self) -> IndexMap<&str, ParseEntry<'_>> {
        self.node
            .entries()
            .iter()
            .filter_map(|e| {
                e.name()
                    .map(|key| (key.value(), ParseEntry::new(self.ctx, e)))
            })
            .collect()
    }
    pub(crate) fn children(&self) -> Vec<Self> {
        self.node
            .children()
            .map(|c| {
                c.nodes()
                    .iter()
                    .map(|n| NodeHelper::new(self.ctx, n))
                    .collect()
            })
            .unwrap_or_default()
    }
}

#[derive(Debug)]
pub(crate) struct ParseEntry<'a> {
    pub(crate) ctx: &'a ParsingContext,
    pub(crate) entry: &'a KdlEntry,
    pub(crate) value: &'a KdlValue,
}

impl<'a> ParseEntry<'a> {
    fn new(ctx: &'a ParsingContext, entry: &'a KdlEntry) -> Self {
        Self {
            ctx,
            entry,
            value: entry.value(),
        }
    }

    fn span(&self) -> SourceSpan {
        (self.entry.span().offset(), self.entry.span().len()).into()
    }
}

impl ParseEntry<'_> {
    pub fn ensure_usize(&self) -> Result<usize, UsageErr> {
        match self.value.as_integer() {
            Some(i) => Ok(i as usize),
            None => bail_parse!(self.ctx, self.span(), "expected usize"),
        }
    }
    #[allow(dead_code)]
    pub fn ensure_f64(&self) -> Result<f64, UsageErr> {
        match self.value.as_float() {
            Some(f) => Ok(f),
            None => bail_parse!(self.ctx, self.span(), "expected float"),
        }
    }
    pub fn ensure_bool(&self) -> Result<bool, UsageErr> {
        match self.value.as_bool() {
            Some(b) => Ok(b),
            None => bail_parse!(self.ctx, self.span(), "expected bool"),
        }
    }
    pub fn ensure_string(&self) -> Result<String, UsageErr> {
        match self.value.as_string() {
            Some(s) => Ok(s.to_string()),
            None => bail_parse!(self.ctx, self.span(), "expected string"),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use kdl::KdlDocument;
    use std::path::Path;

    fn parse_node(input: &str) -> (ParsingContext, KdlDocument) {
        let ctx = ParsingContext::new(Path::new("test.kdl"), input);
        let doc: KdlDocument = input.parse().unwrap();
        (ctx, doc)
    }

    #[test]
    fn test_node_helper_name() {
        let (ctx, doc) = parse_node("test_node \"arg1\"");
        let node = doc.nodes().first().unwrap();
        let helper = NodeHelper::new(&ctx, node);
        assert_eq!(helper.name(), "test_node");
    }

    #[test]
    fn test_node_helper_arg() {
        let (ctx, doc) = parse_node("node \"first\" \"second\"");
        let node = doc.nodes().first().unwrap();
        let helper = NodeHelper::new(&ctx, node);

        assert_eq!(helper.arg(0).unwrap().ensure_string().unwrap(), "first");
        assert_eq!(helper.arg(1).unwrap().ensure_string().unwrap(), "second");
    }

    #[test]
    fn test_node_helper_args_count() {
        let (ctx, doc) = parse_node("node \"a\" \"b\" \"c\"");
        let node = doc.nodes().first().unwrap();
        let helper = NodeHelper::new(&ctx, node);

        assert_eq!(helper.args().count(), 3);
    }

    #[test]
    fn test_node_helper_props() {
        let (ctx, doc) = parse_node("node key1=\"value1\" key2=\"value2\"");
        let node = doc.nodes().first().unwrap();
        let helper = NodeHelper::new(&ctx, node);

        let props = helper.props();
        assert_eq!(props.len(), 2);
        assert_eq!(props["key1"].ensure_string().unwrap(), "value1");
        assert_eq!(props["key2"].ensure_string().unwrap(), "value2");
    }

    #[test]
    fn test_node_helper_get() {
        let (ctx, doc) = parse_node("node name=\"test\"");
        let node = doc.nodes().first().unwrap();
        let helper = NodeHelper::new(&ctx, node);

        assert!(helper.get("name").is_some());
        assert!(helper.get("nonexistent").is_none());
    }

    #[test]
    fn test_node_helper_children() {
        let (ctx, doc) = parse_node("parent { child1; child2 }");
        let node = doc.nodes().first().unwrap();
        let helper = NodeHelper::new(&ctx, node);

        let children = helper.children();
        assert_eq!(children.len(), 2);
        assert_eq!(children[0].name(), "child1");
        assert_eq!(children[1].name(), "child2");
    }

    #[test]
    fn test_node_helper_ensure_arg_len_valid() {
        let (ctx, doc) = parse_node("node \"a\" \"b\"");
        let node = doc.nodes().first().unwrap();
        let helper = NodeHelper::new(&ctx, node);

        assert!(helper.ensure_arg_len(2..=2).is_ok());
        assert!(helper.ensure_arg_len(1..=3).is_ok());
        assert!(helper.ensure_arg_len(0..).is_ok());
    }

    #[test]
    fn test_node_helper_ensure_arg_len_invalid() {
        let (ctx, doc) = parse_node("node \"a\"");
        let node = doc.nodes().first().unwrap();
        let helper = NodeHelper::new(&ctx, node);

        assert!(helper.ensure_arg_len(2..=2).is_err());
    }

    #[test]
    fn test_parse_entry_ensure_usize() {
        let (ctx, doc) = parse_node("node 42");
        let node = doc.nodes().first().unwrap();
        let helper = NodeHelper::new(&ctx, node);

        assert_eq!(helper.arg(0).unwrap().ensure_usize().unwrap(), 42);
    }

    #[test]
    fn test_parse_entry_ensure_bool() {
        let (ctx, doc) = parse_node("node #true");
        let node = doc.nodes().first().unwrap();
        let helper = NodeHelper::new(&ctx, node);

        assert!(helper.arg(0).unwrap().ensure_bool().unwrap());
    }

    #[test]
    fn test_parse_entry_ensure_string() {
        let (ctx, doc) = parse_node("node \"hello\"");
        let node = doc.nodes().first().unwrap();
        let helper = NodeHelper::new(&ctx, node);

        assert_eq!(helper.arg(0).unwrap().ensure_string().unwrap(), "hello");
    }

    #[test]
    fn test_parse_entry_type_mismatch() {
        let (ctx, doc) = parse_node("node \"not_a_number\"");
        let node = doc.nodes().first().unwrap();
        let helper = NodeHelper::new(&ctx, node);

        assert!(helper.arg(0).unwrap().ensure_usize().is_err());
    }
}
