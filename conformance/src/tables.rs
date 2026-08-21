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
use usage::{Spec, SpecArg, SpecChoices, SpecCommand, SpecComplete, SpecFlag, SpecGroup};
use usage_argv::policy::{ColorRole, Verbosity, VerbosityRole};
use usage_argv::spec::{
    ArgMeta, ChoiceAliasMeta, ChoiceMeta, CommandMeta, DefaultIf, Effect, Example, FlagMeta,
    GroupMeta, RequiredIfEq, RequiresIf,
};
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
///
/// `spec_completers` are the spec's top-level `complete` nodes, which every command sees: the
/// reference looks a completer up spec-level first and only then on the command
/// (`cli/src/cli/complete_word.rs`), so they are handed down the tree in that order rather than
/// folded onto the root. fnox is the fleet's proof that this matters — its `complete "key"` is
/// written once at the top level and means the `<KEY>` argument of a dozen subcommands.
pub fn build(
    cmd: &SpecCommand,
    root_unknown_flags: Option<ArgvUnknownFlags>,
    spec_completers: &[&SpecComplete],
) -> Built {
    let unknown_flags = cmd
        .unknown_flags
        .map(convert_unknown_flags)
        .or(root_unknown_flags);
    // Spec-level first, so that the first match wins in the reference's own order of preference.
    let completers: Vec<&SpecComplete> = spec_completers
        .iter()
        .copied()
        .chain(cmd.complete.values())
        .collect();

    let flags: Vec<&'static Flag<'static>> = cmd.flags.iter().map(build_flag).collect();
    let args: Vec<&'static Arg<'static>> = cmd.args.iter().map(build_arg).collect();
    let subs: Vec<Built> = cmd
        .subcommands
        .values()
        // A subcommand states its own or says nothing; there is no spec-level setting to hand
        // it, since the root has already taken that. The completers do carry down, because the
        // reference resolves them for whichever command is being completed.
        .map(|sub| build(sub, None, spec_completers))
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
        disable_help_flag: cmd.disable_help_flag,
        disable_help_subcommand: cmd.disable_help_subcommand,
        disable_version_flag: cmd.disable_version_flag,
        unknown_flags,
        external_subcommand: cmd.external_subcommand,
        arg_required_else_help: cmd.arg_required_else_help,
        subcommand_negates_reqs: cmd.subcommand_negates_reqs,
        args_conflicts_with_subcommands: cmd.args_conflicts_with_subcommands,
        subcommand_precedence_over_arg: cmd.subcommand_precedence_over_arg,
        allow_missing_positional: cmd.allow_missing_positional,
        dont_delimit_trailing_values: cmd.dont_delimit_trailing_values,
        key: 0,
    }));

    let flag_metas: Vec<FlagMeta<'static>> = cmd
        .flags
        .iter()
        .zip(&flags)
        .map(|(f, table)| flag_meta(f, table, &completers))
        .collect();
    let arg_metas: Vec<ArgMeta<'static>> = cmd
        .args
        .iter()
        .zip(&args)
        .map(|(a, table)| arg_meta(a, table, &completers))
        .collect();

    let meta: &'static CommandMeta<'static> = Box::leak(Box::new(CommandMeta {
        cmd: table,
        about: opt(&cmd.help),
        long_about: opt(&cmd.help_long),
        deprecated: opt(&cmd.deprecated),
        deprecated_warn_at: opt(&cmd.deprecated_warn_at),
        deprecated_remove_at: opt(&cmd.deprecated_remove_at),
        hidden_aliases: Box::leak(
            cmd.hidden_aliases
                .iter()
                .map(|a| leak(a))
                .collect::<Vec<_>>()
                .into_boxed_slice(),
        ),
        hide: cmd.hide,
        display_order: cmd.display_order,
        help_heading: opt(&cmd.help_heading),
        effect: cmd.effect.map(effect),
        // A command carries at most one mount in the tables; a spec may list several, and the
        // first is the one the tables can hold.
        mount: cmd.mounts.first().map(|m| leak(&m.run)),
        restart_token: opt(&cmd.restart_token),
        subcommand_required: cmd.subcommand_required,
        subcommand_help_heading: opt(&cmd.subcommand_help_heading),
        subcommand_value_name: opt(&cmd.subcommand_value_name),
        next_line_help: cmd.next_line_help,
        flatten_help: cmd.flatten_help,
        term_width: cmd.term_width,
        max_term_width: cmd.max_term_width,
        args_override_self: cmd.args_override_self,
        before_help: opt(&cmd.before_help),
        before_long_help: opt(&cmd.before_help_long),
        after_help: opt(&cmd.after_help),
        after_long_help: opt(&cmd.after_help_long),
        examples: examples(&cmd.examples),
        groups: groups(&cmd.groups),
        flags: Box::leak(flag_metas.into_boxed_slice()),
        args: Box::leak(arg_metas.into_boxed_slice()),
        // A spec read from KDL has already had its `use` nodes resolved, so there is no seam
        // left to record: these tables come from flags, not from the struct that declared them.
        flatten_groups: &[],
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
    // The third thing a spec writes at the top level and hangs off `Spec` rather than off the
    // root command. Unlike the other two it is not the root's to keep: `build` hands it down to
    // every command, because that is where the reference looks for it.
    let spec_completers: Vec<&SpecComplete> = spec.complete.values().collect();
    let root = build(
        &spec.cmd,
        spec.unknown_flags.map(convert_unknown_flags),
        &spec_completers,
    );
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
        long_version: opt(&spec.long_version),
        author: opt(&spec.author),
        license: opt(&spec.license),
        repository: opt(&spec.repository),
        source_code_link_template: opt(&spec.source_code_link_template),
        min_usage_version: opt(&spec.min_usage_version),
        about: opt(&spec.about),
        long_about: opt(&spec.about_long),
        default_subcommand: opt(&spec.default_subcommand),
        multicall: spec.multicall,
        views: &[],
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
    // A short flag is one byte in the table, so a non-ASCII spelling has no representation there
    // at all: the line arrives as UTF-8, where such a character is two bytes or more, and
    // whatever single byte a cast produced would match nothing anybody could type. Refusing says
    // so; `'é' as u8` would have built a table describing a flag that cannot be reached.
    let shorts: Vec<u8> = f
        .short
        .iter()
        .map(|c| {
            assert!(
                c.is_ascii(),
                "a short flag must be ASCII for usage-argv's tables, and `-{c}` is not"
            );
            *c as u8
        })
        .collect();
    Box::leak(Box::new(Flag {
        key: 0,
        // Dynamic portable specs have no Rust field type to identify. They never participate
        // in derive's typed ancestor mirroring, so zero deliberately declares no contract.
        binding_key: 0,
        binding_type: None,
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
        // A bound counts values, and this is what says how many a word carries. ASCII, not
        // "fits in a byte": `§` is one byte as a scalar and two as UTF-8, and matching its
        // low byte would find the continuation bytes inside unrelated characters. The spec
        // refuses non-ASCII, so this only ever discards something already rejected.
        delimiter: f
            .arg
            .as_ref()
            .and_then(|a| a.delimiter)
            .filter(char::is_ascii)
            .map(|d| d as u8),
        allow_hyphen_values: f.allow_hyphen_values(),
        allow_negative_numbers: f.arg.as_ref().is_some_and(|arg| arg.allow_negative_numbers),
        value_terminator: f
            .arg
            .as_ref()
            .and_then(|arg| arg.value_terminator.as_deref())
            .map(|value| leak(value).as_bytes()),
        require_equals: f.require_equals,
        value_optional: f.value_optional,
        bool_value: f.bool_value,
        default_missing: f.default_missing.as_deref().map(|s| leak(s).as_bytes()),
        global: f.global,
        action: match f.action {
            usage::SpecFlagAction::Set => usage_argv::ArgAction::Set,
            usage::SpecFlagAction::Help => usage_argv::ArgAction::Help,
            usage::SpecFlagAction::HelpShort => usage_argv::ArgAction::HelpShort,
            usage::SpecFlagAction::HelpLong => usage_argv::ArgAction::HelpLong,
            usage::SpecFlagAction::HelpAll => usage_argv::ArgAction::HelpAll,
            usage::SpecFlagAction::Version => usage_argv::ArgAction::Version,
        },
    }))
}

fn build_arg(a: &SpecArg) -> &'static Arg<'static> {
    Box::leak(Box::new(Arg {
        key: 0,
        required: a.required,
        name: leak(&a.name),
        var: a.var,
        var_max: a
            .var_max
            .filter(|_| a.var)
            .map(|max| u32::try_from(max).unwrap_or(u32::MAX)),
        delimiter: a.delimiter.filter(char::is_ascii).map(|d| d as u8),
        allow_negative_numbers: a.allow_negative_numbers,
        value_terminator: a
            .value_terminator
            .as_deref()
            .map(|value| leak(value).as_bytes()),
        double_dash: double_dash(&a.double_dash),
    }))
}

fn flag_meta(
    f: &SpecFlag,
    table: &'static Flag<'static>,
    completers: &[&SpecComplete],
) -> FlagMeta<'static> {
    let arg = f.arg.as_ref();
    let choices = arg.and_then(|a| a.choices.as_ref());
    FlagMeta {
        flag: table,
        hidden_shorts: bytes(&f.hidden_short_aliases),
        hidden_longs: strs(&f.hidden_aliases),
        help: opt(&f.help),
        long_help: opt(&f.help_long),
        deprecated: opt(&f.deprecated),
        deprecated_warn_at: opt(&f.deprecated_warn_at),
        deprecated_remove_at: opt(&f.deprecated_remove_at),
        value_name: arg.map(|a| leak(&a.name)),
        value_names: arg.map_or(&[], |a| strs(&a.value_names)),
        // The value's own bracket bit, which is not the flag's — usage-lib renders a flag from
        // two independent `required` bits and a spec can write either without the other. Folded
        // with the value's own default the way usage-lib folds a positional's, so
        // `arg "<n>" default="4"` inside a flag reads as optional; a default declared on the
        // *flag* is a different statement and stays in `default` below.
        value_optional: arg.is_some_and(|a| !a.required || !a.default.is_empty()),
        env: opt(&f.env),
        env_fallback: strs(&f.env_fallback),
        deprecated_env: strs(&f.deprecated_env),
        default: strs(&f.default),
        accepted_choices: accepted_choices(choices),
        choices: visible_choices(choices),
        choice_aliases: choice_aliases(choices),
        choice_details: choice_details(choices),
        ignore_case: choices.is_some_and(|c| c.ignore_case),
        allow_unknown_choices: choices.is_some_and(|c| !c.strict),
        validate: arg.and_then(|a| a.validate.as_deref()).map(leak),
        validate_error: arg.and_then(|a| a.validate_error.as_deref()).map(leak),
        required: f.required,
        hide: f.hide,
        hide_default_value: f.hide_default_value,
        hide_env: f.hide_env,
        hide_env_values: f.hide_env_values,
        hide_possible_values: f.hide_possible_values,
        hide_short_help: f.hide_short_help,
        hide_long_help: f.hide_long_help,
        count: f.count,
        repeatable: f.var,
        // The separator as declared, a `char`: the metadata is the cold model and says what
        // the spec said, where the binding table beside it holds the byte binding counts by.
        delimiter: arg.and_then(|a| a.delimiter),
        var_min: f.var_min,
        var_max: f.var_max,
        value_var_min: arg.and_then(|a| a.var_min),
        value_var_max: arg.and_then(|a| a.var_max),
        overrides: strs(&f.overrides),
        conflicts: strs(&f.conflicts),
        requires: strs(&f.requires),
        requires_if: Box::leak(
            f.requires_if
                .iter()
                .map(|condition| RequiresIf {
                    value: leak(&condition.value),
                    requires: leak(&condition.requires),
                })
                .collect::<Vec<_>>()
                .into_boxed_slice(),
        ),
        default_if: Box::leak(
            f.default_if
                .iter()
                .map(|condition| DefaultIf {
                    selector: leak(&condition.selector),
                    when: condition.when.as_deref().map(leak),
                    value: leak(&condition.value),
                })
                .collect::<Vec<_>>()
                .into_boxed_slice(),
        ),
        exclusive: f.exclusive,
        required_if: strs(&f.required_if),
        required_if_eq: Box::leak(
            f.required_if_eq
                .iter()
                .map(|condition| RequiredIfEq {
                    selector: leak(&condition.selector),
                    value: leak(&condition.value),
                })
                .collect::<Vec<_>>()
                .into_boxed_slice(),
        ),
        required_if_eq_all: Box::leak(
            f.required_if_eq_all
                .iter()
                .map(|condition| RequiredIfEq {
                    selector: leak(&condition.selector),
                    value: leak(&condition.value),
                })
                .collect::<Vec<_>>()
                .into_boxed_slice(),
        ),
        required_unless: strs(&f.required_unless),
        required_unless_all: strs(&f.required_unless_all),
        help_heading: opt(&f.help_heading),
        display_order: f.display_order,
        effect: f.effect.map(effect),
        verbosity: f.verbosity.map(verbosity_role),
        color: f.color.map(color_role),
        complete_type: complete_type(completers, &f.name, arg.map(|a| a.name.as_str())),
        complete: NO_COMPLETER,
    }
}

fn arg_meta(
    a: &SpecArg,
    table: &'static Arg<'static>,
    completers: &[&SpecComplete],
) -> ArgMeta<'static> {
    let choices = a.choices.as_ref();
    ArgMeta {
        arg: table,
        value_names: strs(&a.value_names),
        help: opt(&a.help),
        long_help: opt(&a.help_long),
        env: opt(&a.env),
        env_fallback: strs(&a.env_fallback),
        deprecated_env: strs(&a.deprecated_env),
        default: strs(&a.default),
        accepted_choices: accepted_choices(choices),
        choices: visible_choices(choices),
        choice_aliases: choice_aliases(choices),
        choice_details: choice_details(choices),
        ignore_case: choices.is_some_and(|c| c.ignore_case),
        allow_unknown_choices: choices.is_some_and(|c| !c.strict),
        validate: a.validate.as_deref().map(leak),
        validate_error: a.validate_error.as_deref().map(leak),
        required: a.required,
        hide: a.hide,
        display_order: a.display_order,
        hide_default_value: a.hide_default_value,
        hide_env: a.hide_env,
        hide_env_values: a.hide_env_values,
        hide_possible_values: a.hide_possible_values,
        hide_short_help: a.hide_short_help,
        hide_long_help: a.hide_long_help,
        conflicts: strs(&a.conflicts),
        requires: strs(&a.requires),
        required_if: strs(&a.required_if),
        required_if_eq: Box::leak(
            a.required_if_eq
                .iter()
                .map(|condition| RequiredIfEq {
                    selector: leak(&condition.selector),
                    value: leak(&condition.value),
                })
                .collect::<Vec<_>>()
                .into_boxed_slice(),
        ),
        required_if_eq_all: Box::leak(
            a.required_if_eq_all
                .iter()
                .map(|condition| RequiredIfEq {
                    selector: leak(&condition.selector),
                    value: leak(&condition.value),
                })
                .collect::<Vec<_>>()
                .into_boxed_slice(),
        ),
        required_unless: strs(&a.required_unless),
        required_unless_all: strs(&a.required_unless_all),
        delimiter: a.delimiter,
        var_min: a.var_min,
        var_max: a.var_max,
        help_heading: opt(&a.help_heading),
        complete_type: complete_type(completers, &a.name, None),
        complete: NO_COMPLETER,
    }
}

