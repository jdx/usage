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

/// What a KDL value could not be read as.
pub(crate) enum ValueError {
    /// An integer KDL accepts (it parses `i128`) that does not fit the spec's `i64`.
    IntegerOutOfRange,
    /// `#inf`, `#-inf` or `#nan`, which KDL accepts and nothing downstream can carry.
    NotFinite,
    /// A string default that the declared type cannot read — `data_type="integer"` beside
    /// `default="lots"`. Carries the type it should have been.
    DoesNotFitType(SpecDataTypes),
}

impl ValueError {
    /// What to tell whoever wrote the spec.
    pub(crate) fn describe(&self) -> String {
        match self {
            Self::IntegerOutOfRange => "config default does not fit in a 64-bit integer".into(),
            Self::NotFinite => {
                "config default must be a finite number: `#inf` and `#nan` cannot be written \
                 back out, rendered, or carried in JSON"
                    .into()
            }
            Self::DoesNotFitType(ty) => {
                format!("config default cannot be read as the declared type `{ty}`")
            }
        }
    }
}

impl SpecConfigValue {
    /// `None` only for an explicit `#null`.
    ///
    /// An out-of-range integer is an error rather than a `None`: returning "absent" for a
    /// number somebody wrote loses their default silently, and every consumer downstream —
    /// the writer, the SDKs — then reports the property as having none.
    pub(crate) fn from_kdl(value: &kdl::KdlValue) -> Result<Option<Self>, ValueError> {
        Ok(match value {
            kdl::KdlValue::Bool(b) => Some(Self::Bool(*b)),
            kdl::KdlValue::Integer(i) => Some(Self::Int(
                i64::try_from(*i).map_err(|_| ValueError::IntegerOutOfRange)?,
            )),
            // Not merely unusual: `serde_json` writes a non-finite float as `null`, so
            // `usage g json` silently reported the property as having no default at all,
            // and the Python generator emitted a bare `inf` — which is a `NameError`, not a
            // number. There is nowhere for this value to go, so it is refused where it is
            // written rather than lost three consumers later.
            kdl::KdlValue::Float(f) if !f.is_finite() => return Err(ValueError::NotFinite),
            kdl::KdlValue::Float(f) => Some(Self::Float(*f)),
            kdl::KdlValue::String(s) => Some(Self::String(s.clone())),
            kdl::KdlValue::Null => None,
        })
    }

    /// A KDL entry for this value.
    ///
    /// No special handling for a whole float: `KdlValue::Float(1.0)` is written `1.0` and
    /// read back as a float. (Reviewed as a risk — that a `1.0` would render as `1` and
    /// reparse as an integer — and measured not to happen, including `1e3` normalizing to
    /// `1000.0`. `a_whole_float_stays_a_float` pins it.)
    fn to_kdl_entry(&self, key: &str) -> KdlEntry {
        match self {
            // Through `string_entry`, like every other string this crate writes: the kdl
            // crate renders a control character literally and the result cannot be read
            // back. Building the entry by hand here meant a default containing one — help
            // text with a colour escape in it, say — wrote a spec that failed to reparse,
            // which is the exact failure this change exists to fix.
            Self::String(s) => string_entry(Some(key), s),
            Self::Bool(b) => KdlEntry::new_prop(key, kdl::KdlValue::Bool(*b)),
            Self::Int(i) => KdlEntry::new_prop(key, kdl::KdlValue::Integer(*i as i128)),
            Self::Float(f) => KdlEntry::new_prop(key, kdl::KdlValue::Float(*f)),
        }
    }

