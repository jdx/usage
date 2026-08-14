//! The config conformance corpus: its format, and the runner that answers it.
//!
//! The argv corpus pins which token becomes which flag. This one pins what a *resolution* is: given
//! a set of settings and some layers that supply values, which value wins, where it is recorded as
//! coming from, and what the merge has to say about the ones it refused.
//!
//! # Why a registry rather than a spec
//!
//! An argv vector carries a KDL spec, because parsing argv is a question about a spec. A resolution
//! is not: it is a question about a *registry* — keys, types, defaults, merge policies — which a
//! CLI's build step produces from the spec long before anything is resolved. So a vector describes
//! the registry directly in KDL, usage's canonical interchange format. How a spec becomes a
//! registry is `usage-config-build`'s question, and its own golden test answers it.
//!
//! Nothing here touches the filesystem, the process environment, or a subprocess. A file layer is a
//! description of what a file said, which is all the merge ever sees of one.

use std::collections::BTreeMap;
use std::path::Path;

use kdl::{KdlDocument, KdlNode, KdlValue};
use serde::{Deserialize, Serialize};
use usage_config::{
    resolve, Const, Layer, LayerCtx, LayerError, LayerOutput, Layers, Merge, Origin, PropMeta,
    Registry, Scope, SourceKind, Trust, Ty, Value, WarningKind,
};

/// One `corpus/config/*.kdl` file: a themed group of vectors.
#[derive(Debug, Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
pub struct VectorFile {
    /// Which part of resolution this file covers, e.g. `"precedence"`.
    pub section: String,
    /// What the group establishes, plus anything a reader needs in order to judge whether these
    /// expectations are the right ones.
    pub about: String,
    pub vectors: Vec<Vector>,
}

/// A single case: resolve `layers` against `settings` and you must get `expect`.
#[derive(Debug, Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
pub struct Vector {
    /// Stable identifier, unique across the corpus. Reports quote it, so renaming one breaks
    /// anybody tracking known failures.
    pub id: String,
    /// What this vector pins down, in one sentence.
    pub doc: String,
    /// The settings that exist, in the order a registry declares them.
    pub settings: Vec<Setting>,
    /// The layers, **highest precedence first** — the order `Layers::then` takes them, and the
    /// order a reader thinks in: the command line, then the environment, then files.
    #[serde(default)]
    pub layers: Vec<LayerSpec>,
    pub expect: Expect,
}

/// One setting, as a registry holds it.
#[derive(Debug, Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
pub struct Setting {
    pub key: String,
    /// The type, in the spec's own grammar: `uint`, `list<string>`, `map<string, string>`,
    /// `option<path>`. A name this corpus does not know is `any`, which is what a union is too.
    #[serde(default = "default_type")]
    pub r#type: String,
    /// The value when no layer supplies one.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub default: Option<serde_json::Value>,
    #[serde(default, skip_serializing_if = "is_default")]
    pub merge: MergePolicy,
    #[serde(default, skip_serializing_if = "is_default")]
    pub scope: ScopeRule,
    /// A named parser, for a layer that hands over one string where several values are meant.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub parse: Option<String>,
    /// The only values this setting accepts.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub choices: Vec<serde_json::Value>,
    /// The setting this one was replaced by.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub renamed_to: Option<String>,
    /// Why not to use this one any more.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub deprecated: Option<String>,
}

fn default_type() -> String {
    "string".to_string()
}

fn is_default<T: Default + PartialEq>(value: &T) -> bool {
    *value == T::default()
}

/// How values from several layers combine.
#[derive(Debug, Default, Clone, Copy, Deserialize, Serialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum MergePolicy {
    /// The highest-precedence layer wins outright.
    #[default]
    Replace,
    /// Collections from every layer are concatenated, keeping first position.
    Union,
    /// Tables are merged key by key.
    Deep,
}

/// Which places may set a setting.
#[derive(Debug, Default, Clone, Copy, Deserialize, Serialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum ScopeRule {
    #[default]
    Any,
    /// Not from anything a repository can carry.
    Global,
    /// Never from a file at all.
    Env,
}

