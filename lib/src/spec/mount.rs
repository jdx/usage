use std::fmt::Display;

use kdl::{KdlEntry, KdlNode};
use serde::Serialize;

use crate::error::Result;
use crate::spec::context::ParsingContext;
use crate::spec::helpers::NodeHelper;

#[derive(Debug, Default, Clone, Serialize)]
pub struct SpecMount {
    pub run: String,
}

impl SpecMount {
    pub(crate) fn parse(ctx: &ParsingContext, node: &NodeHelper) -> Result<Self> {
        let mut mount = SpecMount::default();
        for (k, v) in node.props() {
            match k {
                "run" => mount.run = v.ensure_string()?,
                k => bail_parse!(ctx, *v.entry.span(), "unsupported mount key {k}"),
            }
        }
        for child in node.children() {
            match child.name() {
                "run" => mount.run = child.arg(0)?.ensure_string()?,
                k => bail_parse!(
                    ctx,
                    *child.node.name().span(),
                    "unsupported mount value key {k}"
                ),
            }
        }
        if mount.run.is_empty() {
            bail_parse!(ctx, node.span(), "mount run is required")
        }
        Ok(mount)
    }
    pub fn usage(&self) -> String {
        format!("mount:{}", &self.run)
    }
}

impl From<&SpecMount> for KdlNode {
    fn from(mount: &SpecMount) -> KdlNode {
        let mut node = KdlNode::new("mount");
        node.push(KdlEntry::new_prop("run", mount.run.clone()));
        node
    }
}

impl Display for SpecMount {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}", self.usage())
    }
}
// impl PartialEq for SpecMount {
//     fn eq(&self, other: &Self) -> bool {
//         self.run == other.run
//     }
// }
// impl Eq for SpecMount {}
// impl Hash for SpecMount {
//     fn hash<H: std::hash::Hasher>(&self, state: &mut H) {
//         self.run.hash(state);
//     }
// }