/// A spec cannot supply one.
///
/// `Completer` is a Rust function the binary calls to answer for a value. A spec says `run=`
/// instead, which is a shell command — the two are different mechanisms, and the emitted KDL
/// turns the former into the latter rather than the other way round. Written out rather than
/// left to `EMPTY` so that the exhaustiveness this file relies on stays real.
const NO_COMPLETER: Option<usage_argv::spec::Completer> = None;

/// The built-in completion class declared for a flag or argument, if any.
///
/// `complete` nodes name the thing they complete rather than living on it, so this is a lookup,
/// and it has to key the way the reference keys or the two disagree about a spec neither is free
/// to reinterpret. Two rules come from `cli/src/cli/complete_word.rs`:
///
/// - **The key is the value's name, lowercased.** A completer for a flag is found by the name of
///   the value it takes, never by the flag's own — the reference completes a flag by handing its
///   `SpecArg` to the same code that completes a positional. The flag's name is tried only for a
///   flag that takes no value, which is the fallback `Spec::to_kdl` writes back.
/// - **The comparison ignores case.** `SpecComplete::parse` lowercases the node's name, so a
///   declared `complete "key"` is stored as `key` and matched against `<KEY>` lowercased. fnox
///   writes exactly that, and comparing the name as written found nothing for any of it.
///
/// `completers` are already in the reference's order of preference — the spec's own nodes before
/// the command's — so the first match wins. They arrive as a slice of borrows rather than the
/// `IndexMap` they come from so that this crate need not depend on `indexmap` to name the type.
fn complete_type(
    completers: &[&SpecComplete],
    name: &str,
    value_name: Option<&str>,
) -> Option<&'static str> {
    let key = value_name.unwrap_or(name).to_lowercase();
    let found = completers
        .iter()
        .find(|c| c.name == key)
        .and_then(|c| c.type_.as_deref());
    found.map(leak)
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

