//! Building usage-argv's tables from a [`Spec`].
//!
//! usage-argv reads `static` tables a derive macro emits. A corpus has a spec instead, so
//! this builds the same shapes at run time — both of them: the hot parse tables a binding
//! reads, and the cold metadata everything else does. One builder rather than two, because
//! the metadata borrows the parse-table entry it describes and the two must agree about which
//! flag is which.
//!
//! The tables are leaked. They must outlive the parse and be `'static`-shaped, and a test
//! process that builds a handful of small tables and exits is the one place where leaking is
//! the simplest correct answer. Generated code has no such problem: its tables really are
//! `static`.
//!
//! This is deliberately *not* a general-purpose bridge. It exists so the corpus can ask
//! usage-argv the questions it asks usage-lib; a program wanting a parser for a spec it read
//! at run time should use usage-lib, which is built for exactly that.
//!
//! # Every literal here is exhaustive, on purpose
//!
//! Nothing below ends in `..EMPTY`, so a new field in usage-argv's model breaks this file
//! until somebody says what the spec puts there. That is the point: this is a mirror, and a
//! mirror that quietly defaults a field describes a CLI that is not the one the spec declares.
//! `Spec::usage` arrived this way — the build broke, carrying it took one line, and doing so
//! turned up a rule the two implementations disagree about that nothing had recorded.

use usage::spec::cmd::SpecExample;
use usage::{Spec, SpecArg, SpecCommand, SpecFlag};
use usage_argv::spec::{ArgMeta, CommandMeta, Effect, Example, FlagMeta};
use usage_argv::{Arg, Command, DoubleDash, Flag, UnknownFlags as ArgvUnknownFlags};

/// A command's two tables, built together so the metadata can borrow the parse table.
pub struct Built {
    /// What a binding reads.
    pub cmd: &'static Command<'static>,
    /// What help, completions and spec emission read.
    pub meta: &'static CommandMeta<'static>,
}

/// The spec's spelling of the setting, in the parser's terms.
pub fn convert_unknown_flags(mode: usage::UnknownFlags) -> ArgvUnknownFlags {
    match mode {
        usage::UnknownFlags::Value => ArgvUnknownFlags::Value,
        usage::UnknownFlags::Error => ArgvUnknownFlags::Error,
    }
}

/// Build leaked tables mirroring a spec command.
///
/// `root_unknown_flags` is carried through as the spec states it — `None` where a command says
/// nothing — because the parser inherits it. The root takes the spec-level setting, since that
/// is the command a spec's own property describes.
pub fn build(cmd: &SpecCommand, root_unknown_flags: Option<ArgvUnknownFlags>) -> Built {
    let unknown_flags = cmd
        .unknown_flags
        .map(convert_unknown_flags)
        .or(root_unknown_flags);

    let flags: Vec<&'static Flag<'static>> = cmd.flags.iter().map(build_flag).collect();
    let args: Vec<&'static Arg<'static>> = cmd.args.iter().map(build_arg).collect();
    let subs: Vec<Built> = cmd
        .subcommands
        .values()
        // A subcommand states its own or says nothing; there is no spec-level setting to hand
        // it, since the root has already taken that.
        .map(|sub| build(sub, None))
        .collect();

    let aliases: Vec<&'static str> = cmd
        .aliases
        .iter()
        .chain(cmd.hidden_aliases.iter())
        .map(|a| leak(a))
        .collect();

    let table: &'static Command<'static> = Box::leak(Box::new(Command {
        name: leak(&cmd.name),
        aliases: Box::leak(aliases.into_boxed_slice()),
        flags: Box::leak(flags.clone().into_boxed_slice()),
        args: Box::leak(args.clone().into_boxed_slice()),
        subcommands: Box::leak(
            subs.iter()
                .map(|s| s.cmd)
                .collect::<Vec<_>>()
                .into_boxed_slice(),
        ),
        // Both filled in by the caller for the root, which is the only place a spec declares
        // either.
        default_subcommand: None,
        version: false,
        unknown_flags,
        key: 0,
    }));

    let flag_metas: Vec<FlagMeta<'static>> = cmd
        .flags
        .iter()
        .zip(&flags)
        .map(|(f, table)| flag_meta(f, table))
        .collect();
    let arg_metas: Vec<ArgMeta<'static>> = cmd
        .args
        .iter()
        .zip(&args)
        .map(|(a, table)| arg_meta(a, table))
        .collect();

    let meta: &'static CommandMeta<'static> = Box::leak(Box::new(CommandMeta {
        cmd: table,
        about: opt(&cmd.help),
        long_about: opt(&cmd.help_long),
        hidden_aliases: Box::leak(
            cmd.hidden_aliases
                .iter()
                .map(|a| leak(a))
                .collect::<Vec<_>>()
                .into_boxed_slice(),
        ),
        hide: cmd.hide,
        effect: cmd.effect.map(effect),
        // A command carries at most one mount in the tables; a spec may list several, and the
        // first is the one the tables can hold.
        mount: cmd.mounts.first().map(|m| leak(&m.run)),
        restart_token: opt(&cmd.restart_token),
        subcommand_required: cmd.subcommand_required,
        before_help: opt(&cmd.before_help),
        before_long_help: opt(&cmd.before_help_long),
        after_help: opt(&cmd.after_help),
        after_long_help: opt(&cmd.after_help_long),
        examples: examples(&cmd.examples),
        flags: Box::leak(flag_metas.into_boxed_slice()),
        args: Box::leak(arg_metas.into_boxed_slice()),
        subcommands: Box::leak(
            subs.iter()
                .map(|s| s.meta)
                .collect::<Vec<_>>()
                .into_boxed_slice(),
        ),
    }));

    Built { cmd: table, meta }
}

