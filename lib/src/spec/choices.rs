#[cfg(feature = "unstable_choices_env")]
use kdl::KdlEntry;
use kdl::KdlNode;
use serde::{Deserialize, Serialize};
use std::collections::HashMap;

use crate::error::UsageErr;
use crate::spec::context::ParsingContext;
use crate::spec::helpers::NodeHelper;

#[derive(Debug, Default, Clone, Serialize, Deserialize)]
pub struct SpecChoices {
    pub choices: Vec<String>,
    #[cfg(feature = "unstable_choices_env")]
    #[serde(skip_serializing_if = "Option::is_none")]
    pub env: Option<String>,
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

    pub(crate) fn parse(ctx: &ParsingContext, node: &NodeHelper) -> Result<Self, UsageErr> {
        #[cfg(not(feature = "unstable_choices_env"))]
        node.ensure_arg_len(1..)?;

        #[cfg(feature = "unstable_choices_env")]
        let mut config = Self {
            choices: node
                .args()
                .map(|e| e.ensure_string())
                .collect::<Result<_, _>>()?,
            ..Default::default()
        };

        #[cfg(not(feature = "unstable_choices_env"))]
        let config = Self {
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
                k => bail_parse!(ctx, v.entry.span(), "unsupported choices key {k}"),
            }
        }

        if config.choices.is_empty() {
            #[cfg(feature = "unstable_choices_env")]
            if config.env().is_none() {
                bail_parse!(
                    ctx,
                    node.span(),
                    "choices must have at least 1 argument or env property"
                );
            }
            #[cfg(not(feature = "unstable_choices_env"))]
            bail_parse!(ctx, node.span(), "choices must have at least 1 argument");
        }

        Ok(config)
    }

    pub fn values(&self) -> Vec<String> {
        self.values_with_env(None)
    }

    pub(crate) fn values_with_env(&self, env: Option<&HashMap<String, String>>) -> Vec<String> {
        #[cfg(feature = "unstable_choices_env")]
        let mut values = self.choices.clone();

        #[cfg(not(feature = "unstable_choices_env"))]
        let values = self.choices.clone();

        #[cfg(not(feature = "unstable_choices_env"))]
        let _ = env;

        #[cfg(feature = "unstable_choices_env")]
        {
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
        }

        values
    }
}

impl From<&SpecChoices> for KdlNode {
    fn from(arg: &SpecChoices) -> Self {
        let mut node = KdlNode::new("choices");
        for choice in &arg.choices {
            node.push(choice.to_string());
        }
        #[cfg(feature = "unstable_choices_env")]
        if let Some(env) = arg.env() {
            node.push(KdlEntry::new_prop("env", env.to_string()));
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
}
