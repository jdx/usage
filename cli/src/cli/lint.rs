use std::cmp::Ordering;
use std::collections::HashMap;
use std::path::PathBuf;
use usage::error::UsageErr;
use usage::spec::cmd::SpecExample;
use usage::{Parser, Spec, SpecArg, SpecCommand, SpecFlag, SpecFlagAction};

use crate::cli::generate::parse_file_or_stdin;

/// Lint a usage spec file for common issues
#[derive(usage_rs::Args)]
#[usage(effect = "read")]
pub struct Lint {
    /// A usage spec file to lint, use "-" to read from stdin
    file: PathBuf,

    /// Output format
    #[usage(long, short, default = "text", value_enum)]
    format: OutputFormat,

    /// Treat warnings as errors
    #[usage(long, short = 'W')]
    warnings_as_errors: bool,

    /// Also check that subcommands and flags are declared in sorted order
    ///
    /// Off by default: declaration order is a house convention rather than a
    /// correctness question, so a spec that keeps a different order is not wrong.
    /// Pair it with --warnings-as-errors to hold the order in CI.
    #[usage(long)]
    sorted: bool,
}

/// The rules that only run when asked for.
#[derive(Clone, Copy, Default)]
pub struct LintOptions {
    /// Check that subcommands and flags are declared in sorted order.
    pub sorted: bool,
}

#[derive(Clone, Copy, Default, usage_rs::ValueEnum)]
enum OutputFormat {
    #[default]
    Text,
    Json,
}

impl std::str::FromStr for OutputFormat {
    type Err = String;

    fn from_str(value: &str) -> Result<Self, Self::Err> {
        match value {
            "text" => Ok(Self::Text),
            "json" => Ok(Self::Json),
            _ => Err(format!("`{value}` is not one of: text, json")),
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, serde::Serialize)]
#[serde(rename_all = "lowercase")]
pub enum Severity {
    Error,
    Warning,
    Info,
}

impl std::fmt::Display for Severity {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Severity::Error => write!(f, "error"),
            Severity::Warning => write!(f, "warning"),
            Severity::Info => write!(f, "info"),
        }
    }
}

#[derive(Debug, Clone, serde::Serialize)]
pub struct LintIssue {
    pub severity: Severity,
    pub code: String,
    pub message: String,
    pub location: Option<String>,
}

impl std::fmt::Display for LintIssue {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        let loc = self
            .location
            .as_ref()
            .map(|l| format!(" at {}", l))
            .unwrap_or_default();
        write!(
            f,
            "{} [{}]{}: {}",
            self.severity, self.code, loc, self.message
        )
    }
}

impl Lint {
    pub fn run(&self) -> miette::Result<()> {
        let spec = parse_file_or_stdin(&self.file)?;
        let issues = lint_spec(
            &spec,
            LintOptions {
                sorted: self.sorted,
            },
        );

        match self.format {
            OutputFormat::Text => self.print_text(&issues),
            OutputFormat::Json => self.print_json(&issues)?,
        }

        let has_errors = issues.iter().any(|i| i.severity == Severity::Error);
        let has_warnings = issues.iter().any(|i| i.severity == Severity::Warning);

        if has_errors || (self.warnings_as_errors && has_warnings) {
            std::process::exit(1);
        }

        Ok(())
    }

    fn print_text(&self, issues: &[LintIssue]) {
        if issues.is_empty() {
            println!("No issues found.");
            return;
        }

        let errors = issues
            .iter()
            .filter(|i| i.severity == Severity::Error)
            .count();
        let warnings = issues
            .iter()
            .filter(|i| i.severity == Severity::Warning)
            .count();
        let infos = issues
            .iter()
            .filter(|i| i.severity == Severity::Info)
            .count();

        for issue in issues {
            println!("{}", issue);
        }

        println!();
        println!(
            "Found {} error(s), {} warning(s), {} info(s)",
            errors, warnings, infos
        );
    }

    fn print_json(&self, issues: &[LintIssue]) -> miette::Result<()> {
        let json = serde_json::to_string_pretty(issues)
            .map_err(|e| miette::miette!("Failed to serialize issues: {}", e))?;
        println!("{}", json);
        Ok(())
    }
}

pub fn lint_spec(spec: &Spec, opts: LintOptions) -> Vec<LintIssue> {
    let mut issues = Vec::new();

    // Check default_subcommand reference
    if let Some(default_subcmd) = &spec.default_subcommand {
        // Resolved the way a typed word is, rather than by canonical key alone: the name may
        // be any the command answers to, aliases and hidden aliases included, so a spec
        // declaring `default_subcommand "r"` against `cmd "run" { alias "r" }` is valid and
        // was being reported as naming a command that does not exist.
        if spec.cmd.find_subcommand(default_subcmd).is_none() {
            let valid: Vec<&str> = spec.cmd.subcommands.keys().map(|s| s.as_str()).collect();
            let valid_list = if valid.is_empty() {
                "no subcommands defined".to_string()
            } else {
                format!("valid subcommands: {}", valid.join(", "))
            };
            issues.push(LintIssue {
                severity: Severity::Error,
                code: "invalid-default-subcommand".to_string(),
                message: format!(
                    "default_subcommand '{}' does not exist ({})",
                    default_subcmd, valid_list
                ),
                location: None,
            });
        }
    }

    let has_named_multicall_target = !spec.cmd.subcommands.is_empty();
    if spec.multicall && !has_named_multicall_target {
        issues.push(LintIssue {
            severity: Severity::Error,
            code: "multicall-no-subcommands".to_string(),
            message: "Spec has multicall=#true but no subcommands to select".to_string(),
            location: None,
        });
    }

    for (position, (id, view)) in spec.views.iter().enumerate() {
        if let Err(error) = spec.for_view(id) {
            issues.push(LintIssue {
                severity: Severity::Error,
                code: "invalid-view".to_string(),
                message: error.to_string(),
                location: Some(format!("view {id}")),
            });
        }
        let host_name = program_basename(&spec.name);
        let host_bin = program_basename(&spec.bin);
        if [id.as_str(), view.bin.as_str()].iter().any(|selector| {
            let selector = program_basename(selector);
            selector == host_name || (!host_bin.is_empty() && selector == host_bin)
        }) {
            issues.push(LintIssue {
                severity: Severity::Error,
                code: "view-host-collision".to_string(),
                message: format!(
                    "view `{id}` uses the host command's name or bin as an executable selector"
                ),
                location: Some(format!("view {id}")),
            });
        }
        if let Some((other, declared)) =
            spec.views.iter().take(position).find(|(other, declared)| {
                [other.as_str(), declared.bin.as_str()].iter().any(|prior| {
                    [id.as_str(), view.bin.as_str()]
                        .iter()
                        .any(|current| program_basename(prior) == program_basename(current))
                })
            })
        {
            let duplicate_bin = program_basename(&declared.bin) == program_basename(&view.bin);
            issues.push(LintIssue {
                severity: Severity::Error,
                code: if duplicate_bin {
                    "duplicate-view-bin"
                } else {
                    "ambiguous-view-program"
                }
                .to_string(),
                message: format!(
                    "views `{other}` and `{id}` collide after executable basename normalization in the identifier and executable namespaces"
                ),
                location: Some(format!("view {id}")),
            });
        }
    }

    // Examples declared at the top level belong to the root command; the KDL parser
    // keeps them in a separate field, so `lint_command` never sees them.
    lint_examples(spec, &spec.examples, &spec.cmd.name, &mut issues);

    // Lint the root command
    lint_command(
        spec,
        &spec.cmd,
        &[],
        spec.about.is_some(),
        opts,
        &mut issues,
    );

    issues
}

