use std::collections::BTreeMap;

use crate::kdl::{self, KdlDocument, KdlEntry, KdlNode};
use serde::Serialize;

use crate::error::UsageErr;
use crate::spec::config_type::SpecConfigType;
use crate::spec::context::ParsingContext;
use crate::spec::data_types::SpecDataTypes;
use crate::spec::helpers::{string_entry, NodeHelper, ParseEntry};

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

    /// This value as a bare node argument.
    fn to_kdl_arg(&self) -> KdlEntry {
        match self {
            Self::Bool(b) => KdlEntry::new(*b),
            Self::Int(i) => KdlEntry::new(kdl::KdlValue::Integer(*i as i128)),
            Self::Float(f) => KdlEntry::new(*f),
            Self::String(s) => string_entry(None, s),
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
            // With its point, because `1` is how an *integer* is written and this is not one. This
            // is also what `usage-config` writes a float as, and the two have to agree: a spec's
            // `default=1.0` on a string-typed prop is coerced *here* and its `choice 1.0` is read
            // *there*, so a difference of one character refused a default and a choice that were
            // written identically. `usage-conformance` has a test that holds the two together,
            // because it is the one crate that can see both.
            Self::Float(f) => {
                let text = f.to_string();
                match f.is_finite() && !text.contains(['.', 'e', 'E']) {
                    true => format!("{text}.0"),
                    false => text,
                }
            }
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

#[derive(Debug, Default, Clone, PartialEq, Serialize)]
#[non_exhaustive]
pub struct SpecConfig {
    pub props: BTreeMap<String, SpecConfigProp>,
    /// Source kinds this CLI reads that usage knows nothing about — a git config, a pkl
    /// file, an `.npmrc`. Declared so docs can say where a setting comes from without
    /// usage having to understand the source itself.
    pub sources: BTreeMap<String, SpecConfigSource>,
    /// Config file locations, in ascending precedence: the last one named wins. This is
    /// the rc-style chain that docs have to describe and a resolver has to walk.
    pub files: Vec<SpecConfigFile>,
}

/// A source kind's display metadata.
///
/// usage never reads a git config or an `.npmrc`; it renders what the spec says about
/// them. `{key}` and `{value}` in a hint are substituted with the setting's key in that
/// source and the value being set.
#[derive(Debug, Default, Clone, PartialEq, Serialize)]
#[non_exhaustive]
pub struct SpecConfigSource {
    /// What to call it in prose: "git config", "hk.pkl".
    pub name: Option<String>,
    /// How to describe reading a setting from it: "git config `{key}`".
    pub doc_hint: Option<String>,
    /// How to describe writing one: "git config {key} {value}".
    pub set_hint: Option<String>,
}

/// Where a config file lives, and how it is found.
#[derive(Debug, Default, Clone, PartialEq, Serialize)]
#[non_exhaustive]
pub struct SpecConfigFile {
    pub path: String,
    /// Whether to look for this name in the current directory and every parent.
    pub findup: bool,
    /// Which class of file this is. `Project` files are the ones a repository can carry,
    /// and so the ones a `scope="global"` setting refuses to be read from.
    pub scope: SpecConfigFileScope,
    /// The format, when the extension does not say: "toml", "json", "yaml".
    pub format: Option<String>,
}

/// Which class of file a location belongs to.
#[derive(
    Debug,
    Default,
    Copy,
    Clone,
    PartialEq,
    Eq,
    strum::Display,
    strum::EnumString,
    strum::VariantNames,
    Serialize,
)]
#[strum(serialize_all = "snake_case")]
#[serde(rename_all = "snake_case")]
pub enum SpecConfigFileScope {
    /// Somewhere a repository can carry — the least trusted.
    #[default]
    Project,
    /// The user's own configuration.
    Global,
    /// Installed by whoever administers the machine.
    System,
}

/// How values for one property combine across sources.
#[derive(
    Debug,
    Default,
    Copy,
    Clone,
    PartialEq,
    Eq,
    strum::Display,
    strum::EnumString,
    strum::VariantNames,
    Serialize,
)]
#[strum(serialize_all = "snake_case")]
#[serde(rename_all = "snake_case")]
pub enum SpecConfigMerge {
    /// The highest-precedence source wins outright.
    #[default]
    Replace,
    /// Collections from every source are concatenated, keeping first position.
    Union,
    /// Maps are merged key by key.
    Deep,
}

/// Which sources a property may be read from.
#[derive(
    Debug,
    Default,
    Copy,
    Clone,
    PartialEq,
    Eq,
    strum::Display,
    strum::EnumString,
    strum::VariantNames,
    Serialize,
)]
#[strum(serialize_all = "snake_case")]
#[serde(rename_all = "snake_case")]
pub enum SpecConfigScope {
    /// Any declared source.
    #[default]
    Any,
    /// Only files a repository cannot supply, and the environment or command line.
    ///
    /// mise treats this as a security property — a checked-in file must not be able to
    /// change it — which is why it is declared here rather than left to each tool.
    Global,
    /// Never from a file: the environment or the command line only.
    Env,
}

/// One of a property's allowed values.
#[derive(Debug, Clone, PartialEq, Serialize)]
#[non_exhaustive]
pub struct SpecConfigChoice {
    pub value: SpecConfigValue,
    pub help: Option<String>,
}

impl SpecConfig {
    /// Config properties keyed by their dotted path.
    pub fn new(props: impl IntoIterator<Item = (String, SpecConfigProp)>) -> Self {
        Self {
            props: props.into_iter().collect(),
            ..Default::default()
        }
    }
}

