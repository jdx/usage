use std::collections::BTreeMap;

use indexmap::IndexMap;
use kdl::{KdlEntry, KdlNode, KdlValue};

use crate::bail_parse;
use crate::error::UsageErr;

#[derive(Debug)]
pub struct SpecConfig {
    pub(crate) props: BTreeMap<String, SpecConfigProp>,
}

#[derive(Debug)]
pub struct SpecConfigProp {
    default: KdlValue,
    env: Option<String>,
    help: Option<String>,
    long_help: Option<String>,
}

#[derive(Debug)]
struct ParseHalp<'a> {
    node: &'a KdlNode,
}

#[derive(Debug)]
struct ParseEntry<'a> {
    entry: &'a KdlEntry,
    value: &'a KdlValue,
}

impl<'a> ParseHalp<'a> {
    fn new(node: &'a KdlNode) -> Self {
        Self { node }
    }

    fn name(&self) -> &str {
        self.node.name().value()
    }

    fn arg(&self, i: usize) -> Result<ParseEntry, UsageErr> {
        if let Some(entry) = self.node.entries().get(i) {
            if entry.name().is_some() {
                bail_parse!(entry, "expected argument, got param: {}", entry.to_string())
            }
            return Ok(entry.into());
        }
        bail_parse!(self.node, "missing argument")
    }

    fn props(&self) -> IndexMap<&str, ParseEntry> {
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

impl<'a> From<&'a KdlNode> for ParseHalp<'a> {
    fn from(node: &'a KdlNode) -> Self {
        Self { node }
    }
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
    fn ensure_string(&self) -> Result<&str, UsageErr> {
        match self.value.as_string() {
            Some(s) => Ok(s),
            None => bail_parse!(self.entry, "expected string"),
        }
    }
}

impl TryFrom<&KdlNode> for SpecConfig {
    type Error = UsageErr;
    fn try_from(doc: &KdlNode) -> Result<Self, Self::Error> {
        let mut config = Self::default();
        if let Some(children) = doc.children().map(|doc| doc.nodes()) {
            for node in children {
                let ph = ParseHalp::new(node);
                match ph.name() {
                    "prop" => {
                        let key = ph.arg(0)?;
                        let key = key.ensure_string()?.to_string();
                        let mut prop = SpecConfigProp::default();
                        for (k, v) in ph.props() {
                            match k {
                                "default" => prop.default = v.value.clone(),
                                "env" => prop.env = v.ensure_string()?.to_string().into(),
                                "help" => prop.help = v.ensure_string()?.to_string().into(),
                                "long_help" => {
                                    prop.long_help = v.ensure_string()?.to_string().into()
                                }
                                _ => bail_parse!(ph.node, "unsupported key {k}"),
                            }
                        }
                        config.props.insert(key, prop);
                    }
                    k => bail_parse!(node.name(), "unsupported key {k}"),
                }
            }
        }
        dbg!(&config);
        Ok(config)
    }
}

impl Default for SpecConfig {
    fn default() -> Self {
        Self {
            props: BTreeMap::new(),
        }
    }
}

impl Default for SpecConfigProp {
    fn default() -> Self {
        Self {
            default: KdlValue::Null,
            env: None,
            help: None,
            long_help: None,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_config_defaults() {
        let spec: Spec = r#"
config {
    prop "color" default=true
    prop "user" default="admin"
    prop "jobs" default=4
    prop "timeout" default=1.5
}
        "#
        .parse()
        .unwrap();
        assert_display_snapshot!(spec, @r###"
        "###);
    }
}