fn program_basename(program: &str) -> &str {
    let basename = program.rsplit(['/', '\\']).next().unwrap_or(program);
    match basename.get(basename.len().saturating_sub(4)..) {
        Some(extension) if extension.eq_ignore_ascii_case(".exe") => {
            &basename[..basename.len() - 4]
        }
        _ => basename,
    }
}

fn lint_command(
    spec: &Spec,
    cmd: &SpecCommand,
    path: &[&str],
    has_root_about: bool,
    opts: LintOptions,
    issues: &mut Vec<LintIssue>,
) {
    let cmd_path = if path.is_empty() {
        cmd.name.clone()
    } else {
        format!("{} {}", path.join(" "), cmd.name)
    };

    // Check for missing command help
    if cmd.help.is_none() && !has_root_about && !cmd.name.is_empty() {
        issues.push(LintIssue {
            severity: Severity::Info,
            code: "missing-cmd-help".to_string(),
            message: "Command has no help text".to_string(),
            location: Some(format!("cmd {}", cmd_path)),
        });
    }

    // Check for subcommand_required with no subcommands
    if cmd.subcommand_required && cmd.subcommands.is_empty() {
        issues.push(LintIssue {
            severity: Severity::Error,
            code: "subcommand-required-no-subcommands".to_string(),
            message: "Command has subcommand_required=true but no subcommands defined".to_string(),
            location: Some(format!("cmd {}", cmd_path)),
        });
    }

    // Check for subcommands that answer to the same word
    //
    // One command's alias equal to another command's name leaves one of the two
    // unreachable however it is resolved, so the spec is a mistake rather than
    // something with a right answer. The grammar still picks the name over the
    // alias, for a parser handed a spec nothing validated; a derive rejects the
    // same shape outright, via `usage_argv::assert_unique_subcommand_names`.
    let mut seen_subcommands: std::collections::HashMap<&str, &str> =
        std::collections::HashMap::new();
    for sub in cmd.subcommands.values() {
        for word in std::iter::once(&sub.name)
            .chain(&sub.aliases)
            .chain(&sub.hidden_aliases)
        {
            match seen_subcommands.get(word.as_str()) {
                Some(existing) => issues.push(LintIssue {
                    severity: Severity::Error,
                    code: "duplicate-subcommand".to_string(),
                    message: format!(
                        "Subcommands '{}' and '{}' both answer to '{}'",
                        existing, sub.name, word
                    ),
                    location: Some(format!("cmd {}", cmd_path)),
                }),
                None => {
                    seen_subcommands.insert(word.as_str(), &sub.name);
                }
            }
        }
    }

    // Check for duplicate flag names
    let mut seen_flags: std::collections::HashMap<String, &SpecFlag> =
        std::collections::HashMap::new();
    for flag in &cmd.flags {
        for long in &flag.long {
            let key = format!("--{}", long);
            if let Some(existing) = seen_flags.get(&key) {
                issues.push(LintIssue {
                    severity: Severity::Error,
                    code: "duplicate-flag".to_string(),
                    message: format!(
                        "Flag '{}' is defined multiple times (also defined as '{}')",
                        key, existing.name
                    ),
                    location: Some(format!("cmd {}", cmd_path)),
                });
            } else {
                seen_flags.insert(key, flag);
            }
        }
        for short in &flag.short {
            let key = format!("-{}", short);
            if let Some(existing) = seen_flags.get(&key) {
                issues.push(LintIssue {
                    severity: Severity::Error,
                    code: "duplicate-flag".to_string(),
                    message: format!(
                        "Flag '{}' is defined multiple times (also defined as '{}')",
                        key, existing.name
                    ),
                    location: Some(format!("cmd {}", cmd_path)),
                });
            } else {
                seen_flags.insert(key, flag);
            }
        }
    }

    // Lint individual flags
    for flag in &cmd.flags {
        lint_flag(flag, &cmd_path, issues);
    }

    // Check for duplicate arg names
    let mut seen_args: std::collections::HashMap<&str, &SpecArg> = std::collections::HashMap::new();
    for arg in &cmd.args {
        if let Some(existing) = seen_args.get(arg.name.as_str()) {
            issues.push(LintIssue {
                severity: Severity::Error,
                code: "duplicate-arg".to_string(),
                message: format!("Argument '{}' is defined multiple times", existing.name),
                location: Some(format!("cmd {}", cmd_path)),
            });
        } else {
            seen_args.insert(&arg.name, arg);
        }
    }

    // Lint individual args
    for arg in &cmd.args {
        lint_arg(arg, &cmd_path, issues);
    }

    // Check for optional args before required args
    let mut found_optional = false;
    for arg in &cmd.args {
        if !arg.required {
            found_optional = true;
        } else if found_optional && !arg.var {
            issues.push(LintIssue {
                severity: Severity::Warning,
                code: "required-after-optional".to_string(),
                message: format!(
                    "Required argument '{}' appears after optional arguments",
                    arg.name
                ),
                location: Some(format!("cmd {}", cmd_path)),
            });
        }
    }

    // Check for variadic arg not at the end
    for (i, arg) in cmd.args.iter().enumerate() {
        if arg.var && i < cmd.args.len() - 1 {
            issues.push(LintIssue {
                severity: Severity::Warning,
                code: "variadic-arg-not-last".to_string(),
                message: format!("Variadic argument '{}' is not the last argument", arg.name),
                location: Some(format!("cmd {}", cmd_path)),
            });
        }
    }

    lint_examples(spec, &cmd.examples, &cmd_path, issues);

    if opts.sorted {
        lint_sorted(cmd, &cmd_path, issues);
    }

    // Recursively lint subcommands
    let new_path: Vec<&str> = path
        .iter()
        .copied()
        .chain(std::iter::once(cmd.name.as_str()))
        .collect();
    for subcmd in cmd.subcommands.values() {
        lint_command(spec, subcmd, &new_path, false, opts, issues);
    }
}

/// Checks that a command declares its subcommands and flags in sorted order.
///
/// Three groups are ordered independently, and only within themselves:
///
/// 1. subcommands, alphabetically by name;
/// 2. flags that have a short option, by that short option;
/// 3. flags that have only a long option, by that long option.
///
/// Positional arguments are left alone — their declaration order is what the parser
/// matches them by, so it is not free to change. The order *between* the two flag
/// groups is not checked either: a spec that interleaves short-bearing and long-only
/// flags is still readable, and `include` and `flatten` both merge flags in from
/// elsewhere, so the interleaving is often not the author's to fix.
fn lint_sorted(cmd: &SpecCommand, cmd_path: &str, issues: &mut Vec<LintIssue>) {
    // Subcommands merged in by a `mount` describe another program's CLI, so their
    // order is not this spec's to keep.
    let subcommands: Vec<&str> = cmd
        .subcommands
        .values()
        .filter(|c| !c.mounted)
        .map(|c| c.name.as_str())
        .collect();
    if let Some((out_of_place, predecessor)) = first_unsorted(&subcommands, |a, b| a.cmp(b)) {
        issues.push(LintIssue {
            severity: Severity::Warning,
            code: "unsorted-subcommands".to_string(),
            message: format!(
                "Subcommand '{}' is declared after '{}'",
                out_of_place, predecessor
            ),
            location: Some(format!("cmd {}", cmd_path)),
        });
    }

    // Likewise for flags a mount folded onto this command: every flag here is the
    // mounted program's, listed in the order that program gave them.
    if cmd.flags_from_mount {
        return;
    }

    let with_short: Vec<&SpecFlag> = cmd.flags.iter().filter(|f| !f.short.is_empty()).collect();
    if let Some((out_of_place, predecessor)) =
        first_unsorted(&with_short, |a, b| short_cmp(a.short[0], b.short[0]))
    {
        issues.push(LintIssue {
            severity: Severity::Warning,
            code: "unsorted-flags".to_string(),
            message: format!(
                "Flag '-{}' is declared after '-{}'",
                out_of_place.short[0], predecessor.short[0]
            ),
            location: Some(format!("cmd {}", cmd_path)),
        });
    }

    // A flag with neither a short nor a long is already reported as `flag-no-option`;
    // there is no name here to sort it by, so it takes no part in the ordering.
    let long_only: Vec<&SpecFlag> = cmd
        .flags
        .iter()
        .filter(|f| f.short.is_empty() && !f.long.is_empty())
        .collect();
    if let Some((out_of_place, predecessor)) =
        first_unsorted(&long_only, |a, b| a.long[0].cmp(&b.long[0]))
    {
        issues.push(LintIssue {
            severity: Severity::Warning,
            code: "unsorted-flags".to_string(),
            message: format!(
                "Flag '--{}' is declared after '--{}'",
                out_of_place.long[0], predecessor.long[0]
            ),
            location: Some(format!("cmd {}", cmd_path)),
        });
    }
}

