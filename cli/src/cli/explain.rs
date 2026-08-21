//! What a command line binds to, and where each value came from.
//!
//! `docs/spec/argv.md` exists to define "which token binds to which flag or argument", and
//! until now nothing would show you that for a given command line. The parser knew and threw
//! it away. So a spec author debugging a flag that will not take a value, and a reader
//! learning why `--color bar` leaves `bar` to the positionals, both had to reason about the
//! grammar rather than ask.
//!
//! Two tables, deliberately. A table keyed by token cannot show a value that came from
//! nowhere in argv; a table keyed by declaration cannot show a token that bound to nothing.
//! jdx/mise discussion #8883 — `mise --env=production` silently ignored while
//! `mise --env production` worked — lives in the first. "Why is my default not applying"
//! lives in the second.

use std::collections::HashMap;
use std::path::PathBuf;

use itertools::Itertools;
use miette::Result;
use usage::parse::{ParseOutput, Parser, TokenRole, ValueOrigin};
use usage::{Spec, SpecArg, SpecFlag};

use crate::cli::generate::{file_or_spec, select_view};
use crate::cli::OutputFormat;

/// Explain what a command line binds to
///
/// Prints a row per argv token saying what it became, then the values that came from
/// somewhere other than argv, then anything that went wrong. Exits 0 even when the explained
/// command line does not parse: the report succeeded, and that is the case worth a report.
#[derive(Debug, usage_rs::Args)]
// No `unknown_flags = "value"`, unlike `exec`: `double_dash="automatic"` on `argv` below
// already ends this command's own flag parsing at the program name, so a well-formed
// invocation never offers a foreign flag here. Keeping the root's `error` means
// `usage explain -f f.kdl --nope` says there is no such flag rather than silently explaining
// nothing.
#[usage(effect = "read", verbatim_doc_comment)]
pub struct Explain {
    /// A usage spec file or script with a usage shebang, use "-" to read from stdin
    #[usage(short, long, value_hint = usage_rs::ValueHint::FilePath)]
    file: Option<PathBuf>,

    /// Raw string spec input
    #[usage(short, long, required_unless = "--file", overrides = "--file")]
    spec: Option<String>,

    /// Output format
    #[usage(long, default = "text", value_enum)]
    format: OutputFormat,

    /// A spec-declared executable view to explain
    #[usage(long)]
    view: Option<String>,

    /// Environment to explain against, as KEY=VALUE, repeatable
    ///
    /// Given at all, these are the *whole* environment: an explanation pasted into a bug
    /// report has to mean the same thing on the machine that reads it. Omitted, the process
    /// environment is used, which is what an execution would see.
    #[usage(short, long, var = true)]
    env: Vec<String>,

    /// The command line to explain, starting with the program name
    ///
    /// `usage`'s own flags come before it, and flag parsing ends at the program name, so
    /// both `explain -f f.kdl mycli --env=prod` and `explain -f f.kdl -- mycli --env=prod`
    /// work. Separate with `--` when the explained line carries its own: the first `--` is
    /// still `usage`'s separator, so `explain -f f.kdl mycli a -- b` loses one.
    #[usage(double_dash = "automatic", value_hint = usage_rs::ValueHint::CommandWithArguments)]
    argv: Vec<String>,
}

impl usage_rs::Run for Explain {
    type Output = Result<()>;

    fn run(self) -> Self::Output {
        let spec = select_view(file_or_spec(&self.file, &self.spec)?, self.view.as_deref())?;
        let env = self.env_map()?;
        // A bare invocation is a legitimate question — it asks which defaults and
        // environment values fire with no command line at all — so the program name is
        // filled in rather than refused.
        let argv = if self.argv.is_empty() {
            vec![spec.bin.clone()]
        } else {
            self.argv.clone()
        };

        let explanation = explain(&spec, &argv, env);
        match self.format {
            OutputFormat::Text => print!("{}", explanation.render()),
            OutputFormat::Json => println!(
                "{}",
                serde_json::to_string_pretty(&explanation)
                    .map_err(|e| miette::miette!("failed to serialize the explanation: {e}"))?
            ),
        }
        Ok(())
    }
}

