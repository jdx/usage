use indexmap::IndexMap;
use std::fmt::{Display, Formatter};
use std::iter::once;
use std::path::Path;

use kdl::{KdlDocument, KdlEntry, KdlNode, KdlValue};
use serde::Serialize;
use xx::file;

use crate::error::UsageErr;
use crate::parse::cmd::SpecCommand;
use crate::parse::config::SpecConfig;
use crate::parse::context::ParsingContext;
use crate::parse::helpers::NodeHelper;
use crate::{Complete, SpecArg, SpecFlag};

#[derive(Debug, Default, Clone, Serialize)]
pub struct Spec {
    pub name: String,
    pub bin: String,
    pub cmd: SpecCommand,
    pub config: SpecConfig,
    pub version: Option<String>,
    pub usage: String,
    pub complete: IndexMap<String, Complete>,

    pub author: Option<String>,
    pub about: Option<String>,
    pub long_about: Option<String>,
}

impl Spec {
    pub fn parse_file(file: &Path) -> Result<(Spec, String), UsageErr> {
        let (spec, body) = split_script(file)?;
        let ctx = ParsingContext::new(file, &spec);
        let mut schema = Self::parse(&ctx, &spec)?;
        if schema.bin.is_empty() {
            schema.bin = file.file_name().unwrap().to_str().unwrap().to_string();
        }
        if schema.name.is_empty() {
            schema.name.clone_from(&schema.bin);
        }
        Ok((schema, body))
    }
    pub fn parse_spec(input: &str) -> Result<Spec, UsageErr> {
        Self::parse(&Default::default(), input)
    }

    pub fn is_empty(&self) -> bool {
        self.name.is_empty()
            && self.bin.is_empty()
            && self.usage.is_empty()
            && self.cmd.is_empty()
            && self.config.is_empty()
    }

    pub(crate) fn parse(ctx: &ParsingContext, input: &str) -> Result<Spec, UsageErr> {
        let kdl: KdlDocument = input
            .parse()
            .map_err(|err: kdl::KdlError| UsageErr::KdlError(err))?;
        let mut schema = Self {
            ..Default::default()
        };
        for node in kdl.nodes().iter().map(|n| NodeHelper::new(ctx, n)) {
            match node.name() {
                "name" => schema.name = node.arg(0)?.ensure_string()?,
                "bin" => schema.bin = node.arg(0)?.ensure_string()?,
                "version" => schema.version = Some(node.arg(0)?.ensure_string()?),
                "author" => schema.author = Some(node.arg(0)?.ensure_string()?),
                "about" => schema.about = Some(node.arg(0)?.ensure_string()?),
                "long_about" => schema.long_about = Some(node.arg(0)?.ensure_string()?),
                "usage" => schema.usage = node.arg(0)?.ensure_string()?,
                "arg" => schema.cmd.args.push(SpecArg::parse(ctx, &node)?),
                "flag" => schema.cmd.flags.push(SpecFlag::parse(ctx, &node)?),
                "cmd" => {
                    let node: SpecCommand = SpecCommand::parse(ctx, &node)?;
                    schema.cmd.subcommands.insert(node.name.to_string(), node);
                }
                "config" => schema.config = SpecConfig::parse(ctx, &node)?,
                "complete" => {
                    let complete = Complete::parse(ctx, &node)?;
                    schema.complete.insert(complete.name.clone(), complete);
                }
                "include" => {
                    let file = node
                        .props()
                        .get("file")
                        .map(|v| v.ensure_string())
                        .transpose()?
                        .ok_or_else(|| ctx.build_err("missing file".into(), node.span()))?;
                    let file = Path::new(&file);
                    let file = match file.is_relative() {
                        true => ctx.file.parent().unwrap().join(file),
                        false => file.to_path_buf(),
                    };
                    info!("include: {}", file.display());
                    let (other, _) = Self::parse_file(&file)?;
                    schema.merge(other);
                }
                k => bail_parse!(ctx, *node.node.name().span(), "unsupported spec key {k}"),
            }
        }
        set_subcommand_ancestors(&mut schema.cmd, &[]);
        Ok(schema)
    }

    fn merge(&mut self, other: Spec) {
        if !other.name.is_empty() {
            self.name = other.name;
        }
        if !other.bin.is_empty() {
            self.bin = other.bin;
        }
        if !other.usage.is_empty() {
            self.usage = other.usage;
        }
        if other.about.is_some() {
            self.about = other.about;
        }
        if other.long_about.is_some() {
            self.long_about = other.long_about;
        }
        if !other.config.is_empty() {
            self.config.merge(&other.config);
        }
        if !other.complete.is_empty() {
            self.complete.extend(other.complete);
        }
        self.cmd.merge(other.cmd);
    }
}