/// The first item that sorts before the one declared ahead of it, with that
/// predecessor — the pair a reader would have to swap to start fixing the order.
///
/// One pair rather than the whole expected ordering: a command with fifty
/// subcommands one of which slipped should say which one, not reprint the list.
fn first_unsorted<T: Copy>(items: &[T], cmp: impl Fn(T, T) -> Ordering) -> Option<(T, T)> {
    items
        .windows(2)
        .find(|w| cmp(w[1], w[0]) == Ordering::Less)
        .map(|w| (w[1], w[0]))
}

/// Orders short options by letter, then lowercase ahead of uppercase.
///
/// Plain `char` ordering puts every uppercase letter ahead of every lowercase one,
/// which would ask for `-V -f -v`. A CLI reads better with `-f -v -V`: the pairing of
/// a letter with its capital is the thing worth keeping adjacent.
fn short_cmp(a: char, b: char) -> Ordering {
    a.to_ascii_lowercase()
        .cmp(&b.to_ascii_lowercase())
        .then_with(|| a.is_uppercase().cmp(&b.is_uppercase()))
}

fn lint_flag(flag: &SpecFlag, cmd_path: &str, issues: &mut Vec<LintIssue>) {
    // Check for flags with no short or long
    if flag.short.is_empty() && flag.long.is_empty() {
        issues.push(LintIssue {
            severity: Severity::Error,
            code: "flag-no-option".to_string(),
            message: format!("Flag '{}' has no short or long option", flag.name),
            location: Some(format!("cmd {} flag {}", cmd_path, flag.name)),
        });
    }

    // Check for missing help
    if flag.help.is_none() && !flag.hide {
        issues.push(LintIssue {
            severity: Severity::Info,
            code: "missing-flag-help".to_string(),
            message: format!("Flag '{}' has no help text", flag.name),
            location: Some(format!("cmd {} flag {}", cmd_path, flag.name)),
        });
    }

    // Check for deprecated flags
    if let Some(deprecated) = &flag.deprecated {
        issues.push(LintIssue {
            severity: Severity::Info,
            code: "deprecated-flag".to_string(),
            message: format!("Flag '{}' is deprecated: {}", flag.name, deprecated),
            location: Some(format!("cmd {} flag {}", cmd_path, flag.name)),
        });
    }

    // Check for inconsistent naming (mixing snake_case and kebab-case)
    for long in &flag.long {
        if long.contains('_') && long.contains('-') {
            issues.push(LintIssue {
                severity: Severity::Warning,
                code: "inconsistent-naming".to_string(),
                message: format!("Flag '--{}' mixes underscores and hyphens", long),
                location: Some(format!("cmd {} flag {}", cmd_path, flag.name)),
            });
        }
    }

    // Check for count flag with arg (conflicting semantics)
    if flag.count && flag.arg.is_some() {
        issues.push(LintIssue {
            severity: Severity::Error,
            code: "count-flag-with-arg".to_string(),
            message: format!(
                "Flag '{}' is a count flag but also has an argument",
                flag.name
            ),
            location: Some(format!("cmd {} flag {}", cmd_path, flag.name)),
        });
    }
}

fn lint_arg(arg: &SpecArg, cmd_path: &str, issues: &mut Vec<LintIssue>) {
    // Check for missing help
    if arg.help.is_none() && !arg.hide {
        issues.push(LintIssue {
            severity: Severity::Info,
            code: "missing-arg-help".to_string(),
            message: format!("Argument '{}' has no help text", arg.name),
            location: Some(format!("cmd {} arg {}", cmd_path, arg.name)),
        });
    }

    // Check for inconsistent naming
    if arg.name.contains('_') && arg.name.contains('-') {
        issues.push(LintIssue {
            severity: Severity::Warning,
            code: "inconsistent-naming".to_string(),
            message: format!("Argument '{}' mixes underscores and hyphens", arg.name),
            location: Some(format!("cmd {} arg {}", cmd_path, arg.name)),
        });
    }
}

/// Checks that every `example` still parses against the spec that declares it.
///
/// An example is a command line a reader is invited to type, and until now nothing
/// checked that the spec it sits in still accepts it — so an example outlives the flag
/// it demonstrates, and the docs, help output and manpages keep publishing it. Parsing
/// each one makes `example` load-bearing rather than prose.
///
/// The check is deliberately conservative, because a linter that reports a working
/// example as broken gets switched off. A line is examined only when it is
/// unmistakably an invocation of *this* CLI; output lines, other programs, and shell
/// constructs are left alone rather than guessed at. What that skips is listed on
/// [`invocation_words`].
///
/// It also never runs another program. Examples are parsed with injected — and empty —
/// mount outputs, so a spec whose commands are discovered by running something gets
/// that example skipped rather than a `mise tasks --usage` spawned by a linter. The
/// environment is empty for the same reason a linter should not read the machine it
/// runs on: an example that only parses because of a variable the author happens to
/// have exported is one the reader cannot type.
fn lint_examples(
    spec: &Spec,
    examples: &[SpecExample],
    cmd_path: &str,
    issues: &mut Vec<LintIssue>,
) {
    for example in examples {
        if !is_shell_example(&example.lang) {
            continue;
        }
        for line in logical_lines(&example.code) {
            let Some(words) = invocation_words(spec, &line) else {
                continue;
            };
            match parse_example(spec, &words) {
                Ok(_) => {}
                // Everything the mounted program would have contributed is missing, and
                // this line needed some of it. Said out loud rather than skipped
                // quietly: a lint that stays silent about the lines it could not read
                // reports "checked and fine" over exactly the ones it never checked.
                Err(Unparsed::NeedsAMount) => issues.push(LintIssue {
                    severity: Severity::Info,
                    code: "example-not-checked".to_string(),
                    message: format!(
                        "Example `{line}` was not checked: it reaches past a mount, \
                         which names its commands only when run"
                    ),
                    location: Some(format!("cmd {} example", cmd_path)),
                }),
                Err(Unparsed::Refused(err)) => issues.push(LintIssue {
                    severity: Severity::Warning,
                    code: "example-does-not-parse".to_string(),
                    message: format!("Example `{}` does not parse: {}", line, one_line(&err)),
                    location: Some(format!("cmd {} example", cmd_path)),
                }),
            }
        }
    }
}