fn verbosity_role(role: usage::SpecVerbosityRole) -> VerbosityRole {
    use usage::SpecVerbosityRole as Declared;
    match role {
        Declared::Verbose => VerbosityRole::Verbose,
        Declared::Quiet => VerbosityRole::Quiet,
        Declared::Level => VerbosityRole::Level,
        Declared::Silent => VerbosityRole::Pin(Verbosity::Silent),
        Declared::Error => VerbosityRole::Pin(Verbosity::Error),
        Declared::Warn => VerbosityRole::Pin(Verbosity::Warn),
        Declared::Info => VerbosityRole::Pin(Verbosity::Info),
        Declared::Debug => VerbosityRole::Pin(Verbosity::Debug),
        Declared::Trace => VerbosityRole::Pin(Verbosity::Trace),
    }
}

fn color_role(role: usage::SpecColorRole) -> ColorRole {
    match role {
        usage::SpecColorRole::Always => ColorRole::Always,
        usage::SpecColorRole::Never => ColorRole::Never,
        usage::SpecColorRole::Choice => ColorRole::Choice,
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

fn bytes(list: &[char]) -> &'static [u8] {
    Box::leak(
        list.iter()
            .map(|c| {
                assert!(c.is_ascii(), "a hidden short flag alias must be ASCII");
                *c as u8
            })
            .collect::<Vec<_>>()
            .into_boxed_slice(),
    )
}

fn accepted_choices(choices: Option<&SpecChoices>) -> &'static [&'static str] {
    let Some(choices) = choices else {
        return &[];
    };
    Box::leak(
        choices
            .choices
            .iter()
            .map(|value| leak(value))
            .chain(
                choices
                    .details
                    .iter()
                    .flat_map(|choice| choice.aliases.iter())
                    .map(|alias| leak(&alias.value)),
            )
            .collect::<Vec<_>>()
            .into_boxed_slice(),
    )
}

