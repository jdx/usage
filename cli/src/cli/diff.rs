//! Compare two specs and say what changed about the interface.
//!
//! A CLI is a public API, and a spec is the only machine-readable statement of what
//! that API is — which makes "did this release break somebody" a question about two
//! files rather than a question about a changelog somebody remembered to write.
//! clap#918 has been open since 2017 asking for the export this reads.
//!
//! Every finding lands in one of three categories, and the line between them is one
//! rule: **breaking** means a command line that worked against the old spec now
//! fails, binds differently, or resolves to a different value. **compatible** means
//! the interface gained something or relaxed a rule — every old command line still
//! means what it meant. **metadata** means nothing about parsing moved: help text,
//! headings, declaration order, hidden-ness, `effect`, deprecation notices.
//!
//! Two deliberate silences. `version` and `long_version` are never reported: a
//! release bumps them, and a compatibility check that fires on every release is one
//! nobody leaves running — tak sets `spec.version = None` by hand today for exactly
//! this reason. Derived strings (`usage`, `full_cmd`, `help_first_line`) are not
//! reported either, because they restate what the declarations already say.

use std::collections::HashSet;
use std::path::PathBuf;

use usage::{Spec, SpecArg, SpecCommand, SpecFlag};

use crate::cli::generate::parse_file_or_stdin;
use crate::cli::OutputFormat;
use usage::spec::choices::SpecChoices;
use usage::spec::config::{SpecConfig, SpecConfigProp, SpecConfigValue};
use usage::spec::group::SpecGroup;
use usage::spec::unknown_flags::UnknownFlags;

/// Compare two usage specs and report what changed about the interface
///
/// Findings are grouped into breaking changes (a command line that used to work
/// now fails, binds differently, or resolves to a different value), compatible
/// changes (the interface gained something or relaxed a rule), and metadata
/// changes (help text, effect, deprecation — nothing about parsing).
///
/// Exits 1 when there is a breaking change, so a release job can gate on it, and
/// either spec may be "-":
///
///   mycli --usage-spec | usage diff released.usage.kdl -
///
/// `version` is ignored on purpose: a release bumps it, and a check that fires
/// every release does not get left switched on.
#[derive(usage_rs::Args)]
#[usage(effect = "read", verbatim_doc_comment)]
pub struct Diff {
    /// The spec as it was, typically the released one, use "-" to read from stdin
    #[usage(value_hint = usage_rs::ValueHint::FilePath)]
    old: PathBuf,

    /// The spec as it is now, use "-" to read from stdin
    #[usage(value_hint = usage_rs::ValueHint::FilePath)]
    new: PathBuf,

    /// Output format
    #[usage(long, short, default = "text", value_enum)]
    format: OutputFormat,

    /// Report only breaking changes
    #[usage(long, short)]
    breaking: bool,

    /// Exit 0 even when there are breaking changes
    #[usage(long)]
    exit_zero: bool,
}

/// How much a change costs whoever is calling the CLI.
///
/// Ordered, so a report can put what matters first without a second table:
/// breaking before compatible before metadata.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, serde::Serialize)]
#[serde(rename_all = "lowercase")]
pub enum Category {
    /// A command line that worked before now fails, binds differently, or
    /// resolves to a different value.
    Breaking,
    /// The interface gained something, or relaxed a rule. Every old command line
    /// still means what it meant.
    Compatible,
    /// Nothing about parsing moved.
    Metadata,
}

impl std::fmt::Display for Category {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Category::Breaking => write!(f, "breaking"),
            Category::Compatible => write!(f, "compatible"),
            Category::Metadata => write!(f, "metadata"),
        }
    }
}

/// One difference between two specs.
///
/// The same shape as a lint issue, so a reader who has seen `usage lint --format
/// json` output knows how to read this one.
#[derive(Debug, Clone, serde::Serialize)]
pub struct SpecChange {
    pub category: Category,
    pub code: String,
    pub message: String,
    pub location: String,
}

impl std::fmt::Display for SpecChange {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(
            f,
            "{} [{}] at {}: {}",
            self.category, self.code, self.location, self.message
        )
    }
}

impl usage_rs::Run for Diff {
    type Output = usage::miette::Result<()>;

    fn run(self) -> Self::Output {
        if self.old.as_os_str() == "-" && self.new.as_os_str() == "-" {
            usage::miette::bail!("only one of the two specs can be read from stdin");
        }
        let old = parse_file_or_stdin(&self.old)?;
        let new = parse_file_or_stdin(&self.new)?;
        let mut changes = diff_specs(&old, &new);
        if self.breaking {
            changes.retain(|c| c.category == Category::Breaking);
        }

        match self.format {
            OutputFormat::Text => self.print_text(&changes),
            OutputFormat::Json => self.print_json(&changes)?,
        }

        if !self.exit_zero && changes.iter().any(|c| c.category == Category::Breaking) {
            std::process::exit(1);
        }
        Ok(())
    }
}

impl Diff {
    fn print_text(&self, changes: &[SpecChange]) {
        if changes.is_empty() {
            if self.breaking {
                println!("No breaking changes.");
            } else {
                println!("No interface changes.");
            }
            return;
        }

        for change in changes {
            println!("{change}");
        }

        let count = |category: Category| changes.iter().filter(|c| c.category == category).count();
        println!();
        println!(
            "Found {} breaking, {} compatible, {} metadata change(s)",
            count(Category::Breaking),
            count(Category::Compatible),
            count(Category::Metadata),
        );
    }

    fn print_json(&self, changes: &[SpecChange]) -> usage::miette::Result<()> {
        let json = serde_json::to_string_pretty(changes)
            .map_err(|e| usage::miette::miette!("Failed to serialize changes: {}", e))?;
        println!("{json}");
        Ok(())
    }
}

/// Where findings accumulate, so every comparison function is a `&mut self` push
/// rather than a `Vec` returned and concatenated by its caller.
#[derive(Default)]
struct Changes {
    changes: Vec<SpecChange>,
}

impl Changes {
    fn push(&mut self, category: Category, code: &str, location: &str, message: String) {
        self.changes.push(SpecChange {
            category,
            code: code.to_string(),
            message,
            location: location.to_string(),
        });
    }

    fn breaking(&mut self, code: &str, location: &str, message: String) {
        self.push(Category::Breaking, code, location, message);
    }

    fn compatible(&mut self, code: &str, location: &str, message: String) {
        self.push(Category::Compatible, code, location, message);
    }

    fn metadata(&mut self, code: &str, location: &str, message: String) {
        self.push(Category::Metadata, code, location, message);
    }
}

/// Compare two specs.
///
/// Breaking findings come first, then compatible, then metadata; within a category
/// the order is the walk order — root, its flags, its arguments, then each
/// subcommand — which is the order the spec declares them in.
pub fn diff_specs(old: &Spec, new: &Spec) -> Vec<SpecChange> {
    let mut c = Changes::default();

    let root = if new.bin.is_empty() {
        &new.name
    } else {
        &new.bin
    };
    let root = root.clone();

    if old.bin != new.bin {
        c.breaking(
            "bin-changed",
            &root,
            format!("binary name changed from '{}' to '{}'", old.bin, new.bin),
        );
    }
    if old.name != new.name {
        c.metadata(
            "name-changed",
            &root,
            format!("spec name changed from '{}' to '{}'", old.name, new.name),
        );
    }

    match (&old.default_subcommand, &new.default_subcommand) {
        (Some(was), None) => c.breaking(
            "default-subcommand-removed",
            &root,
            format!(
                "default subcommand '{was}' was removed, so a bare invocation no longer routes"
            ),
        ),
        (None, Some(now)) => c.compatible(
            "default-subcommand-added",
            &root,
            format!("a bare invocation now routes to '{now}'"),
        ),
        (Some(was), Some(now)) if was != now => c.breaking(
            "default-subcommand-changed",
            &root,
            format!("default subcommand changed from '{was}' to '{now}'"),
        ),
        _ => {}
    }

    if old.multicall && !new.multicall {
        c.breaking(
            "multicall-removed",
            &root,
            "multicall was removed, so argv[0] no longer selects an applet".to_string(),
        );
    } else if !old.multicall && new.multicall {
        c.compatible(
            "multicall-added",
            &root,
            "argv[0] now selects an applet".to_string(),
        );
    }

    for id in old.views.keys() {
        if !new.views.contains_key(id) {
            c.breaking(
                "view-removed",
                &root,
                format!("view '{id}' was removed, so its executable no longer has a spec"),
            );
        }
    }
    for id in new.views.keys() {
        if !old.views.contains_key(id) {
            c.compatible("view-added", &root, format!("view '{id}' was added"));
        }
    }

    if old.min_usage_version != new.min_usage_version {
        c.metadata(
            "min-usage-version-changed",
            &root,
            format!(
                "min_usage_version changed from {} to {}",
                option(&old.min_usage_version),
                option(&new.min_usage_version)
            ),
        );
    }

    if [
        old.about != new.about,
        old.about_long != new.about_long,
        old.about_md != new.about_md,
        old.license != new.license,
        old.author != new.author,
        old.repository != new.repository,
    ]
    .iter()
    .any(|changed| *changed)
    {
        c.metadata("about-changed", &root, "root metadata changed".to_string());
    }

    // The spec-level declaration, compared once at the root rather than as an effective
    // value at every command that inherits it: 54 identical findings for one edited node
    // is not a report anybody reads.
    diff_unknown_flags(old.unknown_flags, new.unknown_flags, &root, &mut c);
    diff_command(&old.cmd, &new.cmd, &root, None, &mut c);
    diff_config(&old.config, &new.config, &root, &mut c);

    // Stable: `sort_by_key` keeps the walk order inside each category, so a reader
    // can still follow the tree down while reading the breaking block first.
    c.changes.sort_by_key(|change| change.category);
    c.changes
}

/// `renamed_from` names the word this pair is being compared under when the new command
/// answers to it only as an alias. That alias is what made the rename a rename rather
/// than a removal, and it is already reported as `cmd-renamed`, so it must not also read
/// as an addition at the old command's location.
fn diff_command(
    old: &SpecCommand,
    new: &SpecCommand,
    path: &str,
    renamed_from: Option<&str>,
    c: &mut Changes,
) {
    diff_names(old, new, path, renamed_from, c);
    diff_command_props(old, new, path, c);
    diff_flags(old, new, path, c);
    diff_args(&old.args, &new.args, path, c);
    diff_groups(&old.groups, &new.groups, path, c);
    diff_mounts(old, new, path, c);
    diff_subcommands(old, new, path, c);
}

fn diff_names(
    old: &SpecCommand,
    new: &SpecCommand,
    path: &str,
    renamed_from: Option<&str>,
    c: &mut Changes,
) {
    let was: Vec<String> = old
        .aliases
        .iter()
        .chain(&old.hidden_aliases)
        .cloned()
        .collect();
    let now: Vec<String> = new
        .aliases
        .iter()
        .chain(&new.hidden_aliases)
        .cloned()
        .collect();
    for alias in only_in(&was, &now) {
        c.breaking(
            "alias-removed",
            path,
            format!("alias '{alias}' was removed"),
        );
    }
    for alias in only_in(&now, &was) {
        if renamed_from == Some(alias.as_str()) {
            continue;
        }
        c.compatible("alias-added", path, format!("alias '{alias}' was added"));
    }
    // A visible alias that became hidden still parses; only help and completion
    // stop offering it.
    for alias in only_in(&old.aliases, &new.aliases) {
        if new.hidden_aliases.contains(alias) {
            c.metadata(
                "alias-hidden",
                path,
                format!("alias '{alias}' is no longer shown in help"),
            );
        }
    }
}