/// Why an example did not parse, once a mount has been accounted for.
enum Unparsed {
    /// The spec refused the line, and can answer for it.
    Refused(miette::Error),
    /// The line needs something only the mounted program could have said.
    NeedsAMount,
}

/// Parse one example, without running anything and without reading the environment.
///
/// Mounts are the whole difficulty. usage-lib resolves a command's mounts on the way
/// *into* it, so a spec that mounts anything cannot be parsed at all without either
/// spawning the mounted program or being handed its answer. Injecting an empty set of
/// answers avoids the process but refuses every line under a mounting command, checked
/// or not — which is most of an adopter's spec, since mise, aube and pitchfork all mount.
///
/// So the empty set is only the first attempt. When it comes back short, the mount is
/// answered with a spec that declares nothing and the line is parsed again: a line using
/// only the command's own declared vocabulary now parses on its own merits, and one that
/// still fails is a line the mounted program might have accepted, which nothing here can
/// know. That second group — and only that group — goes unchecked.
fn parse_example(spec: &Spec, words: &[String]) -> Result<(), Unparsed> {
    let parse = |mounts: HashMap<String, String>| {
        Parser::new(spec)
            .with_env(HashMap::new())
            .with_mount_outputs(mounts)
            .parse(words)
    };
    let needs_a_mount = |err: &miette::Error| {
        matches!(
            err.downcast_ref::<UsageErr>(),
            Some(UsageErr::MissingMountOutput(_))
        )
    };

    // `--help` and `--version` end the parse by printing, which usage-lib reports as an
    // error carrying the text — and reports in preference to any other error on the
    // line. The invocation works; showing an author their own help output as a lint
    // message would not. True of either parse: what a mount would have added cannot
    // stop a line from asking for help.
    let printed = |err: &miette::Error| !needs_a_mount(err) && prints_and_exits(spec, words);

    match parse(HashMap::new()) {
        Ok(_) => Ok(()),
        Err(err) if printed(&err) => Ok(()),
        Err(err) if !needs_a_mount(&err) => Err(Unparsed::Refused(err)),
        Err(_) => match parse(empty_mount_answers(&spec.cmd)) {
            Ok(_) => Ok(()),
            Err(err) if printed(&err) => Ok(()),
            // The mounted program contributes commands and flags, so it can only ever
            // make a line parse. One that fails against a mount contributing nothing may
            // still have been fine against the real one, and nothing here can know which.
            Err(_) => Err(Unparsed::NeedsAMount),
        },
    }
}

/// A spec that declares nothing, as the answer to every mount in the tree.
///
/// Keyed by the exact `run` string, which is how injected answers are looked up. It is a
/// whole spec rather than an empty string because that is what a mount's stdout is.
fn empty_mount_answers(cmd: &SpecCommand) -> HashMap<String, String> {
    let mut answers = HashMap::new();
    collect_mount_answers(cmd, &mut answers);
    answers
}

fn collect_mount_answers(cmd: &SpecCommand, answers: &mut HashMap<String, String>) {
    for mount in &cmd.mounts {
        answers.insert(
            mount.run.clone(),
            "name \"mounted\"\nbin \"mounted\"\n".to_string(),
        );
    }
    for sub in cmd.subcommands.values() {
        collect_mount_answers(sub, answers);
    }
}

/// Whether an invocation asks for help or a version rather than doing anything.
///
/// The spellings are the ones the parser answers to: those of any declared flag whose
/// action prints instead of binding, plus the ones the parser supplies rather than a spec
/// declaring them — `--help`, `-h`, `-?` and the `help` subcommand word under
/// `disable_help`, and `--version` and `-V` where the spec declares a version. Collected
/// over the whole tree rather than the routed chain, because a line that asks for help is
/// one whether or not the rest of it routes.
///
/// The supplied version is the exception, because the parser supplies it to the root
/// alone: `mycli run --version` is refused, so suppressing it here would hide a real
/// finding behind a spelling that does not work where the example puts it. It counts only
/// up to the first word that selects a subcommand. Help has no such limit — the parser
/// supplies it on every command.
fn prints_and_exits(spec: &Spec, words: &[String]) -> bool {
    // Skipping argv[0]: a program named `help` is not a request for it.
    let words = &words[1..];

    // A declared flag has its attached value split off before the parser looks the flag
    // up, so `--info=anything` still performs the action `--info` names.
    let mut declared: Vec<String> = Vec::new();
    collect_printing_flags(&spec.cmd, &mut declared);
    if words.iter().any(|word| {
        let name = word.split_once('=').map_or(word.as_str(), |(name, _)| name);
        declared.iter().any(|spelling| spelling == name)
    }) {
        return true;
    }

    // The supplied spellings are compared whole, because that is how the parser compares
    // them: nothing declares them, so nothing splits a value off first, and
    // `--help=bad` is an unknown word rather than a request for help.
    let asks_for_help = spec.disable_help != Some(true)
        && words
            .iter()
            .any(|word| matches!(word.as_str(), "--help" | "-h" | "-?" | "help"));
    let asks_for_a_version = (spec.version.is_some() || spec.long_version.is_some())
        && !spec.cmd.disable_version_flag
        && words
            .iter()
            .take_while(|word| spec.cmd.find_subcommand(word).is_none())
            .any(|word| matches!(word.as_str(), "--version" | "-V"));
    asks_for_help || asks_for_a_version
}

fn collect_printing_flags(cmd: &SpecCommand, spellings: &mut Vec<String>) {
    for flag in &cmd.flags {
        if matches!(flag.action, SpecFlagAction::Set) {
            continue;
        }
        spellings.extend(flag.long.iter().map(|long| format!("--{long}")));
        spellings.extend(flag.short.iter().map(|short| format!("-{short}")));
    }
    for sub in cmd.subcommands.values() {
        collect_printing_flags(sub, spellings);
    }
}

/// The error as one line, since a lint issue is one line.
fn one_line(err: &miette::Error) -> String {
    err.to_string()
        .split_whitespace()
        .collect::<Vec<_>>()
        .join(" ")
}

/// Whether an example's `lang` describes a shell session at all.
///
/// `lang` is there for syntax highlighting, so an example carrying `lang="toml"` is a
/// config file being shown rather than a command line, and reading it as argv would be
/// nonsense. An unset `lang` is the common case and means a shell.
fn is_shell_example(lang: &str) -> bool {
    matches!(
        lang.to_ascii_lowercase().as_str(),
        "" | "sh" | "bash" | "zsh" | "fish" | "shell" | "shell-session" | "console" | "terminal"
    )
}

/// The example's lines, with backslash continuations folded into one.
///
/// A multi-line example is usually a session: some lines are commands and some are the
/// output they printed. Each line is judged on its own, and [`invocation_words`] keeps
/// only the ones that are invocations — but a command split across lines with a
/// trailing `\` has to be rejoined first, or the check sees half a command line and
/// reports the half.
fn logical_lines(code: &str) -> Vec<String> {
    let mut lines = Vec::new();
    let mut pending = String::new();
    for line in code.lines() {
        let line = line.trim();
        match line.strip_suffix('\\') {
            Some(head) => {
                pending.push_str(head.trim_end());
                pending.push(' ');
            }
            None => {
                pending.push_str(line);
                lines.push(std::mem::take(&mut pending));
            }
        }
    }
    if !pending.is_empty() {
        lines.push(pending);
    }
    lines
}

