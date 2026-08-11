pub mod arg;
pub mod builder;
pub mod choices;
pub mod cmd;
pub mod complete;
pub mod config;
mod context;
pub mod data_types;
pub mod effect;
pub mod flag;
pub mod helpers;
pub mod mount;
pub mod unknown_flags;

use indexmap::IndexMap;
use kdl::{KdlDocument, KdlEntry, KdlNode};
use log::{info, warn};
use serde::Serialize;
use std::fmt::{Display, Formatter};
use std::iter::once;
use std::path::Path;
use std::str::FromStr;
use xx::file;

use crate::error::UsageErr;
use crate::spec::cmd::{SpecCommand, SpecExample};
use crate::spec::config::SpecConfig;
use crate::spec::context::ParsingContext;
use crate::spec::helpers::{string_entry, NodeHelper};
use crate::{SpecArg, SpecComplete, SpecFlag};

#[derive(Debug, Default, Clone, Serialize)]
#[non_exhaustive]
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
    /// Where the CLI's source lives, e.g. `https://github.com/jdx/mise`.
    ///
    /// Distinct from [`Self::source_code_link_template`], which is a per-command
    /// deep link with a `{{path}}` placeholder and is only usable for building
    /// "view source" links in generated docs. Scraping a repository out of it
    /// works for one forge and one URL layout and fails everywhere else.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub repository: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub author: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub about: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub about_long: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub about_md: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub license: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub before_help: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub after_help: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub before_help_long: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub after_help_long: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub disable_help: Option<bool>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub min_usage_version: Option<String>,
    #[serde(skip_serializing_if = "Vec::is_empty")]
    pub examples: Vec<SpecExample>,
    /// Default subcommand to use when first non-flag argument is not a known subcommand.
    /// This enables "naked" command syntax like `mise foo` instead of `mise run foo`.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub default_subcommand: Option<String>,
    /// What to do with a flag-like token that names no declared flag, for the whole
    /// CLI. A command may override it; see [`SpecCommand::unknown_flags`].
    pub unknown_flags: Option<crate::spec::unknown_flags::UnknownFlags>,
}

impl Spec {
    /// Parse a spec from a file.
    ///
    /// Automatically detects whether the file is:
    /// - A `.kdl` or `.usage.kdl` file containing a raw spec
    /// - A script file with embedded `#USAGE` comments
    ///
    /// If `bin` is not specified in the spec, it defaults to the filename.
    #[must_use = "parsing result should be used"]
    pub fn parse_file(file: &Path) -> Result<Spec, UsageErr> {
        Self::parse_file_with_metadata_inference(file, true)
    }

    fn parse_file_with_metadata_inference(
        file: &Path,
        infer_metadata_from_filename: bool,
    ) -> Result<Spec, UsageErr> {
        let spec = split_script(file)?;
        let ctx = ParsingContext::new(file, &spec);
        let mut schema = Self::parse(&ctx, &spec)?;
        if infer_metadata_from_filename && schema.bin.is_empty() {
            schema.bin = file
                .file_name()
                .and_then(|n| n.to_str())
                .ok_or_else(|| UsageErr::InvalidPath(file.display().to_string()))?
                .to_string();
        }
        if schema.name.is_empty() {
            schema.name.clone_from(&schema.bin);
        }
        Ok(schema)
    }
    /// Parse a spec from a script file's embedded USAGE comments.
    ///
    /// Extracts the spec from comment lines marked with `#USAGE`, `//USAGE`,
    /// `::USAGE`, or their `[USAGE]` variants.
    /// If `bin` is not specified in the spec, it defaults to the filename.
    #[must_use = "parsing result should be used"]
    pub fn parse_script(file: &Path) -> Result<Spec, UsageErr> {
        let mut spec = Self::parse_script_with_path(&file::read_to_string(file)?, file)?;
        if spec.bin.is_empty() {
            spec.bin = file
                .file_name()
                .and_then(|n| n.to_str())
                .ok_or_else(|| UsageErr::InvalidPath(file.display().to_string()))?
                .to_string();
        }
        if spec.name.is_empty() {
            spec.name.clone_from(&spec.bin);
        }
        Ok(spec)
    }

    /// Parse a spec from a script string's embedded USAGE comments.
    ///
    /// Extracts the spec from comment lines marked with `#USAGE`, `//USAGE`,
    /// `::USAGE`, or their `[USAGE]` variants. Unlike [`Self::parse_script`],
    /// this function cannot infer `bin` or `name` from a filename. Relative
    /// `include` paths are rejected because there is no source path to resolve
    /// them against; absolute `include` paths remain supported.
    #[must_use = "parsing result should be used"]
    pub fn parse_script_str(input: &str) -> Result<Spec, UsageErr> {
        Self::parse_script_with_path(input, Path::new(""))
    }

