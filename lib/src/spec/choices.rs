use crate::kdl::{KdlDocument, KdlEntry, KdlNode};
use serde::{Deserialize, Serialize};
use std::cell::{Cell, RefCell};
use std::collections::HashMap;

use crate::error::UsageErr;
use crate::spec::context::ParsingContext;
use crate::spec::helpers::NodeHelper;

#[derive(Debug, Clone, Serialize, Deserialize)]
#[non_exhaustive]
pub struct SpecChoices {
    pub choices: Vec<String>,
    /// Metadata for canonical values that need more than the shorthand string form.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub details: Vec<SpecChoice>,
    /// Match canonical values and aliases without regard to ASCII case.
    #[serde(default, skip_serializing_if = "crate::spec::is_false")]
    pub ignore_case: bool,
    /// Whether values outside the declared set are rejected.
    #[serde(
        default = "default_strict",
        skip_serializing_if = "crate::spec::is_true"
    )]
    pub strict: bool,
    #[cfg(feature = "unstable_choices_env")]
    #[serde(skip_serializing_if = "Option::is_none")]
    pub env: Option<String>,
    /// A shell command whose output lists further values, one per line.
    ///
    /// Run only where a program is running: when a value is checked, when a word is being
    /// completed, and for a help page asked for with [`crate::docs::cli::render_runtime_help`]
    /// or by `--help` during a parse. Generated documentation and code never run it, and
    /// describe it instead.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub run: Option<String>,
}

const fn default_strict() -> bool {
    true
}

impl Default for SpecChoices {
    fn default() -> Self {
        Self {
            choices: Vec::new(),
            details: Vec::new(),
            ignore_case: false,
            strict: true,
            #[cfg(feature = "unstable_choices_env")]
            env: None,
            run: None,
        }
    }
}

#[derive(Debug, Default, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct SpecChoice {
    pub value: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub help: Option<String>,
    #[serde(default, skip_serializing_if = "crate::spec::is_false")]
    pub hide: bool,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub aliases: Vec<SpecChoiceAlias>,
}

#[derive(Debug, Default, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct SpecChoiceAlias {
    pub value: String,
    #[serde(default, skip_serializing_if = "crate::spec::is_false")]
    pub hide: bool,
}

impl SpecChoices {
    /// The set of values an arg or flag accepts.
    pub fn new(choices: impl IntoIterator<Item = impl Into<String>>) -> Self {
        Self {
            choices: choices.into_iter().map(Into::into).collect(),
            ..Default::default()
        }
    }
}

impl SpecChoices {
    #[cfg(feature = "unstable_choices_env")]
    #[must_use]
    pub fn env(&self) -> Option<&str> {
        self.env.as_deref()
    }

    #[cfg(not(feature = "unstable_choices_env"))]
    #[must_use]
    pub fn env(&self) -> Option<&str> {
        None
    }

    #[cfg(feature = "unstable_choices_env")]
    pub fn set_env(&mut self, env: Option<String>) {
        self.env = env;
    }

    /// The command whose output lists further values, if the choices name one.
    #[must_use]
    pub fn run(&self) -> Option<&str> {
        self.run.as_deref()
    }

