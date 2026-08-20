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

use crate::spec::cmd::SpecExample;
use crate::spec::effect::SpecCommandEffect;
use crate::{
    spec::arg::SpecDoubleDashChoices, SpecArg, SpecChoices, SpecCommand, SpecDefaultIf, SpecFlag,
    SpecRequiredIfEq, SpecRequiresIf,
};

/// Builder for SpecFlag
#[derive(Debug, Default, Clone)]
pub struct SpecFlagBuilder {
    inner: SpecFlag,
    allow_hyphen_values: bool,
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

    /// Add a flag whose presence makes this flag required
    pub fn required_if(mut self, flag: impl Into<String>) -> Self {
        self.inner.required_if.push(flag.into());
        self
    }

    /// Add flags whose presence makes this flag required
    pub fn required_if_any<I, S>(mut self, flags: I) -> Self
    where
        I: IntoIterator<Item = S>,
        S: Into<String>,
    {
        self.inner
            .required_if
            .extend(flags.into_iter().map(Into::into));
        self
    }

    /// Add a selector/value condition that makes this flag required.
    pub fn required_if_eq(mut self, selector: impl Into<String>, value: impl Into<String>) -> Self {
        self.inner.required_if_eq.push(SpecRequiredIfEq {
            selector: selector.into(),
            value: value.into(),
        });
        self
    }

    /// Set selector/value conditions which must all match to require this flag.
    pub fn required_if_eq_all<I, S, V>(mut self, conditions: I) -> Self
    where
        I: IntoIterator<Item = (S, V)>,
        S: Into<String>,
        V: Into<String>,
    {
        self.inner
            .required_if_eq_all
            .extend(
                conditions
                    .into_iter()
                    .map(|(selector, value)| SpecRequiredIfEq {
                        selector: selector.into(),
                        value: value.into(),
                    }),
            );
        self
    }

    /// Add a flag whose absence makes this flag required
    pub fn required_unless(mut self, flag: impl Into<String>) -> Self {
        self.inner.required_unless.push(flag.into());
        self
    }

    /// Add flags where the absence of all of them makes this flag required
    pub fn required_unless_any<I, S>(mut self, flags: I) -> Self
    where
        I: IntoIterator<Item = S>,
        S: Into<String>,
    {
        self.inner
            .required_unless
            .extend(flags.into_iter().map(Into::into));
        self
    }

