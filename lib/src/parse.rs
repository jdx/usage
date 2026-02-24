use heck::ToSnakeCase;
use indexmap::IndexMap;
use itertools::Itertools;
use log::trace;
use miette::bail;
use std::collections::{BTreeMap, HashMap, VecDeque};
use std::fmt::{Debug, Display, Formatter};
use std::sync::Arc;
use strum::EnumTryAs;

#[cfg(feature = "docs")]
use crate::docs;
use crate::error::UsageErr;
use crate::spec::arg::SpecDoubleDashChoices;
use crate::{Spec, SpecArg, SpecCommand, SpecFlag};

/// Extract the flag key from a flag word for lookup in available_flags map
/// Handles both long flags (--flag, --flag=value) and short flags (-f)
fn get_flag_key(word: &str) -> &str {
    if word.starts_with("--") {
        // Long flag: strip =value if present
        word.split_once('=').map(|(k, _)| k).unwrap_or(word)
    } else if word.len() >= 2 {
        // Short flag: first two chars (-X)
        &word[0..2]
    } else {
        word
    }
}

pub struct ParseOutput {
    pub cmd: SpecCommand,
    pub cmds: Vec<SpecCommand>,
    pub args: IndexMap<Arc<SpecArg>, ParseValue>,
    pub flags: IndexMap<Arc<SpecFlag>, ParseValue>,
    pub available_flags: BTreeMap<String, Arc<SpecFlag>>,
    pub flag_awaiting_value: Vec<Arc<SpecFlag>>,
    pub errors: Vec<UsageErr>,
}

#[derive(Debug, EnumTryAs, Clone)]
pub enum ParseValue {
    Bool(bool),
    String(String),
    MultiBool(Vec<bool>),
    MultiString(Vec<String>),
}

/// Builder for parsing command-line arguments with custom options.
///
/// Use this when you need to customize parsing behavior, such as providing
/// a custom environment variable map instead of using the process environment.
///
/// # Example
/// ```
/// use std::collections::HashMap;
/// use usage::Spec;
/// use usage::parse::Parser;
///
/// let spec: Spec = r#"flag "--name <name>" env="NAME""#.parse().unwrap();
/// let env: HashMap<String, String> = [("NAME".into(), "john".into())].into();
///
/// let result = Parser::new(&spec)
///     .with_env(env)
///     .parse(&["cmd".into()])
///     .unwrap();
/// ```
#[non_exhaustive]
pub struct Parser<'a> {
    spec: &'a Spec,
    env: Option<HashMap<String, String>>,
}

impl<'a> Parser<'a> {
    /// Create a new parser for the given spec.
    pub fn new(spec: &'a Spec) -> Self {
        Self { spec, env: None }
    }

    /// Use a custom environment variable map instead of the process environment.
    ///
    /// This is useful when parsing for tasks in a monorepo where the env vars
    /// come from a child config file rather than the current process environment.
    pub fn with_env(mut self, env: HashMap<String, String>) -> Self {
        self.env = Some(env);
        self
    }