fn diff_command_props(old: &SpecCommand, new: &SpecCommand, path: &str, c: &mut Changes) {
    if !old.subcommand_required && new.subcommand_required {
        c.breaking(
            "subcommand-now-required",
            path,
            "a subcommand is now required, so a bare invocation fails".to_string(),
        );
    } else if old.subcommand_required && !new.subcommand_required {
        c.compatible(
            "subcommand-no-longer-required",
            path,
            "a bare invocation is now accepted".to_string(),
        );
    }

    if !old.arg_required_else_help && new.arg_required_else_help {
        c.breaking(
            "arg-required-else-help-added",
            path,
            "a bare invocation now prints help instead of running".to_string(),
        );
    } else if old.arg_required_else_help && !new.arg_required_else_help {
        c.compatible(
            "arg-required-else-help-removed",
            path,
            "a bare invocation now runs instead of printing help".to_string(),
        );
    }

    if old.external_subcommand && !new.external_subcommand {
        c.breaking(
            "external-subcommand-removed",
            path,
            "an unmatched word is no longer forwarded".to_string(),
        );
    } else if !old.external_subcommand && new.external_subcommand {
        c.compatible(
            "external-subcommand-added",
            path,
            "an unmatched word is now forwarded".to_string(),
        );
    }

    match (&old.restart_token, &new.restart_token) {
        (Some(was), None) => c.breaking(
            "restart-token-removed",
            path,
            format!("restart token '{was}' was removed, so the words after it are no longer a fresh command line"),
        ),
        (None, Some(now)) => c.compatible(
            "restart-token-added",
            path,
            format!("restart token '{now}' was added"),
        ),
        (Some(was), Some(now)) if was != now => c.breaking(
            "restart-token-changed",
            path,
            format!("restart token changed from '{was}' to '{now}'"),
        ),
        _ => {}
    }

    match (&old.clause, &new.clause) {
        (Some(was), None) => c.breaking(
            "clause-removed",
            path,
            format!("clause '{}' was removed", was.name),
        ),
        (None, Some(now)) => c.breaking(
            "clause-added",
            path,
            format!(
                "clause '{}' using separator '{}' was added, so matching words now start a new clause",
                now.name, now.separator
            ),
        ),
        (Some(was), Some(now)) if was.separator != now.separator => c.breaking(
            "clause-separator-changed",
            path,
            format!(
                "clause separator changed from '{}' to '{}'",
                was.separator, now.separator
            ),
        ),
        _ => {}
    }

    diff_unknown_flags(old.unknown_flags, new.unknown_flags, path, c);

    if !old.args_conflicts_with_subcommands && new.args_conflicts_with_subcommands {
        c.breaking(
            "args-conflicts-with-subcommands-added",
            path,
            "arguments and a subcommand can no longer appear together".to_string(),
        );
    } else if old.args_conflicts_with_subcommands && !new.args_conflicts_with_subcommands {
        c.compatible(
            "args-conflicts-with-subcommands-removed",
            path,
            "arguments and a subcommand may now appear together".to_string(),
        );
    }
    if !old.subcommand_precedence_over_arg && new.subcommand_precedence_over_arg {
        c.breaking(
            "subcommand-precedence-added",
            path,
            "a word matching a subcommand name now routes instead of binding as an argument"
                .to_string(),
        );
    } else if old.subcommand_precedence_over_arg && !new.subcommand_precedence_over_arg {
        // Breaking in the other direction too, and for the same reason: the word that used
        // to select a command now fills an argument. Nothing fails, which is what makes it
        // worth reporting — a silent change of meaning is the kind a gate exists to catch.
        c.breaking(
            "subcommand-precedence-removed",
            path,
            "a word matching a subcommand name now binds as an argument instead of routing"
                .to_string(),
        );
    }
    if old.disable_help_flag != new.disable_help_flag {
        let (code, category, message) = if new.disable_help_flag {
            (
                "help-flag-disabled",
                Category::Breaking,
                "--help is no longer accepted",
            )
        } else {
            (
                "help-flag-enabled",
                Category::Compatible,
                "--help is now accepted",
            )
        };
        c.push(category, code, path, message.to_string());
    }
    if old.disable_version_flag != new.disable_version_flag {
        let (code, category, message) = if new.disable_version_flag {
            (
                "version-flag-disabled",
                Category::Breaking,
                "--version is no longer accepted",
            )
        } else {
            (
                "version-flag-enabled",
                Category::Compatible,
                "--version is now accepted",
            )
        };
        c.push(category, code, path, message.to_string());
    }

    if !old.hide && new.hide {
        c.metadata(
            "command-hidden",
            path,
            "command is no longer documented".to_string(),
        );
    } else if old.hide && !new.hide {
        c.metadata(
            "command-unhidden",
            path,
            "command is now documented".to_string(),
        );
    }

    if effect_of(&old.effect) != effect_of(&new.effect) {
        c.metadata(
            "effect-changed",
            path,
            format!(
                "effect changed from {} to {}",
                effect_of(&old.effect),
                effect_of(&new.effect)
            ),
        );
    }

    diff_deprecation(
        old.deprecated.as_deref(),
        new.deprecated.as_deref(),
        path,
        "command",
        c,
    );

    if [
        old.help != new.help,
        old.help_long != new.help_long,
        old.help_md != new.help_md,
        old.before_help != new.before_help,
        old.after_help != new.after_help,
        old.help_heading != new.help_heading,
    ]
    .iter()
    .any(|changed| *changed)
    {
        c.metadata("help-changed", path, "help text changed".to_string());
    }

    diff_display_order(old.display_order, new.display_order, path, "command", c);
}

/// Pair the two flag lists up, then report.
///
/// Two passes, because the two rules are not equals. A matching internal name is the
/// primary pairing and claims its flag first; only then may a flag whose spellings
/// moved onto a differently-named declaration claim what is left. Interleaving them let
/// a spelling take a flag whose name still matched some later old flag, which then
/// paired with it a second time and reported one comparison twice while losing another
/// flag's removal entirely.
fn diff_flags(old: &SpecCommand, new: &SpecCommand, path: &str, c: &mut Changes) {
    /// What became of one old flag.
    enum Pairing {
        /// Paired by internal name, which is what the derive and the KDL both key on.
        Named(usize),
        /// Paired by a shared spelling. The name is not something a caller can type, so
        /// this is a rename rather than a removal — reported, and then compared in full,
        /// so a spelling that really did disappear is still reported.
        Renamed(usize),
        Gone,
    }

    let mut claimed: Vec<bool> = vec![false; new.flags.len()];
    let mut pairings: Vec<Pairing> = Vec::with_capacity(old.flags.len());

    for was in &old.flags {
        match new
            .flags
            .iter()
            .enumerate()
            .find(|(position, f)| !claimed[*position] && f.name == was.name)
            .map(|(position, _)| position)
        {
            Some(position) => {
                claimed[position] = true;
                pairings.push(Pairing::Named(position));
            }
            None => pairings.push(Pairing::Gone),
        }
    }

    for (index, was) in old.flags.iter().enumerate() {
        if !matches!(pairings[index], Pairing::Gone) {
            continue;
        }
        let spellings = flag_spellings(was);
        if let Some(position) = new
            .flags
            .iter()
            .enumerate()
            .find(|(position, f)| {
                !claimed[*position] && flag_spellings(f).iter().any(|s| spellings.contains(s))
            })
            .map(|(position, _)| position)
        {
            claimed[position] = true;
            pairings[index] = Pairing::Renamed(position);
        }
    }

    // Reported in the old spec's declaration order, whichever pass did the pairing.
    for (was, pairing) in old.flags.iter().zip(&pairings) {
        match pairing {
            Pairing::Named(position) => diff_flag(was, &new.flags[*position], path, c),
            Pairing::Renamed(position) => {
                let now = &new.flags[*position];
                c.metadata(
                    "flag-renamed",
                    path,
                    format!("flag '{}' was renamed to '{}'", was.name, now.name),
                );
                diff_flag(was, now, path, c);
            }
            Pairing::Gone => c.breaking(
                "flag-removed",
                path,
                format!(
                    "flag '{}' ({}) was removed",
                    was.name,
                    flag_spellings(was).join(", ")
                ),
            ),
        }
    }

    for (position, now) in new.flags.iter().enumerate() {
        if claimed[position] {
            continue;
        }
        let spellings = flag_spellings(now);
        if now.required {
            c.breaking(
                "required-flag-added",
                path,
                format!(
                    "required flag '{}' was added, so an invocation without it fails",
                    spellings.join(", ")
                ),
            );
        } else {
            c.compatible(
                "flag-added",
                path,
                format!("flag '{}' was added", spellings.join(", ")),
            );
        }
    }
}