/// One line of an example as an argv, or `None` if it is not this CLI being invoked.
///
/// Skipped, in each case because the line is not something the spec can answer for:
///
/// - output lines, comments, and blank lines;
/// - other programs, including the shell functions and `curl | sh` an install section
///   shows;
/// - anything whose quoting does not close, which is prose rather than a command line.
///
/// Kept, after being tidied down to the part this spec owns: a `$` or `%` prompt,
/// leading `VAR=value` assignments, and everything from the first shell operator
/// onwards — so `$ mycli ls --json | jq .` is checked as `mycli ls --json`. An operator
/// only counts when it is a whole word, so a quoted `"a|b"` stays a value.
fn invocation_words(spec: &Spec, line: &str) -> Option<Vec<String>> {
    let line = line.trim();
    let line = line
        .strip_prefix("$ ")
        .or_else(|| line.strip_prefix("% "))
        .unwrap_or(line);
    if line.is_empty() || line.starts_with('#') {
        return None;
    }

    let mut words = shell_words::split(line).ok()?;
    if let Some(end) = words
        .iter()
        .position(|word| is_shell_operator(word) || starts_a_redirection(word))
    {
        words.truncate(end);
    }
    let assignments = words
        .iter()
        .position(|word| !is_env_assignment(word))
        .unwrap_or(words.len());
    words.drain(..assignments);

    if !is_this_program(spec, words.first()?) {
        return None;
    }
    Some(words)
}

fn is_shell_operator(word: &str) -> bool {
    matches!(
        word,
        "|" | "||" | "&&" | "&" | ";" | "|&" | ">" | ">>" | "<" | "<<" | "2>" | "2>&1"
    )
}

/// A redirection written without the space that would make it its own word, as in
/// `>out.txt` or `2>/dev/null`, which `shell_words` hands back whole.
///
/// Only the output side. `<` is deliberately absent: `mycli deploy <env>` is how
/// documentation writes a placeholder, and truncating there would report the argument
/// the placeholder stands in for as missing. `#` is absent for the opposite reason —
/// `shell_words` drops an unquoted comment itself, so a `#` that survives to be a word
/// came out of quotes and is a value, as in `mycli issue view "#123"`.
fn starts_a_redirection(word: &str) -> bool {
    word.strip_prefix(|c: char| c.is_ascii_digit() || c == '&')
        .unwrap_or(word)
        .starts_with('>')
}

/// A leading `MISE_DEBUG=1`, which sets the environment rather than being part of argv.
fn is_env_assignment(word: &str) -> bool {
    let Some((name, _)) = word.split_once('=') else {
        return false;
    };
    !name.is_empty()
        && name.starts_with(|c: char| c.is_ascii_alphabetic() || c == '_')
        && name.chars().all(|c| c.is_ascii_alphanumeric() || c == '_')
}

/// Whether the first word of a line invokes the CLI this spec describes.
///
/// Compared by executable basename, so `./mycli` and `/usr/local/bin/mycli` are the
/// same program as `mycli`. A view is matched by the program name it is selected with,
/// and a multicall CLI additionally answers to each of its applets — both are ways for
/// one spec to describe more than one command name, and an example is entitled to show
/// any of them.
fn is_this_program(spec: &Spec, word: &str) -> bool {
    let base = program_basename(word);
    [spec.name.as_str(), spec.bin.as_str()]
        .iter()
        .any(|declared| !declared.is_empty() && program_basename(declared) == base)
        || spec.view_for_program(word).is_some()
        || (spec.multicall && spec.cmd.find_subcommand(base).is_some())
}

#[cfg(test)]
mod tests {
    use super::*;

    /// The CLI reads specs written by other people, so a `validate=` expression that does not
    /// compile has to be refused when the spec is read rather than when a value reaches it.
    ///
    /// Here rather than in usage-lib because what it pins is this crate's manifest: usage-lib
    /// only performs the check under its `validation` feature, and `usage lint` silently loses
    /// the rule if `cli/Cargo.toml` stops asking for it.
    #[test]
    fn a_malformed_validate_expression_is_refused_when_a_spec_is_read() {
        let err = r#"
name "demo"
bin "demo"
arg "<port>" validate="int(value) >"
"#
        .parse::<Spec>()
        .expect_err("a validate= expression that does not compile should be refused");
        // The reason is the label, not the `Display` — which is the generic "Invalid usage
        // config" for every parse error.
        let UsageErr::InvalidInput(reason, ..) = &err else {
            panic!("expected a parse error naming the expression, got {err:?}");
        };
        assert!(reason.contains("invalid validation expression"), "{reason}");
    }

    fn example_issues(spec: &str) -> Vec<LintIssue> {
        let spec: Spec = spec.parse().unwrap();
        lint_spec(&spec, LintOptions::default())
            .into_iter()
            .filter(|i| i.code.starts_with("example-"))
            .collect()
    }

    #[test]
    fn test_lint_example_that_parses() {
        let issues = example_issues(
            r#"
name "demo"
bin "demo"
cmd "deploy" help="deploy" {
    flag "-e --environment <env>" help="env"
    example "demo deploy -e prod"
}
"#,
        );
        assert!(issues.is_empty(), "{issues:?}");
    }

    #[test]
    fn test_lint_example_naming_a_flag_the_spec_dropped() {
        // The failure this rule exists for: the flag was renamed and the example that
        // demonstrates it kept being published by help, docs and manpages.
        let issues = example_issues(
            r#"
name "demo"
bin "demo"
cmd "deploy" help="deploy" {
    flag "--env <env>" help="env"
    example "demo deploy --environment prod"
}
"#,
        );
        assert_eq!(issues.len(), 1, "{issues:?}");
        assert!(
            issues[0].message.contains("--environment"),
            "{}",
            issues[0].message
        );
    }

    #[test]
    fn test_lint_example_declared_at_the_top_level() {
        // Root examples live on the spec rather than on its command, so the walk over
        // commands never sees them.
        let issues = example_issues(
            r#"
name "demo"
bin "demo"
example "demo --nope"
cmd "deploy" help="deploy"
"#,
        );
        assert_eq!(issues.len(), 1, "{issues:?}");
    }

    #[test]
    fn test_lint_example_asking_for_help_or_version() {
        // Both end the parse by printing, which usage-lib reports as an error carrying
        // the text. The invocation works.
        let issues = example_issues(
            r#"
name "demo"
bin "demo"
version "1.0.0"
flag "-V --version" action="version" help="version"
example "demo --help"
example "demo -h"
example "demo --version"
example "demo deploy --help"
cmd "deploy" help="deploy"
"#,
        );
        assert!(issues.is_empty(), "{issues:?}");

        // The same, with the version flag supplied by the parser rather than declared.
        let issues = example_issues(
            r#"
name "demo"
bin "demo"
version "1.0.0"
example "demo --version"
example "demo -V"
"#,
        );
        assert!(issues.is_empty(), "{issues:?}");

        // And where nothing declares a version, nothing supplies one either.
        let issues = example_issues(
            r#"
name "demo"
bin "demo"
example "demo --version"
"#,
        );
        assert_eq!(issues.len(), 1, "{issues:?}");

        // The supplied version belongs to the root, so a subcommand carrying it is a
        // real finding rather than a request to print — the parser refuses that line.
        let issues = example_issues(
            r#"
name "demo"
bin "demo"
version "1.0.0"
cmd "run" help="run"
example "demo run --version"
example "demo run --help"
"#,
        );
        assert_eq!(issues.len(), 1, "{issues:?}");
        assert!(
            issues[0].message.contains("--version"),
            "{}",
            issues[0].message
        );
    }

