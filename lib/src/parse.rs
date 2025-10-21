use heck::ToSnakeCase;
use indexmap::IndexMap;
use itertools::Itertools;
use log::trace;
use miette::bail;
use std::collections::{BTreeMap, VecDeque};
use std::fmt::{Debug, Display, Formatter};
use strum::EnumTryAs;

#[cfg(feature = "docs")]
use crate::docs;
use crate::error::UsageErr;
use crate::{Spec, SpecArg, SpecCommand, SpecFlag};

pub struct ParseOutput {
    pub cmd: SpecCommand,
    pub cmds: Vec<SpecCommand>,
    pub args: IndexMap<SpecArg, ParseValue>,
    pub flags: IndexMap<SpecFlag, ParseValue>,
    pub available_flags: BTreeMap<String, SpecFlag>,
    pub flag_awaiting_value: Vec<SpecFlag>,
    pub errors: Vec<UsageErr>,
}

#[derive(Debug, EnumTryAs, Clone)]
pub enum ParseValue {
    Bool(bool),
    String(String),
    MultiBool(Vec<bool>),
    MultiString(Vec<String>),
}

pub fn parse(spec: &Spec, input: &[String]) -> Result<ParseOutput, miette::Error> {
    let mut out = parse_partial(spec, input)?;
    trace!("{out:?}");

    // Apply env vars and defaults for args
    for arg in out.cmd.args.iter().skip(out.args.len()) {
        if let Some(env_var) = arg.env.as_ref() {
            if let Ok(env_value) = std::env::var(env_var) {
                out.args.insert(arg.clone(), ParseValue::String(env_value));
                continue;
            }
        }
        if let Some(default) = arg.default.as_ref() {
            out.args
                .insert(arg.clone(), ParseValue::String(default.clone()));
        }
    }

    // Apply env vars and defaults for flags
    for flag in out.available_flags.values() {
        if out.flags.contains_key(flag) {
            continue;
        }
        if let Some(env_var) = flag.env.as_ref() {
            if let Ok(env_value) = std::env::var(env_var) {
                if flag.arg.is_some() {
                    out.flags
                        .insert(flag.clone(), ParseValue::String(env_value));
                } else {
                    // For boolean flags, check if env value is truthy
                    let is_true = matches!(env_value.as_str(), "1" | "true" | "True" | "TRUE");
                    out.flags.insert(flag.clone(), ParseValue::Bool(is_true));
                }
                continue;
            }
        }
        if let Some(default) = flag.default.as_ref() {
            out.flags
                .insert(flag.clone(), ParseValue::String(default.clone()));
        }
        if let Some(Some(default)) = flag.arg.as_ref().map(|a| &a.default) {
            out.flags
                .insert(flag.clone(), ParseValue::String(default.clone()));
        }
    }
    if let Some(err) = out.errors.iter().find(|e| matches!(e, UsageErr::Help(_))) {
        bail!("{err}");
    }
    if !out.errors.is_empty() {
        bail!("{}", out.errors.iter().map(|e| e.to_string()).join("\n"));
    }
    Ok(out)
}

