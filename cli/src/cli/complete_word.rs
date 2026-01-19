use std::collections::BTreeMap;
use std::env;
use std::fmt::Debug;
use std::path::{Path, PathBuf};
use std::process::Command;
use std::sync::Arc;

use clap::Args;
use itertools::Itertools;
use miette::IntoDiagnostic;
use once_cell::sync::Lazy;
use xx::process::check_status;
use xx::{regex, XXError, XXResult};

use usage::{Spec, SpecArg, SpecCommand, SpecComplete, SpecFlag};

use crate::cli::generate;

/// Generate shell completion candidates for a partial command line
///
/// This is used internally by shell completion scripts to provide
/// intelligent completions for commands, flags, and arguments.
#[derive(Debug, Args)]
#[clap(visible_alias = "cw")]
pub struct CompleteWord {
    /// User's input from the command line
    words: Vec<String>,

    /// Usage spec file or script with usage shebang
    #[clap(short, long)]
    file: Option<PathBuf>,

    /// Raw string spec input
    #[clap(short, long, required_unless_present = "file", overrides_with = "file")]
    spec: Option<String>,

    /// Current word index
    #[clap(long, allow_hyphen_values = true)]
    cword: Option<usize>,

    #[clap(long, default_value = "bash", value_parser = ["bash", "fish", "zsh"])]
    shell: String,
}

impl CompleteWord {
    pub fn run(&self) -> miette::Result<()> {
        let spec = generate::file_or_spec(&self.file, &self.spec)?;
        let choices = self.complete_word(&spec)?;
        let shell = self.shell.as_ref();
        let any_descriptions = choices.iter().any(|(_, d)| !d.is_empty());
        for (c, description) in choices {
            match shell {
                "bash" => println!("{c}"),
                "fish" => {
                    if any_descriptions {
                        println!("{c}\t{description}")
                    } else {
                        println!("{c}")
                    }
                }
                "zsh" => {
                    let c = c.replace(":", "\\\\:");
                    if any_descriptions {
                        let description = description.replace("'", "'\\''");
                        println!("'{c}'\\:'{description}'")
                    } else {
                        println!("'{c}'")
                    }
                }
                _ => unimplemented!("unsupported shell: {}", shell),
            }
        }

        Ok(())
    }

    fn complete_word(&self, spec: &Spec) -> miette::Result<Vec<(String, String)>> {
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
        if cword > 0 {
            ctx.insert("PREV", &(cword - 1));
        }

        let parsed = usage::parse::parse_partial(spec, &words)?;
        debug!("parsed cmd: {}", parsed.cmd.full_cmd.join(" "));

        // Check if previous token was a restart_token - if so, complete from first arg
        let prev_token = if cword > 0 {
            self.words.get(cword - 1).map(|s| s.as_str())
        } else {
            None
        };
        let after_restart_token = parsed
            .cmd
            .restart_token
            .as_ref()
            .is_some_and(|rt| prev_token == Some(rt.as_str()));

        let mut choices = if ctoken == "-" {
            let shorts = self.complete_short_flag_names(&parsed.available_flags, "");
            let longs = self.complete_long_flag_names(&parsed.available_flags, "");
            shorts.into_iter().chain(longs).collect::<Vec<_>>()
        } else if ctoken.starts_with("--") {
            self.complete_long_flag_names(&parsed.available_flags, &ctoken)
        } else if ctoken.starts_with('-') {
            self.complete_short_flag_names(&parsed.available_flags, &ctoken)
        } else if after_restart_token {
            // After a restart_token, complete from the first arg of the current command
            // This must be checked after flag checks (to allow --flag after :::)
            // but before flag_awaiting_value (since restart clears pending flag values)
            let mut choices = vec![];
            if let Some(arg) = parsed.cmd.args.first() {
                choices.extend(self.complete_arg(&ctx, spec, &parsed.cmd, arg, &ctoken)?);
            }
            choices
        } else if let Some(flag) = parsed.flag_awaiting_value.first() {
            self.complete_arg(&ctx, spec, &parsed.cmd, flag.arg.as_ref().unwrap(), &ctoken)?
        } else {
            let mut choices = vec![];
            if let Some(arg) = parsed.cmd.args.get(parsed.args.len()) {
                choices.extend(self.complete_arg(&ctx, spec, &parsed.cmd, arg, &ctoken)?);
            }
            if !parsed.cmd.subcommands.is_empty() {
                choices.extend(self.complete_subcommands(&parsed.cmd, &ctoken));
            }
            // If at root command with default_subcommand, also include completions from it
            if parsed.cmd.name == spec.cmd.name {
                if let Some(default_name) = &spec.default_subcommand {
                    if let Some(default_cmd) = spec.cmd.find_subcommand(default_name) {
                        // Include completions from default subcommand's first arg
                        if let Some(arg) = default_cmd.args.first() {
                            choices.extend(self.complete_arg(
                                &ctx,
                                spec,
                                default_cmd,
                                arg,
                                &ctoken,
                            )?);
                        }
                    }
                }
            }
            choices
        };
        // Fallback to file completions if nothing is known about this argument and it's not a flag
        if choices.is_empty() && !ctoken.starts_with('-') {
            let cwd = std::env::current_dir().unwrap_or_else(|_| PathBuf::from("."));
            let files = self.complete_path(&cwd, &ctoken, |_| true);
            choices = files.into_iter().map(|n| (n, String::new())).collect();
        }
        trace!("choices: {}", choices.iter().map(|(c, _)| c).join(", "));
        Ok(choices)
    }

