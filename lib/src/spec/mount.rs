use std::fmt::Display;

use kdl::{KdlEntry, KdlNode};
use serde::Serialize;

use crate::error::Result;
use crate::spec::context::ParsingContext;
use crate::spec::helpers::{string_entry, NodeHelper};

#[derive(Debug, Default, Clone, Serialize)]
#[non_exhaustive]
pub struct SpecMount {
    pub run: String,
    /// Whether a discovered command may take precedence over
    /// [`Spec::default_subcommand`](crate::Spec::default_subcommand).
    ///
    /// Off by default, because resolving a mount runs a process: with a default
    /// subcommand declared, every word that is not a known command would otherwise
    /// pay for discovery before falling back — for a task runner, that is a
    /// subprocess per task invocation. Turn it on when a discovered command should
    /// win, and accept the cost.
    pub overrides_default: bool,
}

impl SpecMount {
    /// A mount that runs `run` to produce a spec for the subcommands here.
    pub fn new(run: impl Into<String>) -> Self {
        Self {
            run: run.into(),
            overrides_default: false,
        }
    }

    /// The same, but a discovered command outranks the default subcommand.
    pub fn overriding_default(run: impl Into<String>) -> Self {
        Self {
            run: run.into(),
            overrides_default: true,
        }
    }
}

impl SpecMount {
    pub(crate) fn parse(ctx: &ParsingContext, node: &NodeHelper) -> Result<Self> {
        let mut mount = SpecMount::default();
        for (k, v) in node.props() {
            match k {
                "run" => mount.run = v.ensure_string()?,
                "overrides_default" => mount.overrides_default = v.ensure_bool()?,
                k => bail_parse!(ctx, v.entry.span(), "unsupported mount key {k}"),
            }
        }
        for child in node.children() {
            match child.name() {
                "run" => mount.run = child.arg(0)?.ensure_string()?,
                "overrides_default" => mount.overrides_default = child.arg(0)?.ensure_bool()?,
                k => bail_parse!(
                    ctx,
                    child.node.name().span(),
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
        format!("mount:{}", self.run)
    }
}

impl From<&SpecMount> for KdlNode {
    fn from(mount: &SpecMount) -> KdlNode {
        let mut node = KdlNode::new("mount");
        node.push(string_entry(Some("run"), &mount.run));
        if mount.overrides_default {
            node.push(KdlEntry::new_prop("overrides_default", true));
        }
        node
    }
}

impl Display for SpecMount {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}", self.usage())
    }
}
