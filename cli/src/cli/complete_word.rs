use std::collections::{BTreeMap, VecDeque};
use std::env;
use std::fmt::{Debug, Display, Formatter};
use std::path::{Path, PathBuf};
use std::process::Command;

use clap::Args;
use indexmap::IndexMap;
use itertools::Itertools;
use miette::IntoDiagnostic;
use once_cell::sync::Lazy;
use strum::EnumTryAs;
use xx::process::check_status;
use xx::{XXError, XXResult};

use usage::{Complete, Spec, SpecArg, SpecCommand, SpecFlag};

use crate::cli::generate;

#[derive(Debug, Args)]
#[clap(visible_alias = "cw")]
pub struct CompleteWord {
    // #[clap(value_parser = ["bash", "fish", "zsh"])]
    // shell: String,
    /// user's input from the command line
    words: Vec<String>,

    /// usage spec file or script with usage shebang
    #[clap(short, long)]
    file: Option<PathBuf>,

    /// raw string spec input
    #[clap(short, long, required_unless_present = "file", overrides_with = "file")]
    spec: Option<String>,

    /// current word index
    #[clap(long, allow_hyphen_values = true)]
    cword: Option<usize>,
}

impl CompleteWord {
    pub fn run(&self) -> miette::Result<()> {
        let spec = generate::file_or_spec(&self.file, &self.spec)?;
        let choices = self.complete_word(&spec)?;
        for c in &choices {
            println!("{}", c);
        }

        Ok(())
    }

    fn complete_word(&self, spec: &Spec) -> miette::Result<Vec<String>> {
        let cword = self.cword.unwrap_or(self.words.len().max(1) - 1);
        let ctoken = self.words.get(cword).cloned().unwrap_or_default();
        let words: VecDeque<_> = self.words.iter().take(cword).cloned().collect();

        trace!(
            "cword: {cword} ctoken: {ctoken} words: {}",
            words.iter().join(" ")
        );

        let mut ctx = tera::Context::new();
        ctx.insert("words", &self.words);
        ctx.insert("CURRENT", &cword);
        ctx.insert("PREV", &(cword - 1));

        let parsed = parse(spec, words)?;
        let choices = if !parsed.cmd.subcommands.is_empty() {
            complete_subcommands(parsed.cmd, &ctoken)
        } else if ctoken == "-" {
            let shorts = complete_short_flag_names(&parsed.available_flags, "");
            let longs = complete_long_flag_names(&parsed.available_flags, "");
            shorts.into_iter().chain(longs).collect()
        } else if ctoken.starts_with("--") {
            complete_long_flag_names(&parsed.available_flags, &ctoken)
        } else if ctoken.starts_with('-') {
            complete_short_flag_names(&parsed.available_flags, &ctoken)
        } else if let Some(flag) = parsed.flag_awaiting_value {
            complete_arg(&ctx, spec, flag.arg.as_ref().unwrap(), &ctoken)?
        } else if let Some(arg) = parsed.cmd.args.get(parsed.args.len()) {
            complete_arg(&ctx, spec, arg, &ctoken)?
        } else {
            vec![]
        };
        Ok(choices)
    }
}

struct ParseOutput<'a> {
    cmd: &'a SpecCommand,
    cmds: Vec<&'a SpecCommand>,
    args: IndexMap<&'a SpecArg, ParseValue>,
    _flags: IndexMap<SpecFlag, ParseValue>,
    available_flags: BTreeMap<String, SpecFlag>,
    flag_awaiting_value: Option<SpecFlag>,
}

