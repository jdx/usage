//! A registry written back out as the spec's `config` block.
//!
//! A CLI declares its settings in code with `#[derive(usage::Config)]`, and this is the way
//! back out: the registry it derived, rendered as the `config { prop … }` block the spec
//! grammar defines, so docs, JSON schema, completions and every other spec consumer read
//! declarations made in Rust exactly as they read ones made in KDL.
//!
//! Written by hand rather than through a KDL library for the same reason the argv crate's
//! spec writer is: this crate has no dependencies, and the grammar being emitted is the small
//! fixed one the spec parser defines.

use crate::registry::{Merge, PropMeta, Scope};
use crate::value::Const;
use std::fmt::Write;

/// The spec `config` block for these settings, as KDL.
///
/// Ends with a newline, so it can be appended to an emitted spec as-is. Props are written in
/// registry order, which for a derived registry is declaration order.
pub fn spec_kdl(props: &[PropMeta]) -> String {
    let mut out = String::from("config {\n");
    for meta in props {
        let _ = write_prop(&mut out, meta);
    }
    out.push_str("}\n");
    out
}

fn write_prop(out: &mut String, meta: &PropMeta) -> std::fmt::Result {
    write!(
        out,
        "    prop {} type={}",
        quoted(meta.key),
        quoted(&meta.ty.name())
    )?;
    if let Some(default) = scalar_default(meta.default) {
        write!(out, " default={default}")?;
    }
    if let Some(note) = meta.default_note {
        write!(out, " default_note={}", quoted(note))?;
    }
    if let Some(optional) = meta.optional {
        write!(out, " optional=#{optional}")?;
    }
    match meta.merge {
        Merge::Replace => {}
        Merge::Union => out.push_str(" merge=\"union\""),
        Merge::Deep => out.push_str(" merge=\"deep\""),
    }
    if let Some(parse) = meta.parse {
        write!(out, " parse={}", quoted(parse.name()))?;
    }
    match meta.scope {
        Scope::Any => {}
        Scope::Global => out.push_str(" scope=\"global\""),
        Scope::Env => out.push_str(" scope=\"env\""),
    }
    if meta.hide {
        out.push_str(" hide=#true");
    }
    if let Some(deprecated) = meta.deprecated {
        write!(out, " deprecated={}", quoted(deprecated))?;
    }
    if let Some(at) = meta.deprecated_warn_at {
        write!(out, " deprecated_warn_at={}", quoted(at))?;
    }
    if let Some(at) = meta.deprecated_remove_at {
        write!(out, " deprecated_remove_at={}", quoted(at))?;
    }
    if let Some(renamed_to) = meta.renamed_to {
        write!(out, " renamed_to={}", quoted(renamed_to))?;
    }
    if let Some(since) = meta.since {
        write!(out, " since={}", quoted(since))?;
    }
    if let Some(help) = meta.help {
        write!(out, " help={}", quoted(help))?;
    }
    if let Some(long_help) = meta.long_help {
        write!(out, " long_help={}", quoted(long_help))?;
    }

    let mut children = Vec::new();
    if let Some(Const::List(items)) = meta.default {
        // A list default is a child node — `default 80 443` — because several values do not
        // fit one `default=` entry.
        let rendered: Vec<String> = items.iter().map(|item| const_kdl(*item)).collect();
        children.push(format!("default {}", rendered.join(" ")));
    }
    if !meta.envs.is_empty() {
        children.push(word_list("env", meta.envs));
    }
    if !meta.deprecated_envs.is_empty() {
        children.push(word_list("deprecated_env", meta.deprecated_envs));
    }
    if !meta.aliases.is_empty() {
        children.push(word_list("alias", meta.aliases));
    }
    if !meta.cli.is_empty() {
        children.push(word_list("cli", meta.cli));
    }
    for example in meta.examples {
        children.push(format!("example {}", quoted(example)));
    }
    // One `source` node per kind, holding every key bound in it, in declaration order —
    // `source "pkl" "exclude" "defaults.exclude"`.
    let mut kinds: Vec<&str> = Vec::new();
    for (kind, _) in meta.bindings {
        if !kinds.contains(kind) {
            kinds.push(kind);
        }
    }
    for kind in kinds {
        let keys: Vec<String> = meta
            .bindings
            .iter()
            .filter(|(k, _)| *k == kind)
            .map(|(_, key)| quoted(key))
            .collect();
        children.push(format!("source {} {}", quoted(kind), keys.join(" ")));
    }
    // One `choice` node carries one value, so only a scalar belongs here. A registry written
    // by hand can hold a list or a table; rendering one produced `choice 1 2` or a bare
    // `choice`, neither of which the prop grammar can read back.
    let choices: Vec<String> = meta
        .choices
        .iter()
        .filter(|choice| !matches!(choice, Const::List(_) | Const::Map(_)))
        .map(|choice| const_kdl(*choice))
        .collect();
    if !choices.is_empty() {
        let mut block = String::from("choices {\n");
        for choice in choices {
            let _ = writeln!(block, "            choice {choice}");
        }
        block.push_str("        }");
        children.push(block);
    }

    if children.is_empty() {
        out.push('\n');
    } else {
        out.push_str(" {\n");
        for child in children {
            let _ = writeln!(out, "        {child}");
        }
        out.push_str("    }\n");
    }
    Ok(())
}