pub fn parse_partial(spec: &Spec, input: &[String]) -> Result<ParseOutput, miette::Error> {
    trace!("parse_partial: {input:?}");
    let mut input = input.iter().cloned().collect::<VecDeque<_>>();
    input.pop_front();

    let gather_flags = |cmd: &SpecCommand| {
        cmd.flags
            .iter()
            .flat_map(|f| {
                let mut flags = f
                    .long
                    .iter()
                    .map(|l| (format!("--{l}"), f.clone()))
                    .chain(f.short.iter().map(|s| (format!("-{s}"), f.clone())))
                    .collect::<Vec<_>>();
                if let Some(negate) = &f.negate {
                    flags.push((negate.clone(), f.clone()));
                }
                flags
            })
            .collect()
    };

    let mut out = ParseOutput {
        cmd: spec.cmd.clone(),
        cmds: vec![spec.cmd.clone()],
        args: IndexMap::new(),
        flags: IndexMap::new(),
        available_flags: gather_flags(&spec.cmd),
        flag_awaiting_value: vec![],
        errors: vec![],
    };

    while !input.is_empty() {
        if let Some(subcommand) = out.cmd.find_subcommand(&input[0]) {
            let mut subcommand = subcommand.clone();
            subcommand.mount()?;
            out.available_flags.retain(|_, f| f.global);
            out.available_flags.extend(gather_flags(&subcommand));
            input.pop_front();
            out.cmds.push(subcommand.clone());
            out.cmd = subcommand.clone();
        } else {
            break;
        }
    }

    let mut next_arg = out.cmd.args.first();
    let mut enable_flags = true;
    let mut grouped_flag = false;

    while !input.is_empty() {
        let mut w = input.pop_front().unwrap();

        if w == "--" {
            enable_flags = false;
            continue;
        }

        // long flags
        if enable_flags && w.starts_with("--") {
            grouped_flag = false;
            let (word, val) = w.split_once('=').unwrap_or_else(|| (&w, ""));
            if !val.is_empty() {
                input.push_front(val.to_string());
            }
            if let Some(f) = out.available_flags.get(word) {
                if f.arg.is_some() {
                    out.flag_awaiting_value.push(f.clone());
                } else if f.var {
                    let arr = out
                        .flags
                        .entry(f.clone())
                        .or_insert_with(|| ParseValue::MultiBool(vec![]))
                        .try_as_multi_bool_mut()
                        .unwrap();
                    arr.push(true);
                } else {
                    let negate = f.negate.clone().unwrap_or_default();
                    out.flags.insert(f.clone(), ParseValue::Bool(w != negate));
                }
                continue;
            }
            if is_help_arg(spec, &w) {
                out.errors
                    .push(render_help_err(spec, &out.cmd, w.len() > 2));
                return Ok(out);
            }
        }

        // short flags
        if enable_flags && w.starts_with('-') && w.len() > 1 {
            let short = w.chars().nth(1).unwrap();
            if let Some(f) = out.available_flags.get(&format!("-{short}")) {
                if w.len() > 2 {
                    input.push_front(format!("-{}", &w[2..]));
                    grouped_flag = true;
                }
                if f.arg.is_some() {
                    out.flag_awaiting_value.push(f.clone());
                } else if f.var {
                    let arr = out
                        .flags
                        .entry(f.clone())
                        .or_insert_with(|| ParseValue::MultiBool(vec![]))
                        .try_as_multi_bool_mut()
                        .unwrap();
                    arr.push(true);
                } else {
                    let negate = f.negate.clone().unwrap_or_default();
                    out.flags.insert(f.clone(), ParseValue::Bool(w != negate));
                }
                continue;
            }
            if is_help_arg(spec, &w) {
                out.errors
                    .push(render_help_err(spec, &out.cmd, w.len() > 2));
                return Ok(out);
            }
            if grouped_flag {
                grouped_flag = false;
                w.remove(0);
            }
        }

        if !out.flag_awaiting_value.is_empty() {
            while let Some(flag) = out.flag_awaiting_value.pop() {
                let arg = flag.arg.as_ref().unwrap();
                if flag.var {
                    let arr = out
                        .flags
                        .entry(flag)
                        .or_insert_with(|| ParseValue::MultiString(vec![]))
                        .try_as_multi_string_mut()
                        .unwrap();
                    arr.push(w);
                } else {
                    if let Some(choices) = &arg.choices {
                        if !choices.choices.contains(&w) {
                            if is_help_arg(spec, &w) {
                                out.errors
                                    .push(render_help_err(spec, &out.cmd, w.len() > 2));
                                return Ok(out);
                            }
                            bail!(
                                "Invalid choice for option {}: {w}, expected one of {}",
                                flag.name,
                                choices.choices.join(", ")
                            );
                        }
                    }
                    out.flags.insert(flag, ParseValue::String(w));
                }
                w = "".to_string();
            }
            continue;
        }

        if let Some(arg) = next_arg {
            if arg.var {
                let arr = out
                    .args
                    .entry(arg.clone())
                    .or_insert_with(|| ParseValue::MultiString(vec![]))
                    .try_as_multi_string_mut()
                    .unwrap();
                arr.push(w);
                if arr.len() >= arg.var_max.unwrap_or(usize::MAX) {
                    next_arg = out.cmd.args.get(out.args.len());
                }
            } else {
                if let Some(choices) = &arg.choices {
                    if !choices.choices.contains(&w) {
                        if is_help_arg(spec, &w) {
                            out.errors
                                .push(render_help_err(spec, &out.cmd, w.len() > 2));
                            return Ok(out);
                        }
                        bail!(
                            "Invalid choice for arg {}: {w}, expected one of {}",
                            arg.name,
                            choices.choices.join(", ")
                        );
                    }
                }
                out.args.insert(arg.clone(), ParseValue::String(w));
                next_arg = out.cmd.args.get(out.args.len());
            }
            continue;
        }
        if is_help_arg(spec, &w) {
            out.errors
                .push(render_help_err(spec, &out.cmd, w.len() > 2));
            return Ok(out);
        }
        bail!("unexpected word: {w}");
    }

    for arg in out.cmd.args.iter().skip(out.args.len()) {
        if arg.required && arg.default.is_none() {
            // Check if there's an env var available
            let has_env = arg
                .env
                .as_ref()
                .map(|e| std::env::var(e).is_ok())
                .unwrap_or(false);
            if !has_env {
                out.errors.push(UsageErr::MissingArg(arg.name.clone()));
            }
        }
    }

    for flag in out.available_flags.values() {
        if out.flags.contains_key(flag) {
            continue;
        }
        let has_default = flag.default.is_some() || flag.arg.iter().any(|a| a.default.is_some());
        let has_env = flag
            .env
            .as_ref()
            .map(|e| std::env::var(e).is_ok())
            .unwrap_or(false);
        if flag.required && !has_default && !has_env {
            out.errors.push(UsageErr::MissingFlag(flag.name.clone()));
        }
    }

    Ok(out)
}

