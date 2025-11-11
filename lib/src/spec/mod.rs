pub mod arg;
pub mod choices;
pub mod cmd;
pub mod complete;
pub mod config;
mod context;
mod data_types;
pub mod flag;
pub mod helpers;
pub mod mount;

use indexmap::IndexMap;
use kdl::{KdlDocument, KdlEntry, KdlNode, KdlValue};
use log::{info, warn};
use serde::Serialize;
use std::fmt::{Display, Formatter};
use std::iter::once;
use std::path::Path;
use std::str::FromStr;
use xx::file;

use crate::error::UsageErr;
use crate::spec::cmd::SpecCommand;
use crate::spec::config::SpecConfig;
use crate::spec::context::ParsingContext;
use crate::spec::helpers::NodeHelper;
use crate::{SpecArg, SpecComplete, SpecFlag};

#[derive(Debug, Default, Clone, Serialize)]
pub struct Spec {
    pub name: String,
    pub bin: String,
    pub cmd: SpecCommand,
    pub config: SpecConfig,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub version: Option<String>,
    pub usage: String,
    pub complete: IndexMap<String, SpecComplete>,

    #[serde(skip_serializing_if = "Option::is_none")]
    pub source_code_link_template: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub author: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub about: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub about_long: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub about_md: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub disable_help: Option<bool>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub min_usage_version: Option<String>,
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
    pub fn parse_script(file: &Path) -> Result<Spec, UsageErr> {
        let raw = extract_usage_from_comments(&file::read_to_string(file)?);
        let ctx = ParsingContext::new(file, &raw);
        let mut spec = Self::parse(&ctx, &raw)?;
        if spec.bin.is_empty() {
            spec.bin = file.file_name().unwrap().to_str().unwrap().to_string();
        }
        if spec.name.is_empty() {
            spec.name.clone_from(&spec.bin);
        }
        Ok(spec)
    }

    #[deprecated]
    pub fn parse_spec(input: &str) -> Result<Spec, UsageErr> {
        Self::parse(&Default::default(), input)
    }

    pub fn is_empty(&self) -> bool {
        self.name.is_empty()
            && self.bin.is_empty()
            && self.usage.is_empty()
            && self.cmd.is_empty()
            && self.config.is_empty()
            && self.complete.is_empty()
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
                "bin" => {
                    schema.bin = node.arg(0)?.ensure_string()?;
                    if schema.name.is_empty() {
                        schema.name.clone_from(&schema.bin);
                    }
                }
                "version" => schema.version = Some(node.arg(0)?.ensure_string()?),
                "author" => schema.author = Some(node.arg(0)?.ensure_string()?),
                "source_code_link_template" => {
                    schema.source_code_link_template = Some(node.arg(0)?.ensure_string()?)
                }
                "about" => schema.about = Some(node.arg(0)?.ensure_string()?),
                "long_about" => schema.about_long = Some(node.arg(0)?.ensure_string()?),
                "about_long" => schema.about_long = Some(node.arg(0)?.ensure_string()?),
                "about_md" => schema.about_md = Some(node.arg(0)?.ensure_string()?),
                "usage" => schema.usage = node.arg(0)?.ensure_string()?,
                "arg" => schema.cmd.args.push(SpecArg::parse(ctx, &node)?),
                "flag" => schema.cmd.flags.push(SpecFlag::parse(ctx, &node)?),
                "cmd" => {
                    let node: SpecCommand = SpecCommand::parse(ctx, &node)?;
                    schema.cmd.subcommands.insert(node.name.to_string(), node);
                }
                "config" => schema.config = SpecConfig::parse(ctx, &node)?,
                "complete" => {
                    let complete = SpecComplete::parse(ctx, &node)?;
                    schema.complete.insert(complete.name.clone(), complete);
                }
                "disable_help" => schema.disable_help = Some(node.arg(0)?.ensure_bool()?),
                "min_usage_version" => {
                    let v = node.arg(0)?.ensure_string()?;
                    check_usage_version(&v);
                    schema.min_usage_version = Some(v);
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
                k => bail_parse!(ctx, node.node.name().span(), "unsupported spec key {k}"),
            }
        }
        schema.cmd.name = if schema.bin.is_empty() {
            schema.name.clone()
        } else {
            schema.bin.clone()
        };
        set_subcommand_ancestors(&mut schema.cmd, &[]);
        Ok(schema)
    }

    pub fn merge(&mut self, other: Spec) {
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
        if other.source_code_link_template.is_some() {
            self.source_code_link_template = other.source_code_link_template;
        }
        if other.version.is_some() {
            self.version = other.version;
        }
        if other.author.is_some() {
            self.author = other.author;
        }
        if other.about_long.is_some() {
            self.about_long = other.about_long;
        }
        if other.about_md.is_some() {
            self.about_md = other.about_md;
        }
        if !other.config.is_empty() {
            self.config.merge(&other.config);
        }
        if !other.complete.is_empty() {
            self.complete.extend(other.complete);
        }
        if other.disable_help.is_some() {
            self.disable_help = other.disable_help;
        }
        if other.min_usage_version.is_some() {
            self.min_usage_version = other.min_usage_version;
        }
        self.cmd.merge(other.cmd);
    }
}

