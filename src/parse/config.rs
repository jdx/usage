use std::collections::BTreeMap;

use kdl::{KdlDocument, KdlEntry, KdlNode, KdlValue};

use crate::bail_parse;
use crate::error::UsageErr;
use crate::parse::helpers::NodeHelper;

#[derive(Debug)]
pub struct SpecConfig {
    pub props: BTreeMap<String, SpecConfigProp>,
}

impl SpecConfig {
    pub fn is_empty(&self) -> bool {
        self.props.is_empty()
    }
}

#[derive(Debug)]
pub struct SpecConfigProp {
    pub default: KdlValue,
    pub env: Option<String>,
    pub help: Option<String>,
    pub long_help: Option<String>,
}

impl SpecConfigProp {
    fn to_kdl_node(&self, key: String) -> KdlNode {
        let mut node = KdlNode::new("prop");
        node.push(KdlEntry::new(key));
        if self.default != KdlValue::Null {
            node.push(KdlEntry::new_prop("default", self.default.clone()));
        }
        if let Some(env) = &self.env {
            node.push(KdlEntry::new_prop("env", env.clone()));
        }
        if let Some(help) = &self.help {
            node.push(KdlEntry::new_prop("help", help.clone()));
        }
        if let Some(long_help) = &self.long_help {
            node.push(KdlEntry::new_prop("long_help", long_help.clone()));
        }
        node
    }
}

impl TryFrom<&KdlNode> for SpecConfig {
    type Error = UsageErr;
    fn try_from(doc: &KdlNode) -> Result<Self, Self::Error> {
        let mut config = Self::default();
        if let Some(children) = doc.children().map(|doc| doc.nodes()) {
            for node in children {
                let ph = NodeHelper::new(node);
                ph.ensure_args_count(1, 1)?;
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

impl From<&SpecConfig> for KdlNode {
    fn from(config: &SpecConfig) -> Self {
        let mut node = KdlNode::new("config");
        for (key, prop) in &config.props {
            let doc = node.children_mut().get_or_insert_with(KdlDocument::new);
            doc.nodes_mut().push(prop.to_kdl_node(key.to_string()));
        }
        node
    }
}

#[cfg(test)]
mod tests {
    use crate::Spec;

    #[test]
    fn test_config_defaults() {
        let spec: Spec = r#"
config {
    prop "color" default=true env="COLOR" help="Enable color output"
    prop "user" default="admin" env="USER" help="User to run as"
    prop "jobs" default=4 env="JOBS" help="Number of jobs to run"
    prop "timeout" default=1.5 env="TIMEOUT" help="Timeout in seconds" \
        long_help="Timeout in seconds, can be fractional"
}
        "#
        .parse()
        .unwrap();
        assert_display_snapshot!(spec, @r###"
        config {
            prop "color" default=true env="COLOR" help="Enable color output"
            prop "jobs" default=4 env="JOBS" help="Number of jobs to run"
            prop "timeout" default=1.5 env="TIMEOUT" help="Timeout in seconds" long_help="Timeout in seconds, can be fractional"
            prop "user" default="admin" env="USER" help="User to run as"
        }
        "###);
    }
}