/// A scalar default as a KDL entry value, or `None` for a list (a child node) or a table
/// (which the prop grammar cannot hold, and a generator refuses before it gets here).
fn scalar_default(default: Option<Const>) -> Option<String> {
    match default? {
        Const::List(_) | Const::Map(_) => None,
        scalar => Some(const_kdl(scalar)),
    }
}

/// One constant as KDL writes it: `#true`, `4`, `1.5`, `"text"`.
fn const_kdl(value: Const) -> String {
    match value {
        Const::Bool(b) => format!("#{b}"),
        Const::Int(i) => i.to_string(),
        // `{:?}` keeps the decimal point, which KDL requires of a float — `1.0`, not `1`.
        Const::Float(f) => format!("{f:?}"),
        Const::Str(s) => quoted(s),
        Const::List(items) => items
            .iter()
            .map(|item| const_kdl(*item))
            .collect::<Vec<_>>()
            .join(" "),
        // Unreachable from a derived registry — the derive refuses table defaults — and not
        // something the prop grammar can spell.
        Const::Map(_) => String::new(),
    }
}

fn word_list(name: &str, words: &[&str]) -> String {
    let quoted: Vec<String> = words.iter().map(|word| quoted(word)).collect();
    format!("{name} {}", quoted.join(" "))
}

