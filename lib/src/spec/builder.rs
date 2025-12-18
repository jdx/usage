//! Builder patterns for ergonomic spec construction
//!
//! These builders allow constructing specs without manual Vec allocation,
//! using variadic-friendly methods.
//!
//! # Examples
//!
//! ```
//! use usage::{SpecFlagBuilder, SpecArgBuilder, SpecCommandBuilder};
//!
//! let flag = SpecFlagBuilder::new()
//!     .name("verbose")
//!     .short('v')
//!     .long("verbose")
//!     .help("Enable verbose output")
//!     .build();
//!
//! let arg = SpecArgBuilder::new()
//!     .name("files")
//!     .var(true)
//!     .var_min(1)
//!     .help("Input files")
//!     .build();
//!
//! let cmd = SpecCommandBuilder::new()
//!     .name("install")
//!     .aliases(["i", "add"])
//!     .flag(flag)
//!     .arg(arg)
//!     .build();
//! ```

use crate::{SpecArg, SpecCommand, SpecFlag};

/// Builder for SpecFlag
#[derive(Debug, Default, Clone)]
pub struct SpecFlagBuilder {
    inner: SpecFlag,
}

impl SpecFlagBuilder {
    /// Create a new SpecFlagBuilder
    pub fn new() -> Self {
        Self::default()
    }

    /// Set the flag name
    pub fn name(mut self, name: impl Into<String>) -> Self {
        self.inner.name = name.into();
        self
    }

    /// Add a short flag character (can be called multiple times)
    pub fn short(mut self, c: char) -> Self {
        self.inner.short.push(c);
        self
    }

    /// Add multiple short flags at once
    pub fn shorts(mut self, chars: impl IntoIterator<Item = char>) -> Self {
        self.inner.short.extend(chars);
        self
    }

    /// Add a long flag name (can be called multiple times)
    pub fn long(mut self, name: impl Into<String>) -> Self {
        self.inner.long.push(name.into());
        self
    }

    /// Add multiple long flags at once
    pub fn longs<I, S>(mut self, names: I) -> Self
    where
        I: IntoIterator<Item = S>,
        S: Into<String>,
    {
        self.inner.long.extend(names.into_iter().map(Into::into));
        self
    }

    /// Add a default value (can be called multiple times for var flags)
    pub fn default_value(mut self, value: impl Into<String>) -> Self {
        self.inner.default.push(value.into());
        self.inner.required = false;
        self
    }

    /// Add multiple default values at once
    pub fn default_values<I, S>(mut self, values: I) -> Self
    where
        I: IntoIterator<Item = S>,
        S: Into<String>,
    {
        self.inner
            .default
            .extend(values.into_iter().map(Into::into));
        if !self.inner.default.is_empty() {
            self.inner.required = false;
        }
        self
    }

    /// Set help text
    pub fn help(mut self, text: impl Into<String>) -> Self {
        self.inner.help = Some(text.into());
        self
    }

    /// Set long help text
    pub fn help_long(mut self, text: impl Into<String>) -> Self {
        self.inner.help_long = Some(text.into());
        self
    }

    /// Set markdown help text
    pub fn help_md(mut self, text: impl Into<String>) -> Self {
        self.inner.help_md = Some(text.into());
        self
    }

    /// Set as variadic (can be specified multiple times)
    pub fn var(mut self, is_var: bool) -> Self {
        self.inner.var = is_var;
        self
    }

    /// Set minimum count for variadic flag
    pub fn var_min(mut self, min: usize) -> Self {
        self.inner.var_min = Some(min);
        self
    }

    /// Set maximum count for variadic flag
    pub fn var_max(mut self, max: usize) -> Self {
        self.inner.var_max = Some(max);
        self
    }

    /// Set as required
    pub fn required(mut self, is_required: bool) -> Self {
        self.inner.required = is_required;
        self
    }

    /// Set as global (available to subcommands)
    pub fn global(mut self, is_global: bool) -> Self {
        self.inner.global = is_global;
        self
    }

    /// Set as hidden
    pub fn hide(mut self, is_hidden: bool) -> Self {
        self.inner.hide = is_hidden;
        self
    }

    /// Set as count flag
    pub fn count(mut self, is_count: bool) -> Self {
        self.inner.count = is_count;
        self
    }

    /// Set the argument spec for flags that take values
    pub fn arg(mut self, arg: SpecArg) -> Self {
        self.inner.arg = Some(arg);
        self
    }

    /// Set negate string
    pub fn negate(mut self, negate: impl Into<String>) -> Self {
        self.inner.negate = Some(negate.into());
        self
    }

    /// Set environment variable name
    pub fn env(mut self, env: impl Into<String>) -> Self {
        self.inner.env = Some(env.into());
        self
    }

    /// Set deprecated message
    pub fn deprecated(mut self, msg: impl Into<String>) -> Self {
        self.inner.deprecated = Some(msg.into());
        self
    }