fn diff_flag(old: &SpecFlag, new: &SpecFlag, path: &str, c: &mut Changes) {
    let subject = format!("flag '{}'", primary_spelling(new));
    let was = flag_spellings(old);
    let now = flag_spellings(new);
    for spelling in only_in(&was, &now) {
        c.breaking(
            "flag-spelling-removed",
            path,
            format!("{subject} no longer answers to '{spelling}'"),
        );
    }
    for spelling in only_in(&now, &was) {
        c.compatible(
            "flag-spelling-added",
            path,
            format!("{subject} now answers to '{spelling}'"),
        );
    }

    if !old.required && new.required {
        c.breaking(
            "flag-now-required",
            path,
            format!("{subject} is now required"),
        );
    } else if old.required && !new.required {
        c.compatible(
            "flag-no-longer-required",
            path,
            format!("{subject} is no longer required"),
        );
    }

    match (&old.arg, &new.arg) {
        (None, Some(arg)) => c.breaking(
            "flag-value-added",
            path,
            format!(
                "{subject} now takes a value <{}>, so the bare flag no longer parses",
                arg.name
            ),
        ),
        (Some(arg), None) => c.breaking(
            "flag-value-removed",
            path,
            format!(
                "{subject} no longer takes a value, so the word after it binds elsewhere than <{}>",
                arg.name
            ),
        ),
        (Some(was), Some(now)) => diff_arg(was, now, path, &subject, c),
        (None, None) => {}
    }

    if !old.var && new.var {
        c.compatible(
            "flag-now-variadic",
            path,
            format!("{subject} now takes more than one value"),
        );
    } else if old.var && !new.var {
        c.breaking(
            "flag-no-longer-variadic",
            path,
            format!("{subject} takes only one value"),
        );
    }
    diff_var_max(old.var_max, new.var_max, path, &subject, c);
    diff_var_min(old.var_min, new.var_min, path, &subject, c);

    if !old.require_equals && new.require_equals {
        c.breaking(
            "require-equals-added",
            path,
            format!("{subject} now requires its value attached with '='"),
        );
    } else if old.require_equals && !new.require_equals {
        c.compatible(
            "require-equals-removed",
            path,
            format!("{subject} now accepts a detached value"),
        );
    }

    if old.value_optional && !new.value_optional {
        c.breaking(
            "flag-value-now-mandatory",
            path,
            format!("{subject} no longer accepts being given without a value"),
        );
    } else if !old.value_optional && new.value_optional {
        c.compatible(
            "flag-value-now-optional",
            path,
            format!("{subject} now accepts being given without a value"),
        );
    }

    if old.count != new.count {
        let (code, message) = if new.count {
            (
                "flag-now-counting",
                "repeats now count instead of binding a value",
            )
        } else {
            ("flag-no-longer-counting", "repeats no longer count")
        };
        c.breaking(code, path, format!("{subject}: {message}"));
    }

    if old.bool_value && !new.bool_value {
        c.breaking(
            "bool-value-removed",
            path,
            format!("{subject} no longer accepts '=true' / '=false'"),
        );
    } else if !old.bool_value && new.bool_value {
        c.compatible(
            "bool-value-added",
            path,
            format!("{subject} now accepts '=true' / '=false'"),
        );
    }

    if old.global && !new.global {
        c.breaking(
            "flag-no-longer-global",
            path,
            format!("{subject} is no longer accepted on subcommands"),
        );
    } else if !old.global && new.global {
        c.compatible(
            "flag-now-global",
            path,
            format!("{subject} is now accepted on subcommands"),
        );
    }

    match (&old.negate, &new.negate) {
        (Some(was), None) => c.breaking(
            "negation-removed",
            path,
            format!("{subject} no longer answers to its negated spelling '{was}'"),
        ),
        (None, Some(now)) => c.compatible(
            "negation-added",
            path,
            format!("{subject} now answers to '{now}'"),
        ),
        (Some(was), Some(now)) if was != now => c.breaking(
            "negation-changed",
            path,
            format!("{subject} negated spelling changed from '{was}' to '{now}'"),
        ),
        _ => {}
    }

    match (&old.default_missing, &new.default_missing) {
        (Some(was), None) => c.breaking(
            "default-missing-removed",
            path,
            format!("{subject} no longer binds '{was}' when given without a value"),
        ),
        (None, Some(now)) => c.compatible(
            "default-missing-added",
            path,
            format!("{subject} binds '{now}' when given without a value"),
        ),
        (Some(was), Some(now)) if was != now => c.breaking(
            "default-missing-changed",
            path,
            format!("{subject} binds '{now}' rather than '{was}' when given without a value"),
        ),
        _ => {}
    }

    diff_defaults(&old.default, &new.default, path, &subject, c);
    // A conditional default resolves a value where none was resolved before, so it is
    // read the same way an unconditional one is: gaining one fills a hole, losing one
    // takes ground away. A changed condition or value is a removal and an addition,
    // which is what it is.
    for entry in only_in(&default_if(&new.default_if), &default_if(&old.default_if)) {
        c.compatible(
            "default-if-added",
            path,
            format!("{subject} now defaults to {entry}"),
        );
    }
    for entry in only_in(&default_if(&old.default_if), &default_if(&new.default_if)) {
        c.breaking(
            "default-if-removed",
            path,
            format!("{subject} no longer defaults to {entry}"),
        );
    }
    // First match wins, so where two conditions can both hold, which one is written
    // first decides the value. Reported the way an environment reordering is, and for
    // the same reason.
    if order_changed(&default_if(&old.default_if), &default_if(&new.default_if)) {
        c.breaking(
            "default-if-order-changed",
            path,
            format!(
                "{subject} applies its conditional defaults in a different order, and the \
                 first one that matches wins"
            ),
        );
    }
    diff_env(
        old.env.as_deref(),
        new.env.as_deref(),
        &old.env_fallback,
        &new.env_fallback,
        &old.deprecated_env,
        &new.deprecated_env,
        path,
        &subject,
        c,
    );

    diff_restricting(
        &old.conflicts,
        &new.conflicts,
        path,
        &subject,
        "conflict",
        c,
    );
    diff_restricting(
        &old.requires,
        &new.requires,
        path,
        &subject,
        "requirement",
        c,
    );
    diff_restricting(
        &old.required_if,
        &new.required_if,
        path,
        &subject,
        "required_if",
        c,
    );
    diff_restricting(
        &required_if_eq(&old.required_if_eq),
        &required_if_eq(&new.required_if_eq),
        path,
        &subject,
        "required_if_eq",
        c,
    );
    diff_restricting(
        &required_if_eq(&old.required_if_eq_all),
        &required_if_eq(&new.required_if_eq_all),
        path,
        &subject,
        "required_if_eq_all",
        c,
    );
    diff_restricting(
        &old.required_unless_all,
        &new.required_unless_all,
        path,
        &subject,
        "required_unless_all",
        c,
    );
    diff_restricting(
        &requires_if(&old.requires_if),
        &requires_if(&new.requires_if),
        path,
        &subject,
        "requires_if",
        c,
    );
    diff_required_unless(
        &old.required_unless,
        &new.required_unless,
        path,
        &subject,
        c,
    );
    diff_relaxing(
        &old.overrides,
        &new.overrides,
        path,
        &subject,
        "override",
        c,
    );

    if !old.exclusive && new.exclusive {
        c.breaking(
            "constraint-added",
            path,
            format!("{subject} is now exclusive, so it cannot appear beside any other flag"),
        );
    } else if old.exclusive && !new.exclusive {
        c.compatible(
            "constraint-removed",
            path,
            format!("{subject} is no longer exclusive"),
        );
    }

    if action_of(old.action) != action_of(new.action) {
        c.breaking(
            "flag-action-changed",
            path,
            format!(
                "{subject} action changed from {} to {}",
                action_of(old.action),
                action_of(new.action)
            ),
        );
    }

    if effect_of(&old.effect) != effect_of(&new.effect) {
        c.metadata(
            "effect-changed",
            path,
            format!(
                "{subject} effect changed from {} to {}",
                effect_of(&old.effect),
                effect_of(&new.effect)
            ),
        );
    }

    diff_deprecation(
        old.deprecated.as_deref(),
        new.deprecated.as_deref(),
        path,
        &subject,
        c,
    );

    if !old.hide && new.hide {
        c.metadata("hidden", path, format!("{subject} is no longer documented"));
    } else if old.hide && !new.hide {
        c.metadata("unhidden", path, format!("{subject} is now documented"));
    }

    if [
        old.help != new.help,
        old.help_long != new.help_long,
        old.help_md != new.help_md,
        old.help_heading != new.help_heading,
    ]
    .iter()
    .any(|changed| *changed)
    {
        c.metadata("help-changed", path, format!("{subject} help text changed"));
    }

    diff_display_order(old.display_order, new.display_order, path, &subject, c);
}

fn diff_args(old: &[SpecArg], new: &[SpecArg], path: &str, c: &mut Changes) {
    for (position, was) in old.iter().enumerate() {
        match new.get(position) {
            Some(now) => {
                if was.name != now.name {
                    // The slot still binds the same word, so nothing a caller types
                    // changes — but the name is what help, docs and `usage exec`'s
                    // environment variables are keyed on.
                    c.metadata(
                        "arg-renamed",
                        path,
                        format!(
                            "argument {} was renamed from <{}> to <{}>",
                            position + 1,
                            was.name,
                            now.name
                        ),
                    );
                }
                let subject = format!("argument <{}>", now.name);
                diff_arg(was, now, path, &subject, c);
            }
            None => c.breaking(
                "arg-removed",
                path,
                format!("argument <{}> was removed", was.name),
            ),
        }
    }
    for now in new.iter().skip(old.len()) {
        if now.required {
            c.breaking(
                "required-arg-added",
                path,
                format!(
                    "required argument <{}> was added, so an invocation without it fails",
                    now.name
                ),
            );
        } else {
            c.compatible(
                "arg-added",
                path,
                format!("argument <{}> was added", now.name),
            );
        }
    }
}

/// The shared half of an argument comparison: a positional and a flag's value are
/// the same [`SpecArg`], so `--jobs <n>` losing its choices reports like `<n>` does.
fn diff_arg(old: &SpecArg, new: &SpecArg, path: &str, subject: &str, c: &mut Changes) {
    if !old.required && new.required {
        c.breaking(
            "arg-now-required",
            path,
            format!("{subject} is now required"),
        );
    } else if old.required && !new.required {
        c.compatible(
            "arg-no-longer-required",
            path,
            format!("{subject} is no longer required"),
        );
    }

    if old.var && !new.var {
        c.breaking(
            "arg-no-longer-variadic",
            path,
            format!("{subject} takes only one value, so extra words are now unexpected"),
        );
    } else if !old.var && new.var {
        c.compatible(
            "arg-now-variadic",
            path,
            format!("{subject} now takes more than one value"),
        );
    }
    diff_var_max(old.var_max, new.var_max, path, subject, c);
    diff_var_min(old.var_min, new.var_min, path, subject, c);

    if old.value_names != new.value_names {
        c.metadata(
            "value-names-changed",
            path,
            format!("{subject} value placeholders changed"),
        );
    }

    if old.delimiter != new.delimiter {
        c.breaking(
            "delimiter-changed",
            path,
            match (old.delimiter, new.delimiter) {
                (Some(was), None) => format!("{subject} no longer splits values on '{was}'"),
                (None, Some(now)) => format!("{subject} now splits values on '{now}'"),
                (Some(was), Some(now)) => {
                    format!("{subject} splits values on '{now}' rather than '{was}'")
                }
                (None, None) => unreachable!("compared unequal"),
            },
        );
    }

    if double_dash_of(&old.double_dash) != double_dash_of(&new.double_dash) {
        c.breaking(
            "double-dash-changed",
            path,
            format!(
                "{subject} double_dash policy changed from {} to {}",
                double_dash_of(&old.double_dash),
                double_dash_of(&new.double_dash)
            ),
        );
    }

    if old.value_terminator != new.value_terminator {
        c.breaking(
            "value-terminator-changed",
            path,
            format!(
                "{subject} value terminator changed from {} to {}",
                option(&old.value_terminator),
                option(&new.value_terminator)
            ),
        );
    }

    if old.sigil != new.sigil {
        c.breaking(
            "sigil-changed",
            path,
            format!(
                "{subject} sigil changed from {} to {}",
                option(&old.sigil),
                option(&new.sigil)
            ),
        );
    }

    if old.allow_negative_numbers && !new.allow_negative_numbers {
        c.breaking(
            "allow-negative-numbers-removed",
            path,
            format!("{subject} no longer accepts a negative number"),
        );
    } else if !old.allow_negative_numbers && new.allow_negative_numbers {
        c.compatible(
            "allow-negative-numbers-added",
            path,
            format!("{subject} now accepts a negative number"),
        );
    }

    diff_choices(old.choices.as_ref(), new.choices.as_ref(), path, subject, c);
    diff_defaults(&old.default, &new.default, path, subject, c);
    diff_env(
        old.env.as_deref(),
        new.env.as_deref(),
        &old.env_fallback,
        &new.env_fallback,
        &old.deprecated_env,
        &new.deprecated_env,
        path,
        subject,
        c,
    );

    if old.validate != new.validate {
        c.breaking(
            "validate-changed",
            path,
            format!(
                "{subject} validation expression changed from {} to {}",
                option(&old.validate),
                option(&new.validate)
            ),
        );
    }

    diff_restricting(&old.conflicts, &new.conflicts, path, subject, "conflict", c);
    diff_restricting(
        &old.requires,
        &new.requires,
        path,
        subject,
        "requirement",
        c,
    );
    diff_restricting(
        &old.required_if,
        &new.required_if,
        path,
        subject,
        "required_if",
        c,
    );
    diff_restricting(
        &required_if_eq(&old.required_if_eq),
        &required_if_eq(&new.required_if_eq),
        path,
        subject,
        "required_if_eq",
        c,
    );
    diff_restricting(
        &required_if_eq(&old.required_if_eq_all),
        &required_if_eq(&new.required_if_eq_all),
        path,
        subject,
        "required_if_eq_all",
        c,
    );
    diff_restricting(
        &old.required_unless_all,
        &new.required_unless_all,
        path,
        subject,
        "required_unless_all",
        c,
    );
    diff_relaxing(
        &old.required_unless,
        &new.required_unless,
        path,
        subject,
        "required_unless",
        c,
    );

    if effect_of(&old.effect) != effect_of(&new.effect) {
        c.metadata(
            "effect-changed",
            path,
            format!(
                "{subject} effect changed from {} to {}",
                effect_of(&old.effect),
                effect_of(&new.effect)
            ),
        );
    }

    if !old.hide && new.hide {
        c.metadata("hidden", path, format!("{subject} is no longer documented"));
    } else if old.hide && !new.hide {
        c.metadata("unhidden", path, format!("{subject} is now documented"));
    }

    if [
        old.help != new.help,
        old.help_long != new.help_long,
        old.help_md != new.help_md,
        old.help_heading != new.help_heading,
    ]
    .iter()
    .any(|changed| *changed)
    {
        c.metadata("help-changed", path, format!("{subject} help text changed"));
    }

    diff_display_order(old.display_order, new.display_order, path, subject, c);
}

