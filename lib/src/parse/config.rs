use std::collections::BTreeMap;

use kdl::{KdlDocument, KdlEntry, KdlNode};
use serde::Serialize;

use crate::bail_parse;
use crate::error::UsageErr;
use crate::parse::context::ParsingContext;
use crate::parse::data_types::SpecDataTypes;
use crate::parse::helpers::NodeHelper;

#[derive(Debug, Default, Clone, Serialize)]
pub struct SpecConfig {
    pub props: BTreeMap<String, SpecConfigProp>,
}

impl SpecConfig {
    pub(crate) fn parse(ctx: &ParsingContext, node: &NodeHelper) -> Result<Self, UsageErr> {
        let mut config = Self::default();
        for node in node.children() {
            node.ensure_arg_len(1..=1)?;
            match node.name() {
                "prop" => {
                    let key = node.arg(0)?;
                    let key = key.ensure_string()?.to_string();
                    let mut prop = SpecConfigProp::default();
                    for (k, v) in node.props() {
                        match k {
                            "default" => prop.default = v.value.to_string().into(),
                            "default_note" => prop.default_note = Some(v.ensure_string()?),
                            "data_type" => prop.data_type = v.ensure_string()?.parse()?,
                            "env" => prop.env = v.ensure_string()?.to_string().into(),
                            "help" => prop.help = v.ensure_string()?.to_string().into(),
                            "long_help" => prop.long_help = v.ensure_string()?.to_string().into(),
                            k => bail_parse!(ctx, node.span(), "unsupported config prop key {k}"),
                        }
                    }
                    config.props.insert(key, prop);
                }
                k => bail_parse!(ctx, *node.node.name().span(), "unsupported config key {k}"),
            }
        }
        Ok(config)
    }

    pub(crate) fn merge(&mut self, other: &Self) {
        for (key, prop) in &other.props {
            self.props
                .entry(key.to_string())
                .or_insert_with(|| prop.clone());
        }
    }
}

impl SpecConfig {
    pub fn is_empty(&self) -> bool {
        self.props.is_empty()
    }
}

#[derive(Debug, Clone, Serialize)]
pub struct SpecConfigProp {
    pub default: Option<String>,
    pub default_note: Option<String>,
    pub data_type: SpecDataTypes,
    pub env: Option<String>,
    pub help: Option<String>,
    pub long_help: Option<String>,
}

impl SpecConfigProp {
    fn to_kdl_node(&self, key: String) -> KdlNode {
        let mut node = KdlNode::new("prop");
        node.push(KdlEntry::new(key));
        if let Some(default) = &self.default {
            node.push(KdlEntry::new_prop("default", default.clone()));
        }
        if let Some(default_note) = &self.default_note {
            node.push(KdlEntry::new_prop("default_note", default_note.clone()));
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

impl Default for SpecConfigProp {
    fn default() -> Self {
        Self {
            default: None,
            default_note: None,
            data_type: SpecDataTypes::Null,
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
        let spec = Spec::parse(
            &Default::default(),
            r#"
config {
    prop "color" default=true env="COLOR" help="Enable color output"
    prop "user" default="admin" env="USER" help="User to run as"
    prop "jobs" default=4 env="JOBS" help="Number of jobs to run"
    prop "timeout" default=1.5 env="TIMEOUT" help="Timeout in seconds" \
        long_help="Timeout in seconds, can be fractional"
}
        "#,
        )
        .unwrap();

        assert_snapshot!(spec, @r###"
        config {
            prop "color" default="true" env="COLOR" help="Enable color output"
            prop "jobs" default="4" env="JOBS" help="Number of jobs to run"
            prop "timeout" default="1.5" env="TIMEOUT" help="Timeout in seconds" long_help="Timeout in seconds, can be fractional"
            prop "user" default="\"admin\"" env="USER" help="User to run as"
        }
        "###);
    }
}