#[derive(EnumTryAs)]
enum ParseValue {
    Bool(bool),
    String(String),
    MultiBool(Vec<bool>),
    MultiString(Vec<String>),
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

fn parse(spec: &Spec, mut words: VecDeque<String>) -> miette::Result<ParseOutput> {
    let mut cmd = &spec.cmd;
    let mut cmds = vec![];
    words.pop_front();
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

    while !words.is_empty() {
        if let Some(subcommand) = cmd.find_subcommand(&words[0]) {
            available_flags.retain(|_, f| f.global);
            available_flags.extend(gather_flags(subcommand));
            words.pop_front();
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

    while !words.is_empty() {
        let w = words.pop_front().unwrap();

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
                words.push_front(val.to_string());
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
                    words.push_front(next);
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
        _flags: flags,
        available_flags,
        flag_awaiting_value,
    })
}

fn complete_subcommands(cmd: &SpecCommand, ctoken: &str) -> Vec<String> {
    trace!("complete_subcommands: {ctoken}");
    let mut choices = vec![];
    for subcommand in cmd.subcommands.values() {
        if subcommand.hide {
            continue;
        }
        choices.push(subcommand.name.clone());
        for alias in &subcommand.aliases {
            choices.push(alias.clone());
        }
    }
    choices
        .into_iter()
        .filter(|c| c.starts_with(ctoken))
        .sorted()
        .collect()
}

fn complete_long_flag_names(flags: &BTreeMap<String, SpecFlag>, ctoken: &str) -> Vec<String> {
    trace!("complete_long_flag_names: {ctoken}");
    let ctoken = ctoken.strip_prefix("--").unwrap_or(ctoken);
    flags
        .values()
        .filter(|f| !f.hide)
        .flat_map(|f| &f.long)
        .unique()
        .filter(|c| c.starts_with(ctoken))
        .map(|c| format!("--{c}"))
        .sorted()
        .collect()
}

fn complete_short_flag_names(flags: &BTreeMap<String, SpecFlag>, ctoken: &str) -> Vec<String> {
    trace!("complete_short_flag_names: {ctoken}");
    let cur = ctoken.chars().nth(1);
    flags
        .values()
        .filter(|f| !f.hide)
        .flat_map(|f| &f.short)
        .unique()
        .filter(|c| cur.is_none() || cur == Some(**c))
        .map(|c| format!("-{c}"))
        .sorted()
        .collect()
}

fn complete_builtin(type_: &str, ctoken: &str) -> Vec<String> {
    match (type_, env::current_dir()) {
        ("path", Ok(cwd)) => complete_path(&cwd, ctoken, |_| true),
        ("dir", Ok(cwd)) => complete_path(&cwd, ctoken, |p| p.is_dir()),
        ("file", Ok(cwd)) => complete_path(&cwd, ctoken, |p| p.is_file()),
        _ => vec![],
    }
}

fn complete_arg(
    ctx: &tera::Context,
    spec: &Spec,
    arg: &SpecArg,
    ctoken: &str,
) -> miette::Result<Vec<String>> {
    static EMPTY_COMPL: Lazy<Complete> = Lazy::new(Complete::default);

    trace!("complete_arg: {arg} {ctoken}");
    let name = arg.name.to_lowercase();
    let complete = spec.complete.get(&name).unwrap_or(&EMPTY_COMPL);
    let type_ = complete.type_.as_ref().unwrap_or(&name);

    let builtin = complete_builtin(type_, ctoken);
    if !builtin.is_empty() {
        return Ok(builtin);
    }

    if let Some(run) = &complete.run {
        let run = tera::Tera::one_off(run, ctx, false).into_diagnostic()?;
        trace!("run: {run}");
        let stdout = sh(&run)?;
        // trace!("stdout: {stdout}");
        return Ok(stdout
            .lines()
            .filter(|l| l.starts_with(ctoken))
            .map(|l| l.to_string())
            .collect());
    }

    Ok(vec![])
}

fn complete_path(base: &Path, ctoken: &str, filter: impl Fn(&Path) -> bool) -> Vec<String> {
    trace!("complete_path: {ctoken}");
    let path = PathBuf::from(ctoken);
    let mut dir = path.parent().unwrap_or(&path).to_path_buf();
    if dir.is_relative() {
        dir = base.join(dir);
    }
    let mut prefix = path
        .file_name()
        .unwrap_or_default()
        .to_string_lossy()
        .to_string();
    if path.is_dir() && ctoken.ends_with('/') {
        dir = path.to_path_buf();
        prefix = "".to_string();
    };
    std::fs::read_dir(dir)
        .ok()
        .into_iter()
        .flatten()
        .filter_map(Result::ok)
        .filter(|de| de.file_name().to_string_lossy().starts_with(&prefix))
        .map(|de| de.path())
        .filter(|p| filter(p))
        .map(|p| {
            p.strip_prefix(base)
                .unwrap_or(&p)
                .to_string_lossy()
                .to_string()
        })
        .sorted()
        .collect()
}

fn sh(script: &str) -> XXResult<String> {
    let output = Command::new("sh")
        .arg("-c")
        .arg(script)
        .stdin(std::process::Stdio::null())
        .stderr(std::process::Stdio::inherit())
        .output()
        .map_err(|err| XXError::ProcessError(err, format!("sh -c {script}")))?;

    check_status(output.status)
        .map_err(|err| XXError::ProcessError(err, format!("sh -c {script}")))?;
    let stdout = String::from_utf8(output.stdout).expect("stdout is not utf-8");
    Ok(stdout)
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
                    .map(|(a, w)| format!("{a}: {w}"))
                    .collect_vec(),
            )
            .field(
                "flags",
                &self
                    .available_flags
                    .iter()
                    .map(|(f, w)| format!("{f}: {w}"))
                    .collect_vec(),
            )
            .finish()
    }
}
