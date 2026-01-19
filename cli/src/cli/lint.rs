use std::path::PathBuf;
use usage::{Spec, SpecArg, SpecCommand, SpecFlag};

/// Lint a usage spec file for common issues
#[derive(clap::Args)]
pub struct Lint {
    /// A usage spec file to lint
    #[clap(required = true)]
    file: PathBuf,

    /// Output format
    #[clap(long, short, default_value = "text")]
    format: OutputFormat,

    /// Treat warnings as errors
    #[clap(long, short = 'W')]
    warnings_as_errors: bool,
}

#[derive(Clone, Copy, Default, clap::ValueEnum)]
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
        let spec = Spec::parse_file(&self.file)?;
        let issues = lint_spec(&spec);

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

pub fn lint_spec(spec: &Spec) -> Vec<LintIssue> {
    let mut issues = Vec::new();

    // Check spec-level issues
    if spec.bin.is_empty() && spec.name.is_empty() {
        issues.push(LintIssue {
            severity: Severity::Warning,
            code: "missing-name".to_string(),
            message: "Spec has no name or bin defined".to_string(),
            location: None,
        });
    }

    // Check default_subcommand reference
    if let Some(default_subcmd) = &spec.default_subcommand {
        if !spec.cmd.subcommands.contains_key(default_subcmd) {
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

    // Lint the root command
    lint_command(&spec.cmd, &[], &mut issues);

    issues
}

fn lint_command(cmd: &SpecCommand, path: &[&str], issues: &mut Vec<LintIssue>) {
    let cmd_path = if path.is_empty() {
        cmd.name.clone()
    } else {
        format!("{} {}", path.join(" "), cmd.name)
    };

    // Check for missing command help
    if cmd.help.is_none() && !cmd.name.is_empty() {
        issues.push(LintIssue {
            severity: Severity::Info,
            code: "missing-cmd-help".to_string(),
            message: "Command has no help text".to_string(),
            location: Some(format!("cmd {}", cmd_path)),
        });
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

    // Recursively lint subcommands
    let new_path: Vec<&str> = path
        .iter()
        .copied()
        .chain(std::iter::once(cmd.name.as_str()))
        .collect();
    for subcmd in cmd.subcommands.values() {
        lint_command(subcmd, &new_path, issues);
    }
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

        let issues = lint_spec(&spec);
        assert!(issues.iter().any(|i| i.code == "missing-flag-help"));
        assert!(issues.iter().any(|i| i.code == "missing-arg-help"));
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

        let issues = lint_spec(&spec);
        assert!(issues.iter().any(|i| i.code == "duplicate-flag"));
    }

    #[test]
    fn test_lint_no_option_flag() {
        let spec: Spec = r#"
name "test"
flag "myflag:" help="a flag with only a name"
        "#
        .parse()
        .unwrap();

        let issues = lint_spec(&spec);
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

        let issues = lint_spec(&spec);
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

        let issues = lint_spec(&spec);
        assert!(issues.iter().any(|i| i.code == "required-after-optional"));
    }

    #[test]
    fn test_lint_clean_spec() {
        let spec: Spec = r#"
name "test"
bin "test"
flag "-v --verbose" help="Enable verbose output"
arg "<input>" help="Input file"
cmd "sub" help="A subcommand" {
    flag "-f --force" help="Force operation"
}
        "#
        .parse()
        .unwrap();

        let issues = lint_spec(&spec);
        // Should only have info-level issues (missing-cmd-help for root)
        assert!(issues.iter().all(|i| i.severity == Severity::Info));
    }
}