impl Explain {
    fn env_map(&self) -> Result<Option<HashMap<String, String>>> {
        if self.env.is_empty() {
            return Ok(None);
        }
        let mut env = HashMap::new();
        for entry in &self.env {
            // An empty key is refused with the rest: no such variable can exist, so a
            // report naming one would be describing an environment nothing could produce.
            let (key, value) = entry
                .split_once('=')
                .filter(|(key, _)| !key.is_empty())
                .ok_or_else(|| miette::miette!("--env wants KEY=VALUE, got `{entry}`"))?;
            env.insert(key.to_string(), value.to_string());
        }
        Ok(Some(env))
    }
}

/// The explanation as data rather than as printed lines.
///
/// A seam for the same reason `lint_spec` is one: the layout is worth testing against a value
/// instead of against stdout.
pub fn explain(spec: &Spec, argv: &[String], env: Option<HashMap<String, String>>) -> Explanation {
    let mut parser = Parser::new(spec);
    if let Some(env) = env {
        parser = parser.with_env(env);
    }
    match parser.explain(argv) {
        Ok(out) => Explanation::from_parse(argv, &out, true, None),
        // The parse could not carry on: a mount that will not run, a word no declaration can
        // take, a value refused by `choices`. The binding phase still knows which tokens got
        // that far, so ask it on its own rather than reporting the error with nothing around
        // it. Mounts resolve eagerly on this path and the environment is not consulted, which
        // is why the report says the fallbacks did not run.
        Err(refused) => match usage::parse::parse_partial(spec, argv) {
            Ok(out) => Explanation::from_parse(argv, &out, false, Some(refused.to_string())),
            Err(_) => Explanation {
                argv: argv.to_vec(),
                refused: Some(refused.to_string()),
                fallbacks_applied: false,
                ..Explanation::default()
            },
        },
    }
}

#[derive(Debug, Default, serde::Serialize)]
pub struct Explanation {
    /// The command line, as it was given.
    pub argv: Vec<String>,
    /// The command path the parse resolved to.
    pub command: Vec<String>,
    pub tokens: Vec<TokenRow>,
    pub values: Vec<ValueRow>,
    /// Declared defaults that did not win, and what won instead.
    pub shadowed: Vec<ShadowRow>,
    /// Flags a later occurrence removed, and the flag that removed them.
    pub overridden: Vec<OverrideRow>,
    pub errors: Vec<String>,
    /// The one failure the parse could not continue past, if there was one.
    pub refused: Option<String>,
    /// Whether the environment-and-defaults phase ran. False when only the binding phase
    /// could be reported.
    pub fallbacks_applied: bool,
}

#[derive(Debug, serde::Serialize)]
pub struct TokenRow {
    pub index: usize,
    pub text: String,
    /// The parser read something else here — the basename of a multicall program, the tail
    /// of a short bundle. `text` is what the caller wrote.
    pub synthesized: bool,
    pub roles: Vec<RoleRow>,
}

/// What one token did, named rather than pointing at a declaration.
#[derive(Debug, serde::Serialize)]
#[serde(tag = "kind", rename_all = "snake_case")]
pub enum RoleRow {
    Program,
    Subcommand {
        name: String,
    },
    Flag {
        name: String,
        spelling: String,
        negated: bool,
    },
    Value {
        name: String,
        values: Vec<String>,
        attached: bool,
    },
    Arg {
        name: String,
        values: Vec<String>,
    },
    Separator,
    ValueTerminator {
        ends: String,
    },
    Restart,
    UnknownFlag {
        bound_as: Option<String>,
    },
    Refused {
        reason: String,
    },
    External,
    Unread,
}

#[derive(Debug, serde::Serialize)]
pub struct ValueRow {
    /// `flag` or `arg`.
    pub kind: String,
    pub name: String,
    /// How the declaration is spelled in help: `--jobs`, `<src>`.
    pub display: String,
    pub value: String,
    /// Where the value came from. Several when one declaration took values from more than
    /// one place, which a `var` flag can.
    pub origins: Vec<OriginRow>,
}

#[derive(Debug, Clone, serde::Serialize)]
#[serde(tag = "kind", rename_all = "snake_case")]
pub enum OriginRow {
    /// Typed, at these argv positions.
    Argv {
        tokens: Vec<usize>,
    },
    DefaultMissing,
    Env {
        name: String,
    },
    Default,
    DefaultIf {
        selector: String,
        when: Option<String>,
    },
}

#[derive(Debug, serde::Serialize)]
pub struct ShadowRow {
    pub kind: String,
    pub name: String,
    pub display: String,
    /// The value the declaration would have supplied.
    pub value: String,
    pub lost_to: Vec<OriginRow>,
}