fn diff_choices(
    old: Option<&SpecChoices>,
    new: Option<&SpecChoices>,
    path: &str,
    subject: &str,
    c: &mut Changes,
) {
    let was = accepted_values(old);
    let now = accepted_values(new);
    let old_strict = old.is_some_and(|ch| ch.strict);
    let new_strict = new.is_some_and(|ch| ch.strict);

    if !old_strict && new_strict {
        c.breaking(
            "choices-now-strict",
            path,
            format!(
                "{subject} now rejects values outside its declared set ({})",
                now.join(", ")
            ),
        );
    } else if old_strict && !new_strict {
        c.compatible(
            "choices-no-longer-strict",
            path,
            format!("{subject} now accepts values outside its declared set"),
        );
    }

    // Only a strict set can narrow: where anything is accepted, a value leaving
    // the declared list stops being offered rather than stops being accepted.
    for value in only_in(&was, &now) {
        if new_strict {
            c.breaking(
                "choice-removed",
                path,
                format!("{subject} no longer accepts '{value}'"),
            );
        } else {
            c.metadata(
                "choice-unlisted",
                path,
                format!("{subject} no longer offers '{value}', which it still accepts"),
            );
        }
    }
    // The mirror of the removal above, keyed on the *old* strictness: a value only
    // becomes newly acceptable if the old set was the one refusing it. Where anything
    // was already accepted, listing a value starts offering it and nothing more.
    for value in only_in(&now, &was) {
        if old_strict {
            c.compatible(
                "choice-added",
                path,
                format!("{subject} now accepts '{value}'"),
            );
        } else {
            c.metadata(
                "choice-listed",
                path,
                format!("{subject} now offers '{value}', which it already accepted"),
            );
        }
    }

    if old.is_some_and(|ch| ch.ignore_case) && !new.is_some_and(|ch| ch.ignore_case) {
        c.breaking(
            "choices-case-sensitive",
            path,
            format!("{subject} now compares its values case-sensitively"),
        );
    } else if !old.is_some_and(|ch| ch.ignore_case) && new.is_some_and(|ch| ch.ignore_case) {
        c.compatible(
            "choices-ignore-case",
            path,
            format!("{subject} now compares its values without regard to case"),
        );
    }
}

fn diff_groups(old: &[SpecGroup], new: &[SpecGroup], path: &str, c: &mut Changes) {
    for was in old {
        let Some(now) = new.iter().find(|g| g.name == was.name) else {
            c.compatible(
                "group-removed",
                path,
                format!(
                    "group '{}' was removed, so its rule no longer applies",
                    was.name
                ),
            );
            continue;
        };
        let subject = format!("group '{}'", now.name);
        if !was.required && now.required {
            c.breaking(
                "group-now-required",
                path,
                format!("{subject} now requires one of its members"),
            );
        } else if was.required && !now.required {
            c.compatible(
                "group-no-longer-required",
                path,
                format!("{subject} no longer requires one of its members"),
            );
        }
        if was.multiple && !now.multiple {
            c.breaking(
                "group-now-exclusive",
                path,
                format!("{subject} members are now mutually exclusive"),
            );
        } else if !was.multiple && now.multiple {
            c.compatible(
                "group-no-longer-exclusive",
                path,
                format!("{subject} members may now be given together"),
            );
        }
        // In an exclusive group a new member is a new conflict; where members may
        // appear together, membership only decides what satisfies `required`.
        for member in only_in(&now.members, &was.members) {
            if now.multiple {
                c.metadata(
                    "group-member-added",
                    path,
                    format!("{subject} gained member '{member}'"),
                );
            } else {
                c.breaking(
                    "group-member-added",
                    path,
                    format!(
                        "{subject} gained member '{member}', which now conflicts with the rest"
                    ),
                );
            }
        }
        for member in only_in(&was.members, &now.members) {
            if now.required {
                c.breaking(
                    "group-member-removed",
                    path,
                    format!("{subject} lost member '{member}', which no longer satisfies it"),
                );
            } else {
                c.compatible(
                    "group-member-removed",
                    path,
                    format!("{subject} lost member '{member}'"),
                );
            }
        }
    }
    for now in new {
        if old.iter().any(|g| g.name == now.name) {
            continue;
        }
        let subject = format!("group '{}'", now.name);
        if now.required || !now.multiple {
            c.breaking(
                "group-added",
                path,
                format!(
                    "{subject} was added over {}, constraining combinations that were valid",
                    now.members.join(", ")
                ),
            );
        } else {
            c.metadata(
                "group-added",
                path,
                format!("{subject} was added over {}", now.members.join(", ")),
            );
        }
    }
}

fn diff_mounts(old: &SpecCommand, new: &SpecCommand, path: &str, c: &mut Changes) {
    let was: Vec<String> = old.mounts.iter().map(|m| m.run.clone()).collect();
    let now: Vec<String> = new.mounts.iter().map(|m| m.run.clone()).collect();
    for run in only_in(&was, &now) {
        c.breaking(
            "mount-removed",
            path,
            format!("mount '{run}' was removed, so the commands it discovered are gone"),
        );
    }
    for run in only_in(&now, &was) {
        c.compatible("mount-added", path, format!("mount '{run}' was added"));
    }
}

fn diff_subcommands(old: &SpecCommand, new: &SpecCommand, path: &str, c: &mut Changes) {
    // The commands that turned out to be somewhere an old name went. Reported as renames
    // below and skipped by the addition loop: a command reachable by the word that always
    // reached it is not something the interface gained.
    let mut covering: HashSet<String> = HashSet::new();
    for (name, was) in &old.subcommands {
        let child = format!("{path} {name}");
        match new.subcommands.get(name) {
            Some(now) => diff_command(was, now, &child, None, c),
            None => {
                // A word that now selects some other command still works: rustup's
                // `install` reaching `toolchain install` is a rename, not a removal.
                match new.find_subcommand(name) {
                    Some(covering_cmd) => {
                        c.metadata(
                            "cmd-renamed",
                            path,
                            format!(
                                "command '{name}' was renamed to '{}', which still answers to '{name}'",
                                covering_cmd.name
                            ),
                        );
                        // And then compared, because the rename is not the only thing that
                        // may have happened to it. Located under the *old* name: what a
                        // reader wants to know is what typing `{name}` does now, and the
                        // line above says which command that reaches.
                        diff_command(was, covering_cmd, &child, Some(name), c);
                        covering.insert(covering_cmd.name.clone());
                    }
                    None => {
                        c.breaking("cmd-removed", path, format!("command '{name}' was removed"))
                    }
                }
            }
        }
    }
    for (name, now) in &new.subcommands {
        if old.subcommands.contains_key(name) || covering.contains(name) {
            continue;
        }
        if old.find_subcommand(name).is_some() {
            // The name used to be an alias of a sibling and is now a command of its
            // own: what the word selects changed.
            c.breaking(
                "cmd-shadows-alias",
                path,
                format!("command '{name}' now takes a name that was an alias of another command"),
            );
            continue;
        }
        // A word only becomes newly meaningful if the old interface had nothing to do with
        // it. Where it did, the invocation that used to work now reaches somewhere else,
        // which is the definition of breaking rather than an exception to it.
        if old.external_subcommand {
            c.breaking(
                "cmd-shadows-external",
                path,
                format!(
                    "command '{name}' now takes a word that used to be forwarded to an external command"
                ),
            );
            continue;
        }
        if let Some(arg) = old.args.first() {
            c.breaking(
                "cmd-shadows-arg",
                path,
                format!(
                    "command '{name}' now takes a word that used to bind to '{}'",
                    arg.usage()
                ),
            );
            continue;
        }
        c.compatible(
            "cmd-added",
            path,
            format!("command '{}' was added", now.name),
        );
    }
}

/// The config block is interface too: a property is read from the environment and
/// the command line, and a released CLI that stops reading `MISE_JOBS` broke
/// somebody's shell profile as surely as a removed flag would have.
fn diff_config(old: &SpecConfig, new: &SpecConfig, path: &str, c: &mut Changes) {
    // Keys an old property renamed itself to, for the same reason as `covering` above.
    let mut renamed_to: HashSet<String> = HashSet::new();
    for (key, was) in &old.props {
        let Some(now) = new.props.get(key) else {
            // `renamed_to` is a promise about where the value went; it is only kept if
            // the key it names is actually there. A typo, or a target removed in the
            // same edit, is a removal wearing a rename's clothes.
            match was
                .renamed_to
                .as_deref()
                .filter(|to| new.props.contains_key(*to))
            {
                Some(to) => {
                    c.compatible(
                        "config-prop-renamed",
                        path,
                        format!("config property '{key}' was renamed to '{to}'"),
                    );
                    renamed_to.insert(to.to_string());
                    // A rename is not the only thing that can happen to a property in one
                    // release: its type, default, environment names and choices are still
                    // interface, and comparing the old key against the new one is the only
                    // place that reads them.
                    if let Some(now) = new.props.get(to) {
                        diff_config_prop(key, was, now, path, c);
                    }
                }
                None => c.breaking(
                    "config-prop-removed",
                    path,
                    match was.renamed_to.as_deref() {
                        Some(to) => format!(
                            "config property '{key}' was removed, and the '{to}' it renames to \
                             does not exist"
                        ),
                        None => format!("config property '{key}' was removed"),
                    },
                ),
            }
            continue;
        };
        diff_config_prop(key, was, now, path, c);
    }
    for key in new.props.keys() {
        if !old.props.contains_key(key) && !renamed_to.contains(key) {
            c.compatible(
                "config-prop-added",
                path,
                format!("config property '{key}' was added"),
            );
        }
    }
    for name in old.sources.keys() {
        if !new.sources.contains_key(name) {
            c.breaking(
                "config-source-removed",
                path,
                format!(
                    "config source '{name}' was removed, so what it supplied is no longer read"
                ),
            );
        }
    }
    for name in new.sources.keys() {
        if !old.sources.contains_key(name) {
            c.compatible(
                "config-source-added",
                path,
                format!("config source '{name}' was added"),
            );
        }
    }
    let old_files: Vec<String> = old.files.iter().map(|f| f.path.clone()).collect();
    let new_files: Vec<String> = new.files.iter().map(|f| f.path.clone()).collect();
    for file in only_in(&old_files, &new_files) {
        c.breaking(
            "config-file-removed",
            path,
            format!("config file '{file}' is no longer read"),
        );
    }
    for file in only_in(&new_files, &old_files) {
        c.compatible(
            "config-file-added",
            path,
            format!("config file '{file}' is now read"),
        );
    }
}

