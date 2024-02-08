use indexmap::IndexMap;
use kdl::{KdlEntry, KdlNode, KdlValue};
use miette::SourceSpan;
use std::fmt::Debug;
use std::ops::RangeBounds;

use crate::error::UsageErr;
use crate::parse::context::ParsingContext;

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
    pub(crate) fn get(&self, key: &str) -> Option<ParseEntry> {
        self.node.get(key).map(|e| ParseEntry::new(self.ctx, e))
    }
    pub(crate) fn arg(&self, i: usize) -> Result<ParseEntry, UsageErr> {
        if let Some(entry) = self.args().nth(i) {
            return Ok(entry);
        }
        bail_parse!(self.ctx, self.span(), "missing argument")
    }
    pub(crate) fn args(&self) -> impl Iterator<Item = ParseEntry> + '_ {
        self.node
            .entries()
            .iter()
            .filter(|e| e.name().is_none())
            .map(|e| ParseEntry::new(self.ctx, e))
    }
    pub(crate) fn props(&self) -> IndexMap<&str, ParseEntry> {
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

impl<'a> ParseEntry<'a> {
    pub fn ensure_i64(&self) -> Result<i64, UsageErr> {
        match self.value.as_i64() {
            Some(i) => Ok(i),
            None => bail_parse!(self.ctx, self.span(), "expected integer"),
        }
    }
    #[allow(dead_code)]
    pub fn ensure_f64(&self) -> Result<f64, UsageErr> {
        match self.value.as_f64() {
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