#[derive(Debug, serde::Serialize)]
pub struct OverrideRow {
    pub name: String,
    pub by: String,
}

impl Explanation {
    fn from_parse(
        argv: &[String],
        out: &ParseOutput,
        fallbacks_applied: bool,
        refused: Option<String>,
    ) -> Self {
        let tokens = out.tokens.iter().map(TokenRow::from_binding).collect_vec();
        let mut values = vec![];
        let mut shadowed = vec![];

        for (flag, value) in &out.flags {
            let origins = flag_origins(out, flag);
            values.push(ValueRow {
                kind: "flag".to_string(),
                name: flag.name.clone(),
                display: flag_display(flag),
                value: value.to_string(),
                origins: origins.clone(),
            });
            // A declared default that did not supply the value lost to whatever did. This is
            // the most common thing a spec author is confused about, and the parser records
            // exactly enough to answer it without guessing.
            if let Some(declared) = declared_default(flag) {
                if !origins.iter().any(|o| matches!(o, OriginRow::Default)) {
                    shadowed.push(ShadowRow {
                        kind: "flag".to_string(),
                        name: flag.name.clone(),
                        display: flag_display(flag),
                        value: declared,
                        lost_to: origins,
                    });
                }
            }
        }
        for (arg, value) in &out.args {
            let origins = arg_origins(out, arg);
            values.push(ValueRow {
                kind: "arg".to_string(),
                name: arg.name.clone(),
                display: arg.usage(),
                value: value.to_string(),
                origins: origins.clone(),
            });
            if !arg.default.is_empty() && !origins.iter().any(|o| matches!(o, OriginRow::Default)) {
                shadowed.push(ShadowRow {
                    kind: "arg".to_string(),
                    name: arg.name.clone(),
                    display: arg.usage(),
                    value: arg.default.join(" "),
                    lost_to: origins,
                });
            }
        }

        Self {
            argv: argv.to_vec(),
            command: out.cmds.iter().map(|cmd| cmd.name.clone()).collect(),
            tokens,
            values,
            shadowed,
            overridden: out
                .overridden_flags
                .iter()
                .map(|(name, by)| OverrideRow {
                    name: name.clone(),
                    by: by.clone(),
                })
                .collect(),
            errors: out.errors.iter().map(|e| e.to_string()).collect(),
            refused,
            fallbacks_applied,
        }
    }

    /// The text report, as `usage explain` prints it.
    pub fn render(&self) -> String {
        let mut out = String::new();
        out.push_str(&format!("{}\n", self.argv.join(" ")));
        if !self.command.is_empty() {
            out.push_str(&format!("command  {}\n", self.command.join(" ")));
        }
        if !self.fallbacks_applied {
            out.push_str(
                "\nthe parse stopped before the environment and defaults were applied,\n\
                 so only what argv bound is shown\n",
            );
        }

        out.push_str(&render_table("tokens", &self.tokens, |token| {
            let roles = if token.roles.is_empty() {
                // Not the same as "not read": the word was read and did nothing this parser
                // records, which is worth seeing rather than quietly omitting.
                "bound nothing".to_string()
            } else {
                token.roles.iter().map(render_role).join(", ")
            };
            vec![format!("[{}]", token.index), token.text.clone(), roles]
        }));

        out.push_str(&render_table("values", &self.values, |row| {
            vec![
                row.kind.clone(),
                row.display.clone(),
                row.value.clone(),
                render_origins(&row.origins),
            ]
        }));
        out.push_str(&render_table("shadowed", &self.shadowed, |row| {
            vec![
                row.kind.clone(),
                row.display.clone(),
                format!("default {}", row.value),
                format!("lost to {}", render_origins(&row.lost_to)),
            ]
        }));
        out.push_str(&render_table("overridden", &self.overridden, |row| {
            vec![row.name.clone(), format!("by --{}", row.by)]
        }));

        if !self.errors.is_empty() {
            out.push_str("\nerrors\n");
            for error in &self.errors {
                for line in error.lines() {
                    out.push_str(&format!("  {line}\n"));
                }
            }
        }
        if let Some(refused) = &self.refused {
            out.push_str("\nrefused\n");
            for line in refused.lines() {
                out.push_str(&format!("  {line}\n"));
            }
        }
        out
    }
}