fn diff_config_prop(
    key: &str,
    old: &SpecConfigProp,
    new: &SpecConfigProp,
    path: &str,
    c: &mut Changes,
) {
    let subject = format!("config property '{key}'");

    let old_type = config_type(old);
    let new_type = config_type(new);
    if old_type != new_type {
        c.breaking(
            "config-type-changed",
            path,
            format!("{subject} type changed from {old_type} to {new_type}"),
        );
    }

    match (&old.default, &new.default) {
        (Some(was), None) => c.breaking(
            "config-default-removed",
            path,
            format!("{subject} no longer defaults to {}", config_value(was)),
        ),
        (None, Some(now)) => c.compatible(
            "config-default-added",
            path,
            format!("{subject} now defaults to {}", config_value(now)),
        ),
        (Some(was), Some(now)) if was != now => c.breaking(
            "config-default-changed",
            path,
            format!(
                "{subject} default changed from {} to {}",
                config_value(was),
                config_value(now)
            ),
        ),
        _ => {}
    }

    let was_envs = config_envs(old);
    let now_envs = config_envs(new);
    for name in only_in(&was_envs, &now_envs) {
        c.breaking(
            "config-env-removed",
            path,
            format!("{subject} no longer reads ${name}"),
        );
    }
    for name in only_in(&now_envs, &was_envs) {
        c.compatible(
            "config-env-added",
            path,
            format!("{subject} now reads ${name}"),
        );
    }

    for alias in only_in(&old.aliases, &new.aliases) {
        c.breaking(
            "config-alias-removed",
            path,
            format!("{subject} no longer answers to the key '{alias}'"),
        );
    }
    for alias in only_in(&new.aliases, &old.aliases) {
        c.compatible(
            "config-alias-added",
            path,
            format!("{subject} now answers to the key '{alias}'"),
        );
    }

    let was_choices: Vec<String> = old
        .choices
        .iter()
        .map(|ch| config_value(&ch.value))
        .collect();
    let now_choices: Vec<String> = new
        .choices
        .iter()
        .map(|ch| config_value(&ch.value))
        .collect();
    for value in only_in(&was_choices, &now_choices) {
        c.breaking(
            "config-choice-removed",
            path,
            format!("{subject} no longer accepts {value}"),
        );
    }
    for value in only_in(&now_choices, &was_choices) {
        c.compatible(
            "config-choice-added",
            path,
            format!("{subject} now accepts {value}"),
        );
    }

    if merge_of(&old.merge) != merge_of(&new.merge) {
        c.breaking(
            "config-merge-changed",
            path,
            format!(
                "{subject} merge policy changed from {} to {}",
                merge_of(&old.merge),
                merge_of(&new.merge)
            ),
        );
    }
    if scope_of(&old.scope) != scope_of(&new.scope) {
        c.breaking(
            "config-scope-changed",
            path,
            format!(
                "{subject} scope changed from {} to {}",
                scope_of(&old.scope),
                scope_of(&new.scope)
            ),
        );
    }

    if config_prop_is_optional(old) && !config_prop_is_optional(new) {
        c.breaking(
            "config-now-required",
            path,
            format!("{subject} must now have a value"),
        );
    } else if !config_prop_is_optional(old) && config_prop_is_optional(new) {
        c.compatible(
            "config-now-optional",
            path,
            format!("{subject} may now be absent"),
        );
    }

    diff_deprecation(
        old.deprecated.as_deref(),
        new.deprecated.as_deref(),
        path,
        &subject,
        c,
    );

    if old.help != new.help || old.long_help != new.long_help {
        c.metadata("help-changed", path, format!("{subject} help text changed"));
    }
}

/// Deprecation is an announcement rather than a change of behaviour: the flag still
/// parses, so both directions are metadata. Reported all the same, because a
/// release note wants it and because `usage diff` is where the removal it promises
/// will later show up as breaking.
fn diff_deprecation(
    old: Option<&str>,
    new: Option<&str>,
    path: &str,
    subject: &str,
    c: &mut Changes,
) {
    match (old, new) {
        (None, Some(_)) => c.metadata("deprecated", path, format!("{subject} was deprecated")),
        (Some(_), None) => c.metadata(
            "undeprecated",
            path,
            format!("{subject} is no longer deprecated"),
        ),
        _ => {}
    }
}

/// A default a caller was relying on. Adding one fills a hole — nothing was
/// resolved there before — while changing or removing one moves ground the caller
/// was already standing on.
fn diff_defaults(old: &[String], new: &[String], path: &str, subject: &str, c: &mut Changes) {
    if old == new {
        return;
    }
    match (old.is_empty(), new.is_empty()) {
        (true, false) => c.compatible(
            "default-added",
            path,
            format!("{subject} now defaults to {}", new.join(", ")),
        ),
        (false, true) => c.breaking(
            "default-removed",
            path,
            format!("{subject} no longer defaults to {}", old.join(", ")),
        ),
        _ => c.breaking(
            "default-changed",
            path,
            format!(
                "{subject} default changed from {} to {}",
                old.join(", "),
                new.join(", ")
            ),
        ),
    }
}

#[allow(clippy::too_many_arguments)]
fn diff_env(
    old_env: Option<&str>,
    new_env: Option<&str>,
    old_fallback: &[String],
    new_fallback: &[String],
    old_deprecated: &[String],
    new_deprecated: &[String],
    path: &str,
    subject: &str,
    c: &mut Changes,
) {
    let was = env_names(old_env, old_fallback, old_deprecated);
    let now = env_names(new_env, new_fallback, new_deprecated);
    for name in only_in(&was, &now) {
        c.breaking(
            "env-removed",
            path,
            format!("{subject} no longer reads ${name}"),
        );
    }
    for name in only_in(&now, &was) {
        c.compatible("env-added", path, format!("{subject} now reads ${name}"));
    }
    // Order decides which of several names wins, so a reordering changes what a
    // shell with both of them set resolves to.
    if order_changed(&was, &now) {
        c.breaking(
            "env-order-changed",
            path,
            format!("{subject} consults its environment names in a different order"),
        );
    }
}

/// Whether the entries both lists hold appear in a different relative order.
///
/// Order is precedence in both places this is used: the first environment name that is
/// set wins, and the first matching `default_if` condition wins. So two lists holding
/// the same entries in a different order can resolve one command line to two different
/// values. Only the shared entries are compared, because what was added or removed is
/// reported on its own and would otherwise show up twice.
fn order_changed(old: &[String], new: &[String]) -> bool {
    let was: Vec<&String> = old.iter().filter(|entry| new.contains(entry)).collect();
    let now: Vec<&String> = new.iter().filter(|entry| old.contains(entry)).collect();
    was != now
}

/// A list whose entries each impose a rule: gaining one rejects command lines that
/// used to be valid, losing one accepts more.
fn diff_restricting(
    old: &[String],
    new: &[String],
    path: &str,
    subject: &str,
    what: &str,
    c: &mut Changes,
) {
    for entry in only_in(new, old) {
        c.breaking(
            "constraint-added",
            path,
            format!("{subject} gained {what} '{entry}'"),
        );
    }
    for entry in only_in(old, new) {
        c.compatible(
            "constraint-removed",
            path,
            format!("{subject} lost {what} '{entry}'"),
        );
    }
}

/// The mirror of [`diff_restricting`]: entries that relieve a rule, so gaining one
/// accepts more.
/// Whether absence is a legitimate value for this property.
///
/// `optional = None` is not "required": the spec's rule is that a property with no default,
/// or one typed `option<T>`, is optional unless it says otherwise. Reading `None` as `false`
/// reports a no-default property gaining an explicit `optional=#false` as a change when the
/// contract did not move, and misses the ones where it did.
fn config_prop_is_optional(prop: &SpecConfigProp) -> bool {
    prop.optional.unwrap_or_else(|| {
        prop.default.is_none() && prop.default_list.is_empty()
            || prop.value_type.as_ref().is_some_and(|t| t.is_optional())
    })
}

/// `required_unless`, which is neither restricting nor relaxing but both in turn.
///
/// A non-empty list makes the declaration required unless one of the selectors is present,
/// so declaring one at all is where the requirement appears — and once there is a list, each
/// further entry is one more way to be excused from it. Comparing the entries alone reported
/// a flag becoming conditionally required as compatible, which is backwards.
fn diff_required_unless(
    old: &[String],
    new: &[String],
    path: &str,
    subject: &str,
    c: &mut Changes,
) {
    match (old.is_empty(), new.is_empty()) {
        (true, false) => c.breaking(
            "constraint-added",
            path,
            format!("{subject} is now required unless {}", one_of(new)),
        ),
        (false, true) => c.compatible(
            "constraint-removed",
            path,
            format!("{subject} is no longer required unless {}", one_of(old)),
        ),
        // Both non-empty: another selector is another excuse, and losing one takes an excuse
        // away. That is what `diff_relaxing` says.
        _ => diff_relaxing(old, new, path, subject, "required_unless", c),
    }
}

fn one_of(selectors: &[String]) -> String {
    selectors
        .iter()
        .map(|s| format!("'{s}'"))
        .collect::<Vec<_>>()
        .join(" or ")
}

fn diff_relaxing(
    old: &[String],
    new: &[String],
    path: &str,
    subject: &str,
    what: &str,
    c: &mut Changes,
) {
    for entry in only_in(new, old) {
        c.compatible(
            "constraint-removed",
            path,
            format!("{subject} gained {what} '{entry}'"),
        );
    }
    for entry in only_in(old, new) {
        c.breaking(
            "constraint-added",
            path,
            format!("{subject} lost {what} '{entry}'"),
        );
    }
}

/// One `unknown_flags` declaration against its counterpart.
///
/// Compared where it is written — the spec root, or the command that overrides it —
/// rather than as the value in force at each command. The parser resolves it by
/// inheritance, so comparing effective values reports one edited root node once per
/// descendant, and a child that merely restates what it inherited as a change.
fn diff_unknown_flags(
    old: Option<UnknownFlags>,
    new: Option<UnknownFlags>,
    path: &str,
    c: &mut Changes,
) {
    let was_strict = matches!(old, Some(UnknownFlags::Error));
    let is_strict = matches!(new, Some(UnknownFlags::Error));
    if !was_strict && is_strict {
        c.breaking(
            "unknown-flags-strict",
            path,
            "an undeclared flag is now an error rather than a value".to_string(),
        );
    } else if was_strict && !is_strict {
        c.compatible(
            "unknown-flags-lax",
            path,
            "an undeclared flag is now a value rather than an error".to_string(),
        );
    }
}

/// Where a flag, argument or command sits in help output.
///
/// Presentation only — nothing about parsing moves — but it is a declaration somebody
/// made deliberately, and the documentation lists it among the things a metadata finding
/// covers, so it is compared rather than promised.
fn diff_display_order(
    old: Option<usize>,
    new: Option<usize>,
    path: &str,
    subject: &str,
    c: &mut Changes,
) {
    if old == new {
        return;
    }
    let describe =
        |order: Option<usize>| order.map_or("declaration order".to_string(), |o| o.to_string());
    c.metadata(
        "display-order-changed",
        path,
        format!(
            "{subject} moves from {} to {} in help",
            describe(old),
            describe(new)
        ),
    );
}