/// One layer, described by what it supplies rather than by where it read it.
#[derive(Debug, Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
pub struct LayerSpec {
    /// The kind of place: `cli`, `env`, `file`, or a name only the tool knows, like `git`.
    pub source: String,
    /// What a report calls it: the variable's name, the file's path. Defaults to the source's own
    /// name, which is enough for a vector with one layer of a kind.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub id: Option<String>,
    /// How much this place is trusted, which is what the scope rules read. Defaults to what the
    /// kind implies: the command line and the environment are the user's own, and anything else is
    /// taken to be something a repository could carry until it says otherwise.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub trust: Option<TrustLevel>,
    /// Values as text, the way a layer that reads an environment or a `.ini` hands them over.
    #[serde(default, skip_serializing_if = "BTreeMap::is_empty")]
    pub values: BTreeMap<String, String>,
    /// Values with a shape of their own, the way a layer that reads TOML or JSON hands them over.
    #[serde(default, skip_serializing_if = "BTreeMap::is_empty")]
    pub shaped: BTreeMap<String, serde_json::Value>,
}

/// How far a place is trusted.
#[derive(Debug, Clone, Copy, Deserialize, Serialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum TrustLevel {
    /// Given by whoever ran the command.
    Invocation,
    /// Something the person who set the machine up put there.
    Operator,
    /// Something a checkout can carry, and therefore anybody who can open a pull request.
    Project,
}

/// What a resolution must produce.
#[derive(Debug, Default, Deserialize, Serialize, PartialEq, Eq)]
#[serde(deny_unknown_fields)]
pub struct Expect {
    /// The value of each setting. A setting left out is expected to have no value at all, so a
    /// vector says what it means rather than only what it is interested in.
    #[serde(default)]
    pub values: BTreeMap<String, serde_json::Value>,
    /// Which place each value is recorded as coming from, where the vector is about that. Omitted
    /// keys are not checked.
    #[serde(default, skip_serializing_if = "BTreeMap::is_empty")]
    pub origins: BTreeMap<String, String>,
    /// What the merge had to say, by kind and in order.
    ///
    /// Kinds, not messages: wording is a quality-of-implementation concern and is expected to
    /// differ between implementations, which is the same line the argv corpus holds for its error
    /// codes.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub warnings: Vec<Complaint>,
}

/// The classes of thing a resolution reports.
#[derive(Debug, Clone, Copy, Deserialize, Serialize, PartialEq, Eq)]
#[serde(rename_all = "kebab-case")]
pub enum Complaint {
    /// A key no setting declares.
    UnknownSetting,
    /// A value the declared type cannot read.
    WrongType,
    /// A value the declared choices do not allow.
    NotAllowed,
    /// A place that may not set this setting.
    OutOfScope,
    /// A setting whose spec says not to use it any more.
    Deprecated,
    /// A value read as the setting that replaced the name it was written under.
    Renamed,
    /// A value passed over because another name for the same setting won.
    NotRead,
}

/// Load every config corpus KDL file in a directory, sorted by file name.
pub fn load(dir: impl AsRef<Path>) -> Result<Vec<VectorFile>, String> {
    let dir = dir.as_ref();
    let mut paths: Vec<_> = std::fs::read_dir(dir)
        .map_err(|e| format!("reading {}: {e}", dir.display()))?
        .map(|entry| {
            entry
                .map(|entry| entry.path())
                .map_err(|e| format!("reading an entry of {}: {e}", dir.display()))
        })
        .collect::<Result<Vec<_>, _>>()?
        .into_iter()
        .filter(|path| path.extension().is_some_and(|extension| extension == "kdl"))
        .collect();
    paths.sort();

    paths
        .iter()
        .map(|path| {
            let text = std::fs::read_to_string(path)
                .map_err(|e| format!("reading {}: {e}", path.display()))?;
            parse_file(&text).map_err(|e| format!("parsing {}: {e}", path.display()))
        })
        .collect()
}

/// Parse one config corpus file from canonical KDL.
pub fn parse_file(text: &str) -> Result<VectorFile, String> {
    let document: KdlDocument = text.parse().map_err(|e: kdl::KdlError| e.to_string())?;
    let section = one_string(&document, "section")?;
    let about = one_string(&document, "about")?;
    let vectors = document
        .nodes()
        .iter()
        .filter(|node| node.name().value() == "vector")
        .map(vector_from)
        .collect::<Result<_, _>>()?;
    reject_nodes(&document, &["section", "about", "vector"])?;
    Ok(VectorFile {
        section,
        about,
        vectors,
    })
}