/// A titled block of aligned columns, or nothing at all when there are no rows.
///
/// Every column but the last is padded to its widest cell, which is what makes the report
/// greppable: a reader scanning for "where did this come from" reads down one column.
fn render_table<T>(title: &str, rows: &[T], columns: impl Fn(&T) -> Vec<String>) -> String {
    if rows.is_empty() {
        return String::new();
    }
    let rendered = rows.iter().map(columns).collect_vec();
    let widths = column_widths(&rendered);
    let mut out = format!("\n{title}\n");
    for row in &rendered {
        out.push_str("  ");
        out.push_str(&pad_row(row, &widths));
        out.push('\n');
    }
    out
}

fn column_widths(rows: &[Vec<String>]) -> Vec<usize> {
    let count = rows.iter().map(Vec::len).max().unwrap_or(0);
    (0..count)
        .map(|column| {
            rows.iter()
                .filter_map(|row| row.get(column))
                .map(|cell| cell.chars().count())
                .max()
                .unwrap_or(0)
        })
        .collect()
}

fn pad_row(row: &[String], widths: &[usize]) -> String {
    row.iter()
        .enumerate()
        .map(|(column, cell)| {
            // The last cell is never padded: trailing whitespace is invisible to a reader
            // and annoying to everything else.
            if column + 1 == row.len() {
                cell.clone()
            } else {
                format!("{cell:width$}", width = widths[column])
            }
        })
        .join("  ")
        .trim_end()
        .to_string()
}

impl TokenRow {
    fn from_binding(token: &usage::parse::TokenBinding) -> Self {
        Self {
            index: token.index,
            text: token.word.clone(),
            synthesized: token.synthesized,
            roles: token.roles.iter().map(RoleRow::from_role).collect(),
        }
    }
}

impl RoleRow {
    fn from_role(role: &TokenRole) -> Self {
        match role {
            TokenRole::Program => Self::Program,
            TokenRole::Command { name } => Self::Subcommand { name: name.clone() },
            TokenRole::Flag {
                flag,
                spelling,
                negated,
            } => Self::Flag {
                name: flag.name.clone(),
                spelling: spelling.clone(),
                negated: *negated,
            },
            TokenRole::Value {
                flag,
                values,
                attached,
            } => Self::Value {
                name: flag.name.clone(),
                values: values.clone(),
                attached: *attached,
            },
            TokenRole::Arg { arg, values } => Self::Arg {
                name: arg.name.clone(),
                values: values.clone(),
            },
            TokenRole::Separator => Self::Separator,
            TokenRole::ValueTerminator { ends } => Self::ValueTerminator { ends: ends.clone() },
            TokenRole::Restart => Self::Restart,
            TokenRole::UnknownFlag { bound_as } => Self::UnknownFlag {
                bound_as: bound_as.as_ref().map(|arg| arg.name.clone()),
            },
            TokenRole::Refused { reason } => Self::Refused {
                reason: reason.clone(),
            },
            TokenRole::External => Self::External,
            TokenRole::Unread => Self::Unread,
            // The lib's role list is `#[non_exhaustive]`, so a role added there shows up as
            // an unexplained token rather than failing to compile a report of it.
            _ => Self::Unread,
        }
    }
}

fn render_role(role: &RoleRow) -> String {
    match role {
        RoleRow::Program => "program".to_string(),
        RoleRow::Subcommand { name } => format!("subcommand {name}"),
        RoleRow::Flag {
            spelling, negated, ..
        } => {
            if *negated {
                format!("flag {spelling} (negated)")
            } else {
                format!("flag {spelling}")
            }
        }
        RoleRow::Value {
            name,
            values,
            attached,
        } => {
            let attached = if *attached { ", attached" } else { "" };
            format!("value of {name} = {}{attached}", render_values(values))
        }
        RoleRow::Arg { name, values } => format!("arg {name} = {}", render_values(values)),
        RoleRow::Separator => "separator".to_string(),
        RoleRow::ValueTerminator { ends } => format!("value terminator, ends {ends}"),
        RoleRow::Restart => "restart, positional arguments start over".to_string(),
        RoleRow::UnknownFlag { bound_as } => match bound_as {
            // Under the default `unknown_flags="value"` an unmatched flag-like word is data.
            // Saying which argument took it is the difference between "you have a typo" and
            // "this argument took your typo".
            Some(arg) => format!("unknown flag, bound as {arg}"),
            None => "unknown flag".to_string(),
        },
        RoleRow::Refused { reason } => format!("refused, {reason}"),
        RoleRow::External => "forwarded to an external command".to_string(),
        RoleRow::Unread => "not read".to_string(),
    }
}