fn visible_choices(choices: Option<&SpecChoices>) -> &'static [&'static str] {
    let Some(choices) = choices else {
        return &[];
    };
    Box::leak(
        choices
            .choices
            .iter()
            .filter(|value| {
                !choices
                    .details
                    .iter()
                    .any(|choice| choice.value == value.as_str() && choice.hide)
            })
            .chain(choices.details.iter().flat_map(|choice| {
                choice
                    .aliases
                    .iter()
                    .filter(|alias| !alias.hide)
                    .map(|alias| &alias.value)
            }))
            .map(|value| leak(value))
            .collect::<Vec<_>>()
            .into_boxed_slice(),
    )
}

fn choice_aliases(choices: Option<&SpecChoices>) -> &'static [(&'static str, &'static str)] {
    let Some(choices) = choices else {
        return &[];
    };
    Box::leak(
        choices
            .details
            .iter()
            .flat_map(|choice| {
                choice
                    .aliases
                    .iter()
                    .map(move |alias| (leak(&choice.value), leak(&alias.value)))
            })
            .collect::<Vec<_>>()
            .into_boxed_slice(),
    )
}

fn choice_details(choices: Option<&SpecChoices>) -> &'static [ChoiceMeta<'static>] {
    let Some(choices) = choices else {
        return &[];
    };
    Box::leak(
        choices
            .details
            .iter()
            .map(|choice| ChoiceMeta {
                value: leak(&choice.value),
                help: choice.help.as_deref().map(leak),
                hide: choice.hide,
                aliases: Box::leak(
                    choice
                        .aliases
                        .iter()
                        .map(|alias| ChoiceAliasMeta {
                            value: leak(&alias.value),
                            hide: alias.hide,
                        })
                        .collect::<Vec<_>>()
                        .into_boxed_slice(),
                ),
            })
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

/// The command's groups, as usage-argv's cold model of them.
///
/// Selectors are leaked one at a time rather than joined: a group names flags the way every
/// other relationship does, and the metadata holds them in that form.
fn groups(list: &[SpecGroup]) -> &'static [GroupMeta<'static>] {
    Box::leak(
        list.iter()
            .map(|g| GroupMeta {
                name: leak(&g.name),
                members: Box::leak(
                    g.members
                        .iter()
                        .map(|m| leak(m))
                        .collect::<Vec<_>>()
                        .into_boxed_slice(),
                ),
                required: g.required,
                multiple: g.multiple,
            })
            .collect::<Vec<_>>()
            .into_boxed_slice(),
    )
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

#[cfg(test)]
mod tests {
    use super::*;

    /// The fields no page shows, which is why they need a test of their own.
    ///
    /// `corpus/render` catches a dropped field by the difference it makes to a rendered page,
    /// which is most of them and was how the missing examples surfaced. These two make no
    /// difference to any page — they are read by completions and by spec emission — so nothing
    /// would have noticed them going missing, and `min_usage_version` had.
    #[test]
    fn the_fields_that_do_not_reach_a_page_are_carried_too() {
        let spec: Spec = "name \"ex\"\nbin \"ex\"\nmin_usage_version \"2.1.0\"\n\
             flag \"--out <FILE>\"\narg \"<dir>\"\n\
             complete \"file\" type=\"path\"\ncomplete \"dir\" type=\"dir\"\n"
            .parse()
            .expect("valid spec");
        let built = build_spec(&spec);

        assert_eq!(built.min_usage_version, Some("2.1.0"));
        assert_eq!(built.root.flags[0].complete_type, Some("path"));
        assert_eq!(built.root.args[0].complete_type, Some("dir"));
        // A Rust completer is a function the binary calls, which a spec's `run=` is not — so
        // this stays `None` however a spec is written, and says so rather than defaulting.
        assert!(built.root.flags[0].complete.is_none());
    }

    #[test]
    fn rich_choices_separate_visible_and_accepted_values() {
        let spec: Spec = "name \"ex\"\nbin \"ex\"\n\
             flag \"--mode <MODE>\" {\n  arg \"<MODE>\" {\n    choices {\n\
               choice \"shown\" {\n      alias \"short\"\n      alias \"secret-short\" hide=#true\n    }\n\
               choice \"secret\" hide=#true\n    }\n  }\n}\n\
             "
            .parse()
            .expect("valid spec");
        let built = build_spec(&spec);

        assert_eq!(built.root.flags[0].choices, &["shown", "short"]);
        assert_eq!(
            built.root.flags[0].accepted_choices,
            &["shown", "secret", "short", "secret-short"]
        );
        let emitted: Spec = built.to_kdl().parse().expect("emitted choices stay valid");
        let choices = emitted.cmd.flags[0]
            .arg
            .as_ref()
            .and_then(|arg| arg.choices.as_ref())
            .expect("flag choices");
        assert_eq!(choices.values(), vec!["shown", "short"]);
        assert_eq!(choices.choices, ["shown", "secret"]);
        assert_eq!(choices.details[0].aliases[0].value, "short");
        assert!(!choices.details[0].aliases[0].hide);
        assert_eq!(choices.details[0].aliases[1].value, "secret-short");
        assert!(choices.details[0].aliases[1].hide);
        assert!(choices.details[1].hide);
    }

    /// A completer is keyed by the *value's* name, lowercased, on whichever command asks.
    ///
    /// All three parts are the reference's, and getting any of them wrong resolves nothing for
    /// fnox, whose `complete "key"` is written once at the top level and means the `<KEY>` of a
    /// subcommand. Written as a unit test because `complete_type` reaches no page: the rendering
    /// corpus catches a dropped field by the difference it makes to rendered text, and this
    /// field makes none.
    #[test]
    fn a_completer_is_keyed_the_way_the_reference_keys_it() {
        let spec: Spec = "name \"ex\"\nbin \"ex\"\n\
             flag \"--out <FILE>\"\n\
             cmd \"get\" {\n  arg \"<KEY>\"\n  flag \"--to <DEST>\"\n}\n\
             complete \"key\" type=\"file\"\ncomplete \"file\" type=\"path\"\n\
             complete \"out\" type=\"dir\"\n"
            .parse()
            .expect("valid spec");
        let built = build_spec(&spec);
        let get = built.root.subcommands[0];

        // The value's name, not the flag's: the reference completes a flag by handing its value
        // to the code that completes a positional, so `complete "out"` answers for nothing.
        assert_eq!(built.root.flags[0].complete_type, Some("path"));
        // `<KEY>` against a node stored as `key`, on a subcommand, from the top level.
        assert_eq!(get.args[0].complete_type, Some("file"));
        // And nothing invented for a value no node names.
        assert_eq!(get.flags[0].complete_type, None);
    }

    /// The spec's own nodes are consulted before the command's, which is the reference's order
    /// (`cli/src/cli/complete_word.rs`) and not the intuitive one.
    #[test]
    fn a_spec_level_completer_wins_over_a_commands_own() {
        let spec: Spec = "name \"ex\"\nbin \"ex\"\n\
             cmd \"get\" {\n  arg \"<KEY>\"\n  complete \"key\" type=\"dir\"\n}\n\
             complete \"key\" type=\"file\"\n"
            .parse()
            .expect("valid spec");
        let built = build_spec(&spec);
        assert_eq!(
            built.root.subcommands[0].args[0].complete_type,
            Some("file")
        );
    }

    /// usage-argv holds a short flag as one byte, so a spec declaring one it cannot hold is
    /// refused rather than mirrored into a table describing a flag nobody can type.
    #[test]
    #[should_panic(expected = "a short flag must be ASCII")]
    fn a_non_ascii_short_flag_is_refused_rather_than_truncated() {
        let spec: Spec = "name \"ex\"\nbin \"ex\"\nflag \"-é --etage\"\n"
            .parse()
            .expect("valid spec");
        build_spec(&spec);
    }
}