    pub(crate) fn parse(ctx: &ParsingContext, node: &NodeHelper) -> Result<Self, UsageErr> {
        let mut config = Self {
            choices: node
                .args()
                .map(|e| e.ensure_string())
                .collect::<Result<_, _>>()?,
            ..Default::default()
        };

        for (k, v) in node.props() {
            match k {
                #[cfg(feature = "unstable_choices_env")]
                "env" => config.set_env(Some(v.ensure_string()?)),
                "ignore_case" => config.ignore_case = v.ensure_bool()?,
                "run" => config.run = Some(v.ensure_string()?),
                "strict" => config.strict = v.ensure_bool()?,
                k => bail_parse!(ctx, v.entry.span(), "unsupported choices key {k}"),
            }
        }

        for choice in node.children() {
            if choice.name() != "choice" {
                bail_parse!(
                    ctx,
                    choice.node.name().span(),
                    "a choices block holds `choice` nodes"
                );
            }
            choice.ensure_arg_len(1..=1)?;
            let value = choice.arg(0)?.ensure_string()?;
            let mut detail = SpecChoice {
                value: value.clone(),
                ..Default::default()
            };
            for (key, entry) in choice.props() {
                match key {
                    "help" => detail.help = Some(entry.ensure_string()?),
                    "hide" => detail.hide = entry.ensure_bool()?,
                    key => bail_parse!(ctx, entry.entry.span(), "unsupported choice key {key}"),
                }
            }
            for alias in choice.children() {
                if alias.name() != "alias" {
                    bail_parse!(
                        ctx,
                        alias.node.name().span(),
                        "a choice block holds `alias` nodes"
                    );
                }
                alias.ensure_arg_len(1..=1)?;
                let mut parsed = SpecChoiceAlias {
                    value: alias.arg(0)?.ensure_string()?,
                    ..Default::default()
                };
                for (key, entry) in alias.props() {
                    match key {
                        "hide" => parsed.hide = entry.ensure_bool()?,
                        key => bail_parse!(ctx, entry.entry.span(), "unsupported alias key {key}"),
                    }
                }
                if !alias.children().is_empty() {
                    bail_parse!(ctx, alias.span(), "an alias cannot have children");
                }
                detail.aliases.push(parsed);
            }
            if config.choices.contains(&value) {
                bail_parse!(
                    ctx,
                    choice.span(),
                    "choice `{value}` is declared more than once"
                );
            }
            config.choices.push(value);
            config.details.push(detail);
        }

        if config.choices.is_empty() && config.run.is_none() {
            #[cfg(feature = "unstable_choices_env")]
            if config.env().is_none() {
                bail_parse!(
                    ctx,
                    node.span(),
                    "choices must have at least 1 argument, an env property, or a run property"
                );
            }
            #[cfg(not(feature = "unstable_choices_env"))]
            bail_parse!(
                ctx,
                node.span(),
                "choices must have at least 1 argument or a run property"
            );
        }

        Ok(config)
    }

    /// The declared values, and those named by `env=`. Never runs `run=`: see
    /// [`Self::values_from_run`] and [`Self::resolved_values`] for that.
    pub fn values(&self) -> Vec<String> {
        self.values_with_env(None)
    }

    /// Run `run=` and return the values it printed: one per line, trimmed, with blank lines
    /// skipped. Empty when the choices name no command.
    ///
    /// `env` is added to the command's environment, for a caller parsing against a
    /// different one (see [`crate::Parser::with_env`]). Within a single parse or help page the
    /// same command runs once, however many values are checked against it.
    pub fn values_from_run(
        &self,
        env: Option<&HashMap<String, String>>,
    ) -> Result<Vec<String>, UsageErr> {
        match &self.run {
            Some(run) => run_choices(run, env),
            None => Ok(Vec::new()),
        }
    }

    /// Every value these choices offer: the declared ones, `env=`'s, and what `run=` prints,
    /// without repeats. What completion offers and what a running program's help lists.
    pub fn resolved_values(
        &self,
        env: Option<&HashMap<String, String>>,
    ) -> Result<Vec<String>, UsageErr> {
        let mut values = self.values_with_env(env);
        for value in self.values_from_run(env)? {
            if !values.contains(&value) {
                values.push(value);
            }
        }
        Ok(values)
    }

    /// Whether `value` is one of the choices, running `run=` only when the declared values
    /// and `env=` do not already accept it.
    pub(crate) fn matches_resolved(
        &self,
        value: &str,
        env: Option<&HashMap<String, String>>,
    ) -> Result<bool, UsageErr> {
        if self.matches_with_env(value, env) {
            return Ok(true);
        }
        Ok(self.matches_values(value, self.values_from_run(env)?))
    }

    /// Whether `value` is one of the declared values or those named by `env=`. Never runs
    /// `run=`.
    pub fn matches(&self, value: &str) -> bool {
        self.matches_static(value) || self.matches_values(value, self.values_with_env(None))
    }

    fn matches_static(&self, value: &str) -> bool {
        let equals = |candidate: &str| {
            if self.ignore_case {
                candidate.eq_ignore_ascii_case(value)
            } else {
                candidate == value
            }
        };
        self.choices.iter().any(|choice| equals(choice))
            || self
                .details
                .iter()
                .flat_map(|choice| &choice.aliases)
                .any(|alias| equals(&alias.value))
    }