/// `text` as a quoted KDL string.
fn quoted(text: &str) -> String {
    let mut out = String::with_capacity(text.len() + 2);
    out.push('"');
    for c in text.chars() {
        match c {
            '\\' => out.push_str("\\\\"),
            '"' => out.push_str("\\\""),
            '\n' => out.push_str("\\n"),
            '\r' => out.push_str("\\r"),
            '\t' => out.push_str("\\t"),
            // KDL forbids a raw control character in a document at all, so the rest go through
            // the escape it does spell. A `help` string that carried an ANSI escape wrote a
            // block no parser would read back.
            c if c.is_control() => {
                let _ = write!(out, "\\u{{{:x}}}", c as u32);
            }
            c => out.push(c),
        }
    }
    out.push('"');
    out
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::ty::{Parser, Ty};

    #[test]
    fn a_registry_renders_as_the_config_block_the_spec_grammar_defines() {
        static PROPS: &[PropMeta] = &[
            PropMeta {
                default: Some(Const::Int(4)),
                default_note: Some("0 = one per core"),
                envs: &["HK_JOBS", "HK_JOB"],
                deprecated_envs: &["HK_JOBS_OLD"],
                cli: &["--jobs", "-j"],
                bindings: &[("git", "hk.jobs")],
                help: Some("How many jobs to run at once"),
                ..PropMeta::new("jobs", Ty::Uint)
            },
            PropMeta {
                merge: Merge::Union,
                parse: Some(Parser::ListByComma),
                envs: &["HK_EXCLUDE"],
                bindings: &[("pkl", "exclude"), ("pkl", "defaults.exclude")],
                ..PropMeta::new("exclude", Ty::List(&Ty::String))
            },
            PropMeta {
                default: Some(Const::Str("git")),
                choices: &[
                    Const::Str("git"),
                    Const::Str("patch-file"),
                    Const::Str("none"),
                ],
                help: Some("How to \"stash\" first"),
                ..PropMeta::new("stash", Ty::String)
            },
            PropMeta {
                default: Some(Const::List(&[Const::Int(80), Const::Int(443)])),
                ..PropMeta::new("ports", Ty::List(&Ty::Uint))
            },
            PropMeta {
                scope: Scope::Env,
                hide: true,
                envs: &["CI"],
                ..PropMeta::new("ci", Ty::Bool)
            },
            // The rest of the vocabulary, in one prop: where each of these lands — an entry on
            // the `prop` node, or a child of it — is exactly what a golden string is for.
            PropMeta {
                optional: Some(true),
                aliases: &["fail-fast.legacy", "failfast"],
                examples: &["true", "false"],
                deprecated: Some("use `stop-on-error`"),
                deprecated_warn_at: Some("6.0.0"),
                deprecated_remove_at: Some("7.0.0"),
                since: Some("5.2.0"),
                help: Some("Stop at the first failure"),
                long_help: Some("Whether a failing job stops the rest."),
                ..PropMeta::new("fail_fast", Ty::Option(&Ty::Bool))
            },
            // A `choice` node carries one value, and a registry written by hand can hold a
            // list where one belongs. The scalar survives; the list is not something the prop
            // grammar can spell, so it is left out rather than written as two arguments.
            PropMeta {
                choices: &[
                    Const::Str("plain"),
                    Const::List(&[Const::Int(1), Const::Int(2)]),
                ],
                ..PropMeta::new("level", Ty::Any)
            },
        ];
        let kdl = spec_kdl(PROPS);
        assert_eq!(
            kdl,
            r#"config {
    prop "jobs" type="uint" default=4 default_note="0 = one per core" help="How many jobs to run at once" {
        env "HK_JOBS" "HK_JOB"
        deprecated_env "HK_JOBS_OLD"
        cli "--jobs" "-j"
        source "git" "hk.jobs"
    }
    prop "exclude" type="list<string>" merge="union" parse="list_by_comma" {
        env "HK_EXCLUDE"
        source "pkl" "exclude" "defaults.exclude"
    }
    prop "stash" type="string" default="git" help="How to \"stash\" first" {
        choices {
            choice "git"
            choice "patch-file"
            choice "none"
        }
    }
    prop "ports" type="list<uint>" {
        default 80 443
    }
    prop "ci" type="bool" scope="env" hide=#true {
        env "CI"
    }
    prop "fail_fast" type="option<bool>" optional=#true deprecated="use `stop-on-error`" deprecated_warn_at="6.0.0" deprecated_remove_at="7.0.0" since="5.2.0" help="Stop at the first failure" long_help="Whether a failing job stops the rest." {
        alias "fail-fast.legacy" "failfast"
        example "true"
        example "false"
    }
    prop "level" type="any" {
        choices {
            choice "plain"
        }
    }
}
"#
        );
    }

    /// KDL forbids a raw control character anywhere in a document, so a value carrying one has
    /// to go out as the escape KDL does spell — or the block this renders is one no parser,
    /// including usage's own, will read back.
    #[test]
    fn a_control_character_in_a_value_is_escaped_rather_than_written() {
        static PROPS: &[PropMeta] = &[PropMeta {
            help: Some("plain\u{1b}[0m and \u{0}"),
            ..PropMeta::new("color", Ty::Bool)
        }];
        assert_eq!(
            spec_kdl(PROPS),
            "config {\n    prop \"color\" type=\"bool\" help=\"plain\\u{1b}[0m and \\u{0}\"\n}\n"
        );
    }

    /// A prop whose only choices are shapes the grammar cannot spell gets no `choices` block at
    /// all, rather than one holding a node with no argument.
    #[test]
    fn choices_no_single_value_can_hold_leave_no_block_behind() {
        static PROPS: &[PropMeta] = &[PropMeta {
            choices: &[Const::Map(&[("a", Const::Int(1))])],
            ..PropMeta::new("shape", Ty::Any)
        }];
        assert_eq!(
            spec_kdl(PROPS),
            "config {\n    prop \"shape\" type=\"any\"\n}\n"
        );
    }
}