impl SpecConfig {
    pub(crate) fn parse(ctx: &ParsingContext, node: &NodeHelper) -> Result<Self, UsageErr> {
        let mut config = Self::default();
        for node in node.children() {
            match node.name() {
                "prop" => {
                    node.ensure_arg_len(1..=1)?;
                    let key = node.arg(0)?.ensure_string()?.to_string();
                    let prop = SpecConfigProp::parse(ctx, &node)?;
                    config.props.insert(key, prop);
                }
                "source" => {
                    node.ensure_arg_len(1..=1)?;
                    let kind = node.arg(0)?.ensure_string()?.to_string();
                    let mut source = SpecConfigSource::default();
                    for (k, v) in node.props() {
                        match k {
                            "name" => source.name = Some(v.ensure_string()?),
                            "doc_hint" => source.doc_hint = Some(v.ensure_string()?),
                            "set_hint" => source.set_hint = Some(v.ensure_string()?),
                            k => {
                                bail_parse!(ctx, node.span(), "unsupported config source key {k}")
                            }
                        }
                    }
                    refuse_children(ctx, &node, "source")?;
                    config.sources.insert(kind, source);
                }
                "file" => {
                    node.ensure_arg_len(1..=1)?;
                    let mut file = SpecConfigFile {
                        path: node.arg(0)?.ensure_string()?.to_string(),
                        ..Default::default()
                    };
                    for (k, v) in node.props() {
                        match k {
                            "findup" => file.findup = v.ensure_bool()?,
                            "scope" => file.scope = parse_enum(ctx, &node, "scope", &v)?,
                            "format" => file.format = Some(v.ensure_string()?),
                            k => bail_parse!(ctx, node.span(), "unsupported config file key {k}"),
                        }
                    }
                    refuse_children(ctx, &node, "file")?;
                    config.files.push(file);
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
        // Source kinds are a set, so they merge per kind like props do.
        for (kind, source) in &other.sources {
            self.sources.insert(kind.to_string(), source.clone());
        }
        // Files are not a set but an ordered precedence chain, and there is no meaningful
        // way to interleave two of them — so a spec that declares any replaces the chain
        // whole rather than appending to one it never saw. In the case `include` exists for,
        // only one of the two declares files at all.
        if !other.files.is_empty() {
            self.files = other.files.clone();
        }
    }
}

impl SpecConfig {
    /// Whether there is nothing to write out.
    ///
    /// All three, not just props: a `config` block that declares only where files live is a
    /// perfectly good one, and reporting it empty made the writer drop it.
    pub fn is_empty(&self) -> bool {
        self.props.is_empty() && self.sources.is_empty() && self.files.is_empty()
    }
}

#[derive(Debug, Clone, PartialEq, Serialize)]
#[non_exhaustive]
pub struct SpecConfigProp {
    /// Whether absence is a legitimate resolved value.
    ///
    /// `None` keeps the inferred rule: an `option<T>` or a property without a default is
    /// optional. An explicit value lets a registry state the contract instead of relying on
    /// that inference.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub optional: Option<bool>,
    /// Equivalent config keys accepted without a deprecation warning.
    #[serde(skip_serializing_if = "Vec::is_empty")]
    pub aliases: Vec<String>,
    pub default: Option<SpecConfigValue>,
    pub default_note: Option<String>,
    /// The old five-value type, kept so a spec written against it still means what it said.
    ///
    /// `type` is the one to write now; this is set from it where the two overlap, so a
    /// consumer reading either sees the same thing.
    pub data_type: SpecDataTypes,
    /// The type, in the expression grammar: `list<string>`, `option<path>`, `bool|string`.
    pub value_type: Option<SpecConfigType>,
    /// The first environment variable, kept for the specs that wrote `env=`.
    pub env: Option<String>,
    /// Every environment variable that sets this, highest precedence first.
    pub envs: Vec<String>,
    /// Environment aliases still read after current names, with a deprecation warning.
    #[serde(skip_serializing_if = "Vec::is_empty")]
    pub deprecated_envs: Vec<String>,
    /// Flags that set this, as declared elsewhere in the spec.
    pub cli: Vec<String>,
    /// Keys this setting has in a custom source kind, by kind name.
    pub bindings: BTreeMap<String, Vec<String>>,
    pub help: Option<String>,
    pub long_help: Option<String>,
    /// The section to list this under in generated docs.
    pub help_heading: Option<String>,
    pub choices: Vec<SpecConfigChoice>,
    pub merge: SpecConfigMerge,
    pub scope: SpecConfigScope,
    pub deprecated: Option<String>,
    pub deprecated_warn_at: Option<String>,
    pub deprecated_remove_at: Option<String>,
    /// The property that replaces this one, so a value arriving under the old name can be
    /// folded into the new one rather than ignored.
    pub renamed_to: Option<String>,
    /// Keep it out of docs and completions.
    pub hide: bool,
    /// The version that introduced it.
    pub since: Option<String>,
    /// A named parser for turning one string into this type — `list_by_comma` and friends.
    /// Vocabulary rather than code, so any implementation can honor it.
    pub parse: Option<String>,
    /// Where `config set` should write this, when it is not the usual file.
    pub writes_to: Option<String>,
    pub examples: Vec<String>,
    /// A list-valued default, which cannot be written as a single property.
    ///
    /// Typed like the scalar `default`, so `default 1 2 3` for a `list<int>` stays three
    /// numbers all the way to the JSON schema instead of becoming three strings.
    pub default_list: Vec<SpecConfigValue>,
    /// Anything a tool needs to carry that usage does not interpret.
    ///
    /// Preserved in order and written back out, so a registry with tool-private metadata
    /// (`mise.rust_type`, `aube.npm_shared`) round-trips through the spec untouched.
    pub extensions: Vec<(String, SpecConfigValue)>,
}

impl SpecConfigProp {
    /// A config property. Every field is optional; set what applies.
    pub fn new() -> Self {
        Self::default()
    }

    /// An environment variable that sets this property.
    ///
    /// Call it more than once for aliases, highest precedence first, the way
    /// `env "HK_JOBS" "HK_JOB"` reads in a spec. Both `env` and `envs` are maintained, so a
    /// consumer can read either — the parser holds the same invariant, and a builder that
    /// left `envs` empty meant a programmatically built spec serialized without the variable
    /// every reader of `envs` looks for.
    pub fn env(mut self, env: impl Into<String>) -> Self {
        let env = env.into();
        if self.env.is_none() {
            self.env = Some(env.clone());
        }
        self.envs.push(env);
        self
    }

    /// A deprecated environment alias, read after every current name.
    pub fn deprecated_env(mut self, env: impl Into<String>) -> Self {
        self.deprecated_envs.push(env.into());
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
        if let Some(optional) = self.optional {
            node.push(KdlEntry::new_prop("optional", optional));
        }
        // The grammar spelling when there is one, the old five-value name otherwise: a
        // spec that said `data_type` keeps saying it, and one that says `type` keeps the
        // richer type it declared. Either way it is written, unlike before — a type parsed
        // and not serialized survives exactly one hop.
        match &self.value_type {
            Some(ty) => node.push(string_entry(Some("type"), &ty.to_string())),
            None if self.data_type != SpecDataTypes::Null => {
                node.push(string_entry(Some("data_type"), &self.data_type.to_string()));
            }
            None => {}
        }
        if let Some(default_note) = &self.default_note {
            node.push(string_entry(Some("default_note"), default_note));
        }
        // Only when it is the whole story: several go in a child node, and writing both
        // would say it twice.
        if self.envs.len() <= 1 {
            if let Some(env) = &self.env {
                node.push(string_entry(Some("env"), env));
            }
        }
        if let Some(help) = &self.help {
            node.push(string_entry(Some("help"), help));
        }
        if let Some(long_help) = &self.long_help {
            node.push(string_entry(Some("long_help"), long_help));
        }
        if let Some(heading) = &self.help_heading {
            node.push(string_entry(Some("help_heading"), heading));
        }
        if self.merge != SpecConfigMerge::default() {
            node.push(string_entry(Some("merge"), &self.merge.to_string()));
        }
        if self.scope != SpecConfigScope::default() {
            node.push(string_entry(Some("scope"), &self.scope.to_string()));
        }
        if let Some(deprecated) = &self.deprecated {
            node.push(string_entry(Some("deprecated"), deprecated));
        }
        if let Some(at) = &self.deprecated_warn_at {
            node.push(string_entry(Some("deprecated_warn_at"), at));
        }
        if let Some(at) = &self.deprecated_remove_at {
            node.push(string_entry(Some("deprecated_remove_at"), at));
        }
        if let Some(renamed) = &self.renamed_to {
            node.push(string_entry(Some("renamed_to"), renamed));
        }
        if self.hide {
            node.push(KdlEntry::new_prop("hide", true));
        }
        if let Some(since) = &self.since {
            node.push(string_entry(Some("since"), since));
        }
        if let Some(parse) = &self.parse {
            node.push(string_entry(Some("parse"), parse));
        }
        if let Some(writes_to) = &self.writes_to {
            node.push(string_entry(Some("writes_to"), writes_to));
        }

        let mut children = KdlDocument::new();
        if self.envs.len() > 1 {
            children.nodes_mut().push(string_list("env", &self.envs));
        }
        if !self.deprecated_envs.is_empty() {
            children
                .nodes_mut()
                .push(string_list("deprecated_env", &self.deprecated_envs));
        }
        if !self.aliases.is_empty() {
            children
                .nodes_mut()
                .push(string_list("alias", &self.aliases));
        }
        if !self.cli.is_empty() {
            children.nodes_mut().push(string_list("cli", &self.cli));
        }
        if !self.default_list.is_empty() {
            let mut node = KdlNode::new("default");
            for value in &self.default_list {
                node.push(value.to_kdl_arg());
            }
            children.nodes_mut().push(node);
        }
        for (kind, keys) in &self.bindings {
            let mut node = KdlNode::new("source");
            node.push(string_entry(None, kind));
            for key in keys {
                node.push(string_entry(None, key));
            }
            children.nodes_mut().push(node);
        }
        if !self.choices.is_empty() {
            let mut block = KdlNode::new("choices");
            let mut inner = KdlDocument::new();
            for choice in &self.choices {
                let mut node = KdlNode::new("choice");
                node.push(choice.value.to_kdl_arg());
                if let Some(help) = &choice.help {
                    node.push(string_entry(Some("help"), help));
                }
                inner.nodes_mut().push(node);
            }
            block.set_children(inner);
            children.nodes_mut().push(block);
        }
        for example in &self.examples {
            children
                .nodes_mut()
                .push(string_list("example", std::slice::from_ref(example)));
        }
        for (key, value) in &self.extensions {
            let mut node = KdlNode::new("x");
            node.push(string_entry(None, key));
            node.push(value.to_kdl_arg());
            children.nodes_mut().push(node);
        }
        if !children.nodes().is_empty() {
            node.set_children(children);
        }
        node
    }
}

/// The old five-value type a grammar type corresponds to, where one does.
///
/// A composite — `list<string>`, `map<…>` — has no counterpart, so it reads as `Null`:
/// truthful about what the old vocabulary could say.
fn data_type_of(ty: &SpecConfigType) -> SpecDataTypes {
    use crate::spec::config_type::Base;
    // A union has no counterpart among the old five values, so it gets `Null` like every
    // other composite. Running it through `simplified()` made `bool|string` claim to be
    // `Boolean`, and that legacy field drives how a default is read — so a `bool|string`
    // whose default was the string `"true"` came back as a boolean, contradicting the very
    // `value_type` that produced it.
    if matches!(ty, SpecConfigType::Union(_)) {
        return SpecDataTypes::Null;
    }
    match ty.simplified() {
        SpecConfigType::Base(Base::Bool) => SpecDataTypes::Boolean,
        SpecConfigType::Base(Base::String) => SpecDataTypes::String,
        SpecConfigType::Base(Base::Int | Base::Uint) => SpecDataTypes::Integer,
        SpecConfigType::Base(Base::Float) => SpecDataTypes::Float,
        _ => SpecDataTypes::Null,
    }
}

/// Refuse a child block on a node that has no children in its vocabulary.
///
/// `source` and `file` are properties only. They checked their properties and never looked at
/// their children, so a nested block was dropped in silence — which contradicts the rule the
/// rest of this block follows, and `prop` already enforces: vocabulary this version does not
/// know is refused rather than half-read, because half-read is how a spec means one thing here
/// and another somewhere else.
fn refuse_children(
    ctx: &ParsingContext,
    node: &NodeHelper,
    name: &'static str,
) -> Result<(), UsageErr> {
    if let Some(child) = node.children().into_iter().next() {
        bail_parse!(
            ctx,
            child.node.name().span(),
            "a config {name} takes properties, not a block"
        );
    }
    Ok(())
}

/// A node whose arguments are strings: `cli "--jobs" "-j"`.
fn string_list(name: &str, values: &[String]) -> KdlNode {
    let mut node = KdlNode::new(name);
    for value in values {
        node.push(string_entry(None, value));
    }
    node
}

impl SpecConfigProp {
    fn parse(ctx: &ParsingContext, node: &NodeHelper) -> Result<Self, UsageErr> {
        let mut prop = Self::default();
        for (k, v) in node.props() {
            match k {
                "default" => {
                    prop.default = match SpecConfigValue::from_kdl(v.value) {
                        Ok(value) => value,
                        Err(err) => bail_parse!(ctx, v.entry.span(), "{}", err.describe()),
                    }
                }
                "default_note" => prop.default_note = Some(v.ensure_string()?),
                "optional" => prop.optional = Some(v.ensure_bool()?),
                // `data_type` was the old spelling and stays readable; `type` is the
                // grammar, and setting either fills the other in where they overlap.
                "data_type" | "type" => {
                    let ty: SpecConfigType = v.ensure_string()?.parse()?;
                    // From the parsed type rather than its text: the grammar spells a
                    // boolean `bool` and the old enum spells it `boolean`, so reading the
                    // text twice loses it on the way back out.
                    prop.data_type = data_type_of(&ty);
                    prop.value_type = Some(ty);
                }
                "env" => prop.env = Some(v.ensure_string()?),
                "help" => prop.help = Some(v.ensure_string()?),
                "long_help" => prop.long_help = Some(v.ensure_string()?),
                "help_heading" => prop.help_heading = Some(v.ensure_string()?),
                "merge" => prop.merge = parse_enum(ctx, node, "merge", &v)?,
                "scope" => prop.scope = parse_enum(ctx, node, "scope", &v)?,
                "deprecated" => prop.deprecated = Some(v.ensure_string()?),
                "deprecated_warn_at" => prop.deprecated_warn_at = Some(v.ensure_string()?),
                "deprecated_remove_at" => prop.deprecated_remove_at = Some(v.ensure_string()?),
                "renamed_to" => prop.renamed_to = Some(v.ensure_string()?),
                "hide" => prop.hide = v.ensure_bool()?,
                "since" => prop.since = Some(v.ensure_string()?),
                "parse" => prop.parse = Some(v.ensure_string()?),
                "writes_to" => prop.writes_to = Some(v.ensure_string()?),
                k => bail_parse!(ctx, node.span(), "unsupported config prop key {k}"),
            }
        }

        for child in node.children() {
            match child.name() {
                // The mistake worth naming: it used to parse and lose the inner prop.
                "prop" => bail_parse!(
                    ctx,
                    child.node.name().span(),
                    "config props cannot nest; write the key as \"a.b\""
                ),
                // Where several values, or prose, would not fit on the node.
                // Extended, not assigned: `example` and `source` already accumulate, and a
                // second `env` line is the natural parallel — assigning silently dropped the
                // aliases on the first one.
                "env" => prop.envs.extend(string_args(&child)?),
                "deprecated_env" => prop.deprecated_envs.extend(string_args(&child)?),
                "alias" => prop.aliases.extend(string_args(&child)?),
                "cli" => prop.cli.extend(string_args(&child)?),
                "example" => prop.examples.extend(string_args(&child)?),
                "long_help" => {
                    child.ensure_arg_len(1..=1)?;
                    prop.long_help = Some(child.arg(0)?.ensure_string()?.to_string());
                }
                "default" => {
                    // A list default, which cannot be written as one property. Accumulated
                    // across nodes like `env`, `cli` and `example`, rather than assigned —
                    // clearing the list first meant a second `default` line silently dropped
                    // everything the first one declared.
                    for arg in child.args() {
                        match SpecConfigValue::from_kdl(arg.value) {
                            Ok(Some(value)) => prop.default_list.push(value),
                            // `#null` in a list of defaults says nothing at all; a list
                            // whose default is "one of these is absent" has no meaning.
                            Ok(None) => bail_parse!(
                                ctx,
                                arg.entry.span(),
                                "a default list holds values, not #null"
                            ),
                            Err(err) => {
                                bail_parse!(ctx, arg.entry.span(), "{}", err.describe())
                            }
                        }
                    }
                }
                "source" => {
                    // `source "git" "hk.jobs" "hk.check"` — this setting's keys in a kind
                    // declared at the top of the block.
                    child.ensure_arg_len(1..)?;
                    let mut args = string_args(&child)?;
                    let kind = args.remove(0);
                    prop.bindings.entry(kind).or_default().extend(args);
                }
                "choices" => {
                    for choice in child.children() {
                        if choice.name() != "choice" {
                            bail_parse!(
                                ctx,
                                choice.node.name().span(),
                                "a choices block holds `choice` nodes"
                            );
                        }
                        choice.ensure_arg_len(1..=1)?;
                        let value = match SpecConfigValue::from_kdl(choice.arg(0)?.value) {
                            Ok(Some(value)) => value,
                            Ok(None) => bail_parse!(ctx, choice.span(), "a choice needs a value"),
                            // Same reasons, said about a choice rather than a default: a
                            // number too large to carry, or one nothing can render.
                            Err(err) => {
                                bail_parse!(ctx, choice.span(), "choice: {}", err.describe())
                            }
                        };
                        let mut help = None;
                        for (k, v) in choice.props() {
                            match k {
                                "help" => help = Some(v.ensure_string()?),
                                k => bail_parse!(ctx, choice.span(), "unsupported choice key {k}"),
                            }
                        }
                        refuse_children(ctx, &choice, "choice")?;
                        prop.choices.push(SpecConfigChoice { value, help });
                    }
                }
                "x" => {
                    // The escape hatch: kept in order, written back out, interpreted by
                    // nobody here.
                    child.ensure_arg_len(2..=2)?;
                    let key = child.arg(0)?.ensure_string()?.to_string();
                    let value = match SpecConfigValue::from_kdl(child.arg(1)?.value) {
                        Ok(Some(value)) => value,
                        // An extension promises to come back out exactly as it went in, and
                        // there is no `#null` to come back to — it used to be stored as `""`
                        // and written as `""`, which is tool-private metadata altered on save.
                        Ok(None) => bail_parse!(
                            ctx,
                            child.span(),
                            "an extension value cannot be #null; it would not round-trip"
                        ),
                        Err(err) => {
                            bail_parse!(ctx, child.span(), "extension value: {}", err.describe())
                        }
                    };
                    prop.extensions.push((key, value));
                }
                k => bail_parse!(
                    ctx,
                    child.node.name().span(),
                    "unsupported config prop node {k}"
                ),
            }
        }

        // After both loops: `type` may be written after `default`, and the declared type is
        // what decides how the value is read.
        let declared = prop.data_type;
        prop.default = match prop.default.map(|v| v.coerced_to(declared)) {
            None => None,
            Some(Ok(value)) => Some(value),
            Some(Err(err)) => bail_parse!(ctx, node.span(), "{}", err.describe()),
        };
        // One env spelling, two ways to write it, and they must never disagree. `env=` is
        // shorthand for a one-element list, so both forms feed `envs` in the order they were
        // written and `env` is always its first entry.
        //
        // Syncing only when one side was empty left a prop that wrote *both* with two fields
        // saying different things — `usage g json` exposing the pair, and a writer that picks
        // between them by list length, so one of the values disappeared on a round trip.
        if let Some(env) = prop.env.take() {
            if !prop.envs.contains(&env) {
                prop.envs.insert(0, env);
            }
        }
        prop.env = prop.envs.first().cloned();
        Ok(prop)
    }
}

/// Every argument of a node, as strings.
/// The string arguments of a node, refusing anything that is not one.
///
/// An environment variable, a flag and a source key are names, so a non-string is a mistake
/// rather than something to render. Converting instead — which this used to do — turned
/// `env #true` into a variable named `#true` and wrote it back out quoted, looking for all
/// the world like somebody had meant it.
fn string_args(node: &NodeHelper) -> Result<Vec<String>, UsageErr> {
    node.args().map(|arg| arg.ensure_string()).collect()
}

/// An enum-valued property, with the accepted spellings in the error.
fn parse_enum<T>(
    ctx: &ParsingContext,
    node: &NodeHelper,
    key: &str,
    value: &ParseEntry<'_>,
) -> Result<T, UsageErr>
where
    T: std::str::FromStr + strum::VariantNames,
{
    let text = value.ensure_string()?;
    text.parse().map_err(|_| {
        ctx.build_err(
            format!(
                "`{text}` is not a {key}; the choices are {}",
                T::VARIANTS.join(", ")
            ),
            (node.span().offset(), node.span().len()).into(),
        )
    })
}

impl Default for SpecConfigProp {
    fn default() -> Self {
        Self {
            optional: None,
            aliases: Vec::new(),
            default: None,
            default_note: None,
            data_type: SpecDataTypes::Null,
            value_type: None,
            env: None,
            envs: Vec::new(),
            deprecated_envs: Vec::new(),
            cli: Vec::new(),
            bindings: BTreeMap::new(),
            help: None,
            long_help: None,
            help_heading: None,
            choices: Vec::new(),
            merge: SpecConfigMerge::default(),
            scope: SpecConfigScope::default(),
            deprecated: None,
            deprecated_warn_at: None,
            deprecated_remove_at: None,
            renamed_to: None,
            hide: false,
            since: None,
            parse: None,
            writes_to: None,
            examples: Vec::new(),
            default_list: Vec::new(),
            extensions: Vec::new(),
        }
    }
}

impl From<&SpecConfig> for KdlNode {
    fn from(config: &SpecConfig) -> Self {
        let mut node = KdlNode::new("config");
        let doc = node.children_mut().get_or_insert_with(KdlDocument::new);
        // Declarations first, then locations, then the settings — the order the reference
        // page describes them in, so a written file reads like the documentation.
        for (kind, source) in &config.sources {
            let mut node = KdlNode::new("source");
            node.push(string_entry(None, kind));
            if let Some(name) = &source.name {
                node.push(string_entry(Some("name"), name));
            }
            if let Some(hint) = &source.doc_hint {
                node.push(string_entry(Some("doc_hint"), hint));
            }
            if let Some(hint) = &source.set_hint {
                node.push(string_entry(Some("set_hint"), hint));
            }
            doc.nodes_mut().push(node);
        }
        // In order: a file list is a precedence chain, so the order is the meaning.
        for file in &config.files {
            let mut node = KdlNode::new("file");
            node.push(string_entry(None, &file.path));
            if file.findup {
                node.push(KdlEntry::new_prop("findup", true));
            }
            if file.scope != SpecConfigFileScope::default() {
                node.push(string_entry(Some("scope"), &file.scope.to_string()));
            }
            if let Some(format) = &file.format {
                node.push(string_entry(Some("format"), format));
            }
            doc.nodes_mut().push(node);
        }
        for (key, prop) in &config.props {
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

    use super::{SpecConfigMerge, SpecConfigScope, SpecConfigValue};
    use crate::Spec;
    use insta::assert_snapshot;

    #[test]
    fn optionality_and_key_aliases_round_trip() {
        let spec: Spec = r#"
name "ex"
bin "ex"
config {
    prop "jobs" type="uint" optional=#false {
        alias "parallelism" "threads"
    }
}
"#
        .parse()
        .unwrap();
        let jobs = &spec.config.props["jobs"];
        assert_eq!(jobs.optional, Some(false));
        assert_eq!(jobs.aliases, ["parallelism", "threads"]);

        let written = spec.to_string();
        let reparsed: Spec = written.parse().unwrap();
        assert_eq!(reparsed.config.props["jobs"], *jobs, "{written}");
    }

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

    /// Every node kind the vocabulary has, written and read back.
    ///
    /// The spec is the interchange between authoring and everything that consumes it, so
    /// anything it cannot write out is something a tool would lose by saving its own file.
    #[test]
    fn the_whole_vocabulary_survives_a_round_trip() {
        let spec: Spec = r##"
name "hk"
bin "hk"
config {
    source "git" name="git config" doc_hint="git config `{key}`" set_hint="git config {key} {value}"
    source "pkl" name="hk.pkl"
    file "/etc/hk/config.pkl" scope="system"
    file "~/.config/hk/config.pkl" scope="global"
    file "hk.pkl" findup=#true
    file ".hkrc" format="ini"
    prop "jobs" type="uint" default=0 default_note="0 = auto-detect" \
        help="Number of parallel jobs" since="1.0.0" help_heading="Performance" {
        cli "--jobs" "-j"
        env "HK_JOBS" "HK_JOB"
        deprecated_env "HK_JOBS_OLD"
        source "git" "hk.jobs"
        source "pkl" "jobs" "defaults.jobs"
        example "hk check --jobs 4"
    }
    prop "exclude" type="list<string>" merge="union" {
        default "target" "node_modules"
        env "HK_EXCLUDE"
    }
    prop "stash" type="string" {
        choices {
            choice "git" help="Use `git stash`"
            choice "none" help="No stashing"
        }
    }
    prop "trusted" type="bool" scope="global"
    prop "ci" type="bool" hide=#true scope="env" {
        env "CI"
        x "mise.rust_type" "BoolOrString"
        x "mise.rc" #true
    }
    prop "old.key" deprecated="Use new.key" renamed_to="new.key" \
        deprecated_warn_at="2026.12.0" deprecated_remove_at="2027.12.0"
    prop "urls" type="map<string, url>" parse="list_by_comma" writes_to="npmrc"
}
"##
        .parse()
        .unwrap();

        let written = spec.to_string();
        let back: Spec = written
            .parse()
            .unwrap_or_else(|e| panic!("re-reading what we wrote: {e}\n{written}"));

        assert_eq!(back.config.sources, spec.config.sources, "{written}");
        assert_eq!(back.config.files, spec.config.files, "{written}");
        assert_eq!(
            back.config.props.keys().collect::<Vec<_>>(),
            spec.config.props.keys().collect::<Vec<_>>(),
            "{written}"
        );
        for (key, before) in &spec.config.props {
            let after = &back.config.props[key];
            assert_eq!(after, before, "{key} changed on the way out:\n{written}");
        }

        // And the pieces that are easy to write and forget to read.
        let jobs = &spec.config.props["jobs"];
        assert_eq!(jobs.cli, ["--jobs", "-j"]);
        assert_eq!(jobs.envs, ["HK_JOBS", "HK_JOB"]);
        assert_eq!(jobs.deprecated_envs, ["HK_JOBS_OLD"]);
        assert_eq!(
            jobs.env.as_deref(),
            Some("HK_JOBS"),
            "the first of the list"
        );
        assert_eq!(jobs.bindings["pkl"], ["jobs", "defaults.jobs"]);
        assert_eq!(jobs.examples, ["hk check --jobs 4"]);
        assert_eq!(jobs.help_heading.as_deref(), Some("Performance"));
        assert_eq!(spec.config.props["exclude"].merge, SpecConfigMerge::Union);
        assert_eq!(
            spec.config.props["exclude"].default_list,
            [
                SpecConfigValue::String("target".into()),
                SpecConfigValue::String("node_modules".into()),
            ]
        );
        assert_eq!(spec.config.props["stash"].choices.len(), 2);
        assert_eq!(
            spec.config.props["stash"].choices[0].help.as_deref(),
            Some("Use `git stash`")
        );
        assert_eq!(spec.config.props["trusted"].scope, SpecConfigScope::Global);
        assert_eq!(spec.config.props["ci"].scope, SpecConfigScope::Env);
        assert!(spec.config.props["ci"].hide);
        assert_eq!(
            spec.config.props["ci"].extensions,
            [
                (
                    "mise.rust_type".to_string(),
                    SpecConfigValue::String("BoolOrString".into())
                ),
                ("mise.rc".to_string(), SpecConfigValue::Bool(true)),
            ]
        );
        assert_eq!(
            spec.config.props["old.key"].renamed_to.as_deref(),
            Some("new.key")
        );
        assert_eq!(
            spec.config.props["urls"]
                .value_type
                .as_ref()
                .map(|t| t.to_string()),
            Some("map<string, url>".to_string())
        );
        assert_eq!(
            spec.config.props["urls"].parse.as_deref(),
            Some("list_by_comma")
        );
        assert_eq!(
            spec.config.props["urls"].writes_to.as_deref(),
            Some("npmrc")
        );

        // The serialized model, committed: this is what `usage g json` hands to a docs
        // pipeline, a schema generator, or an implementation in another language, and it is
        // the artifact a port can diff against rather than reading this file.
        assert_snapshot!(serde_json::to_string_pretty(&spec.config).unwrap());
    }

    #[test]
    fn an_unknown_word_in_the_config_block_is_refused() {
        // Strict, deliberately: a spec using vocabulary this version does not have should
        // say so rather than half-load. That is what `min_usage_version` is for.
        for src in [
            "config {\n  prop \"a\" nonsense=1\n}\n",
            "config {\n  nonsense \"a\"\n}\n",
            "config {\n  prop \"a\" {\n    nonsense \"b\"\n  }\n}\n",
            "config {\n  prop \"a\" merge=\"sideways\"\n}\n",
            "config {\n  file \"x\" scope=\"elsewhere\"\n}\n",
        ] {
            assert!(
                Spec::parse(&Default::default(), src).is_err(),
                "should be refused: {src}"
            );
        }
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

    #[test]
    fn an_included_file_can_declare_sources_and_files() {
        // The case `include` exists for: a spec with many settings keeps them in their own
        // file, and that file is where the whole block lives — the source kinds and the file
        // chain included. Merging only props dropped both, so a settings file could describe
        // where values come from and have it silently discarded.
        let mut spec = Spec::parse(&Default::default(), "name \"hk\"\nbin \"hk\"\n").unwrap();
        let included = Spec::parse(
            &Default::default(),
            r#"
config {
    source "git" name="git config"
    file "/etc/hk/config.pkl" scope="system"
    file "hk.pkl" findup=#true
    prop "jobs" type="uint"
}
"#,
        )
        .unwrap();

        spec.merge(included);
        assert_eq!(
            spec.config.sources["git"].name.as_deref(),
            Some("git config")
        );
        assert_eq!(spec.config.files.len(), 2);
        assert_eq!(spec.config.files[1].path, "hk.pkl");
        assert!(spec.config.files[1].findup);
    }

    #[test]
    fn a_block_of_only_files_is_not_empty() {
        // `is_empty` decides whether the writer emits the block at all, so counting only
        // props meant a config block that declared just where files live vanished on a round
        // trip — the same class of loss as the defaults this stack started with.
        let spec = Spec::parse(
            &Default::default(),
            "name \"x\"\nbin \"x\"\nconfig {\n  file \"x.toml\" findup=#true\n}\n",
        )
        .unwrap();
        assert!(!spec.config.is_empty());
        // Unquoted: the writer only quotes what KDL requires it to.
        assert!(spec.to_string().contains("file x.toml"), "{spec}");
    }

    #[test]
    fn a_name_that_is_not_a_string_is_refused() {
        // These are all names — an environment variable, a flag, a key in another source.
        // Rendering a non-string instead of refusing it produced a variable called `#true`
        // and wrote it back out quoted, as though somebody had meant it.
        for body in [
            "prop \"a\" {\n  env #true\n}",
            "prop \"a\" {\n  cli 42\n}",
            "prop \"a\" {\n  source \"git\" 1\n}",
            "prop \"a\" {\n  example #false\n}",
        ] {
            let src = format!("name \"x\"\nbin \"x\"\nconfig {{\n{body}\n}}\n");
            assert!(
                Spec::parse(&Default::default(), &src).is_err(),
                "should not parse:\n{src}"
            );
        }
    }

    #[test]
    fn a_union_has_no_legacy_type_and_does_not_claim_one() {
        // `data_type` is the old five-value field and a union is not one of the five. Mapping
        // it through `simplified()` made `bool|string` say `Boolean` — and that field decides
        // how a default is read, so the string default came back as a boolean, contradicting
        // the `value_type` it was derived from.
        let spec = Spec::parse(
            &Default::default(),
            "name \"x\"\nbin \"x\"\nconfig {\n  prop \"a\" type=\"bool|string\" default=\"true\"\n}\n",
        )
        .expect("should parse");
        let prop = &spec.config.props["a"];
        assert_eq!(prop.data_type, crate::spec::data_types::SpecDataTypes::Null);
        assert_eq!(
            prop.default,
            Some(SpecConfigValue::String("true".into())),
            "a union's default is left as written"
        );
        // The plain boolean it is not the same as still reads as one.
        let spec = Spec::parse(
            &Default::default(),
            "name \"x\"\nbin \"x\"\nconfig {\n  prop \"a\" type=\"bool\" default=\"true\"\n}\n",
        )
        .expect("should parse");
        assert_eq!(
            spec.config.props["a"].default,
            Some(SpecConfigValue::Bool(true))
        );
    }

    #[test]
    fn an_extension_that_could_not_round_trip_is_refused() {
        // `x` nodes promise to come back out exactly as they went in. `#null` had nowhere to
        // come back to: it was stored as an empty string and written as `""`, which is
        // tool-private metadata quietly altered by saving the file.
        let err = Spec::parse(
            &Default::default(),
            "config {\n  prop \"a\" {\n    x \"mise.thing\" #null\n  }\n}\n",
        )
        .expect_err("should not parse");
        assert!(
            detail_of(&err).contains("round-trip"),
            "refused for the wrong reason: {}",
            detail_of(&err)
        );
    }

    #[test]
    fn a_second_default_node_adds_to_the_first() {
        // The same reason `env` and `cli` accumulate: clearing the list first meant a prop
        // that wrote its default over two lines kept only the second.
        let spec = Spec::parse(
            &Default::default(),
            "name \"x\"\nbin \"x\"\nconfig {\n  prop \"a\" type=\"list<string>\" {\n    default \"one\"\n    default \"two\"\n  }\n}\n",
        )
        .expect("should parse");
        assert_eq!(
            spec.config.props["a"].default_list,
            [
                SpecConfigValue::String("one".into()),
                SpecConfigValue::String("two".into()),
            ]
        );
    }

    #[test]
    fn a_second_env_or_cli_node_adds_to_the_first() {
        // `example` and `source` already accumulate across nodes; `env` and `cli` assigned, so
        // a spec that wrote them on two lines lost the first line's values without a word.
        let spec = Spec::parse(
            &Default::default(),
            "name \"x\"\nbin \"x\"\nconfig {\n  prop \"a\" {\n    env \"FIRST\"\n    env \"SECOND\"\n    cli \"--one\"\n    cli \"--two\"\n  }\n}\n",
        )
        .expect("should parse");
        let prop = &spec.config.props["a"];
        assert_eq!(prop.envs, ["FIRST", "SECOND"]);
        assert_eq!(prop.cli, ["--one", "--two"]);
    }

    #[test]
    fn a_block_on_a_node_that_takes_none_is_refused() {
        // `source` and `file` are properties only, and checked only their properties — so a
        // nested block was dropped in silence, which is the one thing this vocabulary is
        // strict about not doing.
        for src in [
            "config {\n  source \"git\" {\n    name \"git config\"\n  }\n}\n",
            "config {\n  file \"x.toml\" {\n    scope \"global\"\n  }\n}\n",
            // A `choice` is properties-only too, and read its own the same way.
            "config {\n  prop \"a\" {\n    choices {\n      choice \"x\" {\n        help \"why\"\n      }\n    }\n  }\n}\n",
        ] {
            let err = Spec::parse(&Default::default(), src).expect_err(src);
            assert!(
                detail_of(&err).contains("not a block"),
                "refused for the wrong reason: {}",
                detail_of(&err)
            );
        }
    }

    #[test]
    fn both_env_spellings_leave_the_same_prop_however_it_was_built() {
        // Two ways to write one thing — `env="X"` and `env "A" "B"` — so `env` and `envs`
        // have to agree whichever way a prop arrived. They did after parsing and did not
        // after building, so a spec assembled in Rust serialized with an empty `envs` and
        // every consumer that reads that field saw a setting no variable could set.
        let built = super::SpecConfigProp::new().env("HK_JOBS").env("HK_JOB");
        assert_eq!(built.env.as_deref(), Some("HK_JOBS"));
        assert_eq!(built.envs, ["HK_JOBS", "HK_JOB"]);

        // Both spellings at once. The sync only ran when one side was empty, so this left
        // `env` and `envs` asserting different things — `usage g json` exposing the pair, and
        // a writer that chooses between them by list length, so a round trip dropped a value.
        let both = Spec::parse(
            &Default::default(),
            "name \"x\"\nbin \"x\"\nconfig {\n  prop \"a\" env=\"FIRST\" {\n    env \"SECOND\"\n  }\n}\n",
        )
        .unwrap();
        assert_eq!(both.config.props["a"].envs, ["FIRST", "SECOND"]);
        assert_eq!(both.config.props["a"].env.as_deref(), Some("FIRST"));
        // And both values survive being written out and read back.
        let round_tripped: Spec = both.to_string().parse().expect("should reparse");
        assert_eq!(round_tripped.config.props["a"].envs, ["FIRST", "SECOND"]);

        // The property spelling, parsed.
        let one = Spec::parse(
            &Default::default(),
            "name \"x\"\nbin \"x\"\nconfig {\n  prop \"a\" env=\"A\"\n}\n",
        )
        .unwrap();
        let one = &one.config.props["a"];
        assert_eq!(one.env.as_deref(), Some("A"));
        assert_eq!(one.envs, ["A"]);

        // The list spelling, parsed.
        let many = Spec::parse(
            &Default::default(),
            "name \"x\"\nbin \"x\"\nconfig {\n  prop \"a\" {\n    env \"A\" \"B\"\n  }\n}\n",
        )
        .unwrap();
        let many = &many.config.props["a"];
        assert_eq!(many.env.as_deref(), Some("A"));
        assert_eq!(many.envs, ["A", "B"]);
    }

    #[test]
    fn a_list_default_keeps_the_type_it_was_written_as() {
        // `default 1 2 3` for a `list<int>` is three numbers, and a schema or an SDK that
        // received three strings would describe the setting wrongly.
        let spec = Spec::parse(
            &Default::default(),
            "name \"x\"\nbin \"x\"\nconfig {\n  prop \"ports\" type=\"list<int>\" {\n    default 80 443\n  }\n}\n",
        )
        .unwrap();
        assert_eq!(
            spec.config.props["ports"].default_list,
            [SpecConfigValue::Int(80), SpecConfigValue::Int(443)]
        );
        // And it says so again when written back out, rather than gaining quotes.
        assert!(spec.to_string().contains("default 80 443"), "{spec}");
    }
}
