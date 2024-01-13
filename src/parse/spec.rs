use std::fmt::{Display, Formatter};
use std::path::{Path, PathBuf};
use std::str::FromStr;

use kdl::{KdlDocument, KdlEntry, KdlNode};

use xx::{context, file};

use crate::error::UsageErr;
use crate::parse::cmd::SchemaCmd;
use crate::parse::config::SpecConfig;
use miette::NamedSource;
use std::cell::RefCell;

#[derive(Debug, Default)]
pub struct Spec {
    pub name: String,
    pub bin: String,
    pub cmd: SchemaCmd,
    pub config: SpecConfig,
}

impl Spec {
    thread_local! {
        static PARSING_FILE: RefCell<Option<(PathBuf, String)>> = RefCell::new(None);
    }

    pub fn parse_file(file: &Path) -> miette::Result<(Spec, String)> {
        let (spec, body) = split_script(file)?;
        Self::set_parsing_file(Some((file.to_path_buf(), spec.clone())));
        let mut schema = Self::from_str(&spec)?;
        if schema.bin.is_empty() {
            schema.bin = file.file_name().unwrap().to_str().unwrap().to_string();
        }
        if schema.name.is_empty() {
            schema.name = schema.bin.clone();
        }
        Self::set_parsing_file(None);
        Ok((schema, body))
    }

    pub fn get_parsing_file() -> NamedSource {
        Self::PARSING_FILE.with(|f| {
            f.borrow()
                .as_ref()
                .map(|(p, s)| NamedSource::new(p.to_string_lossy().to_string(), s.clone()))
                .unwrap_or_else(|| NamedSource::new("".to_string(), "".to_string()))
        })
    }
    fn set_parsing_file(file: Option<(PathBuf, String)>) {
        Self::PARSING_FILE.with(|f| *f.borrow_mut() = file);
    }

    fn merge(&mut self, other: Spec) {
        if !other.name.is_empty() {
            self.name = other.name;
        }
        if !other.bin.is_empty() {
            self.bin = other.bin;
        }
        for flag in other.cmd.flags {
            self.cmd.flags.push(flag);
        }
        for arg in other.cmd.args {
            self.cmd.args.push(arg);
        }
        for (name, cmd) in other.cmd.subcommands {
            self.cmd.subcommands.insert(name, cmd);
        }
    }
}

fn split_script(file: &Path) -> miette::Result<(String, String)> {
    let full = file::read_to_string(file)?;
    let schema = full.strip_prefix("#!/usr/bin/env usage\n").unwrap_or(&full);
    let (schema, body) = schema.split_once("\n#!").unwrap_or((&schema, ""));
    let schema = schema.trim().to_string();
    let body = format!("#!{}", body);
    Ok((schema, body))
}

fn get_string_prop(node: &KdlNode, name: &str) -> Option<String> {
    node.get(name)
        .and_then(|entry| entry.value().as_string())
        .map(|s| s.to_string())
}

impl FromStr for Spec {
    type Err = miette::Error;
    fn from_str(input: &str) -> Result<Spec, Self::Err> {
        let kdl: KdlDocument = input
            .parse()
            .map_err(|err: kdl::KdlError| UsageErr::KdlError(err))?;
        let mut schema = Self {
            ..Default::default()
        };
        for node in kdl.nodes() {
            match node.name().to_string().as_str() {
                "name" => schema.name = node.entries()[0].value().as_string().unwrap().to_string(),
                "bin" => schema.bin = node.entries()[0].value().as_string().unwrap().to_string(),
                "arg" => schema.cmd.args.push(node.try_into()?),
                "flag" => schema.cmd.flags.push(node.try_into()?),
                "cmd" => {
                    let node: SchemaCmd = node.try_into()?;
                    schema.cmd.subcommands.insert(node.name.to_string(), node);
                }
                "config" => schema.config = node.try_into()?,
                "include" => {
                    let file = get_string_prop(node, "file")
                        .map(context::prepend_load_root)
                        .ok_or_else(|| UsageErr::new(node.to_string(), node.span()))?;
                    ensure!(file.exists(), "File not found: {}", file.display());
                    info!("include: {}", file.display());
                    let (spec, _) = split_script(&file)?;
                    schema.merge(spec.parse()?);
                }
                _ => Err(UsageErr::new(node.to_string(), node.span()))?,
            }
        }
        Ok(schema)
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
        for flag in self.cmd.flags.iter() {
            nodes.push(flag.into());
        }
        for arg in self.cmd.args.iter() {
            nodes.push(arg.into());
        }
        for cmd in self.cmd.subcommands.values() {
            nodes.push(cmd.into())
        }
        write!(f, "{}", doc)
    }
}

#[cfg(feature = "clap")]
impl From<&clap::Command> for Spec {
    fn from(cmd: &clap::Command) -> Self {
        Spec {
            bin: cmd.get_bin_name().unwrap_or_default().to_string(),
            name: cmd.get_name().to_string(),
            cmd: cmd.into(),
            config: Default::default(),
        }
    }
}

#[cfg(feature = "clap")]
impl From<&Spec> for clap::Command {
    fn from(schema: &Spec) -> Self {
        let mut cmd = clap::Command::new(&schema.name);
        for flag in schema.cmd.flags.iter() {
            cmd = cmd.arg(flag);
        }
        for arg in schema.cmd.args.iter() {
            let a = clap::Arg::new(&arg.name).required(arg.required);
            cmd = cmd.arg(a);
        }
        for scmd in schema.cmd.subcommands.values() {
            cmd = cmd.subcommand(scmd);
        }
        cmd
    }
}

#[cfg(feature = "clap")]
impl From<clap::Command> for Spec {
    fn from(cmd: clap::Command) -> Self {
        (&cmd).into()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_display() {
        let spec: Spec = r#"
name "Usage CLI"
bin "usage"
arg "arg1"
flag "-f,--force" global=true
cmd "config" {
  cmd "set" {
    arg "key" help="Key to set"
    arg "value"
  }
}
        "#
        .parse()
        .unwrap();
        assert_display_snapshot!(spec, @r###"
        name "Usage CLI"
        bin "usage"
        flag "-f,--force" global=true
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
        assert_display_snapshot!(Spec::from(&cmd), @r###"
        name "test"
        "###);
    }
}