/// How many values are required. Flags carry this as well as arguments do, and a flag
/// that needs two values where it needed one rejects a command line that used to work.
fn diff_var_min(
    old: Option<usize>,
    new: Option<usize>,
    path: &str,
    subject: &str,
    c: &mut Changes,
) {
    let floor = |v: Option<usize>| v.unwrap_or(0);
    if floor(new) > floor(old) {
        c.breaking(
            "var-min-raised",
            path,
            format!("{subject} now needs at least {} values", floor(new)),
        );
    } else if floor(new) < floor(old) {
        c.compatible(
            "var-min-lowered",
            path,
            format!("{subject} now needs at least {} values", floor(new)),
        );
    }
}

fn diff_var_max(
    old: Option<usize>,
    new: Option<usize>,
    path: &str,
    subject: &str,
    c: &mut Changes,
) {
    // `None` is unbounded, so it is the largest value rather than the smallest.
    let ceiling = |v: Option<usize>| v.unwrap_or(usize::MAX);
    if ceiling(new) < ceiling(old) {
        c.breaking(
            "var-max-lowered",
            path,
            format!(
                "{subject} now takes at most {} values",
                new.map_or("unlimited".to_string(), |v| v.to_string())
            ),
        );
    } else if ceiling(new) > ceiling(old) {
        c.compatible(
            "var-max-raised",
            path,
            format!(
                "{subject} now takes at most {} values",
                new.map_or("unlimited".to_string(), |v| v.to_string())
            ),
        );
    }
}

fn only_in<'a>(these: &'a [String], those: &[String]) -> Vec<&'a String> {
    these.iter().filter(|s| !those.contains(s)).collect()
}

fn flag_spellings(flag: &SpecFlag) -> Vec<String> {
    flag.long
        .iter()
        .chain(&flag.hidden_aliases)
        .map(|long| format!("--{long}"))
        .chain(
            flag.short
                .iter()
                .chain(&flag.hidden_short_aliases)
                .map(|short| format!("-{short}")),
        )
        .collect()
}

/// What to call a flag in a message: its first visible spelling, falling back to
/// the internal name for a flag that has only hidden ones.
fn primary_spelling(flag: &SpecFlag) -> String {
    flag.long
        .first()
        .map(|long| format!("--{long}"))
        .or_else(|| flag.short.first().map(|short| format!("-{short}")))
        .unwrap_or_else(|| flag.name.clone())
}

fn accepted_values(choices: Option<&SpecChoices>) -> Vec<String> {
    let Some(choices) = choices else {
        return vec![];
    };
    let mut values = choices.choices.clone();
    for detail in &choices.details {
        if !values.contains(&detail.value) {
            values.push(detail.value.clone());
        }
        for alias in &detail.aliases {
            if !values.contains(&alias.value) {
                values.push(alias.value.clone());
            }
        }
    }
    values
}

fn env_names(env: Option<&str>, fallback: &[String], deprecated: &[String]) -> Vec<String> {
    env.map(str::to_string)
        .into_iter()
        .chain(fallback.iter().cloned())
        .chain(deprecated.iter().cloned())
        .collect()
}

fn required_if_eq(entries: &[usage::spec::arg::SpecRequiredIfEq]) -> Vec<String> {
    entries
        .iter()
        .map(|e| format!("{}={}", e.selector, e.value))
        .collect()
}

fn default_if(entries: &[usage::spec::flag::SpecDefaultIf]) -> Vec<String> {
    entries
        .iter()
        .map(|e| match &e.when {
            Some(when) => format!("'{}' when {}={when}", e.value, e.selector),
            None => format!("'{}' when {} is given", e.value, e.selector),
        })
        .collect()
}

fn requires_if(entries: &[usage::spec::flag::SpecRequiresIf]) -> Vec<String> {
    entries
        .iter()
        .map(|e| format!("{}={}", e.value, e.requires))
        .collect()
}

fn config_envs(prop: &SpecConfigProp) -> Vec<String> {
    // `env` restates `envs[0]` — the spec keeps both so a programmatically built prop
    // serializes — so reading both would report the first name twice.
    let named = if prop.envs.is_empty() {
        prop.env.clone().into_iter().collect()
    } else {
        prop.envs.clone()
    };
    named
        .into_iter()
        .chain(prop.deprecated_envs.iter().cloned())
        .collect()
}

fn config_type(prop: &SpecConfigProp) -> String {
    match &prop.value_type {
        Some(value_type) => value_type.to_string(),
        None => prop.data_type.to_string(),
    }
}

fn config_value(value: &SpecConfigValue) -> String {
    match value {
        SpecConfigValue::Bool(b) => b.to_string(),
        SpecConfigValue::Int(i) => i.to_string(),
        SpecConfigValue::Float(f) => f.to_string(),
        SpecConfigValue::String(s) => format!("\"{s}\""),
    }
}

fn merge_of(merge: &usage::spec::config::SpecConfigMerge) -> &'static str {
    use usage::spec::config::SpecConfigMerge::*;
    match merge {
        Replace => "replace",
        Union => "union",
        Deep => "deep",
    }
}

fn scope_of(scope: &usage::spec::config::SpecConfigScope) -> &'static str {
    use usage::spec::config::SpecConfigScope::*;
    match scope {
        Any => "any",
        Global => "global",
        Env => "env",
    }
}

fn effect_of(effect: &Option<usage::spec::effect::SpecCommandEffect>) -> &'static str {
    match effect {
        Some(effect) => effect.as_str(),
        None => "unset",
    }
}

fn action_of(action: usage::spec::flag::SpecFlagAction) -> &'static str {
    action.as_str()
}

fn double_dash_of(choice: &usage::spec::arg::SpecDoubleDashChoices) -> String {
    choice.to_string()
}

fn option<T: std::fmt::Display>(value: &Option<T>) -> String {
    value
        .as_ref()
        .map(|v| format!("'{v}'"))
        .unwrap_or_else(|| "unset".to_string())
}

#[cfg(test)]
mod tests {
    use super::*;

    fn changes(old: &str, new: &str) -> Vec<SpecChange> {
        let old: Spec = old.parse().unwrap();
        let new: Spec = new.parse().unwrap();
        diff_specs(&old, &new)
    }

    /// `category:code` per finding, which is what most of these tests assert on.
    fn codes(old: &str, new: &str) -> Vec<String> {
        changes(old, new)
            .iter()
            .map(|c| format!("{}:{}", c.category, c.code))
            .collect()
    }

    fn find<'a>(changes: &'a [SpecChange], code: &str) -> &'a SpecChange {
        changes
            .iter()
            .find(|c| c.code == code)
            .unwrap_or_else(|| panic!("no {code} in {changes:?}"))
    }

    const BASE: &str = r#"