fn split_script(file: &Path) -> Result<(String, String), UsageErr> {
    let full = file::read_to_string(file)?;
    if full.contains("# |usage.jdx.dev|") {
        return Ok((extract_usage_from_comments(&full), full));
    }
    let schema = full.strip_prefix("#!/usr/bin/env usage\n").unwrap_or(&full);
    let (schema, body) = schema.split_once("\n#!").unwrap_or((schema, ""));
    let schema = schema
        .trim()
        .lines()
        .filter(|l| !l.starts_with('#'))
        .collect::<Vec<_>>()
        .join("\n");
    let body = format!("#!{}", body);
    Ok((schema, body))
}

fn extract_usage_from_comments(full: &str) -> String {
    let mut usage = vec![];
    let mut inside = false;
    for line in full.lines() {
        if line.starts_with("# |usage.jdx.dev|") {
            inside = !inside;
            continue;
        }
        if inside {
            usage.push(line.strip_prefix("# ").unwrap());
        }
    }
    usage.join("\n")
}

fn set_subcommand_ancestors(cmd: &mut SpecCommand, ancestors: &[String]) {
    if cmd.usage.is_empty() {
        cmd.usage = cmd.usage();
    }
    let ancestors = ancestors.to_vec();
    for subcmd in cmd.subcommands.values_mut() {
        subcmd.full_cmd = ancestors
            .clone()
            .into_iter()
            .chain(once(subcmd.name.clone()))
            .collect();
        set_subcommand_ancestors(subcmd, &subcmd.full_cmd.clone());
    }
}

impl Display for Spec {
    fn fmt(&self, f: &mut Formatter<'_>) -> std::fmt::Result {
        let mut doc = KdlDocument::new();
        let nodes = &mut doc.nodes_mut();
        if !self.name.is_empty() {
            let mut node = KdlNode::new("name");
            node.push(KdlEntry::new(self.name.clone()));
            nodes.push(node);
        }
        if !self.bin.is_empty() {
            let mut node = KdlNode::new("bin");
            node.push(KdlEntry::new(self.bin.clone()));
            nodes.push(node);
        }
        if let Some(version) = &self.version {
            let mut node = KdlNode::new("version");
            node.push(KdlEntry::new(version.clone()));
            nodes.push(node);
        }
        if let Some(author) = &self.author {
            let mut node = KdlNode::new("author");
            node.push(KdlEntry::new(author.clone()));
            nodes.push(node);
        }
        if let Some(about) = &self.about {
            let mut node = KdlNode::new("about");
            node.push(KdlEntry::new(about.clone()));
            nodes.push(node);
        }
        if let Some(long_about) = &self.long_about {
            let mut node = KdlNode::new("long_about");
            node.push(KdlEntry::new(KdlValue::RawString(long_about.clone())));
            nodes.push(node);
        }
        if !self.usage.is_empty() {
            let mut node = KdlNode::new("usage");
            node.push(KdlEntry::new(self.usage.clone()));
            nodes.push(node);
        }
        for flag in self.cmd.flags.iter() {
            nodes.push(flag.into());
        }
        for arg in self.cmd.args.iter() {
            nodes.push(arg.into());
        }
        for cmd in self.cmd.subcommands.values() {
            nodes.push(cmd.into())
        }
        if !self.config.is_empty() {
            nodes.push((&self.config).into());
        }
        write!(f, "{}", doc)
    }
}

#[cfg(feature = "clap")]
impl From<&clap::Command> for Spec {
    fn from(cmd: &clap::Command) -> Self {
        Spec {
            name: cmd.get_name().to_string(),
            bin: cmd.get_bin_name().unwrap_or(cmd.get_name()).to_string(),
            cmd: cmd.into(),
            version: cmd.get_version().map(|v| v.to_string()),
            about: cmd.get_about().map(|a| a.to_string()),
            long_about: cmd.get_long_about().map(|a| a.to_string()),
            usage: cmd.clone().render_usage().to_string(),
            ..Default::default()
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_display() {
        let spec = Spec::parse(
            &Default::default(),
            r#"
name "Usage CLI"
bin "usage"
arg "arg1"
flag "-f --force" global=true
cmd "config" {
  cmd "set" {
    arg "key" help="Key to set"
    arg "value"
  }
}
        "#,
        )
        .unwrap();
        assert_snapshot!(spec, @r###"
        name "Usage CLI"
        bin "usage"
        flag "-f --force" global=true
        arg "<arg1>"
        cmd "config" {
            cmd "set" {
                arg "<key>" help="Key to set"
                arg "<value>"
            }
        }
        "###);
    }

    #[test]
    #[cfg(feature = "clap")]
    fn test_clap() {
        let cmd = clap::Command::new("test");
        assert_snapshot!(Spec::from(&cmd), @r###"
        name "test"
        bin "test"
        usage "Usage: test"
        "###);
    }
}
