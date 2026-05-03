use crate::error::UsageErr;
use crate::Spec;

mod bash;
mod fish;
mod nu;
mod powershell;
mod zsh;

/// Options for generating shell completion scripts.
pub struct CompleteOptions {
    /// Path to the `usage` binary (e.g., "usage" or "/usr/local/bin/usage").
    pub usage_bin: String,
    /// Target shell: "bash", "fish", "zsh", or "powershell".
    pub shell: String,
    /// Name of the CLI binary to generate completions for.
    pub bin: String,
    /// Optional cache key (e.g., version) to avoid regenerating the spec file.
    pub cache_key: Option<String>,
    /// The usage spec to embed directly in the completion script.
    pub spec: Option<Spec>,
    /// Command to run to generate the usage spec dynamically.
    pub usage_cmd: Option<String>,
    /// Whether to include the bash-completion library sourcing (bash only).
    pub include_bash_completion_lib: bool,
    /// Source file path for the `@generated` comment.
    pub source_file: Option<String>,
}

/// Generates a shell completion script for the specified shell.
///
/// # Arguments
/// * `options` - Configuration options including target shell and spec source
///
/// # Returns
/// The generated completion script as a string, or an error if the shell is unsupported.
///
/// # Supported Shells
/// - `bash` - Bash completion using `complete` builtin
/// - `fish` - Fish shell completions
/// - `zsh` - Zsh completion using `compdef`
/// - `powershell` - PowerShell completion using `Register-ArgumentCompleter`
pub fn complete(options: &CompleteOptions) -> Result<String, UsageErr> {
    match options.shell.as_str() {
        "bash" => Ok(bash::complete_bash(options)),
        "fish" => Ok(fish::complete_fish(options)),
        "nu" => Ok(nu::complete_nu(options)),
        "powershell" => Ok(powershell::complete_powershell(options)),
        "zsh" => Ok(zsh::complete_zsh(options)),
        _ => Err(UsageErr::UnsupportedShell(options.shell.clone())),
    }
}

/// Generates a shell-specific "init" script that enables tab-completion for any
/// command on `$PATH` whose first line is a `usage` shebang, without requiring
/// per-script `usage g completion` generation. The user sources this once from
/// their shell rc.
///
/// # Supported Shells
/// - `bash` - registers a `complete -D` default handler
/// - `zsh` - registers a `compdef -default-` fallback handler
/// - `fish` - scans `$PATH` once at startup and registers per-command completers
pub fn complete_init(shell: &str, usage_bin: &str) -> Result<String, UsageErr> {
    match shell {
        "bash" => Ok(bash::complete_bash_init(usage_bin)),
        "fish" => Ok(fish::complete_fish_init(usage_bin)),
        "zsh" => Ok(zsh::complete_zsh_init(usage_bin)),
        _ => Err(UsageErr::UnsupportedShell(shell.to_string())),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn complete_init_supported_shells() {
        for shell in ["bash", "zsh", "fish"] {
            let out = complete_init(shell, "usage").expect("supported shell");
            assert!(!out.is_empty(), "{shell} init should not be empty");
        }
    }

    #[test]
    fn complete_init_rejects_unsupported_shell() {
        let err = complete_init("nu", "usage").expect_err("nu has no init script");
        assert!(matches!(err, UsageErr::UnsupportedShell(ref s) if s == "nu"));
    }
}
