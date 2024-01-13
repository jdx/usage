use indexmap::IndexMap;
use kdl::{KdlEntry, KdlNode, KdlValue};

use crate::error::UsageErr;

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
    pub(crate) fn ensure_args_count(&self, min: usize, max: usize) -> Result<(), UsageErr> {
        let count = self
            .node
            .entries()
            .into_iter()
            .filter(|e| e.name().is_none())
            .count();
        if count < min || count > max {
            bail_parse!(
                self.node,
                "expected {} to {} arguments, got {}",
                min,
                max,
                count
            )
        }
        Ok(())
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
    pub fn ensure_i64(&self) -> Result<i64, UsageErr> {
        match self.value.as_i64() {
            Some(i) => Ok(i),
            None => bail_parse!(self.entry, "expected integer"),
        }
    }
    #[allow(dead_code)]
    pub fn ensure_f64(&self) -> Result<f64, UsageErr> {
        match self.value.as_f64() {
            Some(f) => Ok(f),
            None => bail_parse!(self.entry, "expected float"),
        }
    }
    pub fn ensure_bool(&self) -> Result<bool, UsageErr> {
        match self.value.as_bool() {
            Some(b) => Ok(b),
            None => bail_parse!(self.entry, "expected bool"),
        }
    }
    pub fn ensure_string(&self) -> Result<String, UsageErr> {
        match self.value.as_string() {
            Some(s) => Ok(s.to_string()),
            None => bail_parse!(self.entry, "expected string"),
        }
    }
}
