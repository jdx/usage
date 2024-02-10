use std::collections::VecDeque;
use std::env;
use std::fmt::{Debug, Formatter};
use std::path::{Path, PathBuf};

use clap::Args;
use itertools::Itertools;

use usage::{Spec, SpecArg, SpecCommand, SpecFlag};

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

    #[clap(long)]
    ctoken: Option<String>,
}

impl CompleteWord {
    pub fn run(&self) -> miette::Result<()> {
        let spec = generate::file_or_spec(&self.file, &self.spec)?;
        let cword = self.cword.unwrap_or(self.words.len().max(1) - 1);
        let ctoken = self
            .ctoken
            .as_ref()
            .or(self.words.get(cword))
            .cloned()
            .unwrap_or_default();

        let words: VecDeque<_> = self.words.iter().take(cword).collect();

        trace!(
            "cword: {cword} ctoken: {ctoken} words: {}",
            words.iter().join(" ")
        );

        let parsed = parse(&spec, words)?;
        dbg!(&parsed);

        let mut choices = vec![];
        choices.extend(complete_subcommands(parsed.cmd, &ctoken));
        if let Some(arg) = parsed.cmd.args.get(parsed.args.len()) {
            choices.extend(complete_arg(&spec, arg, &ctoken)?);
        }

        for c in &choices {
            println!("{}", c);
        }

        Ok(())
    }
}

struct ParseOutput<'a> {
    cmd: &'a SpecCommand,
    cmds: Vec<&'a SpecCommand>,
    args: Vec<(&'a SpecArg, Vec<String>)>,
    flags: Vec<(&'a SpecFlag, Vec<String>)>,
}

fn parse<'a>(spec: &'a Spec, mut words: VecDeque<&String>) -> miette::Result<ParseOutput<'a>> {
    let mut cmds = vec![];
    let mut cmd = &spec.cmd;
    cmds.push(cmd);

    while !words.is_empty() {
        if let Some(subcommand) = cmd.subcommands.get(words[0]) {
            words.pop_front();
            cmds.push(subcommand);
            cmd = subcommand;
        } else {
            break;
        }
    }

    let mut flag = None;
    let mut args = vec![];
    let mut arg_specs = cmd.args.iter().collect_vec();

    while !words.is_empty() {
        if words[0].starts_with("--") {
            let long = words[0].strip_prefix("--").unwrap().to_string();
            if let Some(f) = cmd.flags.iter().find(|f| f.long.contains(&long)) {
                dbg!(&f, &words[0]);
                flag = Some(f);
                continue;
            }
        }

        if !arg_specs.is_empty() {
            let arg_spec = arg_specs[0];
            let word = words.pop_front().unwrap();
            args.push((arg_spec, vec![word.clone()]));
            if arg_spec.var {
                // TODO: handle var_min/var_max
                continue;
            } else {
                arg_specs.pop();
                continue;
            }
        }
        panic!("unexpected word: {:?}", words[0]);
    }

    dbg!(&flag);

    Ok(ParseOutput {
        cmd,
        cmds,
        args,
        flags: vec![],
    })
}

fn complete_subcommands(cmd: &SpecCommand, ctoken: &str) -> Vec<String> {
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

fn complete_arg(spec: &Spec, arg: &SpecArg, ctoken: &str) -> miette::Result<Vec<String>> {
    let name = arg.name.to_lowercase();
    dbg!(&name);

    if let Ok(cwd) = env::current_dir() {
        match name.as_str() {
            "path" => return complete_path(&cwd, ctoken, |_| true),
            "dir" => return complete_path(&cwd, ctoken, |p| p.is_dir()),
            "file" => return complete_path(&cwd, ctoken, |p| p.is_file()),
            _ => {}
        }
    }

    if let Some(complete) = spec.complete.get(&name) {
        let stdout = xx::process::sh(&complete.run)?;
        return Ok(stdout
            .lines()
            .filter(|l| l.starts_with(ctoken))
            .map(|l| l.to_string())
            .collect());
    }

    Ok(vec![])
}

fn complete_path(
    base: &Path,
    ctoken: &str,
    filter: impl Fn(&Path) -> bool,
) -> miette::Result<Vec<String>> {
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
    let mut files: Vec<String> = std::fs::read_dir(dir)
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
        .collect();
    files.sort();
    Ok(files)
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
                    .map(|(a, w)| format!("{}: {}", a.name, w.join(",")))
                    .collect_vec(),
            )
            .field(
                "flags",
                &self
                    .flags
                    .iter()
                    .map(|(f, w)| format!("{}: {}", &f.name, w.join(",")))
                    .collect_vec(),
            )
            .finish()
    }
}
