use heck::ToSnakeCase;
use indexmap::IndexMap;
use itertools::Itertools;
use miette::bail;
use std::collections::{BTreeMap, VecDeque};
use std::fmt::{Debug, Display, Formatter};
use strum::EnumTryAs;

use crate::{Spec, SpecArg, SpecCommand, SpecFlag};

pub struct ParseOutput {
    pub cmd: SpecCommand,
    pub cmds: Vec<SpecCommand>,
    pub args: IndexMap<SpecArg, ParseValue>,
    pub flags: IndexMap<SpecFlag, ParseValue>,
    pub available_flags: BTreeMap<String, SpecFlag>,
    pub flag_awaiting_value: Option<SpecFlag>,
    pub errors: Vec<String>,
}

#[derive(Debug, EnumTryAs)]
pub enum ParseValue {
    Bool(bool),
    String(String),
    MultiBool(Vec<bool>),
    MultiString(Vec<String>),
}

pub fn parse(spec: &Spec, input: &[String]) -> Result<ParseOutput, miette::Error> {
    let out = parse_partial(spec, input)?;
    if !out.errors.is_empty() {
        bail!("{}", out.errors.join("\n"));
    }
    Ok(out)
}

pub fn parse_partial(spec: &Spec, input: &[String]) -> Result<ParseOutput, miette::Error> {
    let mut input = input.iter().cloned().collect::<VecDeque<_>>();
    input.pop_front();

    let gather_flags = |cmd: &SpecCommand| {
        cmd.flags
            .iter()
            .flat_map(|f| {
                let mut flags = f
                    .long
                    .iter()
                    .map(|l| (format!("--{}", l), f.clone()))
                    .chain(f.short.iter().map(|s| (format!("-{}", s), f.clone())))
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
        flag_awaiting_value: None,
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

    while !input.is_empty() {
        let w = input.pop_front().unwrap();

        if let Some(flag) = out.flag_awaiting_value {
            out.flag_awaiting_value = None;
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
                        bail!(
                            "invalid choice for option {}: {w}, expected one of {}",
                            flag.name,
                            choices.choices.join(", ")
                        );
                    }
                }
                out.flags.insert(flag, ParseValue::String(w));
            }
            continue;
        }

        if w == "--" {
            enable_flags = false;
            continue;
        }

        // long flags
        if enable_flags && w.starts_with("--") {
            let (word, val) = w.split_once('=').unwrap_or_else(|| (&w, ""));
            if !val.is_empty() {
                input.push_front(val.to_string());
            }
            if let Some(f) = out.available_flags.get(word) {
                if f.arg.is_some() {
                    out.flag_awaiting_value = Some(f.clone());
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
        }

        // short flags
        if enable_flags && w.starts_with('-') && w.len() > 1 {
            let short = w.chars().nth(1).unwrap();
            if let Some(f) = out.available_flags.get(&format!("-{}", short)) {
                let mut next = format!("-{}", &w[2..]);
                if f.arg.is_some() {
                    out.flag_awaiting_value = Some(f.clone());
                    next = w[2..].to_string();
                }
                if !next.is_empty() && next != "-" {
                    input.push_front(next);
                }
                if f.var {
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
                        bail!(
                            "invalid choice for arg {}: {w}, expected one of {}",
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
        bail!("unexpected word: {w}");
    }

    for arg in out.cmd.args.iter().skip(out.args.len()) {
        if arg.required {
            out.errors
                .push(format!("missing required arg <{}>", arg.name));
        }
    }

    for flag in out.available_flags.values() {
        if flag.required && !out.flags.contains_key(flag) {
            out.errors.push(format!(
                "missing required option --{} <{}>",
                flag.name, flag.name
            ));
        }
    }

    Ok(out)
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
                ParseValue::MultiString(s) => s.join(","),
            };
            env.insert(key, val);
        }
        for (arg, val) in &self.args {
            let key = format!("usage_{}", arg.name.to_snake_case());
            let val = match val {
                ParseValue::Bool(b) => if *b { "true" } else { "false" }.to_string(),
                ParseValue::String(s) => s.clone(),
                ParseValue::MultiBool(b) => b.iter().filter(|b| **b).count().to_string(),
                ParseValue::MultiString(s) => s.join(","),
            };
            env.insert(key, val);
        }
        env
    }
}

impl Display for ParseValue {
    fn fmt(&self, f: &mut Formatter<'_>) -> std::fmt::Result {
        match self {
            ParseValue::Bool(b) => write!(f, "{}", b),
            ParseValue::String(s) => write!(f, "{}", s),
            ParseValue::MultiBool(b) => write!(f, "{:?}", b),
            ParseValue::MultiString(s) => write!(f, "{:?}", s),
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
}