/// The whole spec, as usage-argv's cold model of one.
///
/// A KDL spec has one place for surrounding text and examples — the top level — and the two
/// implementations put them in different places: usage-lib keeps them on the `Spec` and
/// usage-argv on the root's metadata, where its renderer reads them both as the root's own and
/// as the default for every other page. So they are folded here. The help texts fold root
/// first, a root command that says something of its own keeping it; the examples concatenate,
/// which is the same thing in practice — `example` at the top level parses onto
/// `Spec::examples` and leaves `spec.cmd.examples` empty, so at most one side is ever filled.
///
/// Examples were dropped on the way through to begin with, which cost every page its Examples
/// section — the one the reference still rendered from the same spec. A fold that copied only
/// the help texts lost them silently, and `render/03-sections.json` pins all three cases now:
/// the root's own page, a page that falls back to them, and a page that has its own instead.
pub fn build_spec(spec: &Spec) -> &'static usage_argv::spec::Spec<'static> {
    let root = build(&spec.cmd, spec.unknown_flags.map(convert_unknown_flags));
    // Whether the parser answers `--version` here, which the derive sets on the root of a CLI
    // that declares one. It has to be on the *table*, not only on the spec: a page offers
    // `--version` where the parser accepts it, and one that offered it otherwise would be
    // describing a flag that never binds.
    let root_cmd: &'static Command<'static> = Box::leak(Box::new(Command {
        version: spec.version.is_some(),
        ..*root.cmd
    }));
    let mut root_examples = root.meta.examples.to_vec();
    root_examples.extend(spec.examples.iter().map(example));
    let root_meta: &'static CommandMeta<'static> = Box::leak(Box::new(CommandMeta {
        cmd: root_cmd,
        before_help: root.meta.before_help.or(opt(&spec.before_help)),
        before_long_help: root.meta.before_long_help.or(opt(&spec.before_help_long)),
        after_help: root.meta.after_help.or(opt(&spec.after_help)),
        after_long_help: root.meta.after_long_help.or(opt(&spec.after_help_long)),
        examples: Box::leak(root_examples.into_boxed_slice()),
        ..*root.meta
    }));
    Box::leak(Box::new(usage_argv::spec::Spec {
        name: leak(&spec.name),
        bin: Some(leak(&spec.bin)),
        version: opt(&spec.version),
        min_usage_version: opt(&spec.min_usage_version),
        about: opt(&spec.about),
        long_about: opt(&spec.about_long),
        default_subcommand: opt(&spec.default_subcommand),
        // An exact synopsis the spec declares, which replaces the generated line on the root's
        // page. usage-lib's manpage renderer honours it and its help renderer does not, so a
        // spec that declares one is a case the two disagree about; `render/03-sections.json`
        // records it rather than this quietly declining to carry it.
        usage: (!spec.usage.trim().is_empty()).then(|| leak(spec.usage.trim())),
        root: root_meta,
    }))
}

