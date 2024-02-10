use serde::{Deserialize, Serialize};

use crate::error::UsageErr;
use crate::parse::context::ParsingContext;
use crate::parse::helpers::NodeHelper;

#[derive(Debug, Default, Clone, Serialize, Deserialize)]
pub struct Complete {
    pub name: String,
    pub run: String,
}

impl Complete {
    pub(crate) fn parse(ctx: &ParsingContext, node: &NodeHelper) -> Result<Self, UsageErr> {
        let mut config = Self::default();
        node.ensure_arg_len(1..=1)?;
        config.name = node.arg(0)?.ensure_string()?.to_string();
        for (k, v) in node.props() {
            match k {
                "run" => config.run = v.ensure_string()?.to_string(),
                k => bail_parse!(ctx, node.span(), "unsupported complete key {k}"),
            }
        }
        Ok(config)
    }
}