    #[test]
    fn test_lint_example_lines_that_are_not_this_program() {
        // A session: a prompt, the output it printed, another program, and a comment.
        // Only the first line is a question this spec can answer.
        let issues = example_issues(
            r#"
name "demo"
bin "demo"
cmd "ls" help="ls" {
    example "$ demo ls\nnode 20.0.0\n# then install it\ncurl -fsSL https://example.com | sh"
}
"#,
        );
        assert!(issues.is_empty(), "{issues:?}");

        // The same session with one of its own commands broken, so the lines above are
        // shown to be skipped rather than the whole example being.
        let issues = example_issues(
            r#"
name "demo"
bin "demo"
cmd "ls" help="ls" {
    example "$ demo ls\nnode 20.0.0\n$ demo ls --nope"
}
"#,
        );
        assert_eq!(issues.len(), 1, "{issues:?}");
    }

    #[test]
    fn test_lint_example_checks_only_up_to_a_shell_operator() {
        let issues = example_issues(
            r#"
name "demo"
bin "demo"
cmd "ls" help="ls" {
    flag "--json" help="json"
    example "demo ls --json | jq ."
    example "demo ls --nope | jq ."
}
"#,
        );
        assert_eq!(issues.len(), 1, "{issues:?}");
        assert!(
            issues[0].message.contains("--nope"),
            "{}",
            issues[0].message
        );
    }

    #[test]
    fn test_lint_example_keeps_a_quoted_operator_as_a_value() {
        let issues = example_issues(
            r#"
name "demo"
bin "demo"
cmd "run" help="run" {
    arg "<task>" help="task"
    example "demo run \"a|b\""
}
"#,
        );
        assert!(issues.is_empty(), "{issues:?}");
    }

    #[test]
    fn test_lint_example_with_a_leading_assignment_and_a_continuation() {
        let issues = example_issues(
            r#"
name "demo"
bin "demo"
cmd "deploy" help="deploy" {
    flag "-e --environment <env>" help="env"
    flag "-f --force" help="force"
    example "DEMO_DEBUG=1 demo deploy \\\n    -e prod --force"
}
"#,
        );
        assert!(issues.is_empty(), "{issues:?}");
    }

    #[test]
    fn test_lint_example_in_a_language_that_is_not_a_shell() {
        let issues = example_issues(
            r#"
name "demo"
bin "demo"
cmd "config" help="config" {
    example "demo = { environment = \"prod\" }" lang="toml"
}
"#,
        );
        assert!(issues.is_empty(), "{issues:?}");
    }

    #[test]
    fn test_lint_example_reaching_a_mount_never_runs_it() {
        // The mount's `run` is not a command: if the linter resolved mounts by running
        // them, the shell would fail. It must not spawn the program it is linting, so a
        // line that only the mounted program could explain goes unchecked — and says so,
        // rather than passing quietly as though it had been read.
        let issues = example_issues(
            r#"
name "demo"
bin "demo"
mount run="this command must never run"
cmd "ls" help="ls" {
    example "demo ls"
}
example "demo discovered --whatever"
"#,
        );
        assert_eq!(issues.len(), 1, "{issues:?}");
        assert_eq!(issues[0].code, "example-not-checked");
        assert_eq!(issues[0].severity, Severity::Info);
    }

    #[test]
    fn test_lint_example_under_a_mount_is_still_checked_where_it_can_be() {
        // A mounting command's own flags are declared, so a line using them is an
        // ordinary question with an ordinary answer. Refusing to check anything below a
        // mount would give up on most of an adopter's spec — mise, aube and pitchfork
        // all mount.
        let issues = example_issues(
            r#"
name "demo"
bin "demo"
cmd "tasks" help="tasks" {
    mount run="this command must never run"
    flag "--all" help="all"
    example "demo tasks --all"
}
"#,
        );
        assert!(issues.is_empty(), "{issues:?}");

        // And the same command's unknown word is the undecidable case: the mounted
        // program might have declared it.
        let issues = example_issues(
            r#"
name "demo"
bin "demo"
cmd "tasks" help="tasks" {
    mount run="this command must never run"
    flag "--all" help="all"
    example "demo tasks --nope"
}
"#,
        );
        assert_eq!(issues.len(), 1, "{issues:?}");
        assert_eq!(issues[0].code, "example-not-checked");

        // A value attached to a supplied spelling is not one of these. The parser
        // compares the whole token, because nothing declares `--help` and so nothing
        // splits a value off it first — `--help=bad` is an unknown word.
        let issues = example_issues(
            r#"
name "demo"
bin "demo"
version "1.0.0"
example "demo --help=bad"
example "demo --version=bad"
example "demo -V=bad"
"#,
        );
        assert_eq!(issues.len(), 3, "{issues:?}");

        // A declared flag is the other way round: its attached value is split off before
        // the lookup, so the action still fires.
        let issues = example_issues(
            r#"
name "demo"
bin "demo"
version "1.0.0"
flag "--info" action="help" help="info"
flag "--ver" action="version" help="ver"
example "demo --info=bad"
example "demo --ver=bad"
"#,
        );
        assert!(issues.is_empty(), "{issues:?}");

        // Asking for help is answered by the command itself, so what the mount would
        // have added cannot change it. Reporting these as unchecked would put a notice
        // on every `--help` example a mounting CLI publishes.
        let issues = example_issues(
            r#"
name "demo"
bin "demo"
version "1.0.0"
cmd "tasks" help="tasks" {
    mount run="this command must never run"
    example "demo tasks --help"
}
example "demo --version"
"#,
        );
        assert!(issues.is_empty(), "{issues:?}");
    }

    #[test]
    fn test_lint_example_with_an_attached_redirection() {
        // `shell_words` hands back `>out.txt` and `2>/dev/null` whole, so the whole-word
        // operator rule does not see them.
        let issues = example_issues(
            r#"
name "demo"
bin "demo"
cmd "ls" help="ls" {
    example "demo ls >out.txt"
    example "demo ls 2>/dev/null"
}
"#,
        );
        assert!(issues.is_empty(), "{issues:?}");
    }

    #[test]
    fn test_lint_example_keeps_a_placeholder_and_a_quoted_hash() {
        // The two shapes an eager redirection rule would break. `<env>` is how
        // documentation writes a placeholder, and a `#` that survives `shell_words` came
        // out of quotes, so it is a value rather than a comment.
        let issues = example_issues(
            r##"
name "demo"
bin "demo"
cmd "deploy" help="deploy" {
    arg "<env>" help="env"
    example "demo deploy <env>"
}
cmd "issue" help="issue" {
    arg "<id>" help="id"
    example "demo issue \"#123\""
}
"##,
        );
        assert!(issues.is_empty(), "{issues:?}");
    }

    #[test]
    fn test_lint_example_lang_is_matched_without_case() {
        let issues = example_issues(
            r#"
name "demo"
bin "demo"
cmd "ls" help="ls" {
    example "demo ls --nope" lang="Bash"
}
"#,
        );
        assert_eq!(issues.len(), 1, "{issues:?}");
    }