    pub(crate) fn matches_with_env(
        &self,
        value: &str,
        env: Option<&HashMap<String, String>>,
    ) -> bool {
        self.matches_static(value) || self.matches_values(value, self.values_with_env(env))
    }

    fn matches_values(&self, value: &str, values: impl IntoIterator<Item = String>) -> bool {
        values.into_iter().any(|candidate| {
            if self.ignore_case {
                candidate.eq_ignore_ascii_case(value)
            } else {
                candidate == value
            }
        })
    }

    pub(crate) fn values_with_env(&self, env: Option<&HashMap<String, String>>) -> Vec<String> {
        let values = self.visible_declared();

        #[cfg(not(feature = "unstable_choices_env"))]
        let _ = env;

        #[cfg(feature = "unstable_choices_env")]
        let values = {
            let mut values = values;
            if let Some(env_key) = self.env() {
                let env_value = if let Some(env_map) = env {
                    env_map.get(env_key).cloned()
                } else {
                    std::env::var(env_key).ok()
                };

                if let Some(env_value) = env_value {
                    for choice in env_value
                        .split(|c: char| c == ',' || c.is_whitespace())
                        .filter(|choice| !choice.is_empty())
                    {
                        let choice = choice.to_string();
                        if !values.contains(&choice) {
                            values.push(choice);
                        }
                    }
                }
            }
            values
        };

        values
    }

    fn visible_declared(&self) -> Vec<String> {
        let mut values: Vec<String> = self
            .choices
            .iter()
            .filter(|value| {
                !self
                    .details
                    .iter()
                    .any(|detail| detail.value == (*value).as_str() && detail.hide)
            })
            .cloned()
            .collect();
        for alias in self
            .details
            .iter()
            .flat_map(|detail| &detail.aliases)
            .filter(|alias| !alias.hide)
        {
            if !values.contains(&alias.value) {
                values.push(alias.value.clone());
            }
        }
        values
    }

    /// The choices as a help page lists them: visible values only, and no details.
    // Only `cli-help` renders help, and without it this is dead code under `-D warnings`.
    #[cfg(feature = "cli-help")]
    pub(crate) fn for_help(&self) -> Self {
        let mut choices = self.clone();
        choices.choices = self.visible_declared();
        choices.details.clear();
        choices
    }

    /// A help page's choices with `run=`'s output folded into the list, for a program that is
    /// running. A command that fails leaves `run` in place, so the page says where the values
    /// come from rather than listing none of them.
    #[cfg(feature = "cli-help")]
    pub(crate) fn resolve_for_help(&mut self, env: Option<&HashMap<String, String>>) {
        if let Ok(found) = self.values_from_run(env) {
            for value in found {
                if !self.choices.contains(&value) {
                    self.choices.push(value);
                }
            }
            self.run = None;
        }
    }
}

/// What each `run=` printed, or why it failed, keyed by the command.
type RunOutputs = HashMap<String, Result<Vec<String>, String>>;

thread_local! {
    /// The outputs for the parse or help page in progress. `None` outside one.
    static RUN_OUTPUTS: RefCell<Option<RunOutputs>> = const { RefCell::new(None) };
    /// Set while a caller that must not start programs is parsing; see [`NoRunScope`].
    static RUNS_DISABLED: Cell<bool> = const { Cell::new(false) };
}

/// Keeps `run=` commands from running until it is dropped.
///
/// For a caller that inspects a command line rather than acting on it, such as
/// `usage explain`, which promises never to start another program: a spec file that
/// arrived with a bug report should not run its commands on the machine of whoever reads
/// the report. While it is held, a value the declared choices do not accept is let through
/// unchecked rather than checked against a command, and help describes the command instead
/// of listing its output.
pub(crate) struct NoRunScope {
    was: bool,
}

impl NoRunScope {
    pub(crate) fn enter() -> Self {
        Self {
            was: RUNS_DISABLED.with(|disabled| disabled.replace(true)),
        }
    }
}