/// Parse one vector, useful to tests that exercise malformed values without making a corpus file.
pub fn parse_vector(text: &str) -> Result<Vector, String> {
    let document: KdlDocument = text.parse().map_err(|e: kdl::KdlError| e.to_string())?;
    let nodes: Vec<_> = document
        .nodes()
        .iter()
        .filter(|node| node.name().value() == "vector")
        .collect();
    if nodes.len() != 1 || document.nodes().len() != 1 {
        return Err("a vector document must contain exactly one `vector` node".to_string());
    }
    vector_from(nodes[0])
}

fn vector_from(node: &KdlNode) -> Result<Vector, String> {
    let id = string(node, 0)?;
    let doc = string(node, "doc")?;
    let children = children(node)?;
    let settings = children
        .nodes()
        .iter()
        .filter(|node| node.name().value() == "setting")
        .map(setting_from)
        .collect::<Result<_, _>>()?;
    let layers = children
        .nodes()
        .iter()
        .filter(|node| node.name().value() == "layer")
        .map(layer_from)
        .collect::<Result<_, _>>()?;
    let expects: Vec<_> = children
        .nodes()
        .iter()
        .filter(|node| node.name().value() == "expect")
        .collect();
    if expects.len() != 1 {
        return Err(format!(
            "vector `{id}` must contain exactly one `expect` node"
        ));
    }
    reject_nodes(children, &["setting", "layer", "expect"])?;
    Ok(Vector {
        id,
        doc,
        settings,
        layers,
        expect: expect_from(expects[0])?,
    })
}

fn setting_from(node: &KdlNode) -> Result<Setting, String> {
    let children = node.children();
    if let Some(children) = children {
        reject_nodes(children, &["default", "choice"])?;
    }
    let defaults = child_values(children, "default")?;
    let default = match (node.get("default"), defaults.is_empty()) {
        (Some(_), false) => {
            return Err("a setting cannot have both scalar and list defaults".into())
        }
        (Some(value), true) => Some(json_value(value)?),
        (None, false) => Some(serde_json::Value::Array(defaults)),
        (None, true) => None,
    };
    Ok(Setting {
        key: string(node, 0)?,
        r#type: optional_string(node, "type")?.unwrap_or_else(default_type),
        default,
        merge: match optional_string(node, "merge")?.as_deref() {
            None | Some("replace") => MergePolicy::Replace,
            Some("union") => MergePolicy::Union,
            Some("deep") => MergePolicy::Deep,
            Some(value) => return Err(format!("no merge policy named `{value}`")),
        },
        scope: match optional_string(node, "scope")?.as_deref() {
            None | Some("any") => ScopeRule::Any,
            Some("global") => ScopeRule::Global,
            Some("env") => ScopeRule::Env,
            Some(value) => return Err(format!("no scope named `{value}`")),
        },
        parse: optional_string(node, "parse")?,
        choices: child_values(children, "choice")?,
        renamed_to: optional_string(node, "renamed-to")?,
        deprecated: optional_string(node, "deprecated")?,
    })
}

fn layer_from(node: &KdlNode) -> Result<LayerSpec, String> {
    let children = node.children();
    if let Some(children) = children {
        reject_nodes(children, &["value", "shaped", "shaped-list"])?;
    }
    let trust = match optional_string(node, "trust")?.as_deref() {
        None => None,
        Some("invocation") => Some(TrustLevel::Invocation),
        Some("operator") => Some(TrustLevel::Operator),
        Some("project") => Some(TrustLevel::Project),
        Some(value) => return Err(format!("no trust level named `{value}`")),
    };
    Ok(LayerSpec {
        source: string(node, 0)?,
        id: optional_string(node, "id")?,
        trust,
        values: keyed_strings(children, "value")?,
        shaped: keyed_values(children, "shaped", "shaped-list")?,
    })
}