    #[test]
    fn test_lint_example_invoking_a_view_or_an_applet() {
        // One spec, more than one command name. An example is entitled to show any of
        // them, so neither shape may be mistaken for another program and skipped.
        // Written so that recognition is what the assertion turns on: an unrecognized
        // program is a skipped line, which is indistinguishable from a clean one.
        let issues = example_issues(
            r#"
name "demo"
bin "demo"
view "runner" root="run" globals=#true
cmd "run" help="run" {
    flag "--all" help="all"
}
example "./runner --all"
example "./runner --nope"
"#,
        );
        assert_eq!(issues.len(), 1, "{issues:?}");
        assert!(
            issues[0].message.contains("--nope"),
            "{}",
            issues[0].message
        );

        let issues = example_issues(
            r#"
name "demo"
bin "demo"
multicall #true
cmd "applet" help="applet" {
    flag "--all" help="all"
    example "applet --all"
    example "applet --nope"
}
"#,
        );
        assert_eq!(issues.len(), 1, "{issues:?}");
        assert!(
            issues[0].message.contains("--nope"),
            "{}",
            issues[0].message
        );
    }

    #[test]
    fn test_lint_missing_help() {
        let spec: Spec = r#"
name "test"
flag "--verbose"
arg "<input>"
        "#
        .parse()
        .unwrap();

        let issues = lint_spec(&spec, LintOptions::default());
        assert!(issues.iter().any(|i| i.code == "missing-flag-help"));
        assert!(issues.iter().any(|i| i.code == "missing-arg-help"));
    }

    #[test]
    fn test_lint_allows_missing_name_and_bin() {
        let spec: Spec = r#"arg "<file>" help="The file to process""#.parse().unwrap();

        let issues = lint_spec(&spec, LintOptions::default());
        assert!(issues.is_empty());
    }

    #[test]
    fn test_lint_duplicate_flags() {
        let spec: Spec = r#"
name "test"
flag "-v --verbose" help="verbose"
flag "-v --very" help="very"
        "#
        .parse()
        .unwrap();

        let issues = lint_spec(&spec, LintOptions::default());
        assert!(issues.iter().any(|i| i.code == "duplicate-flag"));
    }

    #[test]
    fn test_lint_default_subcommand_may_name_an_alias() {
        // `default_subcommand` is resolved the way a typed word is, so any name the command
        // answers to is valid. Checking canonical keys alone called this spec broken.
        for alias in [r#"alias "r""#, r#"alias "r" hide=#true"#] {
            let spec: Spec = format!(
                r#"
name "test"
default_subcommand "r"
cmd "run" help="run" {{
    {alias}
}}
"#
            )
            .parse()
            .unwrap();

            let issues = lint_spec(&spec, LintOptions::default());
            assert!(
                !issues
                    .iter()
                    .any(|i| i.code == "invalid-default-subcommand"),
                "{alias}: {issues:?}"
            );
        }
    }

    #[test]
    fn test_lint_duplicate_subcommand() {
        // One command's alias equal to another's name. The grammar resolves it —
        // the name wins — but the alias is then unreachable, so the spec is still
        // a mistake worth reporting.
        let spec: Spec = r#"
name "test"
cmd "alpha" help="alpha" {
    alias "run"
}
cmd "run" help="run"
        "#
        .parse()
        .unwrap();

        let issues = lint_spec(&spec, LintOptions::default());
        assert!(issues.iter().any(|i| i.code == "duplicate-subcommand"));
    }

    #[test]
    fn test_lint_allows_an_alias_that_collides_with_nothing() {
        let spec: Spec = r#"
name "test"
cmd "install" help="install" {
    alias "i"
}
cmd "run" help="run"
        "#
        .parse()
        .unwrap();

        let issues = lint_spec(&spec, LintOptions::default());
        assert!(!issues.iter().any(|i| i.code == "duplicate-subcommand"));
    }

    #[test]
    fn test_lint_no_option_flag() {
        let spec: Spec = r#"
name "test"
flag "myflag:" help="a flag with only a name"
        "#
        .parse()
        .unwrap();

        let issues = lint_spec(&spec, LintOptions::default());
        assert!(issues.iter().any(|i| i.code == "flag-no-option"));
    }

    #[test]
    fn test_lint_invalid_default_subcommand() {
        let spec: Spec = r#"
name "test"
default_subcommand "nonexistent"
cmd "real" help="a real command"
        "#
        .parse()
        .unwrap();

        let issues = lint_spec(&spec, LintOptions::default());
        assert!(issues
            .iter()
            .any(|i| i.code == "invalid-default-subcommand"));
    }

    #[test]
    fn test_lint_required_after_optional() {
        let spec: Spec = r#"
name "test"
arg "[optional]" help="optional arg"
arg "<required>" help="required arg"
        "#
        .parse()
        .unwrap();

        let issues = lint_spec(&spec, LintOptions::default());
        assert!(issues.iter().any(|i| i.code == "required-after-optional"));
    }

    #[test]
    fn test_lint_clean_spec() {
        let spec: Spec = r#"
name "test"
bin "test"
about "A test CLI"
flag "-v --verbose" help="Enable verbose output"
arg "<input>" help="Input file"
cmd "sub" help="A subcommand" {
    flag "-f --force" help="Force operation"
}
        "#
        .parse()
        .unwrap();

        let issues = lint_spec(&spec, LintOptions::default());
        assert!(issues.is_empty());
    }

    #[test]
    fn test_lint_uses_about_as_root_command_help() {
        let spec: Spec = r#"
name "test"
about "A test CLI"
cmd "documented" help="A documented subcommand"
cmd "undocumented"
        "#
        .parse()
        .unwrap();

        let missing_help: Vec<_> = lint_spec(&spec, LintOptions::default())
            .into_iter()
            .filter(|issue| issue.code == "missing-cmd-help")
            .collect();
        assert_eq!(missing_help.len(), 1);
        assert_eq!(
            missing_help[0].location.as_deref(),
            Some("cmd test undocumented")
        );
    }

    #[test]
    fn test_lint_multicall_no_subcommands() {
        let spec: Spec = r#"
name "busybox"
bin "busybox"
multicall #true
        "#
        .parse()
        .unwrap();

        let issues = lint_spec(&spec, LintOptions::default());
        assert!(issues.iter().any(|i| i.code == "multicall-no-subcommands"));
    }

    #[test]
    fn test_lint_multicall_external_only_has_no_named_target() {
        let spec: Spec = r#"
name "busybox"
bin "busybox"
multicall #true
external_subcommand #true
        "#
        .parse()
        .unwrap();

        let issues = lint_spec(&spec, LintOptions::default());
        assert!(issues.iter().any(|i| i.code == "multicall-no-subcommands"));
    }

    #[test]
    fn test_lint_invalid_executable_view() {
        let spec: Spec = r#"
bin "ex"
flag "--local"
view "runner" root="missing"
view "bad-global" root="run" { global "--local" }
cmd "run"
        "#
        .parse()
        .unwrap();

        let invalid: Vec<_> = lint_spec(&spec, LintOptions::default())
            .into_iter()
            .filter(|issue| issue.code == "invalid-view")
            .collect();
        assert_eq!(invalid.len(), 2);
        assert!(invalid.iter().any(|issue| issue
            .location
            .as_deref()
            .is_some_and(|location| location == "view runner")));
        assert!(invalid.iter().any(|issue| issue
            .location
            .as_deref()
            .is_some_and(|location| location == "view bad-global")));
    }

    #[test]
    fn test_lint_ambiguous_executable_view_dispatch() {
        let spec: Spec = r#"
bin "ex"
view "runner" bin="run-bin" root="run"
view "other" bin="runner" root="other"
cmd "run"
cmd "other"
        "#
        .parse()
        .unwrap();

        let issues = lint_spec(&spec, LintOptions::default());
        assert!(issues
            .iter()
            .any(|issue| issue.code == "ambiguous-view-program"));

        let spec: Spec = r#"
bin "ex"
view "one" bin="runner" root="run"
view "two" bin="/tmp/runner.EXE" root="other"
cmd "run"
cmd "other"
        "#
        .parse()
        .unwrap();
        let issues = lint_spec(&spec, LintOptions::default());
        assert!(issues
            .iter()
            .any(|issue| issue.code == "duplicate-view-bin"));
    }

