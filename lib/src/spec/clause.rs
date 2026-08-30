use crate::error::Result;
use crate::kdl::{KdlDocument, KdlEntry, KdlNode};
use crate::spec::context::ParsingContext;
use crate::spec::helpers::{string_entry, NodeHelper};
use crate::{SpecArg, SpecFlag};
use serde::Serialize;
use std::collections::HashSet;

/// A repeatable group of scoped flags and positional arguments.
#[derive(Debug, Default, Clone, Serialize)]
#[non_exhaustive]
pub struct SpecClause {
    pub name: String,
    pub separator: Option<String>,
    pub flags: Vec<SpecFlag>,
    pub args: Vec<SpecArg>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub help: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub help_long: Option<String>,
    pub usage: String,
}

impl SpecClause {
    pub(crate) fn conflicting_flag_spelling(&self, command_flags: &[SpecFlag]) -> Option<String> {
        fn spellings(flag: &SpecFlag) -> impl Iterator<Item = String> + '_ {
            flag.long
                .iter()
                .chain(&flag.hidden_aliases)
                .map(|name| format!("--{name}"))
                .chain(
                    flag.short
                        .iter()
                        .chain(&flag.hidden_short_aliases)
                        .map(|name| format!("-{name}")),
                )
                .chain(flag.negate.iter().cloned())
        }

        let mut seen = command_flags
            .iter()
            .flat_map(spellings)
            .collect::<HashSet<_>>();
        self.flags
            .iter()
            .flat_map(spellings)
            .find(|spelling| !seen.insert(spelling.clone()))
    }

    pub(crate) fn parse(ctx: &ParsingContext, node: &NodeHelper) -> Result<Self> {
        let mut clause = Self {
            name: node.arg(0)?.ensure_string()?,
            ..Self::default()
        };
        for (key, value) in node.props() {
            match key {
                "separator" => clause.separator = Some(value.ensure_string()?),
                "help" => clause.help = Some(value.ensure_string()?),
                "help_long" | "long_help" => clause.help_long = Some(value.ensure_string()?),
                key => bail_parse!(ctx, value.entry.span(), "unsupported clause key {key}"),
            }
        }
        for child in node.children() {
            match child.name() {
                "arg" => clause.args.push(SpecArg::parse(ctx, &child)?),
                "flag" => clause.flags.push(SpecFlag::parse(ctx, &child)?),
                key => bail_parse!(
                    ctx,
                    child.node.name().span(),
                    "unsupported clause child {key}"
                ),
            }
        }
        if clause.name.is_empty() {
            bail_parse!(ctx, node.span(), "a clause needs a name");
        }
        if clause.separator.as_ref().is_some_and(String::is_empty) {
            bail_parse!(ctx, node.span(), "clause separator cannot be empty");
        }
        if clause
            .separator
            .as_ref()
            .is_some_and(|separator| separator.starts_with('-'))
        {
            bail_parse!(ctx, node.span(), "clause separator cannot start with `-`");
        }
        if clause.args.is_empty() {
            bail_parse!(
                ctx,
                node.span(),
                "clause {} needs at least one argument",
                clause.name
            );
        }
        if clause.separator.is_none()
            && (clause.args.len() != 1 || !clause.args[0].required || clause.args[0].var)
        {
            bail_parse!(
                ctx,
                node.span(),
                "an implicit clause needs exactly one required, non-variadic positional argument"
            );
        }
        clause.usage = clause.usage();
        Ok(clause)
    }

    pub fn usage(&self) -> String {
        let inner = self
            .args
            .iter()
            .map(SpecArg::usage)
            .collect::<Vec<_>>()
            .join(" ");
        match &self.separator {
            Some(separator) => format!("{inner} [{separator} {inner}]…"),
            None => format!("{inner}…"),
        }
    }
}

impl From<&SpecClause> for KdlNode {
    fn from(clause: &SpecClause) -> Self {
        let mut node = KdlNode::new("clause");
        node.push(KdlEntry::new(clause.name.clone()));
        if let Some(separator) = &clause.separator {
            node.push(string_entry(Some("separator"), separator));
        }
        if let Some(help) = &clause.help {
            node.push(string_entry(Some("help"), help));
        }
        if let Some(help) = &clause.help_long {
            node.push(string_entry(Some("help_long"), help));
        }
        let children = node.children_mut().get_or_insert_with(KdlDocument::new);
        children
            .nodes_mut()
            .extend(clause.flags.iter().map(Into::into));
        children
            .nodes_mut()
            .extend(clause.args.iter().map(Into::into));
        node
    }
}