    /// Add selectors which must all be present to waive this flag's requirement.
    pub fn required_unless_all<I, S>(mut self, flags: I) -> Self
    where
        I: IntoIterator<Item = S>,
        S: Into<String>,
    {
        self.inner
            .required_unless_all
            .extend(flags.into_iter().map(Into::into));
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

    /// Allow this flag's value to start with `-`
    pub fn allow_hyphen_values(mut self, allow: bool) -> Self {
        self.allow_hyphen_values = allow;
        if let Some(arg) = &mut self.inner.arg {
            arg.double_dash = if allow {
                crate::spec::arg::SpecDoubleDashChoices::Automatic
            } else {
                crate::spec::arg::SpecDoubleDashChoices::Optional
            };
        }
        self
    }

    /// Require `--flag=value` and refuse `--flag value`.
    pub fn require_equals(mut self, require: bool) -> Self {
        self.inner.require_equals = require;
        self
    }

    /// Value used when the flag is present but no value is given.
    pub fn default_missing(mut self, value: impl Into<String>) -> Self {
        self.inner.default_missing = Some(value.into());
        self
    }

    /// Set the argument spec for flags that take values
    pub fn arg(mut self, arg: SpecArg) -> Self {
        self.inner.arg = Some(arg);
        if self.allow_hyphen_values {
            if let Some(arg) = &mut self.inner.arg {
                arg.double_dash = crate::spec::arg::SpecDoubleDashChoices::Automatic;
            }
        }
        self
    }

    /// Set negate string
    pub fn negate(mut self, negate: impl Into<String>) -> Self {
        self.inner.negate = Some(negate.into());
        self
    }

    /// Add a flag that this flag mutually overrides
    pub fn override_with(mut self, flag: impl Into<String>) -> Self {
        self.inner.overrides.push(flag.into());
        self
    }

    /// Add flags that this flag mutually overrides
    pub fn overrides_with<I, S>(mut self, flags: I) -> Self
    where
        I: IntoIterator<Item = S>,
        S: Into<String>,
    {
        self.inner
            .overrides
            .extend(flags.into_iter().map(Into::into));
        self
    }

    /// Add a flag that must also be given when this one is
    pub fn require(mut self, flag: impl Into<String>) -> Self {
        self.inner.requires.push(flag.into());
        self
    }

    /// Add flags that must also be given when this one is
    pub fn requires<I, S>(mut self, flags: I) -> Self
    where
        I: IntoIterator<Item = S>,
        S: Into<String>,
    {
        self.inner
            .requires
            .extend(flags.into_iter().map(Into::into));
        self
    }

    /// Add a flag required when this flag is explicitly given `value`
    pub fn requires_if(mut self, value: impl Into<String>, flag: impl Into<String>) -> Self {
        self.inner.requires_if.push(SpecRequiresIf {
            value: value.into(),
            requires: flag.into(),
        });
        self
    }

    /// Add value-conditional flag requirements
    pub fn requires_ifs<I, V, S>(mut self, requirements: I) -> Self
    where
        I: IntoIterator<Item = (V, S)>,
        V: Into<String>,
        S: Into<String>,
    {
        self.inner
            .requires_if
            .extend(
                requirements
                    .into_iter()
                    .map(|(value, requires)| SpecRequiresIf {
                        value: value.into(),
                        requires: requires.into(),
                    }),
            );
        self
    }

    /// Bind `value` on this flag when `selector` is present.
    ///
    /// clap's `default_value_if(id, ArgPredicate::IsPresent, value)`.
    pub fn default_if(mut self, selector: impl Into<String>, value: impl Into<String>) -> Self {
        self.inner.default_if.push(SpecDefaultIf {
            selector: selector.into(),
            when: None,
            value: value.into(),
        });
        self
    }

    /// Bind `value` on this flag when `selector` is explicitly `when`.
    ///
    /// clap's `default_value_if(id, ArgPredicate::Equals(when), value)`.
    pub fn default_if_eq(
        mut self,
        selector: impl Into<String>,
        when: impl Into<String>,
        value: impl Into<String>,
    ) -> Self {
        self.inner.default_if.push(SpecDefaultIf {
            selector: selector.into(),
            when: Some(when.into()),
            value: value.into(),
        });
        self
    }

    /// Add several conditional defaults, in first-match-wins order.
    pub fn default_ifs<I>(mut self, conditions: I) -> Self
    where
        I: IntoIterator<Item = SpecDefaultIf>,
    {
        self.inner.default_if.extend(conditions);
        self
    }

    /// Heading to list this under in help output.
    pub fn help_heading(mut self, help_heading: impl Into<String>) -> Self {
        self.inner.help_heading = Some(help_heading.into());
        self
    }

    pub fn env(mut self, env: impl Into<String>) -> Self {
        self.inner.env = Some(env.into());
        self
    }

    /// Set deprecated message
    pub fn deprecated(mut self, msg: impl Into<String>) -> Self {
        self.inner.deprecated = Some(msg.into());
        self
    }

    /// Set the rendered usage string. `build` derives this when unset.
    pub fn usage(mut self, usage: impl Into<String>) -> Self {
        self.inner.usage = usage.into();
        self
    }

    /// Set the first line of help text. Derived from `help` when unset.
    pub fn help_first_line(mut self, text: impl Into<String>) -> Self {
        self.inner.help_first_line = Some(text.into());
        self
    }

    /// Raise the command's effect when this flag is supplied.
    pub fn effect(mut self, effect: SpecCommandEffect) -> Self {
        self.inner.effect = Some(effect);
        self
    }

    /// Build the final SpecFlag
    #[must_use]
    pub fn build(mut self) -> SpecFlag {
        if self.allow_hyphen_values {
            if let Some(arg) = &mut self.inner.arg {
                arg.double_dash = crate::spec::arg::SpecDoubleDashChoices::Automatic;
            }
        }
        if self.inner.default_missing.is_some() {
            if let Some(arg) = &mut self.inner.arg {
                arg.required = false;
            }
        }
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

    /// Set the ordered placeholders for a fixed-arity value.
    pub fn value_names<I, S>(mut self, names: I) -> Self
    where
        I: IntoIterator<Item = S>,
        S: Into<String>,
    {
        self.inner.value_names = names.into_iter().map(Into::into).collect();
        if let Some(first) = self.inner.value_names.first() {
            self.inner.name.clone_from(first);
        }
        if self.inner.value_names.len() > 1 {
            let arity = self.inner.value_names.len();
            self.inner.var = true;
            self.inner.var_min = Some(arity);
            self.inner.var_max = Some(arity);
        }
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

    /// Add arguments that must be satisfied when this positional is present.
    pub fn requires<I, S>(mut self, selectors: I) -> Self
    where
        I: IntoIterator<Item = S>,
        S: Into<String>,
    {
        self.inner
            .requires
            .extend(selectors.into_iter().map(Into::into));
        self
    }

    /// Add selectors whose presence makes this positional required.
    pub fn required_if_any<I, S>(mut self, selectors: I) -> Self
    where
        I: IntoIterator<Item = S>,
        S: Into<String>,
    {
        self.inner
            .required_if
            .extend(selectors.into_iter().map(Into::into));
        self
    }

    /// Add a selector/value condition that makes this positional required.
    pub fn required_if_eq(mut self, selector: impl Into<String>, value: impl Into<String>) -> Self {
        self.inner.required_if_eq.push(SpecRequiredIfEq {
            selector: selector.into(),
            value: value.into(),
        });
        self
    }

    /// Set selector/value conditions which must all match to require this positional.
    pub fn required_if_eq_all<I, S, V>(mut self, conditions: I) -> Self
    where
        I: IntoIterator<Item = (S, V)>,
        S: Into<String>,
        V: Into<String>,
    {
        self.inner
            .required_if_eq_all
            .extend(
                conditions
                    .into_iter()
                    .map(|(selector, value)| SpecRequiredIfEq {
                        selector: selector.into(),
                        value: value.into(),
                    }),
            );
        self
    }

    /// Add selectors where any presence waives this positional's requirement.
    pub fn required_unless_any<I, S>(mut self, selectors: I) -> Self
    where
        I: IntoIterator<Item = S>,
        S: Into<String>,
    {
        self.inner
            .required_unless
            .extend(selectors.into_iter().map(Into::into));
        self
    }

    /// Add selectors which must all be present to waive this positional's requirement.
    pub fn required_unless_all<I, S>(mut self, selectors: I) -> Self
    where
        I: IntoIterator<Item = S>,
        S: Into<String>,
    {
        self.inner
            .required_unless_all
            .extend(selectors.into_iter().map(Into::into));
        self
    }

    /// Set as hidden
    pub fn hide(mut self, is_hidden: bool) -> Self {
        self.inner.hide = is_hidden;
        self
    }

    /// Set environment variable name
    /// Heading to list this under in help output.
    pub fn help_heading(mut self, help_heading: impl Into<String>) -> Self {
        self.inner.help_heading = Some(help_heading.into());
        self
    }

    pub fn env(mut self, env: impl Into<String>) -> Self {
        self.inner.env = Some(env.into());
        self
    }

    /// Set the double-dash behavior
    pub fn double_dash(mut self, behavior: SpecDoubleDashChoices) -> Self {
        self.inner.double_dash = behavior;
        self
    }

    /// Set choices for this argument
    pub fn choices<I, S>(mut self, choices: I) -> Self
    where
        I: IntoIterator<Item = S>,
        S: Into<String>,
    {
        let spec_choices = self.inner.choices.get_or_insert_with(SpecChoices::default);
        #[cfg(feature = "unstable_choices_env")]
        let env = spec_choices.env().map(ToString::to_string);
        spec_choices.choices = choices.into_iter().map(Into::into).collect();
        #[cfg(feature = "unstable_choices_env")]
        spec_choices.set_env(env);
        self
    }

    /// Set a portable expr expression that must accept each raw value.
    pub fn validate(mut self, expression: impl Into<String>) -> Self {
        self.inner.validate = Some(expression.into());
        self
    }

    /// Set the message reported when validation returns false.
    pub fn validate_error(mut self, message: impl Into<String>) -> Self {
        self.inner.validate_error = Some(message.into());
        self
    }

    /// Set choices from an environment variable
    #[cfg(feature = "unstable_choices_env")]
    pub fn choices_env(mut self, env: impl Into<String>) -> Self {
        let choices = self.inner.choices.get_or_insert_with(SpecChoices::default);
        choices.set_env(Some(env.into()));
        self
    }

    /// Set the rendered usage string. `build` derives this when unset.
    pub fn usage(mut self, usage: impl Into<String>) -> Self {
        self.inner.usage = usage.into();
        self
    }

    /// Set the first line of help text. Derived from `help` when unset.
    pub fn help_first_line(mut self, text: impl Into<String>) -> Self {
        self.inner.help_first_line = Some(text.into());
        self
    }

    /// Raise the command's effect when this argument is supplied.
    pub fn effect(mut self, effect: SpecCommandEffect) -> Self {
        self.inner.effect = Some(effect);
        self
    }

    /// Build the final SpecArg
    #[must_use]
    pub fn build(mut self) -> SpecArg {
        if self.inner.validate.is_none() {
            self.inner.validate_error = None;
        }
        if self.inner.value_names.len() > 1 {
            let arity = self.inner.value_names.len();
            self.inner.var = true;
            self.inner.var_min = Some(arity);
            self.inner.var_max = Some(arity);
        }
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

    /// Forward an unmatched word as an external command plus the rest of argv
    pub fn external_subcommand(mut self, enabled: bool) -> Self {
        self.inner.external_subcommand = enabled;
        self
    }

    /// Set whether a later scalar flag occurrence replaces an earlier one.
    ///
    /// Enabled by default. Set false to reject duplicate scalar flags.
    pub fn args_override_self(mut self, enabled: bool) -> Self {
        self.inner.args_override_self = enabled;
        self
    }

    /// Set whether selecting a subcommand suppresses this command's requirements.
    pub fn subcommand_negates_reqs(mut self, enabled: bool) -> Self {
        self.inner.subcommand_negates_reqs = enabled;
        self
    }

    /// Set what running this command does to the world
    pub fn effect(mut self, effect: SpecCommandEffect) -> Self {
        self.inner.effect = Some(effect);
        self
    }

    /// Set deprecated message
    pub fn deprecated(mut self, msg: impl Into<String>) -> Self {
        self.inner.deprecated = Some(msg.into());
        self
    }

    /// Set restart token for resetting argument parsing
    /// e.g., `mise run lint ::: test ::: check` with restart_token=":::"
    pub fn restart_token(mut self, token: impl Into<String>) -> Self {
        self.inner.restart_token = Some(token.into());
        self
    }

    /// Add a subcommand (can be called multiple times)
    pub fn subcommand(mut self, cmd: SpecCommand) -> Self {
        self.inner.subcommands.insert(cmd.name.clone(), cmd);
        self
    }

    /// Add multiple subcommands at once
    pub fn subcommands(mut self, cmds: impl IntoIterator<Item = SpecCommand>) -> Self {
        for cmd in cmds {
            self.inner.subcommands.insert(cmd.name.clone(), cmd);
        }
        self
    }

    /// Set before_help text (displayed before the help message)
    pub fn before_help(mut self, text: impl Into<String>) -> Self {
        self.inner.before_help = Some(text.into());
        self
    }

    /// Set before_help_long text
    pub fn before_help_long(mut self, text: impl Into<String>) -> Self {
        self.inner.before_help_long = Some(text.into());
        self
    }

    /// Set before_help markdown text
    pub fn before_help_md(mut self, text: impl Into<String>) -> Self {
        self.inner.before_help_md = Some(text.into());
        self
    }

    /// Set after_help text (displayed after the help message)
    pub fn after_help(mut self, text: impl Into<String>) -> Self {
        self.inner.after_help = Some(text.into());
        self
    }

    /// Set after_help_long text
    pub fn after_help_long(mut self, text: impl Into<String>) -> Self {
        self.inner.after_help_long = Some(text.into());
        self
    }

    /// Set after_help markdown text
    pub fn after_help_md(mut self, text: impl Into<String>) -> Self {
        self.inner.after_help_md = Some(text.into());
        self
    }

    /// Add an example (can be called multiple times)
    pub fn example(mut self, code: impl Into<String>) -> Self {
        self.inner.examples.push(SpecExample::new(code.into()));
        self
    }

    /// Add an example with header and help text
    pub fn example_with_help(
        mut self,
        code: impl Into<String>,
        header: impl Into<String>,
        help: impl Into<String>,
    ) -> Self {
        let mut example = SpecExample::new(code.into());
        example.header = Some(header.into());
        example.help = Some(help.into());
        self.inner.examples.push(example);
        self
    }

    /// Build the final SpecCommand
    #[must_use]
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
        assert_eq!(flag.long, vec!["verbose".to_string(), "loud".to_string()]);
        assert_eq!(flag.default, vec!["info".to_string(), "warn".to_string()]);
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
    fn test_flag_builder_conditional_requirements() {
        let flag = SpecFlagBuilder::new()
            .long("config")
            .requires_if("special.toml", "--key")
            .requires_ifs([("remote.toml", "--token"), ("signed.toml", "--identity")])
            .build();

        assert_eq!(
            flag.requires_if,
            [
                SpecRequiresIf {
                    value: "special.toml".into(),
                    requires: "--key".into(),
                },
                SpecRequiresIf {
                    value: "remote.toml".into(),
                    requires: "--token".into(),
                },
                SpecRequiresIf {
                    value: "signed.toml".into(),
                    requires: "--identity".into(),
                },
            ]
        );
    }

    #[test]
    fn test_flag_builder_conditional_defaults() {
        let flag = SpecFlagBuilder::new()
            .long("bin-names")
            .default_if("--json", "true")
            .default_if_eq("--output", "json", "pretty")
            .build();

        assert_eq!(
            flag.default_if,
            [
                SpecDefaultIf {
                    selector: "--json".into(),
                    when: None,
                    value: "true".into(),
                },
                SpecDefaultIf {
                    selector: "--output".into(),
                    when: Some("json".into()),
                    value: "pretty".into(),
                },
            ]
        );
    }

    #[test]
    fn test_flag_builder_name_derivation() {
        let flag = SpecFlagBuilder::new().short('v').long("verbose").build();

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

        assert_eq!(arg.default, vec!["a.txt".to_string(), "b.txt".to_string()]);
        assert!(!arg.required);
    }

    #[test]
    fn test_arg_builder_drops_validation_error_without_expression() {
        let arg = SpecArgBuilder::new()
            .name("port")
            .validate_error("must be a valid port")
            .build();

        assert!(arg.validate_error.is_none());
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
        let flag = SpecFlagBuilder::new().short('f').long("force").build();

        let arg = SpecArgBuilder::new().name("package").required(true).build();

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

    #[test]
    fn test_arg_builder_choices() {
        let arg = SpecArgBuilder::new()
            .name("format")
            .choices(["json", "yaml", "toml"])
            .build();

        assert!(arg.choices.is_some());
        let choices = arg.choices.unwrap();
        assert_eq!(
            choices.choices,
            vec!["json".to_string(), "yaml".to_string(), "toml".to_string()]
        );
        assert_eq!(choices.env(), None);
    }

    #[cfg(feature = "unstable_choices_env")]
    #[test]
    fn test_arg_builder_choices_env() {
        let arg = SpecArgBuilder::new()
            .name("env")
            .choices(["local"])
            .choices_env("DEPLOY_ENVS")
            .build();

        let choices = arg.choices.unwrap();
        assert_eq!(choices.choices, vec!["local".to_string()]);
        assert_eq!(choices.env(), Some("DEPLOY_ENVS"));
    }

    #[cfg(feature = "unstable_choices_env")]
    #[test]
    fn test_arg_builder_choices_preserves_choices_env() {
        let arg = SpecArgBuilder::new()
            .name("env")
            .choices_env("DEPLOY_ENVS")
            .choices(["local"])
            .build();

        let choices = arg.choices.unwrap();
        assert_eq!(choices.choices, vec!["local".to_string()]);
        assert_eq!(choices.env(), Some("DEPLOY_ENVS"));
    }

    #[test]
    fn test_command_builder_subcommands() {
        let sub1 = SpecCommandBuilder::new().name("sub1").build();
        let sub2 = SpecCommandBuilder::new().name("sub2").build();

        let cmd = SpecCommandBuilder::new()
            .name("main")
            .subcommand(sub1)
            .subcommand(sub2)
            .build();

        assert_eq!(cmd.subcommands.len(), 2);
        assert!(cmd.subcommands.contains_key("sub1"));
        assert!(cmd.subcommands.contains_key("sub2"));
    }

    #[test]
    fn test_command_builder_before_after_help() {
        let cmd = SpecCommandBuilder::new()
            .name("test")
            .before_help("Before help text")
            .before_help_long("Before help long text")
            .after_help("After help text")
            .after_help_long("After help long text")
            .build();

        assert_eq!(cmd.before_help, Some("Before help text".to_string()));
        assert_eq!(
            cmd.before_help_long,
            Some("Before help long text".to_string())
        );
        assert_eq!(cmd.after_help, Some("After help text".to_string()));
        assert_eq!(
            cmd.after_help_long,
            Some("After help long text".to_string())
        );
    }

    #[test]
    fn test_command_builder_examples() {
        let cmd = SpecCommandBuilder::new()
            .name("test")
            .example("mycli run")
            .example_with_help("mycli build", "Build example", "Build the project")
            .build();

        assert_eq!(cmd.examples.len(), 2);
        assert_eq!(cmd.examples[0].code, "mycli run");
        assert_eq!(cmd.examples[1].code, "mycli build");
        assert_eq!(cmd.examples[1].header, Some("Build example".to_string()));
        assert_eq!(cmd.examples[1].help, Some("Build the project".to_string()));
    }
}