    /// Parse the input arguments.
    ///
    /// Returns the parsed arguments and flags, with defaults and env vars applied.
    pub fn parse(self, input: &[String]) -> Result<ParseOutput, miette::Error> {
        let mut out = parse_partial_with_env(self.spec, input, self.env.as_ref())?;
        trace!("{out:?}");

        let get_env = |key: &str| -> Option<String> {
            if let Some(ref env_map) = self.env {
                env_map.get(key).cloned()
            } else {
                std::env::var(key).ok()
            }
        };

        // Apply env vars and defaults for args
        for arg in out.cmd.args.iter().skip(out.args.len()) {
            if let Some(env_var) = arg.env.as_ref() {
                if let Some(env_value) = get_env(env_var) {
                    out.args
                        .insert(Arc::new(arg.clone()), ParseValue::String(env_value));
                    continue;
                }
            }
            if !arg.default.is_empty() {
                // Consider var when deciding the type of default return value
                if arg.var {
                    // For var=true, always return a vec (MultiString)
                    out.args.insert(
                        Arc::new(arg.clone()),
                        ParseValue::MultiString(arg.default.clone()),
                    );
                } else {
                    // For var=false, return the first default value as String
                    out.args.insert(
                        Arc::new(arg.clone()),
                        ParseValue::String(arg.default[0].clone()),
                    );
                }
            }
        }

        // Apply env vars and defaults for flags
        for flag in out.available_flags.values() {
            if out.flags.contains_key(flag) {
                continue;
            }
            if let Some(env_var) = flag.env.as_ref() {
                if let Some(env_value) = get_env(env_var) {
                    if flag.arg.is_some() {
                        out.flags
                            .insert(Arc::clone(flag), ParseValue::String(env_value));
                    } else {
                        // For boolean flags, check if env value is truthy
                        let is_true = matches!(env_value.as_str(), "1" | "true" | "True" | "TRUE");
                        out.flags
                            .insert(Arc::clone(flag), ParseValue::Bool(is_true));
                    }
                    continue;
                }
            }
            // Apply flag default
            if !flag.default.is_empty() {
                // Consider var when deciding the type of default return value
                if flag.var {
                    // For var=true, always return a vec (MultiString for flags with args, MultiBool for boolean flags)
                    if flag.arg.is_some() {
                        out.flags.insert(
                            Arc::clone(flag),
                            ParseValue::MultiString(flag.default.clone()),
                        );
                    } else {
                        // For boolean flags with var=true, convert default strings to bools
                        let bools: Vec<bool> = flag
                            .default
                            .iter()
                            .map(|s| matches!(s.as_str(), "1" | "true" | "True" | "TRUE"))
                            .collect();
                        out.flags
                            .insert(Arc::clone(flag), ParseValue::MultiBool(bools));
                    }
                } else {
                    // For var=false, return the first default value
                    if flag.arg.is_some() {
                        out.flags.insert(
                            Arc::clone(flag),
                            ParseValue::String(flag.default[0].clone()),
                        );
                    } else {
                        // For boolean flags, convert default string to bool
                        let is_true =
                            matches!(flag.default[0].as_str(), "1" | "true" | "True" | "TRUE");
                        out.flags
                            .insert(Arc::clone(flag), ParseValue::Bool(is_true));
                    }
                }
            }
            // Also check nested arg defaults (for flags like --foo <arg> where the arg has a default)
            if let Some(arg) = flag.arg.as_ref() {
                if !out.flags.contains_key(flag) && !arg.default.is_empty() {
                    if flag.var {
                        out.flags.insert(
                            Arc::clone(flag),
                            ParseValue::MultiString(arg.default.clone()),
                        );
                    } else {
                        out.flags
                            .insert(Arc::clone(flag), ParseValue::String(arg.default[0].clone()));
                    }
                }
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
}

/// Parse command-line arguments according to a spec.
///
/// Returns the parsed arguments and flags, with defaults and env vars applied.
/// Uses `std::env::var` for environment variable lookups.
///
/// For custom environment variable handling, use [`Parser`] instead.
#[must_use = "parsing result should be used"]
pub fn parse(spec: &Spec, input: &[String]) -> Result<ParseOutput, miette::Error> {
    Parser::new(spec).parse(input)
}

/// Parse command-line arguments without applying defaults.
///
/// Use this for help text generation or when you need the raw parsed values.
#[must_use = "parsing result should be used"]
pub fn parse_partial(spec: &Spec, input: &[String]) -> Result<ParseOutput, miette::Error> {
    parse_partial_with_env(spec, input, None)
}

/// Internal version of parse_partial that accepts an optional custom env map.
fn parse_partial_with_env(
    spec: &Spec,
    input: &[String],
    custom_env: Option<&HashMap<String, String>>,
) -> Result<ParseOutput, miette::Error> {
    trace!("parse_partial: {input:?}");
    let mut input = input.iter().cloned().collect::<VecDeque<_>>();
    input.pop_front();

    let gather_flags = |cmd: &SpecCommand| {
        cmd.flags
            .iter()
            .flat_map(|f| {
                let f = Arc::new(f.clone()); // One clone per flag, then cheap Arc refs
                let mut flags = f
                    .long
                    .iter()
                    .map(|l| (format!("--{l}"), Arc::clone(&f)))
                    .chain(f.short.iter().map(|s| (format!("-{s}"), Arc::clone(&f))))
                    .collect::<Vec<_>>();
                if let Some(negate) = &f.negate {
                    flags.push((negate.clone(), Arc::clone(&f)));
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

    // Phase 1: Scan for subcommands and collect global flags
    //
    // This phase identifies subcommands early because they may have mount points
    // that need to be executed with the global flags that appeared before them.
    //
    // Example: "usage --verbose run task"
    //   -> finds "run" subcommand, passes ["--verbose"] to its mount command
    //   -> then finds "task" as a subcommand of "run" (if it exists)
    //
    // We only collect global flags because:
    // - Non-global flags are specific to the current command, not subcommands
    // - Global flags affect all commands and should be passed to mount points
    let mut prefix_words: Vec<String> = vec![];
    let mut idx = 0;
    // Track whether we've already applied the default_subcommand to prevent
    // multiple switches (e.g., if default is "run" and there's a task named "run")
    let mut used_default_subcommand = false;

    while idx < input.len() {
        if let Some(subcommand) = out.cmd.find_subcommand(&input[idx]) {
            let mut subcommand = subcommand.clone();
            // Pass prefix words (global flags before this subcommand) to mount
            subcommand.mount(&prefix_words)?;
            out.available_flags.retain(|_, f| f.global);
            out.available_flags.extend(gather_flags(&subcommand));
            // Remove subcommand from input
            input.remove(idx);
            out.cmds.push(subcommand.clone());
            out.cmd = subcommand.clone();
            prefix_words.clear();
            // Continue from current position (don't reset to 0)
            // After remove(), idx now points to the next element
        } else if input[idx].starts_with('-') {
            // Check if this is a known flag and if it's global
            let word = &input[idx];
            let flag_key = get_flag_key(word);

            if let Some(f) = out.available_flags.get(flag_key) {
                // Only collect global flags for mount execution
                if f.global {
                    prefix_words.push(input[idx].clone());
                    idx += 1;

                    // Only consume next word if flag takes an argument AND value isn't embedded
                    // Example: "--dir foo" consumes "foo", but "--dir=foo" or "--verbose" do not
                    if f.arg.is_some()
                        && !word.contains('=')
                        && idx < input.len()
                        && !input[idx].starts_with('-')
                    {
                        prefix_words.push(input[idx].clone());
                        idx += 1;
                    }
                } else {
                    // Non-global flag encountered - stop subcommand search
                    // This prevents incorrect parsing like: "cmd --local-flag run"
                    // where "run" might be mistaken for a subcommand
                    break;
                }
            } else {
                // Unknown flag - stop looking for subcommands
                // Let the main parsing phase handle the error
                break;
            }
        } else {
            // Found a word that's not a flag or subcommand
            // Check if we should use the default_subcommand (only once)
            if !used_default_subcommand {
                if let Some(default_name) = &spec.default_subcommand {
                    if let Some(subcommand) = out.cmd.find_subcommand(default_name) {
                        let mut subcommand = subcommand.clone();
                        // Pass prefix words (global flags before this) to mount
                        subcommand.mount(&prefix_words)?;
                        out.available_flags.retain(|_, f| f.global);
                        out.available_flags.extend(gather_flags(&subcommand));
                        out.cmds.push(subcommand.clone());
                        out.cmd = subcommand.clone();
                        prefix_words.clear();
                        used_default_subcommand = true;
                        // Continue the loop to check if this word is a subcommand of the
                        // default subcommand (e.g., a task name added via mount).
                        // If it's not a subcommand, the next iteration will break and
                        // Phase 2 will handle it as a positional arg.
                        continue;
                    }
                }
            }
            // This could be a positional argument, so stop subcommand search
            break;
        }
    }

    // Phase 2: Main argument and flag parsing
    //
    // Now that we've identified all subcommands and executed their mounts,
    // we can parse the remaining arguments, flags, and their values.
    let mut next_arg = out.cmd.args.first();
    let mut enable_flags = true;
    let mut grouped_flag = false;

    while !input.is_empty() {
        let mut w = input.pop_front().unwrap();

        // Check for restart_token - resets argument parsing for multiple command invocations
        // e.g., `mise run lint ::: test ::: check` with restart_token=":::"
        if let Some(ref restart_token) = out.cmd.restart_token {
            if w == *restart_token {
                // Reset argument parsing state for a fresh command invocation
                out.args.clear();
                next_arg = out.cmd.args.first();
                out.flag_awaiting_value.clear(); // Clear any pending flag values
                enable_flags = true; // Reset -- separator effect
                                     // Keep flags and continue parsing
                continue;
            }
        }

        if w == "--" {
            // Always disable flag parsing after seeing a "--" token
            enable_flags = false;

            // Only preserve the double dash token if we're collecting values for a variadic arg
            // in double_dash == `preserve` mode
            let should_preserve = next_arg
                .map(|arg| arg.var && arg.double_dash == SpecDoubleDashChoices::Preserve)
                .unwrap_or(false);

            if should_preserve {
                // Fall through to arg parsing
            } else {
                // Default behavior, skip the token
                continue;
            }
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
                    out.flag_awaiting_value.push(Arc::clone(f));
                } else if f.count {
                    let arr = out
                        .flags
                        .entry(Arc::clone(f))
                        .or_insert_with(|| ParseValue::MultiBool(vec![]))
                        .try_as_multi_bool_mut()
                        .unwrap();
                    arr.push(true);
                } else {
                    let negate = f.negate.clone().unwrap_or_default();
                    out.flags
                        .insert(Arc::clone(f), ParseValue::Bool(w != negate));
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
                    out.flag_awaiting_value.push(Arc::clone(f));
                } else if f.count {
                    let arr = out
                        .flags
                        .entry(Arc::clone(f))
                        .or_insert_with(|| ParseValue::MultiBool(vec![]))
                        .try_as_multi_bool_mut()
                        .unwrap();
                    arr.push(true);
                } else {
                    let negate = f.negate.clone().unwrap_or_default();
                    out.flags
                        .insert(Arc::clone(f), ParseValue::Bool(w != negate));
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
                let arr = out
                    .args
                    .entry(Arc::new(arg.clone()))
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
                out.args
                    .insert(Arc::new(arg.clone()), ParseValue::String(w));
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
        if arg.required && arg.default.is_empty() {
            // Check if there's an env var available (custom env map takes precedence)
            let has_env = arg.env.as_ref().is_some_and(|e| {
                custom_env.map(|env| env.contains_key(e)).unwrap_or(false)
                    || std::env::var(e).is_ok()
            });
            if !has_env {
                out.errors.push(UsageErr::MissingArg(arg.name.clone()));
            }
        }
    }

    for flag in out.available_flags.values() {
        if out.flags.contains_key(flag) {
            continue;
        }
        let has_default =
            !flag.default.is_empty() || flag.arg.iter().any(|a| !a.default.is_empty());
        // Check if there's an env var available (custom env map takes precedence)
        let has_env = flag.env.as_ref().is_some_and(|e| {
            custom_env.map(|env| env.contains_key(e)).unwrap_or(false) || std::env::var(e).is_ok()
        });
        if flag.required && !has_default && !has_env {
            out.errors.push(UsageErr::MissingFlag(flag.name.clone()));
        }
    }

    // Validate var_min/var_max constraints for variadic args
    for (arg, value) in &out.args {
        if arg.var {
            if let ParseValue::MultiString(values) = value {
                if let Some(min) = arg.var_min {
                    if values.len() < min {
                        out.errors.push(UsageErr::VarArgTooFew {
                            name: arg.name.clone(),
                            min,
                            got: values.len(),
                        });
                    }
                }
                if let Some(max) = arg.var_max {
                    if values.len() > max {
                        out.errors.push(UsageErr::VarArgTooMany {
                            name: arg.name.clone(),
                            max,
                            got: values.len(),
                        });
                    }
                }
            }
        }
    }

    // Validate var_min/var_max constraints for variadic flags
    for (flag, value) in &out.flags {
        if flag.var {
            let count = match value {
                ParseValue::MultiString(values) => values.len(),
                ParseValue::MultiBool(values) => values.len(),
                _ => continue,
            };
            if let Some(min) = flag.var_min {
                if count < min {
                    out.errors.push(UsageErr::VarFlagTooFew {
                        name: flag.name.clone(),
                        min,
                        got: count,
                    });
                }
            }
            if let Some(max) = flag.var_max {
                if count > max {
                    out.errors.push(UsageErr::VarFlagTooMany {
                        name: flag.name.clone(),
                        max,
                        got: count,
                    });
                }
            }
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
        let cmd = SpecCommand::builder()
            .name("test")
            .arg(SpecArg::builder().name("arg").build())
            .flag(SpecFlag::builder().long("flag").build())
            .build();
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
        let cmd = SpecCommand::builder()
            .name("test")
            .arg(SpecArg::builder().name("arg").build())
            .flag(SpecFlag::builder().long("flag").build())
            .flag(
                SpecFlag::builder()
                    .long("force")
                    .negate("--no-force")
                    .build(),
            )
            .build();
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
        let cmd = SpecCommand::builder()
            .name("test")
            .arg(
                SpecArg::builder()
                    .name("input")
                    .env("TEST_ARG_INPUT")
                    .required(true)
                    .build(),
            )
            .build();
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
        let cmd = SpecCommand::builder()
            .name("test")
            .flag(
                SpecFlag::builder()
                    .long("output")
                    .env("TEST_FLAG_OUTPUT")
                    .arg(SpecArg::builder().name("file").build())
                    .build(),
            )
            .build();
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
        let cmd = SpecCommand::builder()
            .name("test")
            .flag(
                SpecFlag::builder()
                    .long("verbose")
                    .env("TEST_FLAG_VERBOSE")
                    .build(),
            )
            .build();
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
        let cmd = SpecCommand::builder()
            .name("test")
            .arg(
                SpecArg::builder()
                    .name("input")
                    .env("TEST_PRECEDENCE_INPUT")
                    .required(true)
                    .build(),
            )
            .build();
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

    #[test]
    fn test_flag_var_true_with_single_default() {
        // When var=true and default="bar", the default should be MultiString(["bar"])
        let cmd = SpecCommand::builder()
            .name("test")
            .flag(
                SpecFlag::builder()
                    .long("foo")
                    .var(true)
                    .arg(SpecArg::builder().name("foo").build())
                    .default_value("bar")
                    .build(),
            )
            .build();
        let spec = Spec {
            name: "test".to_string(),
            bin: "test".to_string(),
            cmd,
            ..Default::default()
        };

        // User doesn't provide the flag
        let input = vec!["test".to_string()];
        let parsed = parse(&spec, &input).unwrap();

        assert_eq!(parsed.flags.len(), 1);
        let flag = parsed.flags.keys().next().unwrap();
        assert_eq!(flag.name, "foo");
        let value = parsed.flags.values().next().unwrap();
        // Should be MultiString, not String
        match value {
            ParseValue::MultiString(v) => {
                assert_eq!(v.len(), 1);
                assert_eq!(v[0], "bar");
            }
            _ => panic!("Expected MultiString, got {:?}", value),
        }
    }

    #[test]
    fn test_flag_var_true_with_multiple_defaults() {
        // When var=true and multiple defaults, should return MultiString(["xyz", "bar"])
        let cmd = SpecCommand::builder()
            .name("test")
            .flag(
                SpecFlag::builder()
                    .long("foo")
                    .var(true)
                    .arg(SpecArg::builder().name("foo").build())
                    .default_values(["xyz", "bar"])
                    .build(),
            )
            .build();
        let spec = Spec {
            name: "test".to_string(),
            bin: "test".to_string(),
            cmd,
            ..Default::default()
        };

        // User doesn't provide the flag
        let input = vec!["test".to_string()];
        let parsed = parse(&spec, &input).unwrap();

        assert_eq!(parsed.flags.len(), 1);
        let value = parsed.flags.values().next().unwrap();
        // Should be MultiString with both values
        match value {
            ParseValue::MultiString(v) => {
                assert_eq!(v.len(), 2);
                assert_eq!(v[0], "xyz");
                assert_eq!(v[1], "bar");
            }
            _ => panic!("Expected MultiString, got {:?}", value),
        }
    }

    #[test]
    fn test_flag_var_false_with_default_remains_string() {
        // When var=false (default), the default should still be String("bar")
        let cmd = SpecCommand::builder()
            .name("test")
            .flag(
                SpecFlag::builder()
                    .long("foo")
                    .var(false) // Default behavior
                    .arg(SpecArg::builder().name("foo").build())
                    .default_value("bar")
                    .build(),
            )
            .build();
        let spec = Spec {
            name: "test".to_string(),
            bin: "test".to_string(),
            cmd,
            ..Default::default()
        };

        // User doesn't provide the flag
        let input = vec!["test".to_string()];
        let parsed = parse(&spec, &input).unwrap();

        assert_eq!(parsed.flags.len(), 1);
        let value = parsed.flags.values().next().unwrap();
        // Should be String, not MultiString
        match value {
            ParseValue::String(s) => {
                assert_eq!(s, "bar");
            }
            _ => panic!("Expected String, got {:?}", value),
        }
    }

    #[test]
    fn test_arg_var_true_with_single_default() {
        // When arg has var=true and default="bar", the default should be MultiString(["bar"])
        let cmd = SpecCommand::builder()
            .name("test")
            .arg(
                SpecArg::builder()
                    .name("files")
                    .var(true)
                    .default_value("default.txt")
                    .required(false)
                    .build(),
            )
            .build();
        let spec = Spec {
            name: "test".to_string(),
            bin: "test".to_string(),
            cmd,
            ..Default::default()
        };

        // User doesn't provide the arg
        let input = vec!["test".to_string()];
        let parsed = parse(&spec, &input).unwrap();

        assert_eq!(parsed.args.len(), 1);
        let value = parsed.args.values().next().unwrap();
        // Should be MultiString, not String
        match value {
            ParseValue::MultiString(v) => {
                assert_eq!(v.len(), 1);
                assert_eq!(v[0], "default.txt");
            }
            _ => panic!("Expected MultiString, got {:?}", value),
        }
    }

    #[test]
    fn test_arg_var_true_with_multiple_defaults() {
        // When arg has var=true and multiple defaults
        let cmd = SpecCommand::builder()
            .name("test")
            .arg(
                SpecArg::builder()
                    .name("files")
                    .var(true)
                    .default_values(["file1.txt", "file2.txt"])
                    .required(false)
                    .build(),
            )
            .build();
        let spec = Spec {
            name: "test".to_string(),
            bin: "test".to_string(),
            cmd,
            ..Default::default()
        };

        // User doesn't provide the arg
        let input = vec!["test".to_string()];
        let parsed = parse(&spec, &input).unwrap();

        assert_eq!(parsed.args.len(), 1);
        let value = parsed.args.values().next().unwrap();
        // Should be MultiString with both values
        match value {
            ParseValue::MultiString(v) => {
                assert_eq!(v.len(), 2);
                assert_eq!(v[0], "file1.txt");
                assert_eq!(v[1], "file2.txt");
            }
            _ => panic!("Expected MultiString, got {:?}", value),
        }
    }

    #[test]
    fn test_arg_var_false_with_default_remains_string() {
        // When arg has var=false (default), the default should still be String
        let cmd = SpecCommand::builder()
            .name("test")
            .arg(
                SpecArg::builder()
                    .name("file")
                    .var(false)
                    .default_value("default.txt")
                    .required(false)
                    .build(),
            )
            .build();
        let spec = Spec {
            name: "test".to_string(),
            bin: "test".to_string(),
            cmd,
            ..Default::default()
        };

        // User doesn't provide the arg
        let input = vec!["test".to_string()];
        let parsed = parse(&spec, &input).unwrap();

        assert_eq!(parsed.args.len(), 1);
        let value = parsed.args.values().next().unwrap();
        // Should be String, not MultiString
        match value {
            ParseValue::String(s) => {
                assert_eq!(s, "default.txt");
            }
            _ => panic!("Expected String, got {:?}", value),
        }
    }

    #[test]
    fn test_default_subcommand() {
        // Test that default_subcommand routes to the specified subcommand
        let run_cmd = SpecCommand::builder()
            .name("run")
            .arg(SpecArg::builder().name("task").build())
            .build();
        let mut cmd = SpecCommand::builder().name("test").build();
        cmd.subcommands.insert("run".to_string(), run_cmd);

        let spec = Spec {
            name: "test".to_string(),
            bin: "test".to_string(),
            cmd,
            default_subcommand: Some("run".to_string()),
            ..Default::default()
        };

        // "test mytask" should be parsed as if it were "test run mytask"
        let input = vec!["test".to_string(), "mytask".to_string()];
        let parsed = parse(&spec, &input).unwrap();

        // Should have two commands: root and "run"
        assert_eq!(parsed.cmds.len(), 2);
        assert_eq!(parsed.cmds[1].name, "run");

        // Should have parsed the task argument
        assert_eq!(parsed.args.len(), 1);
        let arg = parsed.args.keys().next().unwrap();
        assert_eq!(arg.name, "task");
        let value = parsed.args.values().next().unwrap();
        assert_eq!(value.to_string(), "mytask");
    }

    #[test]
    fn test_default_subcommand_explicit_still_works() {
        // Test that explicit subcommand takes precedence
        let run_cmd = SpecCommand::builder()
            .name("run")
            .arg(SpecArg::builder().name("task").build())
            .build();
        let other_cmd = SpecCommand::builder()
            .name("other")
            .arg(SpecArg::builder().name("other_arg").build())
            .build();
        let mut cmd = SpecCommand::builder().name("test").build();
        cmd.subcommands.insert("run".to_string(), run_cmd);
        cmd.subcommands.insert("other".to_string(), other_cmd);

        let spec = Spec {
            name: "test".to_string(),
            bin: "test".to_string(),
            cmd,
            default_subcommand: Some("run".to_string()),
            ..Default::default()
        };

        // "test other foo" should use "other" subcommand, not default
        let input = vec!["test".to_string(), "other".to_string(), "foo".to_string()];
        let parsed = parse(&spec, &input).unwrap();

        // Should have used "other" subcommand
        assert_eq!(parsed.cmds.len(), 2);
        assert_eq!(parsed.cmds[1].name, "other");
    }

    #[test]
    fn test_default_subcommand_with_nested_subcommands() {
        // Test that default_subcommand works when the default subcommand has nested subcommands.
        // This is the mise use case: "mise say" should be parsed as "mise run say"
        // where "say" is a subcommand of "run" (a task).
        let say_cmd = SpecCommand::builder()
            .name("say")
            .arg(SpecArg::builder().name("name").build())
            .build();
        let mut run_cmd = SpecCommand::builder().name("run").build();
        run_cmd.subcommands.insert("say".to_string(), say_cmd);

        let mut cmd = SpecCommand::builder().name("test").build();
        cmd.subcommands.insert("run".to_string(), run_cmd);

        let spec = Spec {
            name: "test".to_string(),
            bin: "test".to_string(),
            cmd,
            default_subcommand: Some("run".to_string()),
            ..Default::default()
        };

        // "test say hello" should be parsed as "test run say hello"
        let input = vec!["test".to_string(), "say".to_string(), "hello".to_string()];
        let parsed = parse(&spec, &input).unwrap();

        // Should have three commands: root, "run", and "say"
        assert_eq!(parsed.cmds.len(), 3);
        assert_eq!(parsed.cmds[0].name, "test");
        assert_eq!(parsed.cmds[1].name, "run");
        assert_eq!(parsed.cmds[2].name, "say");

        // Should have parsed the "name" argument
        assert_eq!(parsed.args.len(), 1);
        let arg = parsed.args.keys().next().unwrap();
        assert_eq!(arg.name, "name");
        let value = parsed.args.values().next().unwrap();
        assert_eq!(value.to_string(), "hello");
    }

    #[test]
    fn test_default_subcommand_same_name_child() {
        // Test that default_subcommand doesn't cause issues when the default subcommand
        // has a child with the same name (e.g., "run" has a task named "run").
        // This verifies we don't switch multiple times or get stuck in a loop.
        let run_task = SpecCommand::builder()
            .name("run")
            .arg(SpecArg::builder().name("args").build())
            .build();
        let mut run_cmd = SpecCommand::builder().name("run").build();
        run_cmd.subcommands.insert("run".to_string(), run_task);

        let mut cmd = SpecCommand::builder().name("test").build();
        cmd.subcommands.insert("run".to_string(), run_cmd);

        let spec = Spec {
            name: "test".to_string(),
            bin: "test".to_string(),
            cmd,
            default_subcommand: Some("run".to_string()),
            ..Default::default()
        };

        // "test run" explicitly matches the "run" subcommand (not via default_subcommand)
        let input = vec!["test".to_string(), "run".to_string()];
        let parsed = parse(&spec, &input).unwrap();

        // Should have two commands: root and "run"
        assert_eq!(parsed.cmds.len(), 2);
        assert_eq!(parsed.cmds[0].name, "test");
        assert_eq!(parsed.cmds[1].name, "run");

        // "test run run" should descend into the "run" task (child of "run" subcommand)
        let input = vec![
            "test".to_string(),
            "run".to_string(),
            "run".to_string(),
            "hello".to_string(),
        ];
        let parsed = parse(&spec, &input).unwrap();

        assert_eq!(parsed.cmds.len(), 3);
        assert_eq!(parsed.cmds[0].name, "test");
        assert_eq!(parsed.cmds[1].name, "run");
        assert_eq!(parsed.cmds[2].name, "run");
        assert_eq!(parsed.args.len(), 1);
        let value = parsed.args.values().next().unwrap();
        assert_eq!(value.to_string(), "hello");

        // Key test case: "test other" should switch to default subcommand "run"
        // and treat "other" as a positional arg (not try to switch again because
        // "run" also has a "run" child).
        let mut run_cmd = SpecCommand::builder()
            .name("run")
            .arg(SpecArg::builder().name("task").build())
            .build();
        let run_task = SpecCommand::builder().name("run").build();
        run_cmd.subcommands.insert("run".to_string(), run_task);

        let mut cmd = SpecCommand::builder().name("test").build();
        cmd.subcommands.insert("run".to_string(), run_cmd);

        let spec = Spec {
            name: "test".to_string(),
            bin: "test".to_string(),
            cmd,
            default_subcommand: Some("run".to_string()),
            ..Default::default()
        };

        let input = vec!["test".to_string(), "other".to_string()];
        let parsed = parse(&spec, &input).unwrap();

        // Should have two commands: root and "run" (the default)
        // We should NOT have switched again to the "run" task child
        assert_eq!(parsed.cmds.len(), 2);
        assert_eq!(parsed.cmds[0].name, "test");
        assert_eq!(parsed.cmds[1].name, "run");

        // "other" should be parsed as a positional arg
        assert_eq!(parsed.args.len(), 1);
        let value = parsed.args.values().next().unwrap();
        assert_eq!(value.to_string(), "other");
    }

    #[test]
    fn test_restart_token() {
        // Test that restart_token resets argument parsing
        let run_cmd = SpecCommand::builder()
            .name("run")
            .arg(SpecArg::builder().name("task").build())
            .restart_token(":::".to_string())
            .build();
        let mut cmd = SpecCommand::builder().name("test").build();
        cmd.subcommands.insert("run".to_string(), run_cmd);

        let spec = Spec {
            name: "test".to_string(),
            bin: "test".to_string(),
            cmd,
            ..Default::default()
        };

        // "test run task1 ::: task2" - should end up with task2 as the arg
        let input = vec![
            "test".to_string(),
            "run".to_string(),
            "task1".to_string(),
            ":::".to_string(),
            "task2".to_string(),
        ];
        let parsed = parse(&spec, &input).unwrap();

        // After restart, args were cleared and task2 was parsed
        assert_eq!(parsed.args.len(), 1);
        let value = parsed.args.values().next().unwrap();
        assert_eq!(value.to_string(), "task2");
    }

    #[test]
    fn test_restart_token_multiple() {
        // Test multiple restart tokens
        let run_cmd = SpecCommand::builder()
            .name("run")
            .arg(SpecArg::builder().name("task").build())
            .restart_token(":::".to_string())
            .build();
        let mut cmd = SpecCommand::builder().name("test").build();
        cmd.subcommands.insert("run".to_string(), run_cmd);

        let spec = Spec {
            name: "test".to_string(),
            bin: "test".to_string(),
            cmd,
            ..Default::default()
        };

        // "test run task1 ::: task2 ::: task3" - should end up with task3 as the arg
        let input = vec![
            "test".to_string(),
            "run".to_string(),
            "task1".to_string(),
            ":::".to_string(),
            "task2".to_string(),
            ":::".to_string(),
            "task3".to_string(),
        ];
        let parsed = parse(&spec, &input).unwrap();

        // After multiple restarts, args were cleared and task3 was parsed
        assert_eq!(parsed.args.len(), 1);
        let value = parsed.args.values().next().unwrap();
        assert_eq!(value.to_string(), "task3");
    }

    #[test]
    fn test_restart_token_clears_flag_awaiting_value() {
        // Test that restart_token clears pending flag values
        let run_cmd = SpecCommand::builder()
            .name("run")
            .arg(SpecArg::builder().name("task").build())
            .flag(
                SpecFlag::builder()
                    .name("jobs")
                    .long("jobs")
                    .arg(SpecArg::builder().name("count").build())
                    .build(),
            )
            .restart_token(":::".to_string())
            .build();
        let mut cmd = SpecCommand::builder().name("test").build();
        cmd.subcommands.insert("run".to_string(), run_cmd);

        let spec = Spec {
            name: "test".to_string(),
            bin: "test".to_string(),
            cmd,
            ..Default::default()
        };

        // "test run task1 --jobs ::: task2" - task2 should be an arg, not a flag value
        let input = vec![
            "test".to_string(),
            "run".to_string(),
            "task1".to_string(),
            "--jobs".to_string(),
            ":::".to_string(),
            "task2".to_string(),
        ];
        let parsed = parse(&spec, &input).unwrap();

        // task2 should be parsed as the task arg, not as --jobs value
        assert_eq!(parsed.args.len(), 1);
        let value = parsed.args.values().next().unwrap();
        assert_eq!(value.to_string(), "task2");
        // --jobs should not have a value
        assert!(parsed.flag_awaiting_value.is_empty());
    }

    #[test]
    fn test_restart_token_resets_double_dash() {
        // Test that restart_token resets the -- separator effect
        let run_cmd = SpecCommand::builder()
            .name("run")
            .arg(SpecArg::builder().name("task").build())
            .arg(SpecArg::builder().name("extra_args").var(true).build())
            .flag(SpecFlag::builder().name("verbose").long("verbose").build())
            .restart_token(":::".to_string())
            .build();
        let mut cmd = SpecCommand::builder().name("test").build();
        cmd.subcommands.insert("run".to_string(), run_cmd);

        let spec = Spec {
            name: "test".to_string(),
            bin: "test".to_string(),
            cmd,
            ..Default::default()
        };

        // "test run task1 -- extra ::: --verbose task2" - --verbose should be a flag after :::
        let input = vec![
            "test".to_string(),
            "run".to_string(),
            "task1".to_string(),
            "--".to_string(),
            "extra".to_string(),
            ":::".to_string(),
            "--verbose".to_string(),
            "task2".to_string(),
        ];
        let parsed = parse(&spec, &input).unwrap();

        // --verbose should be parsed as a flag (not an arg) after the restart
        assert!(parsed.flags.keys().any(|f| f.name == "verbose"));
        // task2 should be the arg after restart
        let task_arg = parsed.args.keys().find(|a| a.name == "task").unwrap();
        let value = parsed.args.get(task_arg).unwrap();
        assert_eq!(value.to_string(), "task2");
    }

    #[test]
    fn test_double_dashes_without_preserve() {
        // Test that variadic args WITHOUT `preserve` skip "--" tokens (default behavior)
        let run_cmd = SpecCommand::builder()
            .name("run")
            .arg(SpecArg::builder().name("args").var(true).build())
            .build();
        let mut cmd = SpecCommand::builder().name("test").build();
        cmd.subcommands.insert("run".to_string(), run_cmd);

        let spec = Spec {
            name: "test".to_string(),
            bin: "test".to_string(),
            cmd,
            ..Default::default()
        };

        // "test run arg1 -- arg2 -- arg3" - all double dashes should be skipped
        let input = vec![
            "test".to_string(),
            "run".to_string(),
            "arg1".to_string(),
            "--".to_string(),
            "arg2".to_string(),
            "--".to_string(),
            "arg3".to_string(),
        ];
        let parsed = parse(&spec, &input).unwrap();

        let args_arg = parsed.args.keys().find(|a| a.name == "args").unwrap();
        let value = parsed.args.get(args_arg).unwrap();
        assert_eq!(value.to_string(), "arg1 arg2 arg3");
    }

    #[test]
    fn test_double_dashes_with_preserve() {
        // Test that variadic args WITH `preserve` keep all double dashes
        let run_cmd = SpecCommand::builder()
            .name("run")
            .arg(
                SpecArg::builder()
                    .name("args")
                    .var(true)
                    .double_dash(SpecDoubleDashChoices::Preserve)
                    .build(),
            )
            .build();
        let mut cmd = SpecCommand::builder().name("test").build();
        cmd.subcommands.insert("run".to_string(), run_cmd);

        let spec = Spec {
            name: "test".to_string(),
            bin: "test".to_string(),
            cmd,
            ..Default::default()
        };

        // "test run arg1 -- arg2 -- arg3" - all double dashes should be preserved
        let input = vec![
            "test".to_string(),
            "run".to_string(),
            "arg1".to_string(),
            "--".to_string(),
            "arg2".to_string(),
            "--".to_string(),
            "arg3".to_string(),
        ];
        let parsed = parse(&spec, &input).unwrap();

        let args_arg = parsed.args.keys().find(|a| a.name == "args").unwrap();
        let value = parsed.args.get(args_arg).unwrap();
        assert_eq!(value.to_string(), "arg1 -- arg2 -- arg3");
    }

    #[test]
    fn test_double_dashes_with_preserve_only_dashes() {
        // Test that variadic args WITH `preserve` keep all double dashes even
        // if the values are just double dashes
        let run_cmd = SpecCommand::builder()
            .name("run")
            .arg(
                SpecArg::builder()
                    .name("args")
                    .var(true)
                    .double_dash(SpecDoubleDashChoices::Preserve)
                    .build(),
            )
            .build();
        let mut cmd = SpecCommand::builder().name("test").build();
        cmd.subcommands.insert("run".to_string(), run_cmd);

        let spec = Spec {
            name: "test".to_string(),
            bin: "test".to_string(),
            cmd,
            ..Default::default()
        };

        // "test run -- --" - all double dashes should be preserved
        let input = vec![
            "test".to_string(),
            "run".to_string(),
            "--".to_string(),
            "--".to_string(),
        ];
        let parsed = parse(&spec, &input).unwrap();

        let args_arg = parsed.args.keys().find(|a| a.name == "args").unwrap();
        let value = parsed.args.get(args_arg).unwrap();
        assert_eq!(value.to_string(), "-- --");
    }

    #[test]
    fn test_double_dashes_with_preserve_multiple_args() {
        // Test with multiple args where only the second has has `preserve`
        let run_cmd = SpecCommand::builder()
            .name("run")
            .arg(SpecArg::builder().name("task").build())
            .arg(
                SpecArg::builder()
                    .name("extra_args")
                    .var(true)
                    .double_dash(SpecDoubleDashChoices::Preserve)
                    .build(),
            )
            .build();
        let mut cmd = SpecCommand::builder().name("test").build();
        cmd.subcommands.insert("run".to_string(), run_cmd);

        let spec = Spec {
            name: "test".to_string(),
            bin: "test".to_string(),
            cmd,
            ..Default::default()
        };

        // The first arg "task1" is captured normally
        // Then extra_args with `preserve` captures everything, including the "--" tokens
        let input = vec![
            "test".to_string(),
            "run".to_string(),
            "task1".to_string(),
            "--".to_string(),
            "arg1".to_string(),
            "--".to_string(),
            "--foo".to_string(),
        ];
        let parsed = parse(&spec, &input).unwrap();

        let task_arg = parsed.args.keys().find(|a| a.name == "task").unwrap();
        let task_value = parsed.args.get(task_arg).unwrap();
        assert_eq!(task_value.to_string(), "task1");

        let extra_arg = parsed.args.keys().find(|a| a.name == "extra_args").unwrap();
        let extra_value = parsed.args.get(extra_arg).unwrap();
        assert_eq!(extra_value.to_string(), "-- arg1 -- --foo");
    }

    #[test]
    fn test_parser_with_custom_env_for_required_arg() {
        // Test that Parser::with_env works for required args with env vars
        // This should NOT fail validation even though the env var is not in std::env
        let cmd = SpecCommand::builder()
            .name("test")
            .arg(
                SpecArg::builder()
                    .name("name")
                    .env("NAME")
                    .required(true)
                    .build(),
            )
            .build();
        let spec = Spec {
            name: "test".to_string(),
            bin: "test".to_string(),
            cmd,
            ..Default::default()
        };

        // Ensure NAME is not in process env
        std::env::remove_var("NAME");

        // Provide env through custom env map
        let mut env = HashMap::new();
        env.insert("NAME".to_string(), "john".to_string());

        let input = vec!["test".to_string()];
        let result = Parser::new(&spec).with_env(env).parse(&input);

        // Should succeed - custom env map should be used for validation
        let parsed = result.expect("parse should succeed with custom env");
        assert_eq!(parsed.args.len(), 1);
        let value = parsed.args.values().next().unwrap();
        assert_eq!(value.to_string(), "john");
    }

    #[test]
    fn test_parser_with_custom_env_for_required_flag() {
        // Test that Parser::with_env works for required flags with env vars
        let cmd = SpecCommand::builder()
            .name("test")
            .flag(
                SpecFlag::builder()
                    .long("name")
                    .env("NAME")
                    .required(true)
                    .arg(SpecArg::builder().name("name").build())
                    .build(),
            )
            .build();
        let spec = Spec {
            name: "test".to_string(),
            bin: "test".to_string(),
            cmd,
            ..Default::default()
        };

        // Ensure NAME is not in process env
        std::env::remove_var("NAME");

        // Provide env through custom env map
        let mut env = HashMap::new();
        env.insert("NAME".to_string(), "jane".to_string());

        let input = vec!["test".to_string()];
        let result = Parser::new(&spec).with_env(env).parse(&input);

        // Should succeed - custom env map should be used for validation
        let parsed = result.expect("parse should succeed with custom env");
        assert_eq!(parsed.flags.len(), 1);
        let value = parsed.flags.values().next().unwrap();
        assert_eq!(value.to_string(), "jane");
    }

    #[test]
    fn test_parser_with_custom_env_still_fails_when_missing() {
        // Test that validation still fails when env var is missing from both maps
        let cmd = SpecCommand::builder()
            .name("test")
            .arg(
                SpecArg::builder()
                    .name("name")
                    .env("NAME")
                    .required(true)
                    .build(),
            )
            .build();
        let spec = Spec {
            name: "test".to_string(),
            bin: "test".to_string(),
            cmd,
            ..Default::default()
        };

        // Ensure NAME is not in process env
        std::env::remove_var("NAME");

        // Provide a custom env map WITHOUT the required env var
        let env = HashMap::new();

        let input = vec!["test".to_string()];
        let result = Parser::new(&spec).with_env(env).parse(&input);

        // Should fail - env var is missing from both custom and process env
        assert!(result.is_err());
    }

    #[test]
    fn test_variadic_arg_captures_unknown_flags_from_spec_string() {
        let spec: Spec = r#"
            flag "-v --verbose" var=#true
            arg "[database]" default="myapp_dev"
            arg "[args...]"
        "#
        .parse()
        .unwrap();
        let input: Vec<String> = vec!["test", "mydb", "--host", "localhost"]
            .into_iter()
            .map(String::from)
            .collect();
        let parsed = parse(&spec, &input).unwrap();
        let env = parsed.as_env();
        assert_eq!(env.get("usage_database").unwrap(), "mydb");
        assert_eq!(env.get("usage_args").unwrap(), "--host localhost");
    }

    #[test]
    fn test_variadic_arg_captures_unknown_flags() {
        let cmd = SpecCommand::builder()
            .name("test")
            .flag(SpecFlag::builder().short('v').long("verbose").build())
            .arg(SpecArg::builder().name("database").required(false).build())
            .arg(
                SpecArg::builder()
                    .name("args")
                    .required(false)
                    .var(true)
                    .build(),
            )
            .build();
        let spec = Spec {
            name: "test".to_string(),
            bin: "test".to_string(),
            cmd,
            ..Default::default()
        };

        // Unknown --host flag and its value should be captured by [args...]
        let input: Vec<String> = vec!["test", "mydb", "--host", "localhost"]
            .into_iter()
            .map(String::from)
            .collect();
        let parsed = parse(&spec, &input).unwrap();
        assert_eq!(parsed.args.len(), 2);
        let args_val = parsed
            .args
            .iter()
            .find(|(a, _)| a.name == "args")
            .unwrap()
            .1;
        match args_val {
            ParseValue::MultiString(v) => {
                assert_eq!(v, &vec!["--host".to_string(), "localhost".to_string()]);
            }
            _ => panic!("Expected MultiString, got {:?}", args_val),
        }
    }

    #[test]
    fn test_variadic_arg_captures_unknown_flags_with_double_dash() {
        let cmd = SpecCommand::builder()
            .name("test")
            .flag(SpecFlag::builder().short('v').long("verbose").build())
            .arg(SpecArg::builder().name("database").required(false).build())
            .arg(
                SpecArg::builder()
                    .name("args")
                    .required(false)
                    .var(true)
                    .build(),
            )
            .build();
        let spec = Spec {
            name: "test".to_string(),
            bin: "test".to_string(),
            cmd,
            ..Default::default()
        };

        // With explicit -- separator
        let input: Vec<String> = vec!["test", "--", "mydb", "--host", "localhost"]
            .into_iter()
            .map(String::from)
            .collect();
        let parsed = parse(&spec, &input).unwrap();
        assert_eq!(parsed.args.len(), 2);
        let args_val = parsed
            .args
            .iter()
            .find(|(a, _)| a.name == "args")
            .unwrap()
            .1;
        match args_val {
            ParseValue::MultiString(v) => {
                assert_eq!(v, &vec!["--host".to_string(), "localhost".to_string()]);
            }
            _ => panic!("Expected MultiString, got {:?}", args_val),
        }
    }
}