fn expect_from(node: &KdlNode) -> Result<Expect, String> {
    let children = node.children();
    if let Some(children) = children {
        reject_nodes(children, &["value", "list", "map", "origin", "warning"])?;
    }
    let warnings = children
        .into_iter()
        .flat_map(KdlDocument::nodes)
        .filter(|node| node.name().value() == "warning")
        .map(|node| match string(node, 0)?.as_str() {
            "unknown-setting" => Ok(Complaint::UnknownSetting),
            "wrong-type" => Ok(Complaint::WrongType),
            "not-allowed" => Ok(Complaint::NotAllowed),
            "out-of-scope" => Ok(Complaint::OutOfScope),
            "deprecated" => Ok(Complaint::Deprecated),
            "renamed" => Ok(Complaint::Renamed),
            "not-read" => Ok(Complaint::NotRead),
            value => Err(format!("no warning kind named `{value}`")),
        })
        .collect::<Result<_, _>>()?;
    Ok(Expect {
        values: keyed_values(children, "value", "list")?
            .into_iter()
            .chain(keyed_values(children, "map", "unused")?)
            .collect(),
        origins: keyed_strings(children, "origin")?,
        warnings,
    })
}

fn keyed_strings(
    document: Option<&KdlDocument>,
    name: &str,
) -> Result<BTreeMap<String, String>, String> {
    document
        .into_iter()
        .flat_map(KdlDocument::nodes)
        .filter(|node| node.name().value() == name)
        .map(|node| Ok((string(node, 0)?, string(node, 1)?)))
        .collect()
}

fn keyed_values(
    document: Option<&KdlDocument>,
    scalar_or_map: &str,
    list: &str,
) -> Result<BTreeMap<String, serde_json::Value>, String> {
    let mut values = BTreeMap::new();
    for node in document.into_iter().flat_map(KdlDocument::nodes) {
        let name = node.name().value();
        let value = if name == list {
            serde_json::Value::Array(
                positional(node)
                    .skip(1)
                    .map(json_value)
                    .collect::<Result<_, _>>()?,
            )
        } else if name == scalar_or_map {
            if let Some(children) = node.children() {
                serde_json::Value::Object(
                    keyed_values(Some(children), "value", "list")?
                        .into_iter()
                        .collect(),
                )
            } else {
                json_value(
                    node.get(1)
                        .ok_or_else(|| format!("`{name}` needs a key and a value"))?,
                )?
            }
        } else {
            continue;
        };
        let key = string(node, 0)?;
        if values.insert(key.clone(), value).is_some() {
            return Err(format!("`{key}` is declared twice"));
        }
    }
    Ok(values)
}

fn child_values(
    document: Option<&KdlDocument>,
    name: &str,
) -> Result<Vec<serde_json::Value>, String> {
    document
        .into_iter()
        .flat_map(KdlDocument::nodes)
        .filter(|node| node.name().value() == name)
        .map(|node| {
            node.get(0)
                .ok_or_else(|| format!("`{name}` needs a value"))
                .and_then(json_value)
        })
        .collect()
}

fn one_string(document: &KdlDocument, name: &str) -> Result<String, String> {
    let nodes: Vec<_> = document
        .nodes()
        .iter()
        .filter(|node| node.name().value() == name)
        .collect();
    if nodes.len() != 1 {
        return Err(format!(
            "a corpus file must contain exactly one `{name}` node"
        ));
    }
    string(nodes[0], 0)
}

fn string(node: &KdlNode, key: impl Into<kdl::NodeKey>) -> Result<String, String> {
    let name = node.name().value();
    node.get(key)
        .and_then(KdlValue::as_string)
        .map(str::to_string)
        .ok_or_else(|| format!("`{name}` needs a string value"))
}

fn optional_string(node: &KdlNode, key: impl Into<kdl::NodeKey>) -> Result<Option<String>, String> {
    let key = key.into();
    node.get(key.clone())
        .map(|value| {
            value
                .as_string()
                .map(str::to_string)
                .ok_or_else(|| format!("`{}` has a non-string property", node.name().value()))
        })
        .transpose()
}

fn children(node: &KdlNode) -> Result<&KdlDocument, String> {
    node.children()
        .ok_or_else(|| format!("`{}` needs a child block", node.name().value()))
}

fn reject_nodes(document: &KdlDocument, allowed: &[&str]) -> Result<(), String> {
    if let Some(node) = document
        .nodes()
        .iter()
        .find(|node| !allowed.contains(&node.name().value()))
    {
        return Err(format!("unexpected `{}` node", node.name().value()));
    }
    Ok(())
}

