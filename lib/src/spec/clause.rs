use crate::error::Result;
use crate::kdl::{KdlDocument, KdlEntry, KdlNode};
use crate::spec::context::ParsingContext;
use crate::spec::helpers::{string_entry, NodeHelper};
use crate::SpecArg;
use serde::Serialize;

/// A repeatable, separator-delimited group of positional arguments.
#[derive(Debug, Default, Clone, Serialize)]
#[non_exhaustive]
pub struct SpecClause {
    pub name: String,
    pub separator: String,
    pub args: Vec<SpecArg>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub help: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub help_long: Option<String>,
    pub usage: String,
}

impl SpecClause {
    pub(crate) fn parse(ctx: &ParsingContext, node: &NodeHelper) -> Result<Self> {
        let mut clause = Self {
            name: node.arg(0)?.ensure_string()?,
            ..Self::default()
        };
        for (key, value) in node.props() {
            match key {
                "separator" => clause.separator = value.ensure_string()?,
                "help" => clause.help = Some(value.ensure_string()?),
                "help_long" | "long_help" => clause.help_long = Some(value.ensure_string()?),
                key => bail_parse!(ctx, value.entry.span(), "unsupported clause key {key}"),
            }
        }
        for child in node.children() {
            match child.name() {
                "arg" => clause.args.push(SpecArg::parse(ctx, &child)?),
                key => bail_parse!(ctx, child.node.name().span(), "unsupported clause child {key}"),
            }
        }
        if clause.name.is_empty() {
            bail_parse!(ctx, node.span(), "a clause needs a name");
        }
        if clause.separator.is_empty() {
            bail_parse!(ctx, node.span(), "clause {} needs a non-empty separator", clause.name);
        }
        if clause.separator.starts_with('-') {
            bail_parse!(ctx, node.span(), "clause separator cannot start with `-`");
        }
        if clause.args.is_empty() {
            bail_parse!(ctx, node.span(), "clause {} needs at least one argument", clause.name);
        }
        clause.usage = clause.usage();
        Ok(clause)
    }

    pub fn usage(&self) -> String {
        let inner = self.args.iter().map(SpecArg::usage).collect::<Vec<_>>().join(" ");
        format!("{inner} [{} {inner}]…", self.separator)
    }
}

impl From<&SpecClause> for KdlNode {
    fn from(clause: &SpecClause) -> Self {
        let mut node = KdlNode::new("clause");
        node.push(KdlEntry::new(clause.name.clone()));
        node.push(string_entry(Some("separator"), &clause.separator));
        if let Some(help) = &clause.help {
            node.push(string_entry(Some("help"), help));
        }
        if let Some(help) = &clause.help_long {
            node.push(string_entry(Some("help_long"), help));
        }
        let children = node.children_mut().get_or_insert_with(KdlDocument::new);
        children.nodes_mut().extend(clause.args.iter().map(Into::into));
        node
    }
}