fn render_values(values: &[String]) -> String {
    values.iter().map(|value| format!("{value:?}")).join(", ")
}

fn render_origins(origins: &[OriginRow]) -> String {
    if origins.is_empty() {
        // Everything the parser filled has an origin; a bool flag that was simply named has
        // no value to trace, and neither does a value from a source not yet modelled.
        return "-".to_string();
    }
    origins
        .iter()
        .map(|origin| match origin {
            OriginRow::Argv { tokens } => {
                format!("argv [{}]", tokens.iter().map(|i| i.to_string()).join(", "))
            }
            OriginRow::DefaultMissing => "default_missing".to_string(),
            OriginRow::Env { name } => format!("env {name}"),
            OriginRow::Default => "default".to_string(),
            OriginRow::DefaultIf { selector, when } => match when {
                Some(when) => format!("default_if {selector} when={when:?}"),
                None => format!("default_if {selector}"),
            },
        })
        .join(", ")
}

/// The argv positions that supplied a flag, then whatever the fallback phase recorded.
///
/// The token trace is the record of what was typed, so it is scanned rather than duplicated
/// into a second map: a flag's own token counts when the flag holds no separate value, which
/// is how a bool or a counted flag gets an argv origin at all.
fn flag_origins(out: &ParseOutput, flag: &SpecFlag) -> Vec<OriginRow> {
    let mut origins = vec![];
    let mut tokens = vec![];
    for token in &out.tokens {
        for role in &token.roles {
            match role {
                TokenRole::Value { flag: f, .. } if f.name == flag.name => tokens.push(token.index),
                TokenRole::Flag { flag: f, .. } if f.name == flag.name && f.arg.is_none() => {
                    tokens.push(token.index)
                }
                _ => {}
            }
        }
    }
    tokens.dedup();
    if !tokens.is_empty() {
        origins.push(OriginRow::Argv { tokens });
    }
    origins.extend(
        out.flag_origins
            .iter()
            .filter(|(f, _)| f.name == flag.name)
            .flat_map(|(_, recorded)| recorded.iter().map(origin_row)),
    );
    origins
}

fn arg_origins(out: &ParseOutput, arg: &SpecArg) -> Vec<OriginRow> {
    let mut origins = vec![];
    let mut tokens = vec![];
    for token in &out.tokens {
        for role in &token.roles {
            match role {
                TokenRole::Arg { arg: a, .. } if a.name == arg.name => tokens.push(token.index),
                TokenRole::UnknownFlag { bound_as: Some(a) } if a.name == arg.name => {
                    tokens.push(token.index)
                }
                _ => {}
            }
        }
    }
    tokens.dedup();
    if !tokens.is_empty() {
        origins.push(OriginRow::Argv { tokens });
    }
    origins.extend(
        out.arg_origins
            .iter()
            .filter(|(a, _)| a.name == arg.name)
            .flat_map(|(_, recorded)| recorded.iter().map(origin_row)),
    );
    origins
}

fn origin_row(origin: &ValueOrigin) -> OriginRow {
    match origin {
        ValueOrigin::DefaultMissing => OriginRow::DefaultMissing,
        ValueOrigin::Env(name) => OriginRow::Env { name: name.clone() },
        ValueOrigin::Default => OriginRow::Default,
        ValueOrigin::DefaultIf { selector, when } => OriginRow::DefaultIf {
            selector: selector.clone(),
            when: when.clone(),
        },
        // `ValueOrigin` is `#[non_exhaustive]`: a source added to the parser reads as an
        // unnamed default here rather than stopping this from compiling.
        _ => OriginRow::Default,
    }
}

/// The default a flag would supply, from wherever it is declared.
///
/// Two places, and the parser prefers them in this order (`Parser::parse` binds `flag.default`
/// and only then `flag.arg.default`), so a report that read one of them called a shadowed
/// default no default at all — which is the question this table exists to answer.
fn declared_default(flag: &SpecFlag) -> Option<String> {
    if !flag.default.is_empty() {
        return Some(flag.default.join(" "));
    }
    flag.arg
        .as_ref()
        .filter(|arg| !arg.default.is_empty())
        .map(|arg| arg.default.join(" "))
}