fn positional(node: &KdlNode) -> impl Iterator<Item = &KdlValue> {
    node.entries()
        .iter()
        .filter(|entry| entry.name().is_none())
        .map(kdl::KdlEntry::value)
}

fn json_value(value: &KdlValue) -> Result<serde_json::Value, String> {
    Ok(match value {
        KdlValue::String(value) => serde_json::Value::String(value.clone()),
        KdlValue::Integer(value) => {
            if let Ok(value) = i64::try_from(*value) {
                serde_json::Value::Number(value.into())
            } else if let Ok(value) = u64::try_from(*value) {
                serde_json::Value::Number(value.into())
            } else {
                return Err(format!("`{value}` is too large for the corpus value model"));
            }
        }
        KdlValue::Float(value) => serde_json::Number::from_f64(*value)
            .map(serde_json::Value::Number)
            .ok_or_else(|| format!("`{value}` is not a finite number"))?,
        KdlValue::Bool(value) => serde_json::Value::Bool(*value),
        KdlValue::Null => serde_json::Value::Null,
    })
}

impl Complaint {
    fn of(kind: WarningKind) -> Option<Self> {
        Some(match kind {
            WarningKind::UnknownSetting => Self::UnknownSetting,
            WarningKind::WrongType => Self::WrongType,
            WarningKind::NotAllowed => Self::NotAllowed,
            WarningKind::OutOfScope => Self::OutOfScope,
            WarningKind::Deprecated => Self::Deprecated,
            WarningKind::Renamed => Self::Renamed,
            WarningKind::NotRead => Self::NotRead,
            // A layer of a CLI's own, which no vector can describe: this corpus only builds the
            // layers it defines, so there is nothing here that could produce one.
            _ => return None,
        })
    }
}

/// Resolve a vector, and say what came out.
pub fn run(vector: &Vector) -> Result<Expect, String> {
    let registry = registry_of(&vector.settings)?;
    let layers: Vec<Described> = vector.layers.iter().map(Described::new).collect();
    let mut plan = Layers::new();
    for layer in &layers {
        plan = plan.then(layer);
    }
    let resolved = resolve(registry, plan).map_err(|err| format!("{err}"))?;

    let mut values = BTreeMap::new();
    let mut origins = BTreeMap::new();
    for id in registry.ids() {
        let meta = registry.get(id);
        // An old name has no value of its own — every read of it folds — so reporting one would be
        // reporting the replacement's value twice, under a key nothing resolves.
        if meta.renamed_to.is_some() {
            continue;
        }
        if let Some(value) = resolved.get(id) {
            values.insert(meta.key.to_string(), json_of(value));
        }
        if let Some(origin) = resolved.origin(id) {
            origins.insert(meta.key.to_string(), origin.describe().to_string());
        }
    }
    Ok(Expect {
        values,
        origins,
        warnings: resolved
            .warnings
            .iter()
            .filter_map(|warning| Complaint::of(warning.kind))
            .collect(),
    })
}

/// Compare what came out against what a vector asked for.
///
/// `origins` is checked only where the vector names a key, because most vectors are not about where
/// a value came from and listing every one of them would bury the ones that are.
pub fn matches(expect: &Expect, actual: &Expect) -> bool {
    expect.values == actual.values
        && expect.warnings == actual.warnings
        && expect
            .origins
            .iter()
            .all(|(key, origin)| actual.origins.get(key) == Some(origin))
}

/// A layer that supplies exactly what a vector described.
struct Described<'a> {
    spec: &'a LayerSpec,
    kind: SourceKind,
}

impl<'a> Described<'a> {
    fn new(spec: &'a LayerSpec) -> Self {
        let kind = match spec.source.as_str() {
            "cli" => SourceKind::CLI,
            "env" => SourceKind::ENV,
            "file" => SourceKind::FILE,
            // A kind usage does not know is exactly what `source "git"` in a spec declares, and the
            // merge is expected to treat it as one it cannot vouch for.
            other => SourceKind::new(Box::leak(other.to_string().into_boxed_str())),
        };
        Self { spec, kind }
    }