    /// Build the final SpecFlag
    pub fn build(mut self) -> SpecFlag {
        self.inner.usage = self.inner.usage();
        if self.inner.name.is_empty() {
            // Derive name from long or short flags
            if let Some(long) = self.inner.long.first() {
                self.inner.name = long.clone();
            } else if let Some(short) = self.inner.short.first() {
                self.inner.name = short.to_string();
            }
        }
        self.inner
    }
}

/// Builder for SpecArg
#[derive(Debug, Default, Clone)]
pub struct SpecArgBuilder {
    inner: SpecArg,
}

impl SpecArgBuilder {
    /// Create a new SpecArgBuilder
    pub fn new() -> Self {
        Self::default()
    }

    /// Set the argument name
    pub fn name(mut self, name: impl Into<String>) -> Self {
        self.inner.name = name.into();
        self
    }

    /// Add a default value (can be called multiple times for var args)
    pub fn default_value(mut self, value: impl Into<String>) -> Self {
        self.inner.default.push(value.into());
        self.inner.required = false;
        self
    }

    /// Add multiple default values at once
    pub fn default_values<I, S>(mut self, values: I) -> Self
    where
        I: IntoIterator<Item = S>,
        S: Into<String>,
    {
        self.inner
            .default
            .extend(values.into_iter().map(Into::into));
        if !self.inner.default.is_empty() {
            self.inner.required = false;
        }
        self
    }

    /// Set help text
    pub fn help(mut self, text: impl Into<String>) -> Self {
        self.inner.help = Some(text.into());
        self
    }

    /// Set long help text
    pub fn help_long(mut self, text: impl Into<String>) -> Self {
        self.inner.help_long = Some(text.into());
        self
    }

    /// Set markdown help text
    pub fn help_md(mut self, text: impl Into<String>) -> Self {
        self.inner.help_md = Some(text.into());
        self
    }

    /// Set as variadic (accepts multiple values)
    pub fn var(mut self, is_var: bool) -> Self {
        self.inner.var = is_var;
        self
    }

    /// Set minimum count for variadic argument
    pub fn var_min(mut self, min: usize) -> Self {
        self.inner.var_min = Some(min);
        self
    }

    /// Set maximum count for variadic argument
    pub fn var_max(mut self, max: usize) -> Self {
        self.inner.var_max = Some(max);
        self
    }

    /// Set as required
    pub fn required(mut self, is_required: bool) -> Self {
        self.inner.required = is_required;
        self
    }

    /// Set as hidden
    pub fn hide(mut self, is_hidden: bool) -> Self {
        self.inner.hide = is_hidden;
        self
    }

    /// Set environment variable name
    pub fn env(mut self, env: impl Into<String>) -> Self {
        self.inner.env = Some(env.into());
        self
    }

    /// Build the final SpecArg
    pub fn build(mut self) -> SpecArg {
        self.inner.usage = self.inner.usage();
        self.inner
    }
}

/// Builder for SpecCommand
#[derive(Debug, Default, Clone)]
pub struct SpecCommandBuilder {
    inner: SpecCommand,
}

impl SpecCommandBuilder {
    /// Create a new SpecCommandBuilder
    pub fn new() -> Self {
        Self::default()
    }

    /// Set the command name
    pub fn name(mut self, name: impl Into<String>) -> Self {
        self.inner.name = name.into();
        self
    }

    /// Add an alias (can be called multiple times)
    pub fn alias(mut self, alias: impl Into<String>) -> Self {
        self.inner.aliases.push(alias.into());
        self
    }

    /// Add multiple aliases at once
    pub fn aliases<I, S>(mut self, aliases: I) -> Self
    where
        I: IntoIterator<Item = S>,
        S: Into<String>,
    {
        self.inner
            .aliases
            .extend(aliases.into_iter().map(Into::into));
        self
    }

    /// Add a hidden alias (can be called multiple times)
    pub fn hidden_alias(mut self, alias: impl Into<String>) -> Self {
        self.inner.hidden_aliases.push(alias.into());
        self
    }

    /// Add multiple hidden aliases at once
    pub fn hidden_aliases<I, S>(mut self, aliases: I) -> Self
    where
        I: IntoIterator<Item = S>,
        S: Into<String>,
    {
        self.inner
            .hidden_aliases
            .extend(aliases.into_iter().map(Into::into));
        self
    }

    /// Add a flag to the command
    pub fn flag(mut self, flag: SpecFlag) -> Self {
        self.inner.flags.push(flag);
        self
    }

    /// Add multiple flags at once
    pub fn flags(mut self, flags: impl IntoIterator<Item = SpecFlag>) -> Self {
        self.inner.flags.extend(flags);
        self
    }

    /// Add an argument to the command
    pub fn arg(mut self, arg: SpecArg) -> Self {
        self.inner.args.push(arg);
        self
    }

    /// Add multiple arguments at once
    pub fn args(mut self, args: impl IntoIterator<Item = SpecArg>) -> Self {
        self.inner.args.extend(args);
        self
    }

    /// Set help text
    pub fn help(mut self, text: impl Into<String>) -> Self {
        self.inner.help = Some(text.into());
        self
    }