#[cfg(feature = "docs")]
fn render_help_err(spec: &Spec, cmd: &SpecCommand, long: bool) -> UsageErr {
    UsageErr::Help(docs::cli::render_help(spec, cmd, long))
}

#[cfg(not(feature = "docs"))]
fn render_help_err(_spec: &Spec, _cmd: &SpecCommand, _long: bool) -> UsageErr {
    UsageErr::Help("help".to_string())
}

fn is_help_arg(spec: &Spec, w: &str) -> bool {
    spec.disable_help != Some(true)
        && (w == "--help"
            || w == "-h"
            || w == "-?"
            || (spec.cmd.subcommands.is_empty() && w == "help"))
}

impl ParseOutput {
    pub fn as_env(&self) -> BTreeMap<String, String> {
        let mut env = BTreeMap::new();
        for (flag, val) in &self.flags {
            let key = format!("usage_{}", flag.name.to_snake_case());
            let val = match val {
                ParseValue::Bool(b) => if *b { "true" } else { "false" }.to_string(),
                ParseValue::String(s) => s.clone(),
                ParseValue::MultiBool(b) => b.iter().filter(|b| **b).count().to_string(),
                ParseValue::MultiString(s) => shell_words::join(s),
            };
            env.insert(key, val);
        }
        for (arg, val) in &self.args {
            let key = format!("usage_{}", arg.name.to_snake_case());
            env.insert(key, val.to_string());
        }
        env
    }
}

impl Display for ParseValue {
    fn fmt(&self, f: &mut Formatter<'_>) -> std::fmt::Result {
        match self {
            ParseValue::Bool(b) => write!(f, "{b}"),
            ParseValue::String(s) => write!(f, "{s}"),
            ParseValue::MultiBool(b) => write!(f, "{}", b.iter().join(" ")),
            ParseValue::MultiString(s) => write!(f, "{}", shell_words::join(s)),
        }
    }
}