/// How a flag is spelled in a report: the long form if it has one, else the short.
///
/// Not [`SpecFlag::usage`], which renders the whole declaration (`-j --jobs <n>`) and is
/// right for help output and too wide for a column.
fn flag_display(flag: &SpecFlag) -> String {
    if let Some(long) = flag.long.first() {
        return format!("--{long}");
    }
    if let Some(short) = flag.short.first() {
        return format!("-{short}");
    }
    flag.name.clone()
}

#[cfg(test)]
mod tests {
    use super::*;

    fn argv(words: &[&str]) -> Vec<String> {
        words.iter().map(|w| (*w).to_string()).collect()
    }

    fn env(pairs: &[(&str, &str)]) -> Option<HashMap<String, String>> {
        Some(
            pairs
                .iter()
                .map(|(k, v)| ((*k).to_string(), (*v).to_string()))
                .collect(),
        )
    }

    fn fixture() -> Spec {
        std::fs::read_to_string(concat!(
            env!("CARGO_MANIFEST_DIR"),
            "/../examples/explain.usage.kdl"
        ))
        .unwrap()
        .parse()
        .unwrap()
    }

    fn roles(explanation: &Explanation, index: usize) -> Vec<String> {
        explanation
            .tokens
            .iter()
            .find(|token| token.index == index)
            .unwrap_or_else(|| panic!("no token at {index}"))
            .roles
            .iter()
            .map(render_role)
            .collect()
    }

    fn value(explanation: &Explanation, name: &str) -> String {
        let row = explanation
            .values
            .iter()
            .find(|row| row.name == name)
            .unwrap_or_else(|| panic!("no value for {name}"));
        format!("{} {}", row.value, render_origins(&row.origins))
    }

    #[test]
    fn the_attached_form_binds_and_the_report_says_where() {
        let spec = fixture();
        let explanation = explain(
            &spec,
            &argv(&["mycli", "--env=prod", "build", "a"]),
            env(&[]),
        );

        // jdx/mise discussion #8883: mise's hand-written scanner ignored `--env=production`
        // while `--env production` worked, and nothing would show the difference.
        assert_eq!(
            roles(&explanation, 1),
            ["flag --env", "value of env = \"prod\", attached"]
        );
        assert_eq!(value(&explanation, "env"), "prod argv [1]");
    }

    #[test]
    fn a_short_bundle_reads_as_one_token() {
        let spec = fixture();
        let explanation = explain(&spec, &argv(&["mycli", "-sj8", "build", "a"]), env(&[]));

        assert_eq!(
            roles(&explanation, 1),
            ["flag -s", "flag -j", "value of jobs = \"8\", attached"]
        );
        // Not synthesized: `-sj8` is the word the caller wrote, and its tails are
        // continuations of it. That flag is for a position where the parser read something
        // else entirely, which is the multicall case.
        assert!(!explanation.tokens[1].synthesized);
        assert_eq!(explanation.tokens.len(), 4);
    }

    #[test]
    fn a_multicall_applet_name_is_marked_read_rather_than_written() {
        let spec: Spec = "name \"mycli\"\nbin \"mycli\"\nmulticall #true\ncmd \"build\"\n"
            .parse()
            .unwrap();

        // Invoked through a symlink named `build`, argv[0] is both the program and the word
        // that selected the subcommand — and the caller wrote neither of those roles as a
        // word of its own.
        let explanation = explain(&spec, &argv(&["build"]), env(&[]));

        assert_eq!(roles(&explanation, 0), ["program", "subcommand build"]);
        assert!(explanation.tokens[0].synthesized);
    }

    #[test]
    fn a_parse_that_cannot_start_reports_the_refusal_alone() {
        let spec: Spec = "name \"mycli\"\nbin \"mycli\"\n".parse().unwrap();

        // Nothing declared can take `boom`, and the binding phase refuses it too — so there
        // is no partial parse to describe and the report is the refusal by itself.
        let explanation = explain(&spec, &argv(&["mycli", "boom"]), env(&[]));

        assert!(explanation.tokens.is_empty(), "{:?}", explanation.tokens);
        assert!(!explanation.fallbacks_applied);
        assert!(explanation.refused.is_some());
    }

    #[test]
    fn a_separator_is_not_a_value_and_what_follows_it_is() {
        let spec = fixture();
        let explanation = explain(
            &spec,
            &argv(&["mycli", "build", "a", "--", "--raw"]),
            env(&[]),
        );

        assert_eq!(roles(&explanation, 3), ["separator"]);
        assert_eq!(roles(&explanation, 4), ["arg extra = \"--raw\""]);
    }

