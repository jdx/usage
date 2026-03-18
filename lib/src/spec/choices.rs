use kdl::{KdlEntry, KdlNode};
use serde::{Deserialize, Serialize};
use std::collections::HashMap;

use crate::error::UsageErr;
use crate::spec::context::ParsingContext;
use crate::spec::helpers::NodeHelper;

#[derive(Debug, Default, Clone, Serialize, Deserialize)]
pub struct SpecChoices {
    pub choices: Vec<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub env: Option<String>,
}

impl SpecChoices {
    pub(crate) fn parse(ctx: &ParsingContext, node: &NodeHelper) -> Result<Self, UsageErr> {
        let mut config = Self::default();
        config.choices = node
            .args()
            .map(|e| e.ensure_string())
            .collect::<Result<_, _>>()?;

        for (k, v) in node.props() {
            match k {
                "env" => config.env = Some(v.ensure_string()?),
                k => bail_parse!(ctx, v.entry.span(), "unsupported choices key {k}"),
            }
        }

        if config.choices.is_empty() && config.env.is_none() {
            bail_parse!(
                ctx,
                node.span(),
                "choices must have at least 1 argument or env property"
            );
        }

        Ok(config)
    }

    pub fn values(&self) -> Vec<String> {
        self.values_with_env(None)
    }

    pub(crate) fn values_with_env(&self, env: Option<&HashMap<String, String>>) -> Vec<String> {
        let mut values = self.choices.clone();

        if let Some(env_key) = &self.env {
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
    }
}

impl From<&SpecChoices> for KdlNode {
    fn from(arg: &SpecChoices) -> Self {
        let mut node = KdlNode::new("choices");
        for choice in &arg.choices {
            node.push(choice.to_string());
        }
        if let Some(env) = &arg.env {
            node.push(KdlEntry::new_prop("env", env.clone()));
        }
        node
    }
}

#[cfg(test)]
mod tests {
    use super::SpecChoices;
    use std::collections::HashMap;

    #[test]
    fn values_with_env_splits_on_commas_and_whitespace() {
        let choices = SpecChoices {
            choices: vec!["local".into()],
            env: Some("DEPLOY_ENVS".into()),
        };

        let env = HashMap::from([("DEPLOY_ENVS".to_string(), "foo,bar baz\nqux".to_string())]);

        assert_eq!(
            choices.values_with_env(Some(&env)),
            vec!["local", "foo", "bar", "baz", "qux"]
        );
    }

    #[test]
    fn values_with_env_deduplicates_existing_choices() {
        let choices = SpecChoices {
            choices: vec!["foo".into()],
            env: Some("DEPLOY_ENVS".into()),
        };

        let env = HashMap::from([("DEPLOY_ENVS".to_string(), "foo,bar foo".to_string())]);

        assert_eq!(choices.values_with_env(Some(&env)), vec!["foo", "bar"]);
    }

    #[test]
    fn values_with_env_does_not_fallback_when_custom_env_is_present() {
        let choices = SpecChoices {
            choices: vec!["local".into()],
            env: Some("USAGE_TEST_CHOICES_ENV_DOES_NOT_EXIST_A5E0F4D1".into()),
        };

        assert_eq!(
            choices.values_with_env(Some(&HashMap::new())),
            vec!["local"]
        );
    }
}