    fn origin(&self) -> Origin {
        let id = self
            .spec
            .id
            .clone()
            .unwrap_or_else(|| self.spec.source.clone());
        let origin = Origin::new(self.kind, id);
        match self.spec.trust {
            Some(TrustLevel::Invocation) => origin.trusted_as(Trust::Invocation),
            Some(TrustLevel::Operator) => origin.trusted_as(Trust::Operator),
            Some(TrustLevel::Project) => origin.trusted_as(Trust::Project),
            None => origin,
        }
    }
}

impl Layer for Described<'_> {
    fn source(&self) -> SourceKind {
        self.kind
    }

    fn load(&self, ctx: &LayerCtx) -> Result<LayerOutput, LayerError> {
        let mut out = LayerOutput::new();
        for (key, raw) in &self.spec.values {
            match ctx.entry_for_key(key, raw, self.origin()) {
                Ok(entry) => out.push(entry),
                Err(warning) => out.warn(warning),
            }
        }
        for (key, value) in &self.spec.shaped {
            // A value the harness cannot read is a broken *vector*, not a layer with something to
            // say about it: it stops the run rather than resolving to something nobody wrote.
            let value = value_of(value).map_err(|why| LayerError::Unreadable {
                source: format!("the `{key}` value of a {} layer", self.spec.source),
                why,
            })?;
            match ctx.entry_from_value(key, value, self.origin()) {
                Ok(entry) => out.push(entry),
                Err(warning) => out.warn(warning),
            }
        }
        Ok(out)
    }
}

/// The registry a vector's settings describe.
///
/// Leaked, because a registry is `&'static` by design: it is a `const` in every real CLI, built at
/// compile time from the spec. A test harness is the one place that needs to build one while it
/// runs, and a few dozen vectors' worth of settings is a few dozen kilobytes for the life of a test
/// process.
fn registry_of(settings: &[Setting]) -> Result<Registry, String> {
    let mut props = Vec::with_capacity(settings.len());
    for setting in settings {
        let ty = ty_of(&setting.r#type)?;
        props.push(PropMeta {
            key: leak(&setting.key),
            ty,
            default: setting.default.as_ref().map(const_of).transpose()?,
            merge: match setting.merge {
                MergePolicy::Replace => Merge::Replace,
                MergePolicy::Union => Merge::Union,
                MergePolicy::Deep => Merge::Deep,
            },
            scope: match setting.scope {
                ScopeRule::Any => Scope::Any,
                ScopeRule::Global => Scope::Global,
                ScopeRule::Env => Scope::Env,
            },
            parse: match &setting.parse {
                Some(name) => Some(
                    usage_config::Parser::from_name(name)
                        .ok_or_else(|| format!("no parser named `{name}`"))?,
                ),
                None => None,
            },
            envs: &[],
            bindings: &[],
            choices: Box::leak(
                setting
                    .choices
                    .iter()
                    .map(const_of)
                    .collect::<Result<Vec<_>, _>>()?
                    .into_boxed_slice(),
            ),
            hide: false,
            deprecated: setting.deprecated.as_deref().map(leak),
            renamed_to: setting.renamed_to.as_deref().map(leak),
            help: None,
        });
    }
    Ok(Registry::new(Box::leak(props.into_boxed_slice())))
}

/// The corpus's own reading of the type grammar.
///
/// Deliberately its own: a conformance harness that asked the implementation under test what a type
/// means would be checking that it agrees with itself.
fn ty_of(text: &str) -> Result<Ty, String> {
    let text = text.trim();
    if let Some(inner) = wrapped(text, "list") {
        return Ok(Ty::List(Box::leak(Box::new(ty_of(inner)?))));
    }
    if let Some(inner) = wrapped(text, "set") {
        return Ok(Ty::Set(Box::leak(Box::new(ty_of(inner)?))));
    }
    if let Some(inner) = wrapped(text, "option") {
        return Ok(Ty::Option(Box::leak(Box::new(ty_of(inner)?))));
    }
    if let Some(inner) = wrapped(text, "map") {
        // A table's keys are text in every format a settings file is written in, so only the value
        // type is carried.
        let (key, value) = inner
            .split_once(',')
            .ok_or_else(|| format!("`{text}` needs a key type and a value type"))?;
        if key.trim() != "string" {
            return Err(format!("`{text}`: a table's keys are text"));
        }
        return Ok(Ty::Map(Box::leak(Box::new(ty_of(value)?))));
    }
    Ok(match text {
        "bool" => Ty::Bool,
        "int" => Ty::Int,
        "uint" => Ty::Uint,
        "float" => Ty::Float,
        "string" => Ty::String,
        "path" => Ty::Path,
        "url" => Ty::Url,
        "duration" => Ty::Duration,
        "object" => Ty::Object,
        // A union, or a name this corpus does not know: the type says nothing is decided here.
        "any" => Ty::Any,
        other if other.contains('|') => Ty::Any,
        other => return Err(format!("no type named `{other}`")),
    })
}

fn wrapped<'t>(text: &'t str, name: &str) -> Option<&'t str> {
    text.strip_prefix(name)?
        .strip_prefix('<')?
        .strip_suffix('>')
}