impl Drop for NoRunScope {
    fn drop(&mut self) {
        RUNS_DISABLED.with(|disabled| disabled.set(self.was));
    }
}

/// Whether a [`NoRunScope`] is held, so `run=` commands must not start.
pub(crate) fn runs_disabled() -> bool {
    RUNS_DISABLED.with(Cell::get)
}

/// Remembers what `run=` commands printed until it is dropped, so a parse checking five
/// values against one command runs it once. A scope opened inside another shares it.
///
/// Scoped rather than kept on the spec: a long-lived process that parses the same spec
/// twice should see the command's answer as of each parse, not as of the first.
pub(crate) struct RunScope {
    owner: bool,
}

impl RunScope {
    pub(crate) fn enter() -> Self {
        let owner = RUN_OUTPUTS.with(|outputs| {
            let mut outputs = outputs.borrow_mut();
            let owner = outputs.is_none();
            if owner {
                *outputs = Some(HashMap::new());
            }
            owner
        });
        Self { owner }
    }
}

impl Drop for RunScope {
    fn drop(&mut self) {
        if self.owner {
            RUN_OUTPUTS.with(|outputs| outputs.borrow_mut().take());
        }
    }
}

fn run_choices(run: &str, env: Option<&HashMap<String, String>>) -> Result<Vec<String>, UsageErr> {
    if runs_disabled() {
        return Err(UsageErr::ShellError(format!(
            "`{run}` was not run: commands are disabled for this parse"
        )));
    }
    let cached = RUN_OUTPUTS.with(|outputs| {
        outputs
            .borrow()
            .as_ref()
            .and_then(|outputs| outputs.get(run).cloned())
    });
    let result = match cached {
        Some(result) => result,
        None => {
            let result = crate::sh::sh_with_env(run, env)
                .map(|stdout| {
                    let mut values: Vec<String> = Vec::new();
                    for line in stdout.lines().map(str::trim).filter(|l| !l.is_empty()) {
                        if !values.iter().any(|v| v == line) {
                            values.push(line.to_string());
                        }
                    }
                    values
                })
                .map_err(|err| err.to_string());
            RUN_OUTPUTS.with(|outputs| {
                if let Some(outputs) = outputs.borrow_mut().as_mut() {
                    outputs.insert(run.to_string(), result.clone());
                }
            });
            result
        }
    };
    result.map_err(UsageErr::ShellError)
}

impl From<&SpecChoices> for KdlNode {
    fn from(arg: &SpecChoices) -> Self {
        let mut node = KdlNode::new("choices");
        if arg.details.is_empty() {
            for choice in &arg.choices {
                node.push(choice.to_string());
            }
        } else {
            let mut children = KdlDocument::new();
            for value in &arg.choices {
                let detail = arg.details.iter().find(|detail| detail.value == *value);
                let mut choice = KdlNode::new("choice");
                choice.push(crate::spec::helpers::string_entry(None, value));
                if let Some(detail) = detail {
                    if let Some(help) = &detail.help {
                        choice.push(crate::spec::helpers::string_entry(Some("help"), help));
                    }
                    if detail.hide {
                        choice.push(KdlEntry::new_prop("hide", true));
                    }
                    if !detail.aliases.is_empty() {
                        let mut aliases = KdlDocument::new();
                        for item in &detail.aliases {
                            let mut alias = KdlNode::new("alias");
                            alias.push(crate::spec::helpers::string_entry(None, &item.value));
                            if item.hide {
                                alias.push(KdlEntry::new_prop("hide", true));
                            }
                            aliases.nodes_mut().push(alias);
                        }
                        choice.set_children(aliases);
                    }
                }
                children.nodes_mut().push(choice);
            }
            node.set_children(children);
        }
        if arg.ignore_case {
            node.push(KdlEntry::new_prop("ignore_case", true));
        }
        if !arg.strict {
            node.push(KdlEntry::new_prop("strict", false));
        }
        #[cfg(feature = "unstable_choices_env")]
        if let Some(env) = arg.env() {
            node.push(KdlEntry::new_prop("env", env.to_string()));
        }
        if let Some(run) = arg.run() {
            node.push(crate::spec::helpers::string_entry(Some("run"), run));
        }
        node
    }
}

