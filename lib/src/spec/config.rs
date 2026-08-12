use std::collections::BTreeMap;

use kdl::{KdlDocument, KdlEntry, KdlNode};
use serde::Serialize;

use crate::error::UsageErr;
use crate::spec::context::ParsingContext;
use crate::spec::data_types::SpecDataTypes;
use crate::spec::helpers::{string_entry, NodeHelper};

/// A config property's value, as declared.
///
/// Typed rather than a `String` because the previous version stored the KDL *source* form
/// — `KdlValue::to_string()` — so a `default="4"` was read back as the four characters
/// `"4"` with its quotes, and writing it out again added another pair. Each round trip
/// added a layer. Keeping the value means the writer can render it once, correctly.
#[derive(Debug, Clone, PartialEq, Serialize)]
#[serde(untagged)]
pub enum SpecConfigValue {
    Bool(bool),
    Int(i64),
    Float(f64),
    String(String),
}

impl SpecConfigValue {
    fn from_kdl(value: &kdl::KdlValue) -> Option<Self> {
        match value {
            kdl::KdlValue::Bool(b) => Some(Self::Bool(*b)),
            kdl::KdlValue::Integer(i) => i64::try_from(*i).ok().map(Self::Int),
            kdl::KdlValue::Float(f) => Some(Self::Float(*f)),
            kdl::KdlValue::String(s) => Some(Self::String(s.clone())),
            kdl::KdlValue::Null => None,
        }
    }

    fn to_kdl(&self) -> kdl::KdlValue {
        match self {
            Self::Bool(b) => kdl::KdlValue::Bool(*b),
            Self::Int(i) => kdl::KdlValue::Integer(*i as i128),
            Self::Float(f) => kdl::KdlValue::Float(*f),
            Self::String(s) => kdl::KdlValue::String(s.clone()),
        }
    }

    /// The value as a human would read it, for docs and help output.
    pub fn display(&self) -> String {
        match self {
            Self::Bool(b) => b.to_string(),
            Self::Int(i) => i.to_string(),
            Self::Float(f) => f.to_string(),
            Self::String(s) => s.clone(),
        }
    }
}

impl From<bool> for SpecConfigValue {
    fn from(value: bool) -> Self {
        Self::Bool(value)
    }
}

impl From<i64> for SpecConfigValue {
    fn from(value: i64) -> Self {
        Self::Int(value)
    }
}

impl From<f64> for SpecConfigValue {
    fn from(value: f64) -> Self {
        Self::Float(value)
    }
}

impl From<&str> for SpecConfigValue {
    fn from(value: &str) -> Self {
        Self::String(value.to_string())
    }
}

impl From<String> for SpecConfigValue {
    fn from(value: String) -> Self {
        Self::String(value)
    }
}

#[derive(Debug, Default, Clone, Serialize)]
#[non_exhaustive]
pub struct SpecConfig {
    pub props: BTreeMap<String, SpecConfigProp>,
}

impl SpecConfig {
    /// Config properties keyed by their dotted path.
    pub fn new(props: impl IntoIterator<Item = (String, SpecConfigProp)>) -> Self {
        Self {
            props: props.into_iter().collect(),
        }
    }
}

