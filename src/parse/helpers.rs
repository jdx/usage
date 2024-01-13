use crate::error::UsageErr;
use indexmap::IndexMap;
use kdl::{KdlEntry, KdlNode, KdlValue};

#[derive(Debug)]
pub struct NodeHelper<'a> {
    pub(crate) node: &'a KdlNode,
}

impl<'a> NodeHelper<'a> {
    pub(crate) fn new(node: &'a KdlNode) -> Self {
        Self { node }
    }

    pub(crate) fn name(&self) -> &str {
        self.node.name().value()
    }

    pub(crate) fn arg(&self, i: usize) -> Result<ParseEntry, UsageErr> {
        if let Some(entry) = self.node.entries().get(i) {
            if entry.name().is_some() {
                bail_parse!(entry, "expected argument, got param: {}", entry.to_string())
            }
            return Ok(entry.into());
        }
        bail_parse!(self.node, "missing argument")
    }

    pub(crate) fn props(&self) -> IndexMap<&str, ParseEntry> {
        self.node
            .entries()
            .into_iter()
            .filter_map(|e| match e.name() {
                Some(key) => Some((key.value(), e.into())),
                None => None,
            })
            .collect()
    }
}

impl<'a> From<&'a KdlNode> for NodeHelper<'a> {
    fn from(node: &'a KdlNode) -> Self {
        Self { node }
    }
}

#[derive(Debug)]
pub(crate) struct ParseEntry<'a> {
    pub(crate) entry: &'a KdlEntry,
    pub(crate) value: &'a KdlValue,
}

impl<'a> From<&'a KdlEntry> for ParseEntry<'a> {
    fn from(entry: &'a KdlEntry) -> Self {
        Self {
            entry,
            value: entry.value(),
        }
    }
}

impl<'a> ParseEntry<'a> {
    pub fn ensure_string(&self) -> Result<&str, UsageErr> {
        match self.value.as_string() {
            Some(s) => Ok(s),
            None => bail_parse!(self.entry, "expected string"),
        }
    }
}
