use std::collections::BTreeMap;
use std::env;
use std::fmt::Debug;
use std::path::{Path, PathBuf};
use std::process::Command;

use clap::Args;
use itertools::Itertools;
use miette::IntoDiagnostic;
use once_cell::sync::Lazy;
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
        let words: Vec<_> = self.words.iter().take(cword).cloned().collect();

        trace!(
            "cword: {cword} ctoken: {ctoken} words: {}",
            words.iter().join(" ")
        );

        let mut ctx = tera::Context::new();
        ctx.insert("words", &self.words);
        ctx.insert("CURRENT", &cword);
        ctx.insert("PREV", &(cword - 1));

        let parsed = usage::cli::parse(spec, &words)?;
        debug!("parsed cmd: {}", parsed.cmd.full_cmd.join(" "));
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
    debug!("complete_long_flag_names: {ctoken}");
    trace!("flags: {}", flags.keys().join(", "));
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
    debug!("complete_short_flag_names: {ctoken}");
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
        .env("__USAGE", env!("CARGO_PKG_VERSION"))
        .output()
        .map_err(|err| XXError::ProcessError(err, format!("sh -c {script}")))?;

    check_status(output.status)
        .map_err(|err| XXError::ProcessError(err, format!("sh -c {script}")))?;
    let stdout = String::from_utf8(output.stdout).expect("stdout is not utf-8");
    Ok(stdout)
}
