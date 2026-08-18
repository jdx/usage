use std::cmp::Ordering;
use std::path::PathBuf;
use usage::{Spec, SpecArg, SpecCommand, SpecFlag};

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

    if spec.multicall && spec.cmd.subcommands.is_empty() {
        issues.push(LintIssue {
            severity: Severity::Error,
            code: "multicall-no-subcommands".to_string(),
            message: "Spec has multicall=#true but no subcommands to select".to_string(),
            location: None,
        });
    }

    // Lint the root command
    lint_command(&spec.cmd, &[], spec.about.is_some(), opts, &mut issues);

    issues
}

fn lint_command(
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
        lint_command(subcmd, &new_path, false, opts, issues);
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

#[cfg(test)]
mod tests {
    use super::*;

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