name "ex"
bin "ex"
flag "-j --jobs <n>" help="jobs"
flag "-f --force" help="force"
arg "<file>" help="file"
cmd "run" help="run" {
    flag "--watch" help="watch"
    arg "<task>" help="task"
}
    "#;

    #[test]
    fn a_spec_against_itself_reports_nothing() {
        assert!(codes(BASE, BASE).is_empty());
    }

    #[test]
    fn clause_shape_changes_are_breaking() {
        let plain = r#"
name "ex"
bin "ex"
"#;
        let colon = r#"
name "ex"
bin "ex"
clause "tasks" separator=":::" {
    arg "<task>"
}
"#;
        let plus = r#"
name "ex"
bin "ex"
clause "tasks" separator="+++" {
    arg "<task>"
}
"#;

        assert_eq!(codes(plain, colon), ["breaking:clause-added"]);
        assert_eq!(codes(colon, plain), ["breaking:clause-removed"]);
        assert_eq!(codes(colon, plus), ["breaking:clause-separator-changed"]);
    }

    #[test]
    fn version_is_never_reported() {
        // A release bumps it. A compatibility check that fires on every release is one
        // nobody leaves switched on, which is the whole reason this is silent.
        let new = format!("{BASE}\nversion \"9.9.9\"\nlong_version \"9.9.9 (abc)\"\n");
        assert!(codes(BASE, &new).is_empty(), "{:?}", codes(BASE, &new));
    }

    #[test]
    fn breaking_findings_sort_first() {
        let new = r#"
name "ex"
bin "ex"
flag "-j --jobs <n>" help="jobs"
flag "-f --force" help="force it"
flag "--quiet" help="quiet"
arg "<file>" help="file"
        "#;
        let found = codes(BASE, new);
        let first_metadata = found.iter().position(|c| c.starts_with("metadata:"));
        let last_breaking = found.iter().rposition(|c| c.starts_with("breaking:"));
        assert!(
            last_breaking.unwrap() < first_metadata.unwrap(),
            "{found:?}"
        );
    }

    #[test]
    fn a_removed_flag_is_breaking_and_a_lost_short_is_too() {
        let new = r#"
name "ex"
bin "ex"
flag "--jobs <n>" help="jobs"
arg "<file>" help="file"
cmd "run" help="run" {
    flag "--watch" help="watch"
    arg "<task>" help="task"
}
        "#;
        let found = changes(BASE, new);
        assert_eq!(
            find(&found, "flag-removed").message,
            "flag 'force' (--force, -f) was removed"
        );
        assert_eq!(
            find(&found, "flag-spelling-removed").message,
            "flag '--jobs' no longer answers to '-j'"
        );
    }

    #[test]
    fn a_flag_that_kept_every_spelling_was_renamed_not_removed() {
        // The internal name is not something a caller can type, so putting a new
        // spelling in front of `--jobs` renames the flag without breaking anybody —
        // and the spelling it gained is still reported.
        let old = r#"
name "ex"
bin "ex"
flag "-j --jobs <n>" help="jobs"
        "#;
        let new = r#"
name "ex"
bin "ex"
flag "--parallelism -j --jobs <n>" help="jobs"
        "#;
        let found = changes(old, new);
        assert_eq!(
            find(&found, "flag-renamed").message,
            "flag 'jobs' was renamed to 'parallelism'"
        );
        assert_eq!(
            find(&found, "flag-spelling-added").message,
            "flag '--parallelism' now answers to '--parallelism'"
        );
        assert!(
            found.iter().all(|c| c.category != Category::Breaking),
            "{found:?}"
        );
    }

    #[test]
    fn a_new_required_flag_is_breaking_and_an_optional_one_is_not() {
        let required = format!("{BASE}\nflag \"--token <t>\" help=\"token\" required=#true\n");
        assert_eq!(codes(BASE, &required), ["breaking:required-flag-added"]);
        let optional = format!("{BASE}\nflag \"--token <t>\" help=\"token\"\n");
        assert_eq!(codes(BASE, &optional), ["compatible:flag-added"]);
    }

    #[test]
    fn a_narrowed_strict_choice_set_is_breaking_and_a_widened_one_is_not() {
        let old = r#"
name "ex"
bin "ex"
flag "--color <when>" help="color" {
    choices "auto" "always" "never"
}
        "#;
        let narrowed = r#"
name "ex"
bin "ex"
flag "--color <when>" help="color" {
    choices "auto" "always"
}
        "#;
        let widened = r#"
name "ex"
bin "ex"
flag "--color <when>" help="color" {
    choices "auto" "always" "never" "if-tty"
}
        "#;
        assert_eq!(codes(old, narrowed), ["breaking:choice-removed"]);
        assert_eq!(codes(old, widened), ["compatible:choice-added"]);
    }

    #[test]
    fn dropping_a_value_from_a_non_strict_set_only_stops_offering_it() {
        // `strict=#false` accepts anything, so leaving the list costs a completion
        // candidate rather than an accepted value.
        let old = r#"
name "ex"
bin "ex"
flag "--backend <b>" help="backend" {
    choices "npm" "cargo" strict=#false
}
        "#;
        let new = r#"
name "ex"
bin "ex"
flag "--backend <b>" help="backend" {
    choices "npm" strict=#false
}
        "#;
        assert_eq!(codes(old, new), ["metadata:choice-unlisted"]);
    }

    #[test]
    fn a_default_is_free_to_gain_and_costly_to_move() {
        let none = "name \"ex\"\nbin \"ex\"\nflag \"--jobs <n>\" help=\"jobs\"\n";
        let four = "name \"ex\"\nbin \"ex\"\nflag \"--jobs <n>\" help=\"jobs\" default=\"4\"\n";
        let eight = "name \"ex\"\nbin \"ex\"\nflag \"--jobs <n>\" help=\"jobs\" default=\"8\"\n";
        assert_eq!(codes(none, four), ["compatible:default-added"]);
        assert_eq!(codes(four, eight), ["breaking:default-changed"]);
        assert_eq!(codes(four, none), ["breaking:default-removed"]);
    }

    #[test]
    fn a_positional_appended_after_the_last_one_is_only_breaking_when_required() {
        let required = r#"
name "ex"
bin "ex"
arg "<file>" help="file"
arg "<out>" help="out"
        "#;
        let optional = r#"
name "ex"
bin "ex"
arg "<file>" help="file"
arg "[out]" help="out"
        "#;
        let one = "name \"ex\"\nbin \"ex\"\narg \"<file>\" help=\"file\"\n";
        assert_eq!(codes(one, required), ["breaking:required-arg-added"]);
        assert_eq!(codes(one, optional), ["compatible:arg-added"]);
        assert_eq!(codes(required, one), ["breaking:arg-removed"]);
    }

    #[test]
    fn renaming_a_positional_leaves_the_slot_alone() {
        let old = "name \"ex\"\nbin \"ex\"\narg \"<file>\" help=\"a file\"\n";
        let new = "name \"ex\"\nbin \"ex\"\narg \"<path>\" help=\"a file\"\n";
        let found = changes(old, new);
        assert_eq!(codes(old, new), ["metadata:arg-renamed"]);
        assert_eq!(
            find(&found, "arg-renamed").message,
            "argument 1 was renamed from <file> to <path>"
        );
    }

    #[test]
    fn a_removed_command_is_breaking_unless_something_still_answers_to_it() {
        let old = r#"
name "ex"
bin "ex"
cmd "install" help="install"
        "#;
        let gone = "name \"ex\"\nbin \"ex\"\n";
        let renamed = r#"
name "ex"
bin "ex"
cmd "add" help="install" {
    alias "install"
}
        "#;
        assert_eq!(codes(old, gone), ["breaking:cmd-removed"]);
        let found = changes(old, renamed);
        assert_eq!(
            find(&found, "cmd-renamed").message,
            "command 'install' was renamed to 'add', which still answers to 'install'"
        );
        assert!(
            found.iter().all(|c| c.category != Category::Breaking),
            "{found:?}"
        );
    }

    #[test]
    fn a_command_taking_over_a_siblings_alias_changes_what_the_word_selects() {
        let old = r#"
name "ex"
bin "ex"
cmd "remove" help="remove" {
    alias "rm"
}
        "#;
        let new = r#"
name "ex"
bin "ex"
cmd "remove" help="remove"
cmd "rm" help="something else"
        "#;
        let found = codes(old, new);
        assert!(
            found.contains(&"breaking:cmd-shadows-alias".to_string()),
            "{found:?}"
        );
    }

    #[test]
    fn a_command_capturing_a_word_the_old_spec_forwarded_is_breaking() {
        let old = r#"
name "ex"
bin "ex"
external_subcommand #true
cmd "build" help="build"
        "#;
        let new = r#"
name "ex"
bin "ex"
external_subcommand #true
cmd "build" help="build"
cmd "deploy" help="deploy"
        "#;

        // `ex deploy` used to run `ex-deploy`. It now runs the built-in, which is a command
        // line that worked and now does something else — the definition of breaking.
        let found = codes(old, new);
        assert!(
            found.contains(&"breaking:cmd-shadows-external".to_string()),
            "{found:?}"
        );
    }

    #[test]
    fn a_command_capturing_a_word_an_argument_took_is_breaking() {
        let old = r#"
name "ex"
bin "ex"
arg "<task>" help="task"
        "#;
        let new = r#"
name "ex"
bin "ex"
arg "<task>" help="task"
cmd "list" help="list"
        "#;

        // `ex list` used to pass "list" to <task>. The word now selects a subcommand.
        let found = codes(old, new);
        assert!(
            found.contains(&"breaking:cmd-shadows-arg".to_string()),
            "{found:?}"
        );
    }

    #[test]
    fn a_command_added_where_nothing_took_the_word_is_compatible() {
        let old = "name \"ex\"\nbin \"ex\"\ncmd \"build\" help=\"build\"\n";
        let new =
            "name \"ex\"\nbin \"ex\"\ncmd \"build\" help=\"build\"\ncmd \"test\" help=\"test\"\n";

        assert_eq!(codes(old, new), ["compatible:cmd-added"]);
    }

    #[test]
    fn declaring_required_unless_is_the_requirement_appearing() {
        let old = "name \"ex\"\nbin \"ex\"\nflag \"--token <t>\" help=\"token\"\nflag \"--anon\" help=\"anon\"\n";
        let new = "name \"ex\"\nbin \"ex\"\nflag \"--token <t>\" help=\"token\" required_unless=\"--anon\"\nflag \"--anon\" help=\"anon\"\n";

        // `ex` on its own worked and now fails: a non-empty `required_unless` makes the flag
        // required unless one of its selectors is there. Reading only the entries called
        // that compatible.
        assert_eq!(codes(old, new), ["breaking:constraint-added"]);
        assert_eq!(codes(new, old), ["compatible:constraint-removed"]);
    }

    #[test]
    fn another_way_out_of_required_unless_is_still_a_relaxation() {
        let one = r#"
name "ex"
bin "ex"
flag "--token <t>" help="token" {
    required_unless "--anon"
}
flag "--anon" help="anon"
flag "--guest" help="guest"
        "#;
        let two = r#"
name "ex"
bin "ex"
flag "--token <t>" help="token" {
    required_unless "--anon" "--guest"
}
flag "--anon" help="anon"
flag "--guest" help="guest"
        "#;

        // Once there is a list, each further entry is one more excuse from the requirement.
        assert_eq!(codes(one, two), ["compatible:constraint-removed"]);
        assert_eq!(codes(two, one), ["breaking:constraint-added"]);
    }

    #[test]
    fn losing_subcommand_precedence_changes_what_a_word_binds_to() {
        let old = "name \"ex\"\nbin \"ex\"\nsubcommand_precedence_over_arg #true\narg \"<task>\" help=\"task\"\ncmd \"list\" help=\"list\"\n";
        let new =
            "name \"ex\"\nbin \"ex\"\narg \"<task>\" help=\"task\"\ncmd \"list\" help=\"list\"\n";

        // `ex list` reached the subcommand and now fills `<task>`. Nothing fails, which is
        // exactly why a gate should say so.
        assert_eq!(codes(old, new), ["breaking:subcommand-precedence-removed"]);
    }

    #[test]
    fn letting_arguments_and_subcommands_share_a_line_is_compatible() {
        let old = "name \"ex\"\nbin \"ex\"\nargs_conflicts_with_subcommands #true\narg \"[task]\" help=\"task\"\ncmd \"list\" help=\"list\"\n";
        let new =
            "name \"ex\"\nbin \"ex\"\narg \"[task]\" help=\"task\"\ncmd \"list\" help=\"list\"\n";

        assert_eq!(
            codes(old, new),
            ["compatible:args-conflicts-with-subcommands-removed"]
        );
    }

    #[test]
    fn a_rename_is_one_finding_rather_than_a_rename_and_an_addition() {
        let old = "name \"ex\"\nbin \"ex\"\ncmd \"install\" help=\"install\"\n";
        let new =
            "name \"ex\"\nbin \"ex\"\ncmd \"add\" help=\"install\" {\n    alias \"install\"\n}\n";

        // `add` is where `install` went, not something the interface gained.
        assert_eq!(codes(old, new), ["metadata:cmd-renamed"]);
    }

    #[test]
    fn a_renamed_config_property_is_compared_to_what_it_became() {
        let old = r#"
name "ex"
bin "ex"
config {
    prop "jobs" type="int" default=1 env="EX_JOBS" renamed_to="workers"
}
        "#;
        let new = r#"
name "ex"
bin "ex"
config {
    prop "workers" type="int" default=2 env="EX_JOBS"
}
        "#;

        // One rename, and the default that moved underneath it — not a rename plus an
        // addition, with the change of default never looked at.
        let found = codes(old, new);
        assert!(
            found.contains(&"compatible:config-prop-renamed".to_string()),
            "{found:?}"
        );
        assert!(
            !found.contains(&"compatible:config-prop-added".to_string()),
            "{found:?}"
        );
        assert!(
            found.iter().any(|c| c.ends_with(":config-default-changed")),
            "{found:?}"
        );
    }

    #[test]
    fn a_property_with_no_default_is_already_optional() {
        let old = r#"
name "ex"
bin "ex"
config {
    prop "token" type="string"
}
        "#;
        let new = r#"
name "ex"
bin "ex"
config {
    prop "token" type="string" optional=#true
}
        "#;

        // `optional` unset is not `optional=#false`: the spec's rule is that a property with
        // no default is optional. Writing down what was already true is not a change.
        assert!(codes(old, new).is_empty(), "{:?}", codes(old, new));
    }

    #[test]
    fn findings_are_located_by_command_path() {
        let new = r#"
name "ex"
bin "ex"
flag "-j --jobs <n>" help="jobs"
flag "-f --force" help="force"
arg "<file>" help="file"
cmd "run" help="run" {
    arg "<task>" help="task"
}
        "#;
        let found = changes(BASE, new);
        assert_eq!(find(&found, "flag-removed").location, "ex run");
    }

    #[test]
    fn a_lost_environment_variable_is_breaking() {
        let old = r#"
name "ex"
bin "ex"
flag "--jobs <n>" help="jobs" env="EX_JOBS" {
    env_fallback "EX_PARALLEL"
}
        "#;
        let new = "name \"ex\"\nbin \"ex\"\nflag \"--jobs <n>\" help=\"jobs\" env=\"EX_JOBS\"\n";
        let found = changes(old, new);
        assert_eq!(
            find(&found, "env-removed").message,
            "flag '--jobs' no longer reads $EX_PARALLEL"
        );
        assert_eq!(codes(new, old), ["compatible:env-added"]);
    }

    #[test]
    fn a_gained_conflict_restricts_and_a_gained_override_relaxes() {
        let base = r#"
name "ex"
bin "ex"
flag "--file <f>" help="file"
flag "--stdin" help="stdin"
        "#;
        let conflicting = r#"
name "ex"
bin "ex"
flag "--file <f>" help="file" conflicts="--stdin"
flag "--stdin" help="stdin"
        "#;
        let overriding = r#"
name "ex"
bin "ex"
flag "--file <f>" help="file" overrides="--stdin"
flag "--stdin" help="stdin"
        "#;
        assert_eq!(codes(base, conflicting), ["breaking:constraint-added"]);
        assert_eq!(codes(base, overriding), ["compatible:constraint-removed"]);
        // And the same edits read the other way round.
        assert_eq!(codes(conflicting, base), ["compatible:constraint-removed"]);
        assert_eq!(codes(overriding, base), ["breaking:constraint-added"]);
    }

    #[test]
    fn a_new_exclusive_group_constrains_and_a_multiple_one_does_not() {
        let base = r#"
name "ex"
bin "ex"
flag "--file <f>" help="file"
flag "--url <u>" help="url"
        "#;
        let exclusive = format!("{base}\ngroup \"input\" \"--file\" \"--url\"\n");
        let permissive = format!("{base}\ngroup \"input\" \"--file\" \"--url\" multiple=#true\n");
        assert_eq!(codes(base, &exclusive), ["breaking:group-added"]);
        assert_eq!(codes(base, &permissive), ["metadata:group-added"]);
    }

    #[test]
    fn strict_unknown_flags_reject_what_used_to_bind() {
        let old = "name \"ex\"\nbin \"ex\"\narg \"[words]\" help=\"words\" var=#true\n";
        let new = format!("{old}unknown_flags \"error\"\n");
        assert_eq!(codes(old, &new), ["breaking:unknown-flags-strict"]);
        assert_eq!(codes(&new, old), ["compatible:unknown-flags-lax"]);
    }

    #[test]
    fn a_config_property_is_interface_too() {
        let old = r#"
name "ex"
bin "ex"
config {
    prop "jobs" type="uint" default=4 help="jobs" {
        env "EX_JOBS"
    }
    prop "color" type="bool" default=#true help="color"
}
        "#;
        let new = r#"
name "ex"
bin "ex"
config {
    prop "jobs" type="uint" default=8 help="jobs"
}
        "#;
        let found = changes(old, new);
        assert_eq!(
            find(&found, "config-default-changed").message,
            "config property 'jobs' default changed from 4 to 8"
        );
        assert_eq!(
            find(&found, "config-env-removed").message,
            "config property 'jobs' no longer reads $EX_JOBS"
        );
        assert_eq!(
            find(&found, "config-prop-removed").message,
            "config property 'color' was removed"
        );
    }

    #[test]
    fn a_renamed_config_property_says_where_it_went() {
        let old = r#"
name "ex"
bin "ex"
config {
    prop "jobs" type="uint" help="jobs" renamed_to="parallelism"
}
        "#;
        let new = r#"
name "ex"
bin "ex"
config {
    prop "parallelism" type="uint" help="jobs"
}
        "#;
        let found = codes(old, new);
        assert!(
            found.contains(&"compatible:config-prop-renamed".to_string()),
            "{found:?}"
        );
        assert!(
            !found.iter().any(|c| c.starts_with("breaking:")),
            "{found:?}"
        );
    }

    #[test]
    fn help_text_and_effect_are_metadata() {
        let old = r#"
name "ex"
bin "ex"
cmd "rm" help="remove" effect="write"
        "#;
        let new = r#"
name "ex"
bin "ex"
cmd "rm" help="delete a thing" effect="destructive"
        "#;
        let found = codes(old, new);
        assert_eq!(found.len(), 2, "{found:?}");
        assert!(
            found.iter().all(|c| c.starts_with("metadata:")),
            "{found:?}"
        );
    }

    #[test]
    fn a_renamed_command_is_still_compared_to_what_it_became() {
        // The rename is not the only thing that can happen in one release, and a break
        // inside the command would otherwise hide behind the alias that covers its name.
        let old = r#"
name "ex"
bin "ex"
cmd "install" help="install" {
    flag "--force" help="force"
    arg "<pkg>" help="package"
}
        "#;
        let new = r#"
name "ex"
bin "ex"
cmd "add" help="install" {
    alias "install"
    arg "<pkg>" help="package"
}
        "#;
        let found = changes(old, new);
        assert_eq!(
            find(&found, "cmd-renamed").message,
            "command 'install' was renamed to 'add', which still answers to 'install'"
        );
        let removed = find(&found, "flag-removed");
        assert_eq!(removed.category, Category::Breaking);
        assert_eq!(removed.location, "ex install");
    }

    #[test]
    fn a_config_rename_pointing_nowhere_is_a_removal() {
        let old = r#"
name "ex"
bin "ex"
config {
    prop "jobs" type="uint" help="jobs" renamed_to="parallelism"
}
        "#;
        let new = "name \"ex\"\nbin \"ex\"\n";
        let found = changes(old, new);
        let removed = find(&found, "config-prop-removed");
        assert_eq!(removed.category, Category::Breaking);
        assert_eq!(
            removed.message,
            "config property 'jobs' was removed, and the 'parallelism' it renames to does not exist"
        );
    }

    #[test]
    fn a_conditional_default_reads_like_an_unconditional_one() {
        let none = r#"
name "ex"
bin "ex"
flag "--json" help="json"
flag "--bin-names <n>" help="names"
        "#;
        let conditional = r#"
name "ex"
bin "ex"
flag "--json" help="json"
flag "--bin-names <n>" help="names" {
    default_if "--json" "true"
}
        "#;
        let found = changes(none, conditional);
        assert_eq!(
            find(&found, "default-if-added").message,
            "flag '--bin-names' now defaults to 'true' when --json is given"
        );
        assert_eq!(
            find(&found, "default-if-added").category,
            Category::Compatible
        );
        assert_eq!(
            find(&changes(conditional, none), "default-if-removed").category,
            Category::Breaking
        );
    }

    #[test]
    fn a_flag_that_needs_more_values_than_it_did_is_breaking() {
        let one = r#"
name "ex"
bin "ex"
flag "--tags <t>" help="tags" var=#true var_min=1
        "#;
        let two = r#"
name "ex"
bin "ex"
flag "--tags <t>" help="tags" var=#true var_min=2
        "#;
        let found = changes(one, two);
        assert_eq!(
            find(&found, "var-min-raised").message,
            "flag '--tags' now needs at least 2 values"
        );
        assert_eq!(find(&found, "var-min-raised").category, Category::Breaking);
        assert_eq!(
            find(&changes(two, one), "var-min-lowered").category,
            Category::Compatible
        );
    }

    #[test]
    fn listing_a_value_a_non_strict_set_already_accepted_is_metadata() {
        // The mirror of `dropping_a_value_from_a_non_strict_set_only_stops_offering_it`:
        // where anything is accepted, the list decides what is offered and nothing else.
        let old = r#"
name "ex"
bin "ex"
flag "--backend <b>" help="backend" {
    choices "npm" strict=#false
}
        "#;
        let new = r#"
name "ex"
bin "ex"
flag "--backend <b>" help="backend" {
    choices "npm" "cargo" strict=#false
}
        "#;
        let found = changes(old, new);
        assert_eq!(codes(old, new), ["metadata:choice-listed"]);
        assert_eq!(
            find(&found, "choice-listed").message,
            "flag '--backend' now offers 'cargo', which it already accepted"
        );
    }

    #[test]
    fn reordering_conditional_defaults_changes_what_they_resolve_to() {
        // First match wins, so where two conditions can both hold, the order is the
        // answer. Set membership alone would call this pair identical.
        let one = r#"
name "ex"
bin "ex"
flag "--json" help="json"
flag "--pretty" help="pretty"
flag "--style <s>" help="style" {
    default_if "--json" "compact"
    default_if "--pretty" "wide"
}
        "#;
        let other = r#"
name "ex"
bin "ex"
flag "--json" help="json"
flag "--pretty" help="pretty"
flag "--style <s>" help="style" {
    default_if "--pretty" "wide"
    default_if "--json" "compact"
}
        "#;
        assert_eq!(codes(one, other), ["breaking:default-if-order-changed"]);
    }

    #[test]
    fn reordering_environment_names_changes_which_one_wins() {
        // The same rule the conditional defaults follow, and the reason both go through
        // one comparison.
        let one = r#"
name "ex"
bin "ex"
flag "--jobs <n>" help="jobs" env="EX_JOBS" {
    env_fallback "EX_PARALLEL"
}
        "#;
        let other = r#"
name "ex"
bin "ex"
flag "--jobs <n>" help="jobs" env="EX_PARALLEL" {
    env_fallback "EX_JOBS"
}
        "#;
        assert_eq!(codes(one, other), ["breaking:env-order-changed"]);
    }

    #[test]
    fn the_alias_that_covers_a_rename_is_not_also_an_addition() {
        // `cmd-renamed` already says the old word still works. Saying it again as
        // `alias-added`, at the old command's own location, reads as two changes.
        let old = r#"
name "ex"
bin "ex"
cmd "install" help="install" {
    alias "i"
}
        "#;
        let new = r#"
name "ex"
bin "ex"
cmd "add" help="install" {
    alias "install"
}
        "#;
        let found = changes(old, new);
        assert!(!found.iter().any(|c| c.code == "alias-added"), "{found:?}");
        // The alias the old command really did lose is still reported.
        let removed = find(&found, "alias-removed");
        assert_eq!(removed.message, "alias 'i' was removed");
        assert_eq!(removed.location, "ex install");
        assert_eq!(removed.category, Category::Breaking);
    }

    #[test]
    fn a_name_match_claims_its_flag_before_a_moved_spelling_can() {
        // `--bar` moves from the flag named `foo` to the flag named `bar`, whose own
        // `--baz` goes away. Pairing in one pass let `foo` claim the new `bar` by
        // spelling, after which the old `bar` matched the same flag by name and was
        // compared to it a second time — so `--baz` was never reported as removed.
        let old = r#"
name "ex"
bin "ex"
flag "foo: --bar" help="one"
flag "bar: --baz" help="two"
        "#;
        let new = r#"
name "ex"
bin "ex"
flag "bar: --bar" help="two"
        "#;
        let found = changes(old, new);
        // One comparison per new flag: `bar` pairs with `bar` by name, and `foo` is gone.
        let removed: Vec<&str> = found
            .iter()
            .filter(|c| c.code == "flag-removed")
            .map(|c| c.message.as_str())
            .collect();
        assert_eq!(removed, ["flag 'foo' (--bar) was removed"]);
        assert_eq!(
            find(&found, "flag-spelling-removed").message,
            "flag '--bar' no longer answers to '--baz'"
        );
        assert!(!found.iter().any(|c| c.code == "flag-renamed"), "{found:?}");
    }

    #[test]
    fn display_order_is_compared_because_the_docs_say_it_is() {
        let old = r#"
name "ex"
bin "ex"
flag "--verbose" help="verbose" display_order=10
        "#;
        let new = r#"
name "ex"
bin "ex"
flag "--verbose" help="verbose" display_order=20
        "#;
        let found = changes(old, new);
        let moved = find(&found, "display-order-changed");
        assert_eq!(moved.category, Category::Metadata);
        assert_eq!(
            moved.message,
            "flag '--verbose' moves from 10 to 20 in help"
        );
        // And dropping it entirely says what it falls back to.
        let none = "name \"ex\"\nbin \"ex\"\nflag \"--verbose\" help=\"verbose\"\n";
        assert_eq!(
            find(&changes(old, none), "display-order-changed").message,
            "flag '--verbose' moves from 10 to declaration order in help"
        );
    }
}