    /// The same value read as the type the prop declares.
    ///
    /// A spec may write the value as a string and the type as a number —
    /// `data_type="float" default="1.5"` — and reading it as declared means every consumer
    /// downstream sees a number.
    ///
    /// A string the declared type *cannot* read is an error. It used to stay a string, which
    /// kept it away from anything that would treat its text as a number but left the spec
    /// saying two contradictory things: the Python generator then emitted an `int` field
    /// whose default is `"lots"`, and a 20-digit number written in quotes bypassed the
    /// range check that the same number unquoted would have hit. Refusing it is both safe
    /// and honest, and across mise's 280 settings — the largest registry in the fleet —
    /// there is not one string default that fails to read as its declared type.
    fn coerced_to(self, data_type: SpecDataTypes) -> Result<Self, ValueError> {
        let Self::String(text) = &self else {
            // The other direction, which only matters for a declared `string`: an unquoted
            // `default=4` on a string-typed prop left the value a number, so the generated
            // Python field was typed `str` and defaulted to `4`. Every other declared type is
            // a number or a boolean, and one of *those* written as a bare value is already the
            // right shape.
            return Ok(match data_type {
                SpecDataTypes::String => Self::String(self.display()),
                _ => self,
            });
        };
        let mismatch = || ValueError::DoesNotFitType(data_type);
        match data_type {
            SpecDataTypes::Integer => text.parse().map(Self::Int).map_err(|_| mismatch()),
            SpecDataTypes::Float => match text.parse::<f64>() {
                // The same reason as above, by the other road: `default="inf"` for a float.
                Ok(f) if !f.is_finite() => Err(ValueError::NotFinite),
                Ok(f) => Ok(Self::Float(f)),
                Err(_) => Err(mismatch()),
            },
            SpecDataTypes::Boolean => text.parse().map(Self::Bool).map_err(|_| mismatch()),
            _ => Ok(self),
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
                            "default" => {
                                prop.default = match SpecConfigValue::from_kdl(v.value) {
                                    Ok(value) => value,
                                    Err(err) => {
                                        bail_parse!(ctx, v.entry.span(), "{}", err.describe())
                                    }
                                }
                            }
                            "default_note" => prop.default_note = Some(v.ensure_string()?),
                            "data_type" => prop.data_type = v.ensure_string()?.parse()?,
                            "env" => prop.env = v.ensure_string()?.to_string().into(),
                            "help" => prop.help = v.ensure_string()?.to_string().into(),
                            "long_help" => prop.long_help = v.ensure_string()?.to_string().into(),
                            k => bail_parse!(ctx, node.span(), "unsupported config prop key {k}"),
                        }
                    }
                    // After the loop, not inside it: `data_type` may be written after
                    // `default` on the same node, and the declared type is what decides how
                    // the value is read.
                    prop.default = match prop.default.map(|v| v.coerced_to(prop.data_type)) {
                        None => None,
                        Some(Ok(value)) => Some(value),
                        Some(Err(err)) => {
                            bail_parse!(ctx, node.span(), "{}", err.describe())
                        }
                    };
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
        // The key too: a dotted path is unlikely to hold anything exotic, but "unlikely"
        // is not the standard the rest of the writer holds itself to.
        node.push(string_entry(None, &key));
        if let Some(default) = &self.default {
            node.push(default.to_kdl_entry("default"));
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
    /// The reason inside a parse error.
    ///
    /// `UsageErr::InvalidInput` renders as "Invalid usage config" whatever went wrong — the
    /// specifics are the diagnostic's label. Asserting on that matters here: a test that only
    /// checks `is_err()` passes just as happily when the spec was refused for some unrelated
    /// reason, which is how a check gets credit for work it is not doing.
    fn detail_of(err: &crate::error::UsageErr) -> String {
        match err {
            crate::error::UsageErr::InvalidInput(detail, _, _) => detail.clone(),
            other => other.to_string(),
        }
    }

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
    fn a_default_the_declared_type_cannot_read_is_refused() {
        // It used to stay a string. That kept it away from anything treating its text as a
        // number — the point of the original fix — but left the spec asserting two
        // contradictory things, and the Python generator then wrote an `int` field defaulting
        // to `"__import__('os')"` in quotes. Refusing it is safe *and* honest.
        //
        // Not a theoretical strictness: across mise's 280 settings, the largest registry in
        // the fleet, every string default reads as its declared type.
        for src in [
            "prop \"nope\" data_type=\"integer\" default=\"__import__('os')\"",
            "prop \"nope\" data_type=\"boolean\" default=\"perhaps\"",
            // The quoted road to the range error the unquoted number already hit.
            "prop \"nope\" data_type=\"integer\" default=\"99999999999999999999\"",
        ] {
            let spec = format!("name \"ex\"\nbin \"ex\"\nconfig {{\n  {src}\n}}\n");
            let err = Spec::parse(&Default::default(), &spec)
                .expect_err(&format!("should not parse: {src}"));
            let detail = detail_of(&err);
            assert!(
                detail.contains("declared type") || detail.contains("64-bit integer"),
                "refused for the wrong reason: {detail}"
            );
        }
    }

    #[test]
    fn a_declared_string_holds_a_string_however_it_was_written() {
        // `type="string" default=4` left the value a number, so the generated Python field was
        // typed `str` and defaulted to `4` — the declared type and the emitted literal
        // disagreeing, which is the whole thing this pass is about.
        let spec = Spec::parse(
            &Default::default(),
            "name \"x\"\nbin \"x\"\nconfig {\n  prop \"a\" data_type=\"string\" default=4\n  prop \"b\" data_type=\"string\" default=#true\n}\n",
        )
        .expect("should parse");
        assert_eq!(
            spec.config.props["a"].default,
            Some(SpecConfigValue::String("4".into()))
        );
        assert_eq!(
            spec.config.props["b"].default,
            Some(SpecConfigValue::String("true".into()))
        );
    }

    #[test]
    fn a_default_that_is_not_a_finite_number_is_refused() {
        // KDL accepts `#inf` and `#nan`; nothing downstream can carry one. `serde_json`
        // writes a non-finite float as `null`, so `usage g json` reported the property as
        // having no default at all, and the Python generator emitted a bare `inf`, which is a
        // `NameError` rather than a number. Measured, not assumed: both were the behaviour
        // before this check.
        for value in ["#inf", "#-inf", "#nan", "\"inf\" data_type=\"float\""] {
            let spec =
                format!("name \"ex\"\nbin \"ex\"\nconfig {{\n  prop \"a\" default={value}\n}}\n");
            let err = Spec::parse(&Default::default(), &spec)
                .expect_err(&format!("should not parse: default={value}"));
            let detail = detail_of(&err);
            assert!(
                detail.contains("finite"),
                "refused for the wrong reason: {detail}"
            );
        }
        // A finite float is untouched.
        let spec = Spec::parse(
            &Default::default(),
            "name \"ex\"\nbin \"ex\"\nconfig {\n  prop \"a\" default=1.5\n}\n",
        )
        .expect("should parse");
        assert_eq!(
            spec.config.props["a"].default,
            Some(SpecConfigValue::Float(1.5))
        );
    }

    #[test]
    fn a_default_a_reader_cannot_render_is_still_written_readably() {
        // `string_entry` exists because the kdl crate writes some values in a form this
        // crate cannot read back: a control character goes out literally, and the result
        // fails to reparse. Help text really does contain them — a CLI that colours its
        // help has an escape character in the middle of it — and so, therefore, does a
        // default. The typed-default writer built its entry by hand and skipped that
        // protection, so the one hop this whole change is about broke again for exactly
        // one shape of value.
        let spec: Spec =
            "name \"ex\"\nbin \"ex\"\nconfig {\n  prop \"prompt\" default=\"a\\u{1b}[0mb\"\n}\n"
                .parse()
                .expect("should parse");
        assert_eq!(
            spec.config.props["prompt"].default,
            Some(SpecConfigValue::String("a\u{1b}[0mb".to_string()))
        );

        let written = spec.to_string();
        let reparsed: Spec = written
            .parse()
            .unwrap_or_else(|e| panic!("written spec does not parse: {e}\n{written}"));
        assert_eq!(
            reparsed.config.props["prompt"].default,
            spec.config.props["prompt"].default,
        );

        // The key travels the same road. Nothing sensible puts a control character in a
        // dotted path, but the writer's job is to write back what it was given whatever that
        // was, and "nothing sensible would" is not a guarantee about what a spec holds.
        let spec: Spec = "name \"ex\"\nbin \"ex\"\nconfig {\n  prop \"a\\u{1b}b\" default=1\n}\n"
            .parse()
            .expect("should parse");
        let written = spec.to_string();
        let reparsed: Spec = written
            .parse()
            .unwrap_or_else(|e| panic!("written spec does not parse: {e}\n{written}"));
        assert_eq!(
            reparsed.config.props.keys().collect::<Vec<_>>(),
            spec.config.props.keys().collect::<Vec<_>>(),
        );
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
    fn a_whole_float_stays_a_float() {
        // A pin rather than a fix: review raised that `Float(1.0)` might render as `1` and
        // come back an integer. It does not — kdl writes `1.0` — and this keeps it that way.
        let spec: Spec = "name \"ex\"\nbin \"ex\"\nconfig {\n  prop \"rate\" default=1.0\n}\n"
            .parse()
            .unwrap();
        assert_eq!(
            spec.config.props["rate"].default,
            Some(SpecConfigValue::Float(1.0))
        );

        let written = spec.to_string();
        let round_tripped: Spec = written.parse().unwrap();
        assert_eq!(
            round_tripped.config.props["rate"].default,
            Some(SpecConfigValue::Float(1.0)),
            "a whole float should not come back an integer: {written}"
        );
    }

    #[test]
    fn a_default_too_large_for_an_i64_is_an_error() {
        // KDL parses integers as `i128`. Reading one that does not fit as "no default" loses
        // a number somebody wrote, and every consumer downstream then reports the property
        // as having none.
        let err = Spec::parse(
            &Default::default(),
            "config {\n  prop \"big\" default=99999999999999999999\n}\n",
        )
        .expect_err("an out-of-range default should not be silently dropped");
        match err {
            crate::error::UsageErr::InvalidInput(msg, _, _) => {
                assert!(msg.contains("64-bit integer"), "unhelpful message: {msg}");
            }
            err => panic!("unexpected error: {err:?}"),
        }
    }

    #[test]
    fn a_declared_type_decides_how_a_default_is_read() {
        // A spec may write the value as a string and the type as a number. Reading it as
        // declared means consumers see a number — and, just as important, that a string
        // which is *not* a number stays a string rather than being handed to something that
        // will treat its text as one.
        let spec: Spec = r#"
name "ex"
bin "ex"
config {
    prop "rate" data_type="float" default="1.5"
    prop "jobs" data_type="integer" default="4"
    prop "shell" data_type="string" default="true"
}
"#
        .parse()
        .unwrap();
        assert_eq!(
            spec.config.props["rate"].default,
            Some(SpecConfigValue::Float(1.5))
        );
        assert_eq!(
            spec.config.props["jobs"].default,
            Some(SpecConfigValue::Int(4))
        );
        assert_eq!(
            spec.config.props["shell"].default,
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