    #[test]
    fn test_lint_executable_view_cannot_capture_host() {
        let spec: Spec = r#"
name "ex"
bin "/usr/bin/ex.exe"
view "ex" root="run"
cmd "run"
        "#
        .parse()
        .unwrap();

        let issues = lint_spec(&spec, LintOptions::default());
        assert!(issues
            .iter()
            .any(|issue| issue.code == "view-host-collision"));
    }

    #[test]
    fn test_lint_subcommand_required_no_subcommands() {
        let spec: Spec = r#"
name "test"
cmd "sub" subcommand_required=#true help="a subcommand with no subcommands"
        "#
        .parse()
        .unwrap();

        let issues = lint_spec(&spec, LintOptions::default());
        assert!(issues
            .iter()
            .any(|i| i.code == "subcommand-required-no-subcommands"));
    }

    #[test]
    fn test_lint_variadic_arg_not_last() {
        let spec: Spec = r#"
name "test"
arg "<files>…" help="files" var=#true
arg "<output>" help="output"
        "#
        .parse()
        .unwrap();

        let issues = lint_spec(&spec, LintOptions::default());
        assert!(issues.iter().any(|i| i.code == "variadic-arg-not-last"));
    }

    #[test]
    fn test_lint_count_flag_with_arg() {
        let spec: Spec = r#"
name "test"
flag "-v --verbose" count=#true {
    arg "<level>"
}
        "#
        .parse()
        .unwrap();

        let issues = lint_spec(&spec, LintOptions::default());
        assert!(issues.iter().any(|i| i.code == "count-flag-with-arg"));
    }

    fn sorted_issues(spec: &str) -> Vec<LintIssue> {
        let spec: Spec = spec.parse().unwrap();
        lint_spec(&spec, LintOptions { sorted: true })
            .into_iter()
            .filter(|i| i.code.starts_with("unsorted-"))
            .collect()
    }

    #[test]
    fn test_lint_sorted_is_off_by_default() {
        let spec: Spec = r#"
name "test"
cmd "list" help="list"
cmd "add" help="add"
        "#
        .parse()
        .unwrap();

        let issues = lint_spec(&spec, LintOptions::default());
        assert!(!issues.iter().any(|i| i.code.starts_with("unsorted-")));
    }

    #[test]
    fn test_lint_unsorted_subcommands() {
        let issues = sorted_issues(
            r#"
name "test"
cmd "list" help="list"
cmd "add" help="add"
cmd "delete" help="delete"
        "#,
        );
        assert_eq!(issues.len(), 1);
        assert_eq!(issues[0].code, "unsorted-subcommands");
        assert_eq!(
            issues[0].message,
            "Subcommand 'add' is declared after 'list'"
        );
        assert_eq!(issues[0].location.as_deref(), Some("cmd test"));
    }

    #[test]
    fn test_lint_sorted_accepts_a_sorted_spec() {
        let issues = sorted_issues(
            r#"
name "test"
flag "-d --debug" help="debug"
flag "-o --output" help="output"
flag "--config" help="config"
flag "--no-color" help="no color"
arg "<second>" help="second"
arg "<first>" help="first"
cmd "add" help="add"
cmd "delete" help="delete"
cmd "list" help="list"
        "#,
        );
        assert!(issues.is_empty(), "{:?}", issues);
    }

    #[test]
    fn test_lint_unsorted_short_flags() {
        let issues = sorted_issues(
            r#"
name "test"
flag "-v --verbose" help="verbose"
flag "-d --debug" help="debug"
        "#,
        );
        assert_eq!(issues.len(), 1);
        assert_eq!(issues[0].code, "unsorted-flags");
        assert_eq!(issues[0].message, "Flag '-d' is declared after '-v'");
    }

    #[test]
    fn test_lint_unsorted_long_only_flags() {
        let issues = sorted_issues(
            r#"
name "test"
flag "--zebra" help="zebra"
flag "--alpha" help="alpha"
        "#,
        );
        assert_eq!(issues.len(), 1);
        assert_eq!(issues[0].code, "unsorted-flags");
        assert_eq!(
            issues[0].message,
            "Flag '--alpha' is declared after '--zebra'"
        );
    }

    #[test]
    fn test_lint_sorted_keeps_the_two_flag_groups_apart() {
        // `--alpha` sorts before `--output`'s long name and is declared after it, but
        // one group is long-only and the other is not, so the two never compare against
        // each other — each group only has to be sorted within itself.
        let issues = sorted_issues(
            r#"
name "test"
flag "-o --output" help="output"
flag "--alpha" help="alpha"
flag "-v --verbose" help="verbose"
flag "--config" help="config"
        "#,
        );
        assert!(issues.is_empty(), "{:?}", issues);
    }

    #[test]
    fn test_lint_sorted_puts_lowercase_before_its_capital() {
        let ok = sorted_issues(
            r#"
name "test"
flag "-i --inject" help="inject"
flag "-I --index" help="index"
        "#,
        );
        assert!(ok.is_empty(), "{:?}", ok);

        let flipped = sorted_issues(
            r#"
name "test"
flag "-I --index" help="index"
flag "-i --inject" help="inject"
        "#,
        );
        assert_eq!(flipped.len(), 1);
        assert_eq!(flipped[0].message, "Flag '-i' is declared after '-I'");
    }

    #[test]
    fn test_lint_sorted_reaches_into_subcommands() {
        let issues = sorted_issues(
            r#"
name "test"
cmd "sub" help="sub" {
    flag "-o --output" help="output"
    flag "-d --debug" help="debug"
}
        "#,
        );
        assert_eq!(issues.len(), 1);
        assert_eq!(issues[0].location.as_deref(), Some("cmd test sub"));
        assert_eq!(issues[0].message, "Flag '-d' is declared after '-o'");
    }

    #[test]
    fn test_lint_sorted_reports_one_pair_per_group() {
        // Three subcommands out of place, one issue naming the first pair to swap.
        let issues = sorted_issues(
            r#"
name "test"
cmd "zebra" help="zebra"
cmd "yak" help="yak"
cmd "xerus" help="xerus"
cmd "walrus" help="walrus"
        "#,
        );
        assert_eq!(issues.len(), 1);
        assert_eq!(
            issues[0].message,
            "Subcommand 'yak' is declared after 'zebra'"
        );
    }

    #[test]
    fn test_lint_sorted_ignores_a_flag_with_no_option() {
        // `flag-no-option` already reports it; it has no name to sort by.
        let issues = sorted_issues(
            r#"
name "test"
flag "--alpha" help="alpha"
flag "nameless:" help="a flag with only a name"
flag "--beta" help="beta"
        "#,
        );
        assert!(issues.is_empty(), "{:?}", issues);
    }

    #[test]
    fn test_lint_invalid_default_subcommand_shows_valid() {
        let spec: Spec = r#"
name "test"
default_subcommand "nonexistent"
cmd "install" help="install"
cmd "update" help="update"
        "#
        .parse()
        .unwrap();

        let issues = lint_spec(&spec, LintOptions::default());
        let issue = issues
            .iter()
            .find(|i| i.code == "invalid-default-subcommand")
            .unwrap();
        assert!(issue.message.contains("valid subcommands:"));
        assert!(issue.message.contains("install"));
        assert!(issue.message.contains("update"));
    }
}
