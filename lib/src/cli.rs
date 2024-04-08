use std::collections::{BTreeMap, VecDeque};
use std::fmt::{Debug, Display, Formatter};

use indexmap::IndexMap;
use itertools::Itertools;
use strum::EnumTryAs;

use crate::{Spec, SpecArg, SpecCommand, SpecFlag};

pub struct ParseOutput<'a> {
    pub cmd: &'a SpecCommand,
    pub cmds: Vec<&'a SpecCommand>,
    pub args: IndexMap<&'a SpecArg, ParseValue>,
    pub flags: IndexMap<SpecFlag, ParseValue>,
    pub available_flags: BTreeMap<String, SpecFlag>,
    pub flag_awaiting_value: Option<SpecFlag>,
}

#[derive(Debug, EnumTryAs)]
pub enum ParseValue {
    Bool(bool),
    String(String),
    MultiBool(Vec<bool>),
    MultiString(Vec<String>),
}

pub fn parse<'a>(spec: &'a Spec, input: &[String]) -> Result<ParseOutput<'a>, miette::Error> {
    let mut input = input.iter().cloned().collect::<VecDeque<_>>();
    let mut cmd = &spec.cmd;
    let mut cmds = vec![];
    input.pop_front();
    cmds.push(cmd);

    let gather_flags = |cmd: &SpecCommand| {
        cmd.flags
            .iter()
            .flat_map(|f| {
                f.long
                    .iter()
                    .map(|l| (format!("--{}", l), f.clone()))
                    .chain(f.short.iter().map(|s| (format!("-{}", s), f.clone())))
            })
            .collect()
    };

    let mut available_flags: BTreeMap<String, SpecFlag> = gather_flags(cmd);

    while !input.is_empty() {
        if let Some(subcommand) = cmd.find_subcommand(&input[0]) {
            available_flags.retain(|_, f| f.global);
            available_flags.extend(gather_flags(subcommand));
            input.pop_front();
            cmds.push(subcommand);
            cmd = subcommand;
        } else {
            break;
        }
    }

    let mut args: IndexMap<&SpecArg, ParseValue> = IndexMap::new();
    let mut flags: IndexMap<SpecFlag, ParseValue> = IndexMap::new();
    let mut next_arg = cmd.args.first();
    let mut flag_awaiting_value: Option<SpecFlag> = None;
    let mut enable_flags = true;

    while !input.is_empty() {
        let w = input.pop_front().unwrap();

        if let Some(flag) = flag_awaiting_value {
            flag_awaiting_value = None;
            if flag.var {
                let arr = flags
                    .entry(flag)
                    .or_insert_with(|| ParseValue::MultiString(vec![]))
                    .try_as_multi_string_mut()
                    .unwrap();
                arr.push(w);
            } else {
                flags.insert(flag, ParseValue::String(w));
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
            if let Some(f) = available_flags.get(word) {
                if f.arg.is_some() {
                    flag_awaiting_value = Some(f.clone());
                } else if f.var {
                    let arr = flags
                        .entry(f.clone())
                        .or_insert_with(|| ParseValue::MultiBool(vec![]))
                        .try_as_multi_bool_mut()
                        .unwrap();
                    arr.push(true);
                } else {
                    flags.insert(f.clone(), ParseValue::Bool(true));
                }
                continue;
            }
        }

        // short flags
        if enable_flags && w.starts_with('-') && w.len() > 1 {
            let short = w.chars().nth(1).unwrap();
            if let Some(f) = available_flags.get(&format!("-{}", short)) {
                let mut next = format!("-{}", &w[2..]);
                if f.arg.is_some() {
                    flag_awaiting_value = Some(f.clone());
                    next = w[2..].to_string();
                }
                if next != "-" {
                    input.push_front(next);
                }
                if f.var {
                    let arr = flags
                        .entry(f.clone())
                        .or_insert_with(|| ParseValue::MultiBool(vec![]))
                        .try_as_multi_bool_mut()
                        .unwrap();
                    arr.push(true);
                } else {
                    flags.insert(f.clone(), ParseValue::Bool(true));
                }
                continue;
            }
        }

        if let Some(arg) = next_arg {
            if arg.var {
                let arr = args
                    .entry(arg)
                    .or_insert_with(|| ParseValue::MultiString(vec![]))
                    .try_as_multi_string_mut()
                    .unwrap();
                arr.push(w);
                if arr.len() >= arg.var_max.unwrap_or(usize::MAX) {
                    next_arg = cmd.args.get(args.len());
                }
            } else {
                args.insert(arg, ParseValue::String(w));
                next_arg = cmd.args.get(args.len());
            }
            continue;
        }
        panic!("unexpected word: {w}");
    }

    Ok(ParseOutput {
        cmd,
        cmds,
        args,
        flags,
        available_flags,
        flag_awaiting_value,
    })
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

impl Debug for ParseOutput<'_> {
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