    /// Set long help text
    pub fn help_long(mut self, text: impl Into<String>) -> Self {
        self.inner.help_long = Some(text.into());
        self
    }

    /// Set markdown help text
    pub fn help_md(mut self, text: impl Into<String>) -> Self {
        self.inner.help_md = Some(text.into());
        self
    }

    /// Set as hidden
    pub fn hide(mut self, is_hidden: bool) -> Self {
        self.inner.hide = is_hidden;
        self
    }

    /// Set subcommand required
    pub fn subcommand_required(mut self, required: bool) -> Self {
        self.inner.subcommand_required = required;
        self
    }

    /// Set deprecated message
    pub fn deprecated(mut self, msg: impl Into<String>) -> Self {
        self.inner.deprecated = Some(msg.into());
        self
    }

    /// Build the final SpecCommand
    pub fn build(mut self) -> SpecCommand {
        self.inner.usage = self.inner.usage();
        self.inner
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_flag_builder_basic() {
        let flag = SpecFlagBuilder::new()
            .name("verbose")
            .short('v')
            .long("verbose")
            .help("Enable verbose output")
            .build();

        assert_eq!(flag.name, "verbose");
        assert_eq!(flag.short, vec!['v']);
        assert_eq!(flag.long, vec!["verbose".to_string()]);
        assert_eq!(flag.help, Some("Enable verbose output".to_string()));
    }

    #[test]
    fn test_flag_builder_multiple_values() {
        let flag = SpecFlagBuilder::new()
            .shorts(['v', 'V'])
            .longs(["verbose", "loud"])
            .default_values(["info", "warn"])
            .build();

        assert_eq!(flag.short, vec!['v', 'V']);
        assert_eq!(
            flag.long,
            vec!["verbose".to_string(), "loud".to_string()]
        );
        assert_eq!(
            flag.default,
            vec!["info".to_string(), "warn".to_string()]
        );
        assert!(!flag.required); // Should be false due to defaults
    }

    #[test]
    fn test_flag_builder_variadic() {
        let flag = SpecFlagBuilder::new()
            .long("file")
            .var(true)
            .var_min(1)
            .var_max(10)
            .build();

        assert!(flag.var);
        assert_eq!(flag.var_min, Some(1));
        assert_eq!(flag.var_max, Some(10));
    }

    #[test]
    fn test_flag_builder_name_derivation() {
        let flag = SpecFlagBuilder::new()
            .short('v')
            .long("verbose")
            .build();

        // Name should be derived from long flag
        assert_eq!(flag.name, "verbose");

        let flag2 = SpecFlagBuilder::new().short('v').build();

        // Name should be derived from short flag if no long
        assert_eq!(flag2.name, "v");
    }

    #[test]
    fn test_arg_builder_basic() {
        let arg = SpecArgBuilder::new()
            .name("file")
            .help("Input file")
            .required(true)
            .build();

        assert_eq!(arg.name, "file");
        assert_eq!(arg.help, Some("Input file".to_string()));
        assert!(arg.required);
    }

    #[test]
    fn test_arg_builder_variadic() {
        let arg = SpecArgBuilder::new()
            .name("files")
            .var(true)
            .var_min(1)
            .var_max(10)
            .help("Input files")
            .build();

        assert_eq!(arg.name, "files");
        assert!(arg.var);
        assert_eq!(arg.var_min, Some(1));
        assert_eq!(arg.var_max, Some(10));
    }

    #[test]
    fn test_arg_builder_defaults() {
        let arg = SpecArgBuilder::new()
            .name("file")
            .default_values(["a.txt", "b.txt"])
            .build();

        assert_eq!(
            arg.default,
            vec!["a.txt".to_string(), "b.txt".to_string()]
        );
        assert!(!arg.required);
    }

    #[test]
    fn test_command_builder_basic() {
        let cmd = SpecCommandBuilder::new()
            .name("install")
            .help("Install packages")
            .build();

        assert_eq!(cmd.name, "install");
        assert_eq!(cmd.help, Some("Install packages".to_string()));
    }

    #[test]
    fn test_command_builder_aliases() {
        let cmd = SpecCommandBuilder::new()
            .name("install")
            .alias("i")
            .aliases(["add", "get"])
            .hidden_aliases(["inst"])
            .build();

        assert_eq!(
            cmd.aliases,
            vec!["i".to_string(), "add".to_string(), "get".to_string()]
        );
        assert_eq!(cmd.hidden_aliases, vec!["inst".to_string()]);
    }

    #[test]
    fn test_command_builder_with_flags_and_args() {
        let flag = SpecFlagBuilder::new()
            .short('f')
            .long("force")
            .build();

        let arg = SpecArgBuilder::new()
            .name("package")
            .required(true)
            .build();

        let cmd = SpecCommandBuilder::new()
            .name("install")
            .flag(flag)
            .arg(arg)
            .build();

        assert_eq!(cmd.flags.len(), 1);
        assert_eq!(cmd.flags[0].name, "force");
        assert_eq!(cmd.args.len(), 1);
        assert_eq!(cmd.args[0].name, "package");
    }
}
