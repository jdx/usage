use std::fmt::{Display, Formatter};

use crate::kdl::{self, KdlEntry, KdlNode};
use serde::Serialize;

use crate::error::Result;
use crate::spec::context::ParsingContext;
use crate::spec::helpers::{string_entry, NodeHelper};

/// A named executable surface derived from one command in the canonical spec.
///
/// Multicall binaries use this to describe an applet without copying or mutating the
/// generated command tree. `root` is a space-separated command path. Root global flags may be
/// carried wholesale with `globals=#true`, or selected explicitly with `global` children.
#[derive(Debug, Default, Clone, Serialize)]
#[non_exhaustive]
pub struct SpecView {
    /// Stable identifier used to select the view.
    pub id: String,
    /// Program name shown in prose. Defaults to [`Self::id`].
    pub name: String,
    /// Executable name used in usage lines. Defaults to [`Self::name`].
    pub bin: String,
    /// Command path promoted to the view's root.
    pub root: String,
    /// Carry every root global into the promoted command.
    pub all_globals: bool,
    /// Root-global selectors to carry when [`Self::all_globals`] is false.
    pub globals: Vec<String>,
}

impl SpecView {
    pub(crate) fn parse(ctx: &ParsingContext, node: &NodeHelper<'_>) -> Result<Self> {
        let id = node.arg(0)?.ensure_string()?;
        let mut view = Self {
            name: id.clone(),
            bin: id.clone(),
            id,
            ..Self::default()
        };
        for (key, value) in node.props() {
            match key {
                "name" => view.name = value.ensure_string()?,
                "bin" => view.bin = value.ensure_string()?,
                "root" => view.root = value.ensure_string()?,
                "globals" => view.all_globals = value.ensure_bool()?,
                key => bail_parse!(ctx, value.entry.span(), "unsupported view key {key}"),
            }
        }
        for child in node.children() {
            match child.name() {
                "global" => {
                    for selector in child.args() {
                        view.globals.push(selector.ensure_string()?);
                    }
                }
                key => bail_parse!(
                    ctx,
                    child.node.name().span(),
                    "unsupported view value {key}"
                ),
            }
        }
        if view.id.is_empty() {
            bail_parse!(ctx, node.span(), "a view needs a non-empty identifier");
        }
        if view.name.is_empty() || view.bin.is_empty() {
            bail_parse!(ctx, node.span(), "a view's name and bin cannot be empty");
        }
        if view.root.trim().is_empty() {
            bail_parse!(
                ctx,
                node.span(),
                "a view needs the command path it promotes in `root`"
            );
        }
        Ok(view)
    }
}

impl Display for SpecView {
    fn fmt(&self, f: &mut Formatter<'_>) -> std::fmt::Result {
        let mut node = KdlNode::new("view");
        node.push(string_entry(None, &self.id));
        if self.name != self.id {
            node.push(string_entry(Some("name"), &self.name));
        }
        if self.bin != self.name {
            node.push(string_entry(Some("bin"), &self.bin));
        }
        node.push(string_entry(Some("root"), &self.root));
        if self.all_globals {
            node.push(KdlEntry::new_prop("globals", true));
        }
        if !self.globals.is_empty() {
            let mut children = kdl::KdlDocument::new();
            let mut global = KdlNode::new("global");
            for selector in &self.globals {
                global.push(string_entry(None, selector));
            }
            children.nodes_mut().push(global);
            node.set_children(children);
        }
        write!(f, "{node}")
    }
}