fn leak(text: &str) -> &'static str {
    Box::leak(text.to_string().into_boxed_str())
}

/// A corpus number as the number this crate holds, or a reason it is not one.
///
/// Refused rather than clamped or zeroed. A harness that quietly misreads its own corpus is worse
/// than one that cannot read it: the vector would go on to pass or fail for a reason that has
/// nothing to do with what it says.
fn number(n: &serde_json::Number) -> Result<Number, String> {
    if let Some(i) = n.as_i64() {
        return Ok(Number::Int(i));
    }
    if let Some(f) = n.as_f64().filter(|_| n.is_f64()) {
        return Ok(Number::Float(f));
    }
    Err(format!(
        "`{n}` is not a number this crate can hold: an integer is 64 bits and signed"
    ))
}

/// One of the two shapes a number arrives in.
enum Number {
    Int(i64),
    Float(f64),
}

/// A corpus value as a declared constant.
fn const_of(value: &serde_json::Value) -> Result<Const, String> {
    Ok(match value {
        serde_json::Value::Bool(b) => Const::Bool(*b),
        serde_json::Value::Number(n) => match number(n)? {
            Number::Int(i) => Const::Int(i),
            Number::Float(f) => Const::Float(f),
        },
        serde_json::Value::Array(items) => Const::List(Box::leak(
            items
                .iter()
                .map(const_of)
                .collect::<Result<Vec<_>, _>>()?
                .into_boxed_slice(),
        )),
        serde_json::Value::Object(entries) => Const::Map(Box::leak(
            entries
                .iter()
                .map(|(key, value)| const_of(value).map(|value| (leak(key), value)))
                .collect::<Result<Vec<_>, _>>()?
                .into_boxed_slice(),
        )),
        // `null` is a key that is not there, and a default of one is a default of nothing.
        serde_json::Value::Null => Const::Str(""),
        serde_json::Value::String(s) => Const::Str(leak(s)),
    })
}

/// A corpus value as a resolved one.
fn value_of(value: &serde_json::Value) -> Result<Value, String> {
    Ok(match value {
        serde_json::Value::Bool(b) => Value::Bool(*b),
        serde_json::Value::Number(n) => match number(n)? {
            Number::Int(i) => Value::Int(i),
            Number::Float(f) => Value::Float(f),
        },
        serde_json::Value::Array(items) => {
            Value::List(items.iter().map(value_of).collect::<Result<Vec<_>, _>>()?)
        }
        serde_json::Value::Object(entries) => Value::Map(
            entries
                .iter()
                .map(|(key, value)| value_of(value).map(|value| (key.clone(), value)))
                .collect::<Result<_, _>>()?,
        ),
        serde_json::Value::Null => Value::String(String::new()),
        serde_json::Value::String(s) => Value::String(s.clone()),
    })
}

/// A resolved value as JSON, for comparing with what a vector wrote.
fn json_of(value: &Value) -> serde_json::Value {
    match value {
        Value::Bool(b) => serde_json::Value::Bool(*b),
        Value::Int(i) => serde_json::Value::Number((*i).into()),
        Value::Float(f) => serde_json::Number::from_f64(*f)
            .map(serde_json::Value::Number)
            .unwrap_or(serde_json::Value::Null),
        Value::String(s) => serde_json::Value::String(s.clone()),
        Value::List(items) => serde_json::Value::Array(items.iter().map(json_of).collect()),
        Value::Map(entries) => serde_json::Value::Object(
            entries
                .iter()
                .map(|(key, value)| (key.clone(), json_of(value)))
                .collect(),
        ),
    }
}