impl Debug for ParseOutput {
    fn fmt(&self, f: &mut Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("ParseOutput")
            .field("cmds", &self.cmds.iter().map(|c| &c.name).join(" ").trim())
            .field(
                "args",
                &self
                    .args
                    .iter()
                    .map(|(a, w)| format!("{}: {w}", &a.name))
                    .collect_vec(),
            )
            .field(
                "available_flags",
                &self
                    .available_flags
                    .iter()
                    .map(|(f, w)| format!("{f}: {w}"))
                    .collect_vec(),
            )
            .field(
                "flags",
                &self
                    .flags
                    .iter()
                    .map(|(f, w)| format!("{}: {w}", &f.name))
                    .collect_vec(),
            )
            .field("flag_awaiting_value", &self.flag_awaiting_value)
            .field("errors", &self.errors)
            .finish()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_parse() {
        let mut cmd = SpecCommand::default();
        cmd.name = "test".to_string();
        cmd.args = vec![SpecArg {
            name: "arg".to_string(),
            ..Default::default()
        }];
        cmd.flags = vec![SpecFlag {
            name: "flag".to_string(),
            long: vec!["flag".to_string()],
            ..Default::default()
        }];
        let spec = Spec {
            name: "test".to_string(),
            bin: "test".to_string(),
            cmd,
            ..Default::default()
        };
        let input = vec!["test".to_string(), "arg1".to_string(), "--flag".to_string()];
        let parsed = parse(&spec, &input).unwrap();
        assert_eq!(parsed.cmds.len(), 1);
        assert_eq!(parsed.cmds[0].name, "test");
        assert_eq!(parsed.args.len(), 1);
        assert_eq!(parsed.flags.len(), 1);
        assert_eq!(parsed.available_flags.len(), 1);
    }

    #[test]
    fn test_as_env() {
        let mut cmd = SpecCommand::default();
        cmd.name = "test".to_string();
        cmd.args = vec![SpecArg {
            name: "arg".to_string(),
            ..Default::default()
        }];
        cmd.flags = vec![
            SpecFlag {
                name: "flag".to_string(),
                long: vec!["flag".to_string()],
                ..Default::default()
            },
            SpecFlag {
                name: "force".to_string(),
                long: vec!["force".to_string()],
                negate: Some("--no-force".to_string()),
                ..Default::default()
            },
        ];
        let spec = Spec {
            name: "test".to_string(),
            bin: "test".to_string(),
            cmd,
            ..Default::default()
        };
        let input = vec![
            "test".to_string(),
            "--flag".to_string(),
            "--no-force".to_string(),
        ];
        let parsed = parse(&spec, &input).unwrap();
        let env = parsed.as_env();
        assert_eq!(env.len(), 2);
        assert_eq!(env.get("usage_flag"), Some(&"true".to_string()));
        assert_eq!(env.get("usage_force"), Some(&"false".to_string()));
    }

    #[test]
    fn test_arg_env_var() {
        let mut cmd = SpecCommand::default();
        cmd.name = "test".to_string();
        cmd.args = vec![SpecArg {
            name: "input".to_string(),
            env: Some("TEST_ARG_INPUT".to_string()),
            required: true,
            ..Default::default()
        }];
        let spec = Spec {
            name: "test".to_string(),
            bin: "test".to_string(),
            cmd,
            ..Default::default()
        };

        // Set env var
        std::env::set_var("TEST_ARG_INPUT", "test_file.txt");

        let input = vec!["test".to_string()];
        let parsed = parse(&spec, &input).unwrap();

        assert_eq!(parsed.args.len(), 1);
        let arg = parsed.args.keys().next().unwrap();
        assert_eq!(arg.name, "input");
        let value = parsed.args.values().next().unwrap();
        assert_eq!(value.to_string(), "test_file.txt");

        // Clean up
        std::env::remove_var("TEST_ARG_INPUT");
    }

    #[test]
    fn test_flag_env_var_with_arg() {
        let mut cmd = SpecCommand::default();
        cmd.name = "test".to_string();
        cmd.flags = vec![SpecFlag {
            name: "output".to_string(),
            long: vec!["output".to_string()],
            env: Some("TEST_FLAG_OUTPUT".to_string()),
            arg: Some(SpecArg {
                name: "file".to_string(),
                ..Default::default()
            }),
            ..Default::default()
        }];
        let spec = Spec {
            name: "test".to_string(),
            bin: "test".to_string(),
            cmd,
            ..Default::default()
        };

        // Set env var
        std::env::set_var("TEST_FLAG_OUTPUT", "output.txt");

        let input = vec!["test".to_string()];
        let parsed = parse(&spec, &input).unwrap();

        assert_eq!(parsed.flags.len(), 1);
        let flag = parsed.flags.keys().next().unwrap();
        assert_eq!(flag.name, "output");
        let value = parsed.flags.values().next().unwrap();
        assert_eq!(value.to_string(), "output.txt");

        // Clean up
        std::env::remove_var("TEST_FLAG_OUTPUT");
    }

    #[test]
    fn test_flag_env_var_boolean() {
        let mut cmd = SpecCommand::default();
        cmd.name = "test".to_string();
        cmd.flags = vec![SpecFlag {
            name: "verbose".to_string(),
            long: vec!["verbose".to_string()],
            env: Some("TEST_FLAG_VERBOSE".to_string()),
            ..Default::default()
        }];
        let spec = Spec {
            name: "test".to_string(),
            bin: "test".to_string(),
            cmd,
            ..Default::default()
        };

        // Set env var to true
        std::env::set_var("TEST_FLAG_VERBOSE", "true");

        let input = vec!["test".to_string()];
        let parsed = parse(&spec, &input).unwrap();

        assert_eq!(parsed.flags.len(), 1);
        let flag = parsed.flags.keys().next().unwrap();
        assert_eq!(flag.name, "verbose");
        let value = parsed.flags.values().next().unwrap();
        assert_eq!(value.to_string(), "true");

        // Clean up
        std::env::remove_var("TEST_FLAG_VERBOSE");
    }

    #[test]
    fn test_env_var_precedence() {
        // CLI args should take precedence over env vars
        let mut cmd = SpecCommand::default();
        cmd.name = "test".to_string();
        cmd.args = vec![SpecArg {
            name: "input".to_string(),
            env: Some("TEST_PRECEDENCE_INPUT".to_string()),
            required: true,
            ..Default::default()
        }];
        let spec = Spec {
            name: "test".to_string(),
            bin: "test".to_string(),
            cmd,
            ..Default::default()
        };

        // Set env var
        std::env::set_var("TEST_PRECEDENCE_INPUT", "env_file.txt");

        let input = vec!["test".to_string(), "cli_file.txt".to_string()];
        let parsed = parse(&spec, &input).unwrap();

        assert_eq!(parsed.args.len(), 1);
        let value = parsed.args.values().next().unwrap();
        // CLI arg should take precedence
        assert_eq!(value.to_string(), "cli_file.txt");

        // Clean up
        std::env::remove_var("TEST_PRECEDENCE_INPUT");
    }
}