    #[test]
    fn an_env_value_names_the_variable() {
        let spec = fixture();
        let explanation = explain(
            &spec,
            &argv(&["mycli", "build", "a"]),
            env(&[("MYCLI_COLOR", "never")]),
        );

        assert_eq!(value(&explanation, "color"), "never env MYCLI_COLOR");
    }

    #[test]
    fn a_shadowed_default_says_what_beat_it() {
        let spec = fixture();
        let explanation = explain(
            &spec,
            &argv(&["mycli", "-j", "8", "build", "a"]),
            env(&[("MYCLI_COLOR", "never")]),
        );

        let shadowed = explanation
            .shadowed
            .iter()
            .map(|row| {
                format!(
                    "{} {} {}",
                    row.display,
                    row.value,
                    render_origins(&row.lost_to)
                )
            })
            .collect::<Vec<_>>();
        assert!(
            shadowed.contains(&"--jobs 1 argv [2]".to_string()),
            "{shadowed:?}"
        );
        assert!(
            shadowed.contains(&"--color auto env MYCLI_COLOR".to_string()),
            "{shadowed:?}"
        );
    }

    #[test]
    fn a_default_if_says_which_condition_fired() {
        let spec = fixture();
        let explanation = explain(
            &spec,
            &argv(&["mycli", "--profile", "prod", "build", "a"]),
            env(&[]),
        );

        assert_eq!(
            value(&explanation, "strict"),
            "true default_if --profile when=\"prod\""
        );
    }

    #[test]
    fn an_unknown_flag_says_what_took_it() {
        let spec = fixture();
        let explanation = explain(&spec, &argv(&["mycli", "build", "--wat"]), env(&[]));

        // Lax unknown flags are the default, so the word became data. Which is the useful
        // thing to be told: the other reading is "you have a typo".
        assert_eq!(roles(&explanation, 2), ["unknown flag, bound as target"]);
    }

    #[test]
    fn a_word_offered_to_an_argument_that_refused_it_says_so() {
        let spec = fixture();
        let explanation = explain(&spec, &argv(&["mycli", "build", "a", "b"]), env(&[]));

        // `extra` only accepts words after `--`, so `b` was dropped. Reporting the token as
        // having done nothing would hide the one thing that happened to it.
        assert_eq!(
            roles(&explanation, 3),
            ["refused, extra only accepts words after `--`"]
        );
    }

    #[test]
    fn a_command_line_that_fails_still_gets_a_report() {
        let spec = fixture();
        let explanation = explain(&spec, &argv(&["mycli", "--env=prod", "build"]), env(&[]));

        // The bindings that worked, then the complaint — rather than the complaint alone,
        // which is the report the caller already had.
        assert_eq!(value(&explanation, "env"), "prod argv [1]");
        assert!(
            explanation.errors.iter().any(|e| e.contains("target")),
            "{:?}",
            explanation.errors
        );
        assert!(explanation.refused.is_none());
        assert!(explanation.fallbacks_applied);
    }

    #[test]
    fn a_bare_invocation_explains_the_fallbacks_alone() {
        let spec = fixture();
        let explanation = explain(&spec, &argv(&["mycli"]), env(&[("MYCLI_COLOR", "never")]));

        assert_eq!(roles(&explanation, 0), ["program"]);
        assert_eq!(value(&explanation, "color"), "never env MYCLI_COLOR");
        assert_eq!(value(&explanation, "jobs"), "1 default");
    }

    #[test]
    fn the_json_shape_carries_the_same_facts() {
        let spec = fixture();
        let explanation = explain(
            &spec,
            &argv(&["mycli", "--env=prod", "build", "a"]),
            env(&[]),
        );
        let json = serde_json::to_value(&explanation).unwrap();

        assert_eq!(json["tokens"][1]["roles"][0]["kind"], "flag");
        assert_eq!(json["tokens"][1]["roles"][1]["kind"], "value");
        assert_eq!(json["tokens"][1]["roles"][1]["attached"], true);
        let env_row = json["values"]
            .as_array()
            .unwrap()
            .iter()
            .find(|row| row["name"] == "env")
            .unwrap()
            .clone();
        assert_eq!(env_row["origins"][0]["kind"], "argv");
        assert_eq!(env_row["origins"][0]["tokens"][0], 1);
    }
}
