use kdl::{KdlEntry, KdlNode};
use serde::{Deserialize, Serialize};

use crate::error::UsageErr;
use crate::spec::context::ParsingContext;
use crate::spec::helpers::NodeHelper;

#[derive(Debug, Default, Clone, Serialize, Deserialize)]
pub struct SpecComplete {
    pub name: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub run: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub type_: Option<String>,
}

impl SpecComplete {
    pub(crate) fn parse(ctx: &ParsingContext, node: &NodeHelper) -> Result<Self, UsageErr> {
        let mut config = Self::default();
        node.ensure_arg_len(1..=1)?;
        config.name = node.arg(0)?.ensure_string()?.to_string().to_lowercase();
        for (k, v) in node.props() {
            match k {
                "run" => {
                    if config.type_.is_some() {
                        bail_parse!(ctx, *v.entry.span(), "can set run or type, not both")
                    }
                    config.run = Some(v.ensure_string()?.to_string())
                }
                "type" => {
                    if config.run.is_some() {
                        bail_parse!(ctx, *v.entry.span(), "can set run or type, not both")
                    }
                    config.type_ = Some(v.ensure_string()?.to_string())
                }
                k => bail_parse!(ctx, *v.entry.span(), "unsupported complete key {k}"),
            }
        }
        Ok(config)
    }
}

impl From<&SpecComplete> for KdlNode {
    fn from(complete: &SpecComplete) -> Self {
        let mut node = KdlNode::new("complete");
        node.push(KdlEntry::new(complete.name.clone()));
        if let Some(run) = &complete.run {
            node.push(KdlEntry::new_prop("run", run.clone()));
        }
        if let Some(type_) = &complete.type_ {
            node.push(KdlEntry::new_prop("type", type_.clone()));
        }
        node
    }
}