    fn parse_script_with_path(input: &str, file: &Path) -> Result<Spec, UsageErr> {
        let raw = extract_usage_from_comments(input);
        let ctx = ParsingContext::new(file, &raw);
        Self::parse(&ctx, &raw)
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
            && self.examples.is_empty()
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
                "repository" => schema.repository = Some(node.arg(0)?.ensure_string()?),
                "about" => schema.about = Some(node.arg(0)?.ensure_string()?),
                "long_about" => schema.about_long = Some(node.arg(0)?.ensure_string()?),
                "about_long" => schema.about_long = Some(node.arg(0)?.ensure_string()?),
                "about_md" => schema.about_md = Some(node.arg(0)?.ensure_string()?),
                "license" => schema.license = Some(node.arg(0)?.ensure_string()?),
                "before_help" => schema.before_help = Some(node.arg(0)?.ensure_string()?),
                "after_help" => schema.after_help = Some(node.arg(0)?.ensure_string()?),
                "before_long_help" | "before_help_long" => {
                    schema.before_help_long = Some(node.arg(0)?.ensure_string()?)
                }
                "after_long_help" | "after_help_long" => {
                    schema.after_help_long = Some(node.arg(0)?.ensure_string()?)
                }
                "usage" => schema.usage = node.arg(0)?.ensure_string()?,
                "arg" => schema.cmd.args.push(SpecArg::parse(ctx, &node)?),
                "flag" => schema.cmd.flags.push(SpecFlag::parse(ctx, &node)?),
                // The root is a command like any other, so it can discover its own
                // subcommands by running something. A CLI whose top-level commands
                // come from plugins has no other way to say so.
                "mount" => schema.cmd.mounts.push(crate::SpecMount::parse(ctx, &node)?),
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
                "unknown_flags" => {
                    let raw = node.arg(0)?.ensure_string()?;
                    match raw.parse() {
                        Ok(mode) => schema.unknown_flags = Some(mode),
                        Err(_) => bail_parse!(
                            ctx,
                            node.span(),
                            "unsupported unknown_flags {raw}, expected one of: {}",
                            crate::spec::unknown_flags::UNKNOWN_FLAGS_VALUES
                        ),
                    }
                }
                "default_subcommand" => {
                    schema.default_subcommand = Some(node.arg(0)?.ensure_string()?)
                }
                "example" => {
                    let code = node.ensure_arg_len(1..=1)?.arg(0)?.ensure_string()?;
                    let mut example = SpecExample::new(code.trim().to_string());
                    for (k, v) in node.props() {
                        match k {
                            "header" => example.header = Some(v.ensure_string()?),
                            "help" => example.help = Some(v.ensure_string()?),
                            "lang" => example.lang = v.ensure_string()?,
                            k => bail_parse!(ctx, v.entry.span(), "unsupported example key {k}"),
                        }
                    }
                    schema.examples.push(example);
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
                        true => ctx
                            .file
                            .parent()
                            .ok_or_else(|| {
                                let msg = if ctx.file.as_os_str().is_empty() {
                                    "relative includes require a source file".to_string()
                                } else {
                                    format!("cannot get parent of {}", ctx.file.display())
                                };
                                ctx.build_err(msg, node.span())
                            })?
                            .join(file),
                        false => file.to_path_buf(),
                    };
                    info!("include: {}", file.display());
                    let other = Self::parse_file_with_metadata_inference(&file, false)?;
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
        macro_rules! merge_str {
            ($field:ident) => {
                if !other.$field.is_empty() {
                    self.$field = other.$field;
                }
            };
        }
        macro_rules! merge_opt {
            ($field:ident) => {
                if other.$field.is_some() {
                    self.$field = other.$field;
                }
            };
        }
        macro_rules! merge_extend {
            ($field:ident) => {
                if !other.$field.is_empty() {
                    self.$field.extend(other.$field);
                }
            };
        }

        merge_str!(name);
        merge_str!(bin);
        merge_str!(usage);
        merge_opt!(about);
        merge_opt!(source_code_link_template);
        merge_opt!(repository);
        merge_opt!(version);
        merge_opt!(author);
        merge_opt!(about_long);
        merge_opt!(about_md);
        merge_opt!(license);
        merge_opt!(before_help);
        merge_opt!(after_help);
        merge_opt!(before_help_long);
        merge_opt!(after_help_long);
        merge_opt!(disable_help);
        merge_opt!(min_usage_version);
        merge_opt!(default_subcommand);
        merge_opt!(unknown_flags);
        merge_extend!(complete);
        merge_extend!(examples);

        if !other.config.is_empty() {
            self.config.merge(&other.config);
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

fn split_script(file: &Path) -> Result<String, UsageErr> {
    let full = file::read_to_string(file)?;
    // If file has a shebang and USAGE comments, extract the spec from comments
    if full.starts_with("#!") {
        let usage_regex = xx::regex!(r"^(?:#|//|::)(?:USAGE| ?\[USAGE\])");
        if full.lines().any(|l| usage_regex.is_match(l)) {
            return Ok(extract_usage_from_comments(&full));
        }
    }
    // Otherwise treat the whole file as a KDL spec (e.g., .usage.kdl files)
    Ok(full)
}

fn extract_usage_from_comments(full: &str) -> String {
    let usage_regex = xx::regex!(r"^(?:#|//|::)(?:USAGE| ?\[USAGE\])(.*)$");
    let blank_comment_regex = xx::regex!(r"^(?:#|//|::)\s*$");
    let mut usage = vec![];
    let mut found = false;
    for line in full.lines() {
        if let Some(captures) = usage_regex.captures(line) {
            found = true;
            let content = captures.get(1).map_or("", |m| m.as_str());
            usage.push(content.trim());
        } else if found {
            // Allow blank comment lines to continue parsing
            if blank_comment_regex.is_match(line) {
                continue;
            }
            // if there is a non-blank non-USAGE line, stop reading
            break;
        }
    }
    usage.join("\n")
}

fn set_subcommand_ancestors(cmd: &mut SpecCommand, ancestors: &[String]) {
    for subcmd in cmd.subcommands.values_mut() {
        subcmd.full_cmd = ancestors
            .iter()
            .cloned()
            .chain(once(subcmd.name.clone()))
            .collect();
        let child_ancestors = subcmd.full_cmd.clone();
        set_subcommand_ancestors(subcmd, &child_ancestors);
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
            node.push(string_entry(None, &self.name));
            nodes.push(node);
        }
        if !self.bin.is_empty() {
            let mut node = KdlNode::new("bin");
            node.push(string_entry(None, &self.bin));
            nodes.push(node);
        }
        if let Some(version) = &self.version {
            let mut node = KdlNode::new("version");
            node.push(string_entry(None, version));
            nodes.push(node);
        }
        if let Some(author) = &self.author {
            let mut node = KdlNode::new("author");
            node.push(string_entry(None, author));
            nodes.push(node);
        }
        if let Some(about) = &self.about {
            let mut node = KdlNode::new("about");
            node.push(string_entry(None, about));
            nodes.push(node);
        }
        if let Some(source_code_link_template) = &self.source_code_link_template {
            let mut node = KdlNode::new("source_code_link_template");
            node.push(string_entry(None, source_code_link_template));
            nodes.push(node);
        }
        if let Some(repository) = &self.repository {
            let mut node = KdlNode::new("repository");
            node.push(string_entry(None, repository));
            nodes.push(node);
        }
        if let Some(about_md) = &self.about_md {
            let mut node = KdlNode::new("about_md");
            node.push(string_entry(None, about_md));
            nodes.push(node);
        }
        if let Some(long_about) = &self.about_long {
            let mut node = KdlNode::new("long_about");
            node.push(string_entry(None, long_about));
            nodes.push(node);
        }
        if let Some(license) = &self.license {
            let mut node = KdlNode::new("license");
            node.push(string_entry(None, license));
            nodes.push(node);
        }
        if let Some(before_help) = &self.before_help {
            let mut node = KdlNode::new("before_help");
            node.push(string_entry(None, before_help));
            nodes.push(node);
        }
        if let Some(after_help) = &self.after_help {
            let mut node = KdlNode::new("after_help");
            node.push(string_entry(None, after_help));
            nodes.push(node);
        }
        if let Some(before_help_long) = &self.before_help_long {
            let mut node = KdlNode::new("before_long_help");
            node.push(string_entry(None, before_help_long));
            nodes.push(node);
        }
        if let Some(after_help_long) = &self.after_help_long {
            let mut node = KdlNode::new("after_long_help");
            node.push(string_entry(None, after_help_long));
            nodes.push(node);
        }
        if let Some(disable_help) = self.disable_help {
            let mut node = KdlNode::new("disable_help");
            node.push(KdlEntry::new(disable_help));
            nodes.push(node);
        }
        if let Some(min_usage_version) = &self.min_usage_version {
            let mut node = KdlNode::new("min_usage_version");
            node.push(string_entry(None, min_usage_version));
            nodes.push(node);
        }
        if let Some(unknown_flags) = &self.unknown_flags {
            let mut node = KdlNode::new("unknown_flags");
            node.push(string_entry(None, unknown_flags.as_str()));
            nodes.push(node);
        }
        if let Some(default_subcommand) = &self.default_subcommand {
            let mut node = KdlNode::new("default_subcommand");
            node.push(string_entry(None, default_subcommand));
            nodes.push(node);
        }
        if !self.usage.is_empty() {
            let mut node = KdlNode::new("usage");
            node.push(string_entry(None, &self.usage));
            nodes.push(node);
        }
        for flag in self.cmd.flags.iter() {
            nodes.push(flag.into());
        }
        for arg in self.cmd.args.iter() {
            nodes.push(arg.into());
        }
        // Written here rather than by SpecCommand, because the root's own nodes
        // live at the top level of the document instead of inside a `cmd` block.
        for mount in self.cmd.mounts.iter() {
            nodes.push(mount.into());
        }
        for example in self.examples.iter() {
            nodes.push(example.into());
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
    fn test_repository_round_trips() {
        let spec = Spec::parse(
            &Default::default(),
            r#"
bin "mise"
repository "https://github.com/jdx/mise"
source_code_link_template "https://github.com/jdx/mise/blob/main/src/cli/{{path}}.rs"
        "#,
        )
        .unwrap();
        assert_eq!(
            spec.repository.as_deref(),
            Some("https://github.com/jdx/mise")
        );
        // A spec that is parsed and re-emitted must not lose it, which is the
        // failure mode for every field added to this struct.
        assert_snapshot!(spec, @r#"
        name mise
        bin mise
        source_code_link_template "https://github.com/jdx/mise/blob/main/src/cli/{{path}}.rs"
        repository "https://github.com/jdx/mise"
        "#);
    }

    #[test]
    fn test_repository_merges_like_the_other_optionals() {
        // Extra specs are merged over a generated one, which is how a clap CLI
        // declares anything clap has no concept of.
        let mut generated = Spec::parse(&Default::default(), r#"bin "mise""#).unwrap();
        let extra = Spec::parse(
            &Default::default(),
            r#"repository "https://github.com/jdx/mise""#,
        )
        .unwrap();
        generated.merge(extra);
        assert_eq!(
            generated.repository.as_deref(),
            Some("https://github.com/jdx/mise")
        );
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

    #[test]
    fn test_spec_with_examples() {
        let spec = Spec::parse(
            &Default::default(),
            r#"
name "demo"
bin "demo"
example "demo --help" header="Getting help" help="Display help information"
example "demo --version" header="Check version"
        "#,
        )
        .unwrap();

        assert_eq!(spec.examples.len(), 2);

        assert_eq!(spec.examples[0].code, "demo --help");
        assert_eq!(spec.examples[0].header, Some("Getting help".to_string()));
        assert_eq!(
            spec.examples[0].help,
            Some("Display help information".to_string())
        );

        assert_eq!(spec.examples[1].code, "demo --version");
        assert_eq!(spec.examples[1].header, Some("Check version".to_string()));
        assert_eq!(spec.examples[1].help, None);
    }

    #[test]
    fn test_spec_examples_display() {
        let spec = Spec::parse(
            &Default::default(),
            r#"
name "demo"
bin "demo"
example "demo --help" header="Getting help" help="Show help"
example "demo --version"
        "#,
        )
        .unwrap();

        let output = format!("{}", spec);
        assert!(
            output.contains("example \"demo --help\" header=\"Getting help\" help=\"Show help\"")
        );
        assert!(output.contains("example \"demo --version\""));
    }

    #[test]
    fn test_parse_script_str() {
        let spec = Spec::parse_script_str(
            r#"
#!/bin/bash
#USAGE bin "test"
#USAGE flag "--foo" help="test"
echo "hello"
            "#,
        )
        .unwrap();

        assert_eq!(spec.bin, "test");
        assert_eq!(spec.name, "test");
        assert_eq!(spec.cmd.flags.len(), 1);
        assert_eq!(spec.cmd.flags[0].long, ["foo"]);
    }

    #[test]
    fn test_parse_script_str_rejects_relative_includes() {
        let err = Spec::parse_script_str(r#"#USAGE include file="relative.usage.kdl""#)
            .expect_err("relative includes need a source path");

        match err {
            UsageErr::InvalidInput(msg, _, _) => {
                assert_eq!(msg, "relative includes require a source file");
            }
            err => panic!("unexpected error: {err:?}"),
        }
    }

    #[test]
    fn test_include_does_not_infer_metadata_from_included_filename() {
        let dir = tempfile::tempdir().unwrap();
        let included = dir.path().join("overrides.usage.kdl");
        let root = dir.path().join("my-script.usage.kdl");
        std::fs::write(&included, "").unwrap();
        std::fs::write(&root, "include file=\"./overrides.usage.kdl\"\n").unwrap();

        let spec = Spec::parse_file(&root).unwrap();

        assert_eq!(spec.name, "my-script.usage.kdl");
        assert_eq!(spec.bin, "my-script.usage.kdl");
        assert!(spec.cmd.name.is_empty());
    }
}