#[cfg(test)]
mod tests {
    #[cfg(feature = "unstable_choices_env")]
    use super::SpecChoices;
    #[cfg(feature = "unstable_choices_env")]
    use std::collections::HashMap;

    #[test]
    fn rich_choices_round_trip_and_match_their_aliases() {
        let source = r#"
name "ex"
bin "ex"
arg "<color>" {
  choices ignore_case=#true {
    choice "always" help="Always use color" {
      alias "yes"
      alias "on" hide=#true
    }
    choice "never" hide=#true
  }
}
"#;
        let spec: crate::Spec = source.parse().unwrap();
        let choices = spec.cmd.args[0].choices.as_ref().unwrap();
        assert!(choices.matches("ALWAYS"));
        assert!(choices.matches("YES"));
        assert!(choices.matches("ON"));
        assert_eq!(choices.values(), vec!["always", "yes"]);
        crate::parse(&spec, &["ex".into(), "YES".into()]).unwrap();
        crate::parse(&spec, &["ex".into(), "NEVER".into()]).unwrap();
        assert!(crate::parse(&spec, &["ex".into(), "sometimes".into()]).is_err());

        let rendered = spec.to_string();
        let reparsed: crate::Spec = rendered.parse().unwrap();
        let choices = reparsed.cmd.args[0].choices.as_ref().unwrap();
        assert_eq!(
            choices.details,
            spec.cmd.args[0].choices.as_ref().unwrap().details
        );
        assert!(choices.ignore_case);
        #[cfg(feature = "markdown")]
        assert_eq!(choices.for_help().choices, vec!["always", "yes"]);
    }

    #[test]
    fn duplicate_structured_choices_are_rejected() {
        let source = r#"
name "ex"
arg "<color>" {
  choices {
    choice "always"
    choice "always" help="duplicate"
  }
}
"#;
        let err = format!("{:?}", source.parse::<crate::Spec>().unwrap_err());
        assert!(err.contains("declared more than once"), "{err}");
    }

    #[cfg(feature = "unstable_choices_env")]
    #[test]
    fn values_with_env_splits_on_commas_and_whitespace() {
        let mut choices = SpecChoices {
            choices: vec!["local".into()],
            ..Default::default()
        };
        choices.set_env(Some("DEPLOY_ENVS".into()));

        let env = HashMap::from([("DEPLOY_ENVS".to_string(), "foo,bar baz\nqux".to_string())]);

        assert_eq!(
            choices.values_with_env(Some(&env)),
            vec!["local", "foo", "bar", "baz", "qux"]
        );
    }

    #[cfg(feature = "unstable_choices_env")]
    #[test]
    fn values_with_env_deduplicates_existing_choices() {
        let mut choices = SpecChoices {
            choices: vec!["foo".into()],
            ..Default::default()
        };
        choices.set_env(Some("DEPLOY_ENVS".into()));

        let env = HashMap::from([("DEPLOY_ENVS".to_string(), "foo,bar foo".to_string())]);

        assert_eq!(choices.values_with_env(Some(&env)), vec!["foo", "bar"]);
    }

    #[cfg(feature = "unstable_choices_env")]
    #[test]
    fn values_with_env_does_not_fallback_when_custom_env_is_present() {
        let mut choices = SpecChoices {
            choices: vec!["local".into()],
            ..Default::default()
        };
        choices.set_env(Some(
            "USAGE_TEST_CHOICES_ENV_DOES_NOT_EXIST_A5E0F4D1".into(),
        ));

        assert_eq!(
            choices.values_with_env(Some(&HashMap::new())),
            vec!["local"]
        );
    }

    #[cfg(feature = "unstable_choices_env")]
    #[test]
    fn matches_resolves_env_backed_choices() {
        const KEY: &str = "USAGE_TEST_MATCHES_CHOICES_ENV_9C47C3C5";
        let mut choices = SpecChoices {
            ignore_case: true,
            ..Default::default()
        };
        choices.set_env(Some(KEY.into()));
        std::env::set_var(KEY, "staging");

        assert!(choices.matches("STAGING"));

        std::env::remove_var(KEY);
    }
}
