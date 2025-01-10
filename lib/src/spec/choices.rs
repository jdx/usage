use kdl::KdlNode;
use serde::{Deserialize, Serialize};

use crate::error::UsageErr;
use crate::spec::context::ParsingContext;
use crate::spec::helpers::NodeHelper;

#[derive(Debug, Default, Clone, Serialize, Deserialize)]
pub struct SpecChoices {
    pub choices: Vec<String>,
}

impl SpecChoices {
    pub(crate) fn parse(_ctx: &ParsingContext, node: &NodeHelper) -> Result<Self, UsageErr> {
        let mut config = Self::default();
        node.ensure_arg_len(1..)?;
        config.choices = node
            .args()
            .map(|e| e.ensure_string())
            .collect::<Result<_, _>>()?;
        Ok(config)
    }
}

impl From<&SpecChoices> for KdlNode {
    fn from(arg: &SpecChoices) -> Self {
        let mut node = KdlNode::new("choices");
        for choice in &arg.choices {
            node.push(choice.to_string());
        }
        node
    }
}