    fn complete_subcommands(&self, cmd: &SpecCommand, ctoken: &str) -> Vec<(String, String)> {
        trace!("complete_subcommands: {ctoken}");
        let mut choices = vec![];
        for subcommand in cmd.subcommands.values() {
            if subcommand.hide {
                continue;
            }
            choices.push((
                subcommand.name.clone(),
                subcommand.help.clone().unwrap_or_default(),
            ));
            for alias in &subcommand.aliases {
                choices.push((alias.clone(), subcommand.help.clone().unwrap_or_default()));
            }
        }
        choices
            .into_iter()
            .filter(|(c, _)| c.starts_with(ctoken))
            .sorted()
            .collect()
    }

    fn complete_long_flag_names(
        &self,
        flags: &BTreeMap<String, Arc<SpecFlag>>,
        ctoken: &str,
    ) -> Vec<(String, String)> {
        debug!("complete_long_flag_names: {ctoken}");
        trace!("flags: {}", flags.keys().join(", "));
        flags
            .values()
            .filter(|f| !f.hide)
            .flat_map(|f| {
                let mut flags = f
                    .long
                    .iter()
                    .map(|l| (format!("--{l}"), f.help.clone().unwrap_or_default()))
                    .collect::<Vec<_>>();
                if let Some(negate) = &f.negate {
                    flags.push((negate.clone(), String::new()))
                }
                flags
            })
            .unique_by(|(f, _)| f.to_string())
            .filter(|(f, _)| f.starts_with(ctoken))
            // TODO: get flag description
            .sorted()
            .collect()
    }

    fn complete_short_flag_names(
        &self,
        flags: &BTreeMap<String, Arc<SpecFlag>>,
        ctoken: &str,
    ) -> Vec<(String, String)> {
        debug!("complete_short_flag_names: {ctoken}");
        let cur = ctoken.chars().nth(1);
        flags
            .values()
            .filter(|f| !f.hide)
            .flat_map(|f| &f.short)
            .unique()
            .filter(|c| cur.is_none() || cur == Some(**c))
            // TODO: get flag description
            .map(|c| (format!("-{c}"), String::new()))
            .sorted()
            .collect()
    }

    fn complete_builtin(&self, type_: &str, ctoken: &str) -> Vec<(String, String)> {
        let names = match (type_, env::current_dir()) {
            ("path" | "file", Ok(cwd)) => self.complete_path(&cwd, ctoken, |_| true),
            ("dir", Ok(cwd)) => self.complete_path(&cwd, ctoken, |p| p.is_dir()),
            // ("file", Ok(cwd)) => self.complete_path(&cwd, ctoken, |p| p.is_file()),
            _ => vec![],
        };
        names.into_iter().map(|n| (n, String::new())).collect()
    }

    fn complete_arg(
        &self,
        ctx: &tera::Context,
        spec: &Spec,
        cmd: &SpecCommand,
        arg: &SpecArg,
        ctoken: &str,
    ) -> miette::Result<Vec<(String, String)>> {
        static EMPTY_COMPL: Lazy<SpecComplete> = Lazy::new(SpecComplete::default);

        trace!("complete_arg: {arg} {ctoken}");
        let name = arg.name.to_lowercase();
        let complete = spec
            .complete
            .get(&name)
            .or(cmd.complete.get(&name))
            .unwrap_or(&EMPTY_COMPL);
        let type_ = complete.type_.as_ref().unwrap_or(&name);

        let builtin = self.complete_builtin(type_, ctoken);
        if !builtin.is_empty() {
            return Ok(builtin);
        }

        if let Some(choices) = &arg.choices {
            return Ok(choices
                .choices
                .iter()
                .map(|c| (c.clone(), String::new()))
                .filter(|(c, _)| c.starts_with(ctoken))
                .collect());
        }
        if let Some(run) = &complete.run {
            let run = tera::Tera::one_off(run, ctx, false).into_diagnostic()?;
            trace!("run: {run}");
            let stdout = sh(&run)?;
            // trace!("stdout: {stdout}");
            let re = regex!(r"[^\\]:");
            return Ok(stdout
                .lines()
                .filter(|l| l.starts_with(ctoken))
                .map(|l| {
                    if complete.descriptions {
                        match re.find(l).map(|m| l.split_at(m.end() - 1)) {
                            Some((l, d)) if d.len() <= 1 => {
                                (l.trim().replace("\\:", ":"), String::new())
                            }
                            Some((l, d)) => (
                                l.trim().replace("\\:", ":"),
                                d[1..].trim().replace("\\:", ":"),
                            ),
                            None => (l.trim().replace("\\:", ":"), String::new()),
                        }
                    } else {
                        (l.trim().to_string(), String::new())
                    }
                })
                .collect());
        }

        Ok(vec![])
    }

    fn complete_path(
        &self,
        base: &Path,
        ctoken: &str,
        filter: impl Fn(&Path) -> bool,
    ) -> Vec<String> {
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
            .filter(|de| {
                let name = de.file_name();
                let name = name.to_string_lossy();
                !name.starts_with('.') && name.starts_with(&prefix)
            })
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