fn check_usage_version(version: &str) {
    let cur = versions::Versioning::new(env!("CARGO_PKG_VERSION")).unwrap();
    match versions::Versioning::new(version) {
        Some(v) => {
            if cur < v {
                warn!(
                    "This usage spec requires at least version {version}, but you are using version {cur} of usage"
                );
            }
        }
        _ => warn!("Invalid version: {version}"),
    }
}

fn split_script(file: &Path) -> Result<(String, String), UsageErr> {
    let full = file::read_to_string(file)?;
    if full.starts_with("#!") {
        let usage_regex = xx::regex!(r"^(?:#|//|::)(?:USAGE| ?\[USAGE\])");
        if full.lines().any(|l| usage_regex.is_match(l)) {
            return Ok((extract_usage_from_comments(&full), full));
        }
    }
    let schema = full.strip_prefix("#!/usr/bin/env usage\n").unwrap_or(&full);
    let (schema, body) = schema.split_once("\n#!").unwrap_or((schema, ""));
    let schema = schema
        .trim()
        .lines()
        .filter(|l| !l.starts_with('#'))
        .collect::<Vec<_>>()
        .join("\n");
    let body = format!("#!{body}");
    Ok((schema, body))
}

fn extract_usage_from_comments(full: &str) -> String {
    let usage_regex = xx::regex!(r"^(?:#|//|::)(?:USAGE| ?\[USAGE\])(.*)$");
    let mut usage = vec![];
    let mut found = false;
    for line in full.lines() {
        if let Some(captures) = usage_regex.captures(line) {
            found = true;
            let content = captures.get(1).map_or("", |m| m.as_str());
            usage.push(content.trim());
        } else if found {
            // if there is a gap, stop reading
            break;
        }
    }
    usage.join("\n")
}

fn set_subcommand_ancestors(cmd: &mut SpecCommand, ancestors: &[String]) {
    let ancestors = ancestors.to_vec();
    for subcmd in cmd.subcommands.values_mut() {
        subcmd.full_cmd = ancestors
            .clone()
            .into_iter()
            .chain(once(subcmd.name.clone()))
            .collect();
        set_subcommand_ancestors(subcmd, &subcmd.full_cmd.clone());
    }
    if cmd.usage.is_empty() {
        cmd.usage = cmd.usage();
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
        if let Some(source_code_link_template) = &self.source_code_link_template {
            let mut node = KdlNode::new("source_code_link_template");
            node.push(KdlEntry::new(source_code_link_template.clone()));
            nodes.push(node);
        }
        if let Some(about_md) = &self.about_md {
            let mut node = KdlNode::new("about_md");
            node.push(KdlEntry::new(KdlValue::String(about_md.clone())));
            nodes.push(node);
        }
        if let Some(long_about) = &self.about_long {
            let mut node = KdlNode::new("long_about");
            node.push(KdlEntry::new(KdlValue::String(long_about.clone())));
            nodes.push(node);
        }
        if let Some(disable_help) = self.disable_help {
            let mut node = KdlNode::new("disable_help");
            node.push(KdlEntry::new(disable_help));
            nodes.push(node);
        }
        if let Some(min_usage_version) = &self.min_usage_version {
            let mut node = KdlNode::new("min_usage_version");
            node.push(KdlEntry::new(min_usage_version.clone()));
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
        for complete in self.complete.values() {
            nodes.push(complete.into());
        }
        for complete in self.cmd.complete.values() {
            nodes.push(complete.into());
        }
        for cmd in self.cmd.subcommands.values() {
            nodes.push(cmd.into())
        }
        if !self.config.is_empty() {
            nodes.push((&self.config).into());
        }
        doc.autoformat_config(&kdl::FormatConfigBuilder::new().build());
        write!(f, "{doc}")
    }
}

impl FromStr for Spec {
    type Err = UsageErr;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        Self::parse(&Default::default(), s)
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
            about_long: cmd.get_long_about().map(|a| a.to_string()),
            usage: cmd.clone().render_usage().to_string(),
            ..Default::default()
        }
    }
}