fn build_flag(f: &SpecFlag) -> &'static Flag<'static> {
    let longs: Vec<&'static str> = f.long.iter().map(|l| leak(l)).collect();
    let shorts: Vec<u8> = f.short.iter().map(|c| *c as u8).collect();
    Box::leak(Box::new(Flag {
        key: 0,
        name: leak(&f.name),
        longs: Box::leak(longs.into_boxed_slice()),
        shorts: Box::leak(shorts.into_boxed_slice()),
        // usage-lib stores the negation with its dashes; the table wants the bare name.
        negate: f.negate.as_ref().map(|n| leak(n.trim_start_matches('-'))),
        takes_value: f.arg.is_some(),
        // Only a variadic *argument* is greedy. A `var` flag with a single-value argument is
        // repeatable instead: one value per occurrence, which the parser gets by not
        // collecting.
        variadic: f.arg.as_ref().is_some_and(|a| a.var),
        // The bound on one occurrence's values, which is the argument's. A repeatable flag's
        // own `var_max` counts occurrences and is checked after the parse, so it does not
        // belong in this table.
        var_max: f
            .arg
            .as_ref()
            .filter(|a| a.var)
            .and_then(|a| a.var_max)
            // Saturating rather than truncating: `4294967296 as u32` is zero, which would read
            // as "stop at once" rather than "no real limit".
            .map(|max| u32::try_from(max).unwrap_or(u32::MAX)),
        global: f.global,
    }))
}

fn build_arg(a: &SpecArg) -> &'static Arg<'static> {
    Box::leak(Box::new(Arg {
        key: 0,
        name: leak(&a.name),
        var: a.var,
        var_max: a
            .var_max
            .filter(|_| a.var)
            .map(|max| u32::try_from(max).unwrap_or(u32::MAX)),
        double_dash: double_dash(&a.double_dash),
    }))
}

fn flag_meta(f: &SpecFlag, table: &'static Flag<'static>) -> FlagMeta<'static> {
    let arg = f.arg.as_ref();
    FlagMeta {
        flag: table,
        help: opt(&f.help),
        long_help: opt(&f.help_long),
        value_name: arg.map(|a| leak(&a.name)),
        // The value's own bracket bit, folded with the value's own default the way usage-lib
        // folds a positional's — a default declared on the *flag* is a different statement and
        // stays in `default` below.
        value_required: arg.is_none_or(|a| a.required && a.default.is_empty()),
        env: opt(&f.env),
        default: strs(&f.default),
        choices: arg
            .and_then(|a| a.choices.as_ref())
            .map(|c| strs(&c.choices))
            .unwrap_or(&[]),
        required: f.required,
        hide: f.hide,
        count: f.count,
        repeatable: f.var,
        var_min: f.var_min.or(arg.and_then(|a| a.var_min)),
        var_max: f.var_max.or(arg.and_then(|a| a.var_max)),
        overrides: strs(&f.overrides),
        conflicts: strs(&f.conflicts),
        requires: strs(&f.requires),
        required_if: strs(&f.required_if),
        required_unless: strs(&f.required_unless),
        help_heading: opt(&f.help_heading),
        effect: f.effect.map(effect),
        ..FlagMeta::EMPTY
    }
}

fn arg_meta(a: &SpecArg, table: &'static Arg<'static>) -> ArgMeta<'static> {
    ArgMeta {
        arg: table,
        help: opt(&a.help),
        long_help: opt(&a.help_long),
        env: opt(&a.env),
        default: strs(&a.default),
        choices: a.choices.as_ref().map(|c| strs(&c.choices)).unwrap_or(&[]),
        required: a.required,
        hide: a.hide,
        var_min: a.var_min,
        var_max: a.var_max,
        help_heading: opt(&a.help_heading),
        ..ArgMeta::EMPTY
    }
}

fn double_dash(mode: &usage::SpecDoubleDashChoices) -> DoubleDash {
    match mode {
        usage::SpecDoubleDashChoices::Required => DoubleDash::Required,
        usage::SpecDoubleDashChoices::Preserve => DoubleDash::Preserve,
        usage::SpecDoubleDashChoices::Automatic => DoubleDash::Automatic,
        _ => DoubleDash::Optional,
    }
}

fn effect(effect: usage::SpecCommandEffect) -> Effect {
    match effect {
        usage::SpecCommandEffect::Read => Effect::Read,
        usage::SpecCommandEffect::Write => Effect::Write,
        usage::SpecCommandEffect::Destructive => Effect::Destructive,
    }
}

fn opt(s: &Option<String>) -> Option<&'static str> {
    s.as_deref().map(leak)
}

fn strs(list: &[String]) -> &'static [&'static str] {
    Box::leak(
        list.iter()
            .map(|s| leak(s))
            .collect::<Vec<_>>()
            .into_boxed_slice(),
    )
}

fn example(e: &SpecExample) -> Example<'static> {
    Example {
        code: leak(&e.code),
        header: opt(&e.header),
        help: opt(&e.help),
    }
}

fn examples(list: &[SpecExample]) -> &'static [Example<'static>] {
    Box::leak(
        list.iter()
            .map(example)
            .collect::<Vec<_>>()
            .into_boxed_slice(),
    )
}

pub fn leak(s: &str) -> &'static str {
    Box::leak(s.to_string().into_boxed_str())
}