impl SpecConfig {
    pub(crate) fn parse(ctx: &ParsingContext, node: &NodeHelper) -> Result<Self, UsageErr> {
        let mut config = Self::default();
        for node in node.children() {
            node.ensure_arg_len(1..=1)?;
            match node.name() {
                "prop" => {
                    let key = node.arg(0)?;
                    let key = key.ensure_string()?.to_string();
                    // A `prop` with children used to parse and lose them: the loop below
                    // reads properties only, so `prop "a" { prop "b" }` kept `a` and
                    // dropped `b` without a word. Nesting is spelled with a dotted key.
                    if let Some(children) = node.node.children() {
                        if let Some(child) = children.nodes().first() {
                            bail_parse!(
                                ctx,
                                child.name().span(),
                                "config props cannot nest; write the key as \"a.b\""
                            );
                        }
                    }
                    let mut prop = SpecConfigProp::default();
                    for (k, v) in node.props() {
                        match k {
                            "default" => prop.default = SpecConfigValue::from_kdl(v.value),
                            "default_note" => prop.default_note = Some(v.ensure_string()?),
                            "data_type" => prop.data_type = v.ensure_string()?.parse()?,
                            "env" => prop.env = v.ensure_string()?.to_string().into(),
                            "help" => prop.help = v.ensure_string()?.to_string().into(),
                            "long_help" => prop.long_help = v.ensure_string()?.to_string().into(),
                            k => bail_parse!(ctx, node.span(), "unsupported config prop key {k}"),
                        }
                    }
                    config.props.insert(key, prop);
                }
                k => bail_parse!(ctx, node.node.name().span(), "unsupported config key {k}"),
            }
        }
        Ok(config)
    }

    /// Later declarations win, whole prop at a time.
    ///
    /// `Spec::merge` is other-wins for everything else (`merge_opt!`), and config was the
    /// one place an included file could add a prop but never correct one. Field-wise
    /// refinement is deliberately not offered until something needs it.
    pub(crate) fn merge(&mut self, other: &Self) {
        for (key, prop) in &other.props {
            self.props.insert(key.to_string(), prop.clone());
        }
    }
}

impl SpecConfig {
    pub fn is_empty(&self) -> bool {
        self.props.is_empty()
    }
}

#[derive(Debug, Clone, Serialize)]
#[non_exhaustive]
pub struct SpecConfigProp {
    pub default: Option<SpecConfigValue>,
    pub default_note: Option<String>,
    pub data_type: SpecDataTypes,
    pub env: Option<String>,
    pub help: Option<String>,
    pub long_help: Option<String>,
}

impl SpecConfigProp {
    /// A config property. Every field is optional; set what applies.
    pub fn new() -> Self {
        Self::default()
    }

    /// Environment variable that sets this property.
    pub fn env(mut self, env: impl Into<String>) -> Self {
        self.env = Some(env.into());
        self
    }

    /// Short help text.
    pub fn help(mut self, help: impl Into<String>) -> Self {
        self.help = Some(help.into());
        self
    }

    /// Default value, rendered in docs.
    pub fn default_value(mut self, default: impl Into<SpecConfigValue>) -> Self {
        self.default = Some(default.into());
        self
    }
}

impl SpecConfigProp {
    fn to_kdl_node(&self, key: String) -> KdlNode {
        let mut node = KdlNode::new("prop");
        node.push(KdlEntry::new(key));
        if let Some(default) = &self.default {
            node.push(KdlEntry::new_prop("default", default.to_kdl()));
        }
        // Written, unlike before: a type that is parsed and not serialized is a type that
        // survives exactly one hop.
        if self.data_type != SpecDataTypes::Null {
            node.push(string_entry(Some("data_type"), &self.data_type.to_string()));
        }
        if let Some(default_note) = &self.default_note {
            node.push(string_entry(Some("default_note"), default_note));
        }
        if let Some(env) = &self.env {
            node.push(string_entry(Some("env"), env));
        }
        if let Some(help) = &self.help {
            node.push(string_entry(Some("help"), help));
        }
        if let Some(long_help) = &self.long_help {
            node.push(string_entry(Some("long_help"), long_help));
        }
        node
    }
}

impl Default for SpecConfigProp {
    fn default() -> Self {
        Self {
            default: None,
            default_note: None,
            data_type: SpecDataTypes::Null,
            env: None,
            help: None,
            long_help: None,
        }
    }
}

impl From<&SpecConfig> for KdlNode {
    fn from(config: &SpecConfig) -> Self {
        let mut node = KdlNode::new("config");
        for (key, prop) in &config.props {
            let doc = node.children_mut().get_or_insert_with(KdlDocument::new);
            doc.nodes_mut().push(prop.to_kdl_node(key.to_string()));
        }
        node
    }
}