#[inline]
pub fn is_true(b: &bool) -> bool {
    *b
}

#[inline]
pub fn is_false(b: &bool) -> bool {
    !is_true(b)
}

#[cfg(test)]
mod tests {
    use super::*;
    use insta::assert_snapshot;

    #[test]
    fn test_display() {
        let spec = Spec::parse(
            &Default::default(),
            r#"
name "Usage CLI"
bin "usage"
arg "arg1"
flag "-f --force" global=#true
cmd "config" {
  cmd "set" {
    arg "key" help="Key to set"
    arg "value"
  }
}
complete "file" run="ls" descriptions=#true
        "#,
        )
        .unwrap();
        assert_snapshot!(spec, @r#"
        name "Usage CLI"
        bin usage
        flag "-f --force" global=#true
        arg <arg1>
        complete file run=ls descriptions=#true
        cmd config {
            cmd set {
                arg <key> help="Key to set"
                arg <value>
            }
        }
        "#);
    }

    #[test]
    #[cfg(feature = "clap")]
    fn test_clap() {
        let cmd = clap::Command::new("test");
        assert_snapshot!(Spec::from(&cmd), @r#"
        name test
        bin test
        usage "Usage: test"
        "#);
    }

    macro_rules! extract_usage_tests {
        ($($name:ident: $input:expr, $expected:expr,)*) => {
        $(
            #[test]
            fn $name() {
                let result = extract_usage_from_comments($input);
                let expected = $expected.trim_start_matches('\n').trim_end();
                assert_eq!(result, expected);
            }
        )*
        }
    }

    extract_usage_tests! {
        test_extract_usage_from_comments_original_hash:
            r#"
#!/bin/bash
#USAGE bin "test"
#USAGE flag "--foo" help="test"
echo "hello"
            "#,
            r#"
bin "test"
flag "--foo" help="test"
            "#,

        test_extract_usage_from_comments_original_double_slash:
            r#"
#!/usr/bin/env node
//USAGE bin "test"
//USAGE flag "--foo" help="test"
console.log("hello");
            "#,
            r#"
bin "test"
flag "--foo" help="test"
            "#,

        test_extract_usage_from_comments_bracket_with_space:
            r#"
#!/bin/bash
# [USAGE] bin "test"
# [USAGE] flag "--foo" help="test"
echo "hello"
            "#,
            r#"
bin "test"
flag "--foo" help="test"
            "#,

        test_extract_usage_from_comments_bracket_no_space:
            r#"
#!/bin/bash
#[USAGE] bin "test"
#[USAGE] flag "--foo" help="test"
echo "hello"
            "#,
            r#"
bin "test"
flag "--foo" help="test"
            "#,

        test_extract_usage_from_comments_double_slash_bracket_with_space:
            r#"
#!/usr/bin/env node
// [USAGE] bin "test"
// [USAGE] flag "--foo" help="test"
console.log("hello");
            "#,
            r#"
bin "test"
flag "--foo" help="test"
            "#,

        test_extract_usage_from_comments_double_slash_bracket_no_space:
            r#"
#!/usr/bin/env node
//[USAGE] bin "test"
//[USAGE] flag "--foo" help="test"
console.log("hello");
            "#,
            r#"
bin "test"
flag "--foo" help="test"
            "#,

        test_extract_usage_from_comments_stops_at_gap:
            r#"
#!/bin/bash
#USAGE bin "test"
#USAGE flag "--foo" help="test"

#USAGE flag "--bar" help="should not be included"
echo "hello"
            "#,
            r#"
bin "test"
flag "--foo" help="test"
            "#,

        test_extract_usage_from_comments_with_content_after_marker:
            r#"
#!/bin/bash
# [USAGE] bin "test"
# [USAGE] flag "--verbose" help="verbose mode"
# [USAGE] arg "input" help="input file"
echo "hello"
            "#,
            r#"
bin "test"
flag "--verbose" help="verbose mode"
arg "input" help="input file"
            "#,

        test_extract_usage_from_comments_double_colon_original:
            r#"
::USAGE bin "test"
::USAGE flag "--foo" help="test"
echo "hello"
            "#,
            r#"
bin "test"
flag "--foo" help="test"
            "#,

        test_extract_usage_from_comments_double_colon_bracket_with_space:
            r#"
:: [USAGE] bin "test"
:: [USAGE] flag "--foo" help="test"
echo "hello"
            "#,
            r#"
bin "test"
flag "--foo" help="test"
            "#,

        test_extract_usage_from_comments_double_colon_bracket_no_space:
            r#"
::[USAGE] bin "test"
::[USAGE] flag "--foo" help="test"
echo "hello"
            "#,
            r#"
bin "test"
flag "--foo" help="test"
            "#,

        test_extract_usage_from_comments_double_colon_stops_at_gap:
            r#"
::USAGE bin "test"
::USAGE flag "--foo" help="test"

::USAGE flag "--bar" help="should not be included"
echo "hello"
            "#,
            r#"
bin "test"
flag "--foo" help="test"
            "#,

        test_extract_usage_from_comments_double_colon_with_content_after_marker:
            r#"
::USAGE bin "test"
::USAGE flag "--verbose" help="verbose mode"
::USAGE arg "input" help="input file"
echo "hello"
            "#,
            r#"
bin "test"
flag "--verbose" help="verbose mode"
arg "input" help="input file"
            "#,

        test_extract_usage_from_comments_double_colon_bracket_with_space_multiple_lines:
            r#"
:: [USAGE] bin "myapp"
:: [USAGE] flag "--config <file>" help="config file"
:: [USAGE] flag "--verbose" help="verbose output"
:: [USAGE] arg "input" help="input file"
:: [USAGE] arg "[output]" help="output file" required=#false
echo "done"
            "#,
            r#"
bin "myapp"
flag "--config <file>" help="config file"
flag "--verbose" help="verbose output"
arg "input" help="input file"
arg "[output]" help="output file" required=#false
            "#,

        test_extract_usage_from_comments_empty:
            r#"
#!/bin/bash
echo "hello"
            "#,
            "",

        test_extract_usage_from_comments_lowercase_usage:
            r#"
#!/bin/bash
#usage bin "test"
#usage flag "--foo" help="test"
echo "hello"
            "#,
            "",

        test_extract_usage_from_comments_mixed_case_usage:
            r#"
#!/bin/bash
#Usage bin "test"
#Usage flag "--foo" help="test"
echo "hello"
            "#,
            "",

        test_extract_usage_from_comments_space_before_usage:
            r#"
#!/bin/bash
# USAGE bin "test"
# USAGE flag "--foo" help="test"
echo "hello"
            "#,
            "",

        test_extract_usage_from_comments_double_slash_lowercase:
            r#"
#!/usr/bin/env node
//usage bin "test"
//usage flag "--foo" help="test"
console.log("hello");
            "#,
            "",

        test_extract_usage_from_comments_double_slash_mixed_case:
            r#"
#!/usr/bin/env node
//Usage bin "test"
//Usage flag "--foo" help="test"
console.log("hello");
            "#,
            "",

        test_extract_usage_from_comments_double_slash_space_before_usage:
            r#"
#!/usr/bin/env node
// USAGE bin "test"
// USAGE flag "--foo" help="test"
console.log("hello");
            "#,
            "",

        test_extract_usage_from_comments_bracket_lowercase:
            r#"
#!/bin/bash
#[usage] bin "test"
#[usage] flag "--foo" help="test"
echo "hello"
            "#,
            "",

        test_extract_usage_from_comments_bracket_mixed_case:
            r#"
#!/bin/bash
#[Usage] bin "test"
#[Usage] flag "--foo" help="test"
echo "hello"
            "#,
            "",

        test_extract_usage_from_comments_bracket_space_lowercase:
            r#"
#!/bin/bash
# [usage] bin "test"
# [usage] flag "--foo" help="test"
echo "hello"
            "#,
            "",

        test_extract_usage_from_comments_double_colon_lowercase:
            r#"
::usage bin "test"
::usage flag "--foo" help="test"
echo "hello"
            "#,
            "",

        test_extract_usage_from_comments_double_colon_mixed_case:
            r#"
::Usage bin "test"
::Usage flag "--foo" help="test"
echo "hello"
            "#,
            "",

        test_extract_usage_from_comments_double_colon_space_before_usage:
            r#"
:: USAGE bin "test"
:: USAGE flag "--foo" help="test"
echo "hello"
            "#,
            "",

        test_extract_usage_from_comments_double_colon_bracket_lowercase:
            r#"
::[usage] bin "test"
::[usage] flag "--foo" help="test"
echo "hello"
            "#,
            "",

        test_extract_usage_from_comments_double_colon_bracket_mixed_case:
            r#"
::[Usage] bin "test"
::[Usage] flag "--foo" help="test"
echo "hello"
            "#,
            "",

        test_extract_usage_from_comments_double_colon_bracket_space_lowercase:
            r#"
:: [usage] bin "test"
:: [usage] flag "--foo" help="test"
echo "hello"
            "#,
            "",
    }
}