#[cfg(test)]
mod tests {
    use super::SpecConfigValue;
    use crate::Spec;
    use insta::assert_snapshot;

    #[test]
    fn test_config_defaults() {
        let spec = Spec::parse(
            &Default::default(),
            r#"
config {
    prop "color" default=#true env="COLOR" help="Enable color output"
    prop "user" default="admin" env="USER" help="User to run as"
    prop "jobs" default=4 env="JOBS" help="Number of jobs to run"
    prop "timeout" default=1.5 env="TIMEOUT" help="Timeout in seconds" \
        long_help="Timeout in seconds, can be fractional"
}
        "#,
        )
        .unwrap();

        // The values, not their KDL source text: the old snapshot recorded
        // `default="#true"` and `default="4"`, which is what a stringly default looks
        // like once it has been through the writer.
        assert_snapshot!(spec, @r##"
        config {
            prop color default=#true env=COLOR help="Enable color output"
            prop jobs default=4 env=JOBS help="Number of jobs to run"
            prop timeout default=1.5 env=TIMEOUT help="Timeout in seconds" long_help="Timeout in seconds, can be fractional"
            prop user default=admin env=USER help="User to run as"
        }
        "##);
    }

    #[test]
    fn a_config_block_survives_being_written_out() {
        // Each of these was lost or corrupted by one hop through the writer.
        let spec: Spec = r#"
name "ex"
bin "ex"
config {
    prop "jobs" data_type="integer" default=4 env="EX_JOBS" help="How many"
    prop "color" data_type="boolean" default=#true
    prop "shell" data_type="string" default="true"
}
"#
        .parse()
        .unwrap();

        let written = spec.to_string();
        let round_tripped: Spec = written.parse().unwrap();
        for (key, before) in &spec.config.props {
            let after = round_tripped
                .config
                .props
                .get(key)
                .unwrap_or_else(|| panic!("{key} should survive"));
            assert_eq!(
                after.data_type, before.data_type,
                "{key}'s type should survive: {written}"
            );
            assert_eq!(
                after.default, before.default,
                "{key}'s default should survive unchanged: {written}"
            );
        }
        // `"true"` is a string whose text looks like a boolean — the case that shows the
        // difference between keeping a value and keeping its spelling.
        assert_eq!(
            round_tripped.config.props["shell"].default,
            Some(SpecConfigValue::String("true".into()))
        );
    }

    #[test]
    fn a_nested_prop_is_refused_rather_than_dropped() {
        let err = Spec::parse(
            &Default::default(),
            r#"
config {
    prop "status" {
        prop "missing_tools"
    }
}
"#,
        )
        .expect_err("nesting should not be silently accepted");
        // The message is in the diagnostic rather than the summary line, which is where
        // this crate keeps parse detail.
        match err {
            crate::error::UsageErr::InvalidInput(msg, _, _) => {
                assert!(msg.contains("cannot nest"), "unhelpful message: {msg}");
            }
            err => panic!("unexpected error: {err:?}"),
        }
    }

    #[test]
    fn a_later_declaration_of_a_prop_wins() {
        // What `include` needs: everything else in a spec is other-wins, and config was
        // the one place a later file could add a prop but never correct one.
        let mut spec = Spec::parse(
            &Default::default(),
            "config {\n  prop \"jobs\" default=1 help=\"first\"\n}\n",
        )
        .unwrap();
        let other = Spec::parse(
            &Default::default(),
            "config {\n  prop \"jobs\" default=8 help=\"second\"\n  prop \"color\"\n}\n",
        )
        .unwrap();

        spec.merge(other);
        assert_eq!(
            spec.config.props["jobs"].default,
            Some(SpecConfigValue::Int(8))
        );
        assert_eq!(spec.config.props["jobs"].help.as_deref(), Some("second"));
        assert!(spec.config.props.contains_key("color"));
    }
}
