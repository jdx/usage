//! Config files as a layer: where they are, and what reading one means.
//!
//! This is the rc-style chain a spec's `file` nodes describe — a system file, the user's own,
//! then the project's, found by walking up from the working directory. Every CLI in the fleet
//! has written this walk by hand, and the interesting part is never the walk: it is what
//! happens at the edges. A file that does not exist is not an error. A file that exists and
//! cannot be parsed *is*. A key nobody recognizes is a warning, because a config file written
//! for a newer binary must still work with an older one. A value of the wrong type is a
//! warning too, and only for that key — a typo in a system-wide file must not stop a CLI from
//! starting for every user on the machine.
//!
//! Formats live behind features so the crate's default is still no dependencies at all: a CLI
//! whose files are pkl or `.npmrc` writes its own layer against [`Layer`] and takes nothing it
//! does not use.

use std::path::{Component, Path, PathBuf};

use crate::layer::{Layer, LayerCtx, LayerError, LayerOutput};
use crate::source::{FileScope, Origin, SourceKind};
use crate::value::Value;

/// A hook that rewrites a file's text before it is parsed.
///
/// Named because clippy will not have it written inline, and it reads better here anyway: text
/// in, text out, with the failure a string this crate does not interpret.
type Preprocess = Box<dyn Fn(&str) -> Result<String, String> + Send + Sync>;

/// How to read a file's bytes into keys and values.
#[derive(Debug, Copy, Clone, PartialEq, Eq)]
pub enum Format {
    #[cfg(feature = "toml")]
    Toml,
    #[cfg(feature = "json")]
    Json,
    #[cfg(feature = "yaml")]
    Yaml,
}

impl Format {
    /// The format an extension implies, when one does.
    ///
    /// A path with no extension, or one nobody claims, returns `None` — the caller names the
    /// format itself rather than a guess being made on its behalf. `.myclirc` is TOML in one
    /// CLI and YAML in another, and neither is discoverable from the name.
    pub fn of(path: &Path) -> Option<Self> {
        match path.extension().and_then(|e| e.to_str()) {
            #[cfg(feature = "toml")]
            Some("toml") => Some(Self::Toml),
            #[cfg(feature = "json")]
            Some("json") => Some(Self::Json),
            // Both spellings, because a project picks one and the other still turns up: a CI
            // file is `.yml` and the config next to it is `.yaml` as often as not.
            #[cfg(feature = "yaml")]
            Some("yaml") | Some("yml") => Some(Self::Yaml),
            _ => None,
        }
    }
}

/// One config file, or a name to look for in a directory and its parents.
///
/// The scope is what the merge's trust check reads, so it is required rather than defaulted:
/// a file layer that forgot to say where it came from would be trusted as an operator's by
/// accident, and that is the check a `scope="global"` setting exists for.
pub struct FileLayer {
    paths: Vec<PathBuf>,
    scope: FileScope,
    format: Option<Format>,
    prefix: Option<String>,
    preprocess: Option<Preprocess>,
}

impl FileLayer {
    /// One file at a known path.
    pub fn at(path: impl Into<PathBuf>, scope: FileScope) -> Self {
        Self {
            paths: vec![path.into()],
            scope,
            format: None,
            prefix: None,
            preprocess: None,
        }
    }

    /// A file name looked for in `from` and every directory above it, stopping at `ceiling`.
    ///
    /// Ordered farthest-first, because the merge takes the last writer: the nearest file to
    /// where the user is standing is the one that should win. Getting this backwards is the
    /// bug the fleet's hand-written walks make, and it is invisible until two files disagree.
    ///
    /// `ceiling` is inclusive and is how a CLI avoids reading a stranger's file above a
    /// checkout — nothing here decides that policy, because only the CLI knows what a project
    /// boundary is for it.
    pub fn find_up(
        name: &str,
        from: impl AsRef<Path>,
        ceiling: Option<&Path>,
        scope: FileScope,
    ) -> Self {
        // The same normalization on both sides, because the comparison below is between path
        // components: a caller who passed a relative ceiling — or one reached through a symlink,
        // or one holding a `..` — matched nothing and either walked to the filesystem root or
        // stopped before its first step, and a boundary that silently reads nothing is no better
        // than one that reads a stranger's file.
        let from = normalize(from.as_ref());
        let ceiling = ceiling.map(normalize);

        let mut found = Vec::new();
        let mut dir = Some(from.as_path());
        while let Some(current) = dir {
            // Above the ceiling, or never inside it: stop without reading. A ceiling that is
            // not an ancestor at all means the caller and this walk disagree about where the
            // boundary is, and reading everything up to the root is the worse of the two ways
            // to be wrong about that.
            if let Some(ceiling) = &ceiling {
                if !current.starts_with(ceiling) {
                    break;
                }
            }
            found.push(current.join(name));
            if ceiling.as_deref() == Some(current) {
                break;
            }
            dir = current.parent();
        }
        found.reverse();
        Self {
            paths: found,
            scope,
            format: None,
            prefix: None,
            preprocess: None,
        }
    }

    /// Read these files as `format`, whatever their names say.
    pub fn as_format(mut self, format: Format) -> Self {
        self.format = Some(format);
        self
    }

    /// Read settings from a table rather than from the top level.
    ///
    /// mise's settings live under `[settings]` in a file that also holds tools and tasks; hk's
    /// are at the top level. Without this a CLI would have to pre-extract the table, which
    /// means parsing the file twice or teaching this layer about the rest of the format.
    pub fn under(mut self, table: impl Into<String>) -> Self {
        self.prefix = Some(table.into());
        self
    }

    /// Transform the text before it is parsed.
    ///
    /// Where mise's tera templating goes. The hook takes and returns text so this crate never
    /// learns what a template is; an `Err` is a read failure naming the file, because a
    /// template that will not render is not something to carry on past.
    pub fn preprocess(
        mut self,
        f: impl Fn(&str) -> Result<String, String> + Send + Sync + 'static,
    ) -> Self {
        self.preprocess = Some(Box::new(f));
        self
    }

    /// The paths this layer will read, farthest first.
    pub fn paths(&self) -> &[PathBuf] {
        &self.paths
    }

    fn read(&self, path: &Path, ctx: &LayerCtx, out: &mut LayerOutput) -> Result<(), LayerError> {
        // Absent is the normal case, not a failure: a find-up chain is mostly directories with
        // no config file in them. *Only* absent, though — a file that is there and cannot be
        // read is the case this module's own doc calls an error, and `read_to_string` also
        // fails for a permission denied, a directory at that path, and content that is not
        // UTF-8. Treating those as absence dropped settings the user wrote and said nothing.
        let text = match std::fs::read_to_string(path) {
            Ok(text) => text,
            // `NotFound` is also what a link pointing at nothing reports — about its target, while
            // the link is plainly there — and what a path under such a link reports. Read as
            // absence, a user whose global config is a link into a directory they have since moved
            // had it ignored in silence. The message names the link rather than the file, because
            // the link is the thing to go and fix.
            Err(err) if err.kind() == std::io::ErrorKind::NotFound => match broken_link(path) {
                Some(link) => {
                    return Err(LayerError::Unreadable {
                        source: path.display().to_string(),
                        why: format!("`{}` leads nowhere", link.display()),
                    })
                }
                None => return Ok(()),
            },
            Err(err) => {
                return Err(LayerError::Unreadable {
                    source: path.display().to_string(),
                    why: err.to_string(),
                })
            }
        };
        let text = match &self.preprocess {
            Some(f) => f(&text).map_err(|why| LayerError::Unreadable {
                source: path.display().to_string(),
                why,
            })?,
            None => text,
        };
        let format =
            self.format
                .or_else(|| Format::of(path))
                .ok_or_else(|| LayerError::Unreadable {
                    source: path.display().to_string(),
                    why: "cannot tell what format this is; name it with `as_format`".to_string(),
                })?;
        // A table stops being a path to settings and becomes one *itself* where the spec says
        // so: a `map` or `object` setting *is* a table, and flattening through it turned its
        // entries into dotted keys nobody declared — so the map was never set and every key
        // inside it was reported unknown. Which tables those are is the registry's answer, so
        // the flattener asks rather than guessing.
        // A table is a *path* to settings until it is a setting itself, and the registry is the
        // only thing that knows which: if the dotted key names a declared setting, the table
        // belongs to it, whatever its type. Asking about the type instead took three tries to
        // get wrong in three ways — `Map` and `Object` alone left a union's table walked into,
        // and an *empty* table under a scalar key produced no keys at all, so nothing was set
        // and nothing was said. Handing the table over lets the declared type answer: a `map`
        // takes it, a union takes it, a `uint` refuses it and that refusal is the warning.
        let names_a_setting = |key: &str| ctx.registry().names_file_value(key);
        let flat =
            parse(format, &text, self.prefix.as_deref(), &names_a_setting).map_err(|why| {
                LayerError::Unreadable {
                    source: path.display().to_string(),
                    why,
                }
            })?;
        for (key, found) in flat {
            let origin = Origin::file(format!("{}#{key}", path.display()), self.scope);
            match found {
                // Through `entry_for_key`, which is what makes an unknown key a warning,
                // follows a rename while remembering the name that was written, and reads the
                // value as the type the spec declares rather than as the type the file used.
                Read::Text(raw) => match ctx.entry_for_key(&key, &raw, origin) {
                    Ok(entry) => out.push(entry),
                    Err(warning) => out.warn(warning),
                },
                // A value the file already gave a shape to — a table the spec declared, or an
                // array, neither of which any text parser could produce. Through the declared
                // type all the same: skipping it stored a number inside a
                // `map<string, string>` without a word, which is the "wrong type costs a
                // warning" promise broken for the settings that need it most.
                Read::Shaped(value) => match ctx.entry_from_value(&key, value, origin) {
                    Ok(entry) => out.push(entry),
                    Err(warning) => out.warn(warning),
                },
            }
        }
        Ok(())
    }
}

impl Layer for FileLayer {
    fn source(&self) -> SourceKind {
        SourceKind::FILE
    }

    fn load(&self, ctx: &LayerCtx) -> Result<LayerOutput, LayerError> {
        let mut out = LayerOutput::new();
        for path in &self.paths {
            self.read(path, ctx, &mut out)?;
        }
        Ok(out)
    }
}

/// What one key in a file turned out to hold.
enum Read {
    /// Text, which the spec's own named parser and declared type will make sense of.
    Text(String),
    /// A value the file already gave a shape to: an array, or a table the spec declared as a
    /// `map` or an `object`.
    ///
    /// Joining an array back into text was lossy in two ways that only showed up together with
    /// the spec: `["a,b", "c"]` came out as `"a,b,c"` and a comma parser then read three items
    /// where the file said two, and a list setting declaring `list_by_colon` — or no parser at
    /// all — got one item holding the joined text. A file has structure; the named parsers are
    /// for sources that do not, like an environment variable.
    Shaped(Value),
}

/// A file's contents as dotted keys and what each one holds.
///
/// Text, because the *spec* decides what a value means: a setting declared `list<string>` with
/// `parse="list_by_comma"` reads `"a,b"` as two items whether the file said so or not, and a
/// layer that pre-decided would disagree with the environment about the same setting. Nested
/// tables become dotted keys, which is the shape the registry is keyed by.
fn parse(
    format: Format,
    text: &str,
    prefix: Option<&str>,
    names_a_setting: &dyn Fn(&str) -> bool,
) -> Result<Vec<(String, Read)>, String> {
    match format {
        #[cfg(feature = "toml")]
        Format::Toml => {
            // `from_str`, not `str::parse`: in toml 0.9 the latter reads a single *value*, so a
            // whole document came back as "unexpected content, expected nothing".
            let value: toml::Value = toml::from_str(text).map_err(|e| e.to_string())?;
            let value = match prefix {
                Some(table) => match value.get(table) {
                    Some(inner) if inner.is_table() => inner.clone(),
                    // Present, and not a table. Flattening from there produced an *empty* key —
                    // reported as an unknown setting called nothing — while the real settings
                    // went unread. The file is wrong in the one place this layer was pointed at,
                    // which is the same kind of wrong as failing to parse.
                    Some(_) => {
                        return Err(format!(
                            "`{table}` should be a table of settings, and is not"
                        ))
                    }
                    // A file that simply has no settings table is not a broken file.
                    None => return Ok(Vec::new()),
                },
                None => value,
            };
            let mut flat = Vec::new();
            flatten_toml(String::new(), &value, names_a_setting, &mut flat);
            Ok(flat)
        }
        #[cfg(feature = "json")]
        Format::Json => {
            let value: serde_json::Value = serde_json::from_str(text).map_err(|e| e.to_string())?;
            // Asked about the root before the table under it, because a root that is not an
            // object has no keys to read *either* way — and asked only in the no-prefix branch,
            // `get` returned `None` for a list or a scalar root, which is the same answer as a
            // file that simply has no settings table, so a file that is plainly the wrong shape
            // was rejected without `under` and ignored with it. TOML reaches neither state: its
            // root is always a table.
            if !(value.is_object() || value.is_null()) {
                return Err("the file should be a table of settings, and is not".into());
            }
            let value = match prefix {
                Some(table) => match value.get(table) {
                    Some(inner) if inner.is_object() => inner.clone(),
                    // `null` is how a JSON file says a key is not there, and a file that leaves
                    // the table out entirely is allowed — so the same meaning must not be the
                    // one spelling of it that fails the read.
                    Some(serde_json::Value::Null) | None => return Ok(Vec::new()),
                    Some(_) => {
                        return Err(format!(
                            "`{table}` should be a table of settings, and is not"
                        ))
                    }
                },
                // Checked above: an object, or a `null` root that flattens to no keys at all —
                // a file that says nothing rather than a file that is wrong.
                None => value,
            };
            let mut flat = Vec::new();
            flatten_json(String::new(), &value, names_a_setting, &mut flat);
            Ok(flat)
        }
        #[cfg(feature = "yaml")]
        Format::Yaml => {
            // An empty file, a file of comments, and a `---` with nothing after it all parse as
            // `null`, which flattens to no keys at all: a file that says nothing, which is what
            // a freshly-touched config file is.
            //
            // More than one document is the one thing the parser refuses outright, and it says
            // so in terms of deserializing rather than in terms of the file, so its words are
            // replaced with the file's. Matched on the message because that is all the parser
            // exposes — should it ever be reworded, its own message comes through instead, which
            // is the same information less plainly put.
            let root = yaml_serde::from_str::<yaml_serde::Value>(text).map_err(|e| {
                let message = e.to_string();
                match message.contains("more than one document") {
                    true => {
                        "a settings file is one document, and this is more than one".to_string()
                    }
                    false => message,
                }
            })?;
            // Asked before the table under it, and for the same reason as in JSON: a root that
            // is not a mapping has no keys to read either way, and `get` cannot tell a list root
            // apart from a file with no settings table in it.
            if !(root.is_mapping() || root.is_null()) {
                return Err("the file should be a table of settings, and is not".into());
            }
            let value = match prefix {
                Some(table) => match root.get(table).map(untagged) {
                    Some(inner) if inner.is_mapping() => inner,
                    // `null` is how a YAML file says a key is not there, and a file that leaves
                    // the table out entirely is allowed — the same meaning must not be the one
                    // spelling of it that fails the read. Through `untagged` above, because this
                    // arm is a variant and a tagged null would otherwise reach the one below it.
                    Some(yaml_serde::Value::Null) | None => return Ok(Vec::new()),
                    Some(_) => {
                        return Err(format!(
                            "`{table}` should be a table of settings, and is not"
                        ))
                    }
                },
                // Checked above: a mapping, or a `null` root that flattens to nothing.
                None => &root,
            };
            let mut flat = Vec::new();
            flatten_yaml(String::new(), value, names_a_setting, &mut flat)?;
            Ok(flat)
        }
    }
}

/// `path` in the form two paths have to be in to be compared.
///
/// The filesystem answers where it can: canonicalizing resolves symlinks and `..` together, which
/// no amount of string work can do — `a/link/..` is wherever `link` points, not `a`.
///
/// A path that is not there yet cannot be canonicalized, and the fallback is where the care is.
/// Left as written, a relative side made every comparison false; made merely absolute, a `..`
/// survived as a component of its own and did the same; resolved lexically, `link/..` went to the
/// wrong tree — the one thing the sentence above says string work cannot do. So it is resolved one
/// component at a time, canonicalizing each as it is added: a link is followed before the `..`
/// after it is applied, and a component that does not exist cannot be a link to anywhere, so
/// stepping back out of it lexically is right. Both sides go through here, so a boundary check
/// compares like with like even when both paths are strange.
fn normalize(path: &Path) -> PathBuf {
    if let Ok(real) = path.canonicalize() {
        return real;
    }
    let absolute = std::path::absolute(path).unwrap_or_else(|_| path.to_path_buf());
    let mut out = PathBuf::new();
    for part in absolute.components() {
        match part {
            // `.` is nothing. `..` steps back out of what has been resolved so far — and at a root
            // there is nothing to step back to, where `pop` does nothing, which is what `/..`
            // means.
            Component::CurDir => {}
            Component::ParentDir => {
                out.pop();
            }
            other => {
                out.push(other);
                if let Ok(real) = out.canonicalize() {
                    out = real;
                }
            }
        }
    }
    out
}

/// The link along `path` that leads nowhere, if there is one.
///
/// `read_to_string` reports `NotFound` both for a link pointing at nothing and for a path *below*
/// one, and neither is the absence this layer passes over in silence: something on the way is
/// there, and does not lead anywhere. A directory nobody has created is a different matter, and
/// the common case — so the walk stops at the first thing that exists, and only runs at all once a
/// read has already failed.
fn broken_link(path: &Path) -> Option<PathBuf> {
    let mut current = Some(path);
    while let Some(step) = current {
        match step.symlink_metadata() {
            // Something is here. If it is a link that cannot be followed, that is the answer; if
            // it is anything else, everything above it exists and there is nothing left to ask.
            Ok(meta) => {
                return (meta.is_symlink() && step.metadata().is_err()).then(|| step.to_path_buf());
            }
            Err(_) => current = step.parent(),
        }
    }
    None
}

/// Join a dotted path, without a leading dot at the root.
fn joined(prefix: &str, key: &str) -> String {
    if prefix.is_empty() {
        key.to_string()
    } else {
        format!("{prefix}.{key}")
    }
}

#[cfg(feature = "toml")]
fn flatten_toml(
    prefix: String,
    value: &toml::Value,
    names_a_setting: &dyn Fn(&str) -> bool,
    out: &mut Vec<(String, Read)>,
) {
    match value {
        // A table the spec declared as one is the value, not the way to it.
        toml::Value::Table(_) if !prefix.is_empty() && names_a_setting(&prefix) => {
            out.push((prefix, Read::Shaped(table_toml(value))));
        }
        toml::Value::Table(table) => {
            for (key, inner) in table {
                flatten_toml(joined(&prefix, key), inner, names_a_setting, out);
            }
        }
        // A list arrives as the text a named parser would have produced, so a setting reads the
        // same whether it came from a file or from an environment variable. A list of tables is
        // not something a settings file expresses; its items render as their own text and the
        // declared type refuses them, which is a warning naming the key.
        toml::Value::Array(_) => out.push((prefix, Read::Shaped(table_toml(value)))),
        scalar => out.push((prefix, Read::Text(scalar_toml(scalar)))),
    }
}

/// A table as a [`Value`], nested tables and all, for a setting declared to hold one.
///
/// The values keep the shape the file gave them rather than becoming text: a `map` says what its
/// values are, and there is no named parser in the middle to reinterpret them.
#[cfg(feature = "toml")]
fn table_toml(value: &toml::Value) -> Value {
    match value {
        toml::Value::Table(table) => Value::Map(
            table
                .iter()
                .map(|(key, inner)| (key.clone(), table_toml(inner)))
                .collect(),
        ),
        toml::Value::Array(items) => Value::List(items.iter().map(table_toml).collect()),
        toml::Value::Boolean(b) => Value::Bool(*b),
        toml::Value::Integer(i) => Value::Int(*i),
        toml::Value::Float(f) => Value::Float(*f),
        other => Value::String(scalar_toml(other)),
    }
}

#[cfg(feature = "toml")]
fn scalar_toml(value: &toml::Value) -> String {
    match value {
        toml::Value::String(s) => s.clone(),
        other => other.to_string(),
    }
}

#[cfg(feature = "json")]
fn flatten_json(
    prefix: String,
    value: &serde_json::Value,
    names_a_setting: &dyn Fn(&str) -> bool,
    out: &mut Vec<(String, Read)>,
) {
    match value {
        serde_json::Value::Object(_) if !prefix.is_empty() && names_a_setting(&prefix) => {
            out.push((prefix, Read::Shaped(table_json(value))));
        }
        // `null` is how a JSON file says "no value", so the key is not there: storing it turned
        // an explicitly-nulled field into the *text* `null` — a string setting holding the word,
        // and a list setting holding one item of it.
        serde_json::Value::Null => {}
        serde_json::Value::Object(map) => {
            for (key, inner) in map {
                flatten_json(joined(&prefix, key), inner, names_a_setting, out);
            }
        }
        serde_json::Value::Array(_) => out.push((prefix, Read::Shaped(table_json(value)))),
        scalar => out.push((prefix, Read::Text(scalar_json(scalar)))),
    }
}

/// A JSON object as a [`Value`], for a setting declared to hold a table.
#[cfg(feature = "json")]
fn table_json(value: &serde_json::Value) -> Value {
    match value {
        // Same rule inside a declared table or list: a null entry is one that is not there.
        serde_json::Value::Object(map) => Value::Map(
            map.iter()
                .filter(|(_, inner)| !inner.is_null())
                .map(|(key, inner)| (key.clone(), table_json(inner)))
                .collect(),
        ),
        serde_json::Value::Array(items) => Value::List(
            items
                .iter()
                .filter(|item| !item.is_null())
                .map(table_json)
                .collect(),
        ),
        serde_json::Value::Bool(b) => Value::Bool(*b),
        serde_json::Value::Number(n) => match n.as_i64() {
            Some(i) => Value::Int(i),
            None => Value::Float(n.as_f64().unwrap_or_default()),
        },
        other => Value::String(scalar_json(other)),
    }
}

#[cfg(feature = "json")]
fn scalar_json(value: &serde_json::Value) -> String {
    match value {
        serde_json::Value::String(s) => s.clone(),
        other => other.to_string(),
    }
}

/// The key YAML uses to mean "and everything from there".
#[cfg(feature = "yaml")]
const MERGE_KEY: &str = "<<";

#[cfg(feature = "yaml")]
fn flatten_yaml(
    prefix: String,
    value: &yaml_serde::Value,
    names_a_setting: &dyn Fn(&str) -> bool,
    out: &mut Vec<(String, Read)>,
) -> Result<(), String> {
    match untagged(value) {
        // A mapping the spec declared as a setting is the value, not the way to it.
        yaml_serde::Value::Mapping(_) if !prefix.is_empty() && names_a_setting(&prefix) => {
            let table = table_yaml(value, &prefix)?;
            out.push((prefix, Read::Shaped(table)));
        }
        // The same rule as JSON, and reached by more spellings: `key:`, `key: ~` and
        // `key: null` all say there is no value, so the key is not there. Read as text they
        // stored the *word* — a string setting holding `null`, a list setting holding one item
        // of it — and an explicitly-emptied field set the setting rather than leaving it alone.
        yaml_serde::Value::Null => {}
        yaml_serde::Value::Mapping(map) => {
            for (key, inner) in yaml_entries(map, &prefix)? {
                flatten_yaml(joined(&prefix, &key), inner, names_a_setting, out)?;
            }
        }
        yaml_serde::Value::Sequence(_) => {
            let list = table_yaml(value, &prefix)?;
            out.push((prefix, Read::Shaped(list)));
        }
        scalar => out.push((prefix, Read::Text(scalar_yaml(scalar)))),
    }
    Ok(())
}

/// A YAML mapping as a [`Value`], for a setting declared to hold a table.
#[cfg(feature = "yaml")]
fn table_yaml(value: &yaml_serde::Value, at: &str) -> Result<Value, String> {
    Ok(match untagged(value) {
        // Same rule inside a declared table or list as outside it: a null entry is one that is
        // not there.
        yaml_serde::Value::Mapping(map) => Value::Map(
            yaml_entries(map, at)?
                .into_iter()
                .filter(|(_, inner)| !inner.is_null())
                .map(|(key, inner)| table_yaml(inner, at).map(|inner| (key, inner)))
                .collect::<Result<_, _>>()?,
        ),
        yaml_serde::Value::Sequence(items) => Value::List(
            items
                .iter()
                .filter(|item| !item.is_null())
                .map(|item| table_yaml(item, at))
                .collect::<Result<_, _>>()?,
        ),
        yaml_serde::Value::Bool(b) => Value::Bool(*b),
        yaml_serde::Value::Number(n) => match n.as_i64() {
            Some(i) => Value::Int(i),
            None => Value::Float(n.as_f64().unwrap_or_default()),
        },
        other => Value::String(scalar_yaml(other)),
    })
}

#[cfg(feature = "yaml")]
fn scalar_yaml(value: &yaml_serde::Value) -> String {
    match value {
        yaml_serde::Value::String(s) => s.clone(),
        yaml_serde::Value::Bool(b) => b.to_string(),
        // `1.5`, and also `.inf` and `.nan`, which YAML can write and a `float` setting will
        // refuse — the same answer TOML gives for the same file.
        yaml_serde::Value::Number(n) => n.to_string(),
        // Nothing reaches here: a null is a key that is not there wherever one can appear, and
        // the shaped values are matched above. Spelled out rather than left to a catch-all so
        // that a value holding one reads as the file wrote it.
        _ => "null".to_string(),
    }
}

/// A YAML value with any tags on the way to it stepped through.
///
/// `!Custom 3` is the file naming a type for a reader that has types of its own; this layer's
/// types come from the spec, so a tag changes nothing about what the value is. Read as part of
/// the value it made every tagged setting the wrong type, which is a warning about a file that
/// is perfectly good.
///
/// Only wanted where a value is matched by variant. The parser's own accessors — `as_str`,
/// `as_mapping`, `is_null`, `get` — see through a tag already, which is why they are what the
/// checks around here are written with.
#[cfg(feature = "yaml")]
fn untagged(value: &yaml_serde::Value) -> &yaml_serde::Value {
    let mut value = value;
    while let yaml_serde::Value::Tagged(tagged) = value {
        value = &tagged.value;
    }
    value
}

/// How a message names the mapping a bad key is in.
///
/// A function rather than a string built up front, because it is only ever wanted on the way to
/// an error and every mapping in every file goes through the caller.
#[cfg(feature = "yaml")]
fn describe(at: &str) -> String {
    match at.is_empty() {
        true => "the file".to_string(),
        false => format!("`{at}`"),
    }
}

/// A mapping's entries, keyed by the text a settings key is, with merge keys resolved.
///
/// Two things a YAML file can express that neither TOML nor JSON can, and both have to be
/// answered here rather than left to the flattener:
///
/// - A **key that is not text**. `18: "18.20.0"` is an integer key where the same file written
///   in TOML holds a string, so a scalar key becomes its text and a `map` setting reads the same
///   from either. A sequence or a mapping as a key is not a name at all, and neither is a null:
///   a file keyed by those is not a table of settings, which is the same kind of wrong as a file
///   that will not parse.
/// - A **merge key**. `<<: *defaults` is what anchors exist for, and the parser does not resolve
///   it — an alias becomes the value it points at, but the `<<` stays a key. Left alone it was
///   reported as an unknown setting called `<<` while every setting it carried went unread, which
///   is the loudest way to lose a value in silence. Explicit keys win over merged ones, and an
///   earlier item of `<<: [*a, *b]` wins over a later one, which is what the merge key means.
///
/// `at` is the dotted key the mapping sits under, and is only ever used to say where a bad key
/// is: a file with a stray one somewhere in it is no use to a reader who has to find it.
#[cfg(feature = "yaml")]
fn yaml_entries<'a>(
    mapping: &'a yaml_serde::Mapping,
    at: &str,
) -> Result<Vec<(String, &'a yaml_serde::Value)>, String> {
    let mut entries: Vec<(String, &yaml_serde::Value)> = Vec::new();
    let mut merged: Vec<(String, &yaml_serde::Value)> = Vec::new();
    for (key, value) in mapping {
        // `as_str` sees through a tag, so an application's own tag on the merge key — the only
        // spelling that reaches here as a tag at all, the standard `!!merge` being resolved away
        // by the parser — is still the merge key rather than an unknown setting called `<<`.
        if key.as_str() == Some(MERGE_KEY) {
            for source in merge_sources(untagged(value)).ok_or_else(|| {
                format!(
                    "{} merges `{MERGE_KEY}` from something that is not a mapping",
                    describe(at)
                )
            })? {
                merged.extend(yaml_entries(source, at)?);
            }
            continue;
        }
        entries.push((
            yaml_key(key)
                .ok_or_else(|| format!("{} has a key that is not a name", describe(at)))?,
            value,
        ));
    }
    // Added last and only where nothing has the key yet, which makes an explicit key beat a
    // merged one and the first of several merged ones beat the rest.
    for (key, value) in merged {
        if !entries.iter().any(|(existing, _)| *existing == key) {
            entries.push((key, value));
        }
    }
    Ok(entries)
}

/// The mappings a `<<` merges in, in the order they take precedence.
#[cfg(feature = "yaml")]
fn merge_sources(value: &yaml_serde::Value) -> Option<Vec<&yaml_serde::Mapping>> {
    match value {
        yaml_serde::Value::Mapping(map) => Some(vec![map]),
        yaml_serde::Value::Sequence(items) => items.iter().map(|item| item.as_mapping()).collect(),
        _ => None,
    }
}

/// A mapping key as the name of a setting, when it is one.
#[cfg(feature = "yaml")]
fn yaml_key(key: &yaml_serde::Value) -> Option<String> {
    match untagged(key) {
        yaml_serde::Value::String(s) => Some(s.clone()),
        // A number or a boolean is a name a user plainly meant as one — `18:` under a
        // `map<string, string>`, `true:` in a table of flags — and is the text TOML and JSON
        // would have held for the same file.
        yaml_serde::Value::Bool(b) => Some(b.to_string()),
        yaml_serde::Value::Number(n) => Some(n.to_string()),
        _ => None,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::registry::{PropMeta, Registry};
    use crate::resolve::{resolve, Layers};
    use crate::ty::{Parser, Ty};
    use crate::value::Value;

    static PROPS: &[PropMeta] = &[
        PropMeta::new("jobs", Ty::Uint),
        PropMeta {
            parse: Some(Parser::ListByComma),
            ..PropMeta::new("exclude", Ty::List(&Ty::String))
        },
        PropMeta::new("task.output", Ty::String),
        // A list whose *text* form is colon-separated, which is what made joining arrays with a
        // comma and re-splitting them wrong rather than merely lossy.
        PropMeta {
            parse: Some(Parser::ListByColon),
            ..PropMeta::new("path", Ty::List(&Ty::String))
        },
        // And one with no named parser at all.
        PropMeta::new("plain_list", Ty::List(&Ty::String)),
        // A type usage cannot know — a union, or one only the tool understands. A table under it
        // is the value, because the spec has said as much.
        PropMeta::new("either", Ty::Any),
        PropMeta {
            scope: crate::registry::Scope::Global,
            ..PropMeta::new("trusted", Ty::Bool)
        },
        // A setting that *is* a table, which is the case flattening used to walk straight
        // through.
        PropMeta {
            merge: crate::registry::Merge::Deep,
            ..PropMeta::new("url_replacements", Ty::Map(&Ty::String))
        },
    ];
    const REGISTRY: Registry = Registry::new(PROPS);

    /// A directory tree, cleaned up when the test ends.
    struct Tree(PathBuf);

    impl Tree {
        fn new(name: &str) -> Self {
            let dir = std::env::temp_dir()
                .join(format!("usage_config_files_{}_{name}", std::process::id()));
            let _ = std::fs::remove_dir_all(&dir);
            std::fs::create_dir_all(&dir).expect("temp dir");
            Self(dir)
        }

        fn write(&self, rel: &str, text: &str) -> PathBuf {
            let path = self.0.join(rel);
            if let Some(parent) = path.parent() {
                std::fs::create_dir_all(parent).expect("parent");
            }
            std::fs::write(&path, text).expect("write");
            path
        }
    }

    impl Drop for Tree {
        fn drop(&mut self) {
            let _ = std::fs::remove_dir_all(&self.0);
        }
    }

    #[cfg(feature = "toml")]
    #[test]
    fn a_file_supplies_values_the_spec_understands() {
        let tree = Tree::new("basic");
        let path = tree.write(
            "hk.toml",
            "jobs = 4\nexclude = [\"target\", \"vendor\"]\n\n[task]\noutput = \"prefix\"\n",
        );
        let layer = FileLayer::at(&path, FileScope::Project);
        let resolved = resolve(REGISTRY, Layers::new().then(&layer)).expect("should resolve");

        assert_eq!(resolved.get_key("jobs"), Some(&Value::Int(4)));
        // A nested table is a dotted key, which is what the registry is keyed by.
        assert_eq!(
            resolved.get_key("task.output"),
            Some(&Value::from("prefix"))
        );
        // And a list arrives as the declared parser would have read it, so the same setting
        // means the same thing from a file and from the environment.
        assert_eq!(
            resolved.get_key("exclude"),
            Some(&Value::List(vec![
                Value::from("target"),
                Value::from("vendor")
            ]))
        );
        // The origin names the file *and* the key inside it, which is what makes `explain`
        // worth reading.
        let origin = resolved.origin_key("jobs").unwrap();
        assert!(origin.describe().ends_with("hk.toml#jobs"), "{origin:?}");
    }

    #[cfg(feature = "toml")]
    #[test]
    fn a_setting_that_is_a_table_arrives_as_one() {
        // Flattening walked straight through a `map`, turning its entries into dotted keys
        // nobody had declared — so the setting was never set and every key inside it was
        // reported as unknown. Which tables are settings is the registry's answer.
        let tree = Tree::new("maps");
        let path = tree.write(
            "hk.toml",
            "[url_replacements]\n\"https://a\" = \"https://b\"\n\"https://c\" = \"https://d\"\n",
        );
        let layer = FileLayer::at(&path, FileScope::Project);
        let resolved = resolve(REGISTRY, Layers::new().then(&layer)).expect("should resolve");
        assert_eq!(
            resolved.get_key("url_replacements"),
            Some(&Value::Map(
                [
                    ("https://a".to_string(), Value::from("https://b")),
                    ("https://c".to_string(), Value::from("https://d")),
                ]
                .into_iter()
                .collect()
            ))
        );
        assert!(resolved.warnings.is_empty(), "{:?}", resolved.warnings);

        // And being a table does not stop it merging key by key, which is what `deep` is for.
        let higher = tree.write(
            "higher.toml",
            "[url_replacements]\n\"https://a\" = \"https://z\"\n",
        );
        let lower = FileLayer::at(&path, FileScope::System);
        let upper = FileLayer::at(&higher, FileScope::Project);
        let resolved =
            resolve(REGISTRY, Layers::new().then(&upper).then(&lower)).expect("should resolve");
        let Some(Value::Map(merged)) = resolved.get_key("url_replacements") else {
            panic!(
                "expected a table: {:?}",
                resolved.get_key("url_replacements")
            );
        };
        assert_eq!(merged.get("https://a"), Some(&Value::from("https://z")));
        assert_eq!(merged.get("https://c"), Some(&Value::from("https://d")));
    }

    #[cfg(feature = "toml")]
    #[test]
    fn an_array_keeps_the_boundaries_the_file_gave_it() {
        // Arrays used to be joined with a comma and handed to the setting's named parser, which
        // was lossy in two ways that only appear alongside the spec: an item *containing* a comma
        // came apart, and a list declaring any other separator — or none — got one item holding
        // the joined text. A file has structure; the named parsers are for sources that do not.
        let tree = Tree::new("arrays");
        let path = tree.write(
            "hk.toml",
            "exclude = [\"a,b\", \"c\"]\npath = [\"/bin\", \"/usr/bin\"]\nplain_list = [\"one\", \"two\"]\n",
        );
        let layer = FileLayer::at(&path, FileScope::Project);
        let resolved = resolve(REGISTRY, Layers::new().then(&layer)).expect("should resolve");

        assert_eq!(
            resolved.get_key("exclude"),
            Some(&Value::List(vec![Value::from("a,b"), Value::from("c")])),
            "an item containing the separator survives"
        );
        assert_eq!(
            resolved.get_key("path"),
            Some(&Value::List(vec![
                Value::from("/bin"),
                Value::from("/usr/bin")
            ])),
            "a colon-parsed list is not re-split on commas"
        );
        assert_eq!(
            resolved.get_key("plain_list"),
            Some(&Value::List(vec![Value::from("one"), Value::from("two")])),
            "a list with no named parser is still a list"
        );

        // The *text* form still goes through the declared parser, which is the other half of the
        // rule: a string in a file means what an environment variable would mean.
        let path = tree.write("text.toml", "path = \"/bin:/usr/bin\"\n");
        let layer = FileLayer::at(&path, FileScope::Project);
        let resolved = resolve(REGISTRY, Layers::new().then(&layer)).expect("should resolve");
        assert_eq!(
            resolved.get_key("path"),
            Some(&Value::List(vec![
                Value::from("/bin"),
                Value::from("/usr/bin")
            ]))
        );
    }

    #[cfg(feature = "toml")]
    #[test]
    fn a_table_under_a_type_usage_cannot_know_is_still_the_value() {
        // `Any` is what a union or a tool-private type becomes: the spec has said usage cannot
        // know what belongs there, so a table under it is the value. Walking in reported every
        // key inside as unknown and left the setting unset.
        let tree = Tree::new("any");
        let path = tree.write("hk.toml", "[either]\nnested = \"yes\"\n");
        let layer = FileLayer::at(&path, FileScope::Project);
        let resolved = resolve(REGISTRY, Layers::new().then(&layer)).expect("should resolve");
        assert_eq!(
            resolved.get_key("either"),
            Some(&Value::Map(
                [("nested".to_string(), Value::from("yes"))]
                    .into_iter()
                    .collect()
            ))
        );
        assert!(resolved.warnings.is_empty(), "{:?}", resolved.warnings);
    }

    #[cfg(feature = "toml")]
    #[test]
    fn a_table_where_a_scalar_was_declared_is_reported() {
        // A scalar type does mean the table is a path to settings — and an *empty* table under
        // one produced no keys at all, so nothing was set and nothing was said. Reading it as
        // the value it is makes the type refuse it, which is the warning the rule promises.
        let tree = Tree::new("emptytable");
        let path = tree.write("hk.toml", "[jobs]\n");
        let layer = FileLayer::at(&path, FileScope::Project);
        let resolved = resolve(REGISTRY, Layers::new().then(&layer)).expect("should resolve");
        assert_eq!(resolved.get_key("jobs"), None);
        assert!(
            resolved
                .warnings
                .iter()
                .any(|w| w.message.contains("jobs expected")),
            "{:?}",
            resolved.warnings
        );
    }

    #[cfg(feature = "toml")]
    #[test]
    fn a_shaped_value_of_the_wrong_type_is_still_a_warning() {
        // The table path skipped `Ty::coerce` entirely, so a number inside a
        // `map<string, string>` was stored rather than reported — the "a wrong type costs a
        // warning" promise broken for the settings that need it most.
        let tree = Tree::new("shaped");
        let path = tree.write(
            "hk.toml",
            "jobs = 2\n[url_replacements]\n\"https://a\" = { nested = true }\n",
        );
        let layer = FileLayer::at(&path, FileScope::Project);
        let resolved = resolve(REGISTRY, Layers::new().then(&layer)).expect("should resolve");
        assert_eq!(resolved.get_key("url_replacements"), None);
        assert!(
            resolved.warnings[0]
                .message
                .contains("url_replacements expected"),
            "{:?}",
            resolved.warnings
        );
        // And only that key: the rest of the file still applies.
        assert_eq!(resolved.get_key("jobs"), Some(&Value::Int(2)));
    }

    #[cfg(feature = "toml")]
    #[test]
    fn a_file_that_cannot_be_read_is_not_treated_as_absent() {
        // `read_to_string` fails for more than a missing file, and this module's own doc says a
        // file that is there and cannot be read is an error. Reading a *directory* is the
        // portable way to prove it: a permission bit means nothing when the tests run as root.
        let tree = Tree::new("unreadable");
        let dir = tree.0.join("hk.toml");
        std::fs::create_dir_all(&dir).expect("a directory where a file should be");
        let layer = FileLayer::at(&dir, FileScope::Project);
        let err = resolve(REGISTRY, Layers::new().then(&layer)).expect_err("should fail");
        assert!(err.to_string().contains("hk.toml"), "{err}");
    }

    #[cfg(feature = "toml")]
    #[test]
    fn a_ceiling_written_differently_is_still_a_ceiling() {
        // The comparison is path equality, so a relative ceiling — or one reached through a
        // symlink — matched nothing and the walk ran to the filesystem root, reading files from
        // above the boundary the caller asked for.
        let tree = Tree::new("relative");
        let deep = tree.0.join("a").join("b");
        std::fs::create_dir_all(&deep).expect("dirs");

        let absolute = FileLayer::find_up("hk.toml", &deep, Some(&tree.0), FileScope::Project);
        // The same directory named through a `..` hop, which is what a caller who joined paths
        // by hand ends up with.
        let indirect = tree.0.join("a").join("..");
        let roundabout = FileLayer::find_up("hk.toml", &deep, Some(&indirect), FileScope::Project);
        assert_eq!(
            roundabout.paths().len(),
            absolute.paths().len(),
            "the walk should stop in the same place: {:?}",
            roundabout.paths()
        );
    }

    #[cfg(feature = "toml")]
    #[test]
    fn a_ceiling_that_is_not_there_and_written_the_long_way_round_is_still_a_boundary() {
        // The two fallbacks compounding: a ceiling that cannot be canonicalized *and* holds a
        // `..`. Absolute is not enough — the `..` stays a component of its own, matches nothing,
        // and the walk stops before its first step.
        let cwd = std::env::current_dir().expect("cwd");
        let root = format!("usage_config_absent_dots_{}", std::process::id());
        let from = cwd.join(&root).join("project").join("src");
        let ceiling = PathBuf::from(&root).join("nowhere").join("..");
        assert!(!from.exists(), "the point is that it is not there");

        let layer = FileLayer::find_up("hk.toml", &from, Some(&ceiling), FileScope::Project);
        assert_eq!(
            layer.paths(),
            [
                cwd.join(&root).join("hk.toml"),
                cwd.join(&root).join("project").join("hk.toml"),
                from.join("hk.toml"),
            ],
            "`{}/nowhere/..` is `{}`",
            root,
            root
        );
    }

    #[cfg(all(unix, feature = "toml"))]
    #[test]
    fn a_directory_that_is_not_there_below_a_link_that_is_still_lands_inside_the_ceiling() {
        // Where the two halves of the normalization meet: the link exists and has to be followed,
        // the directory under it does not exist and so cannot be canonicalized with it. Resolving
        // only what is really there — and putting the rest back on the end — is what leaves both
        // sides comparable. Done lexically throughout, `..` through a link would be wrong; done
        // by the filesystem alone, a path that is not there yet gets no answer at all.
        let tree = Tree::new("link_dir");
        let target = tree.0.join("target");
        std::fs::create_dir_all(&target).expect("dirs");
        let link = tree.0.join("link");
        std::os::unix::fs::symlink(&target, &link).expect("symlink");

        let from = link.join("not-created-yet");
        let layer = FileLayer::find_up("hk.toml", &from, Some(&target), FileScope::Project);
        let real_target = target.canonicalize().expect("exists");
        assert_eq!(
            layer.paths(),
            [
                real_target.join("hk.toml"),
                real_target.join("not-created-yet").join("hk.toml"),
            ],
            "the walk is inside the ceiling, by way of the link"
        );
    }

    #[cfg(all(unix, feature = "toml"))]
    #[test]
    fn a_link_to_a_file_that_is_gone_is_not_the_same_as_no_file() {
        // `read_to_string` reports `NotFound` for a dangling symlink, which is about its target;
        // the link itself is plainly there. Read as absence, a user whose global config is a link
        // into a directory they have since moved would have had it ignored in silence — the one
        // outcome this module's own rule about `NotFound` exists to prevent.
        let tree = Tree::new("dangling");
        let link = tree.0.join("hk.toml");
        std::os::unix::fs::symlink(tree.0.join("gone.toml"), &link).expect("symlink");

        let layer = FileLayer::at(&link, FileScope::Project);
        let err = resolve(REGISTRY, Layers::new().then(&layer)).expect_err("should fail");
        assert!(err.to_string().contains("hk.toml"), "{err}");
        assert!(err.to_string().contains("leads nowhere"), "{err}");

        // And a link that resolves is read like any other file.
        tree.write("gone.toml", "jobs = 6\n");
        let resolved = resolve(REGISTRY, Layers::new().then(&layer)).expect("should resolve");
        assert_eq!(resolved.get_key("jobs"), Some(&Value::Int(6)));
    }

    #[cfg(all(unix, feature = "toml"))]
    #[test]
    fn a_file_under_a_link_that_leads_nowhere_is_reported_too() {
        // `~/.config/hk` linked onto a drive that is no longer mounted. Nothing is at the
        // configured path and nothing is at its parent either, so both questions answer
        // `NotFound` — and the broken link is two steps up, which is where the walk finds it.
        // A directory nobody ever created stays silent, which is the common case and the reason
        // the walk stops at the first thing that does exist.
        let tree = Tree::new("dangling_dir");
        let link = tree.0.join("config");
        std::os::unix::fs::symlink(tree.0.join("not-mounted"), &link).expect("symlink");

        let layer = FileLayer::at(link.join("hk.toml"), FileScope::Project);
        let err = resolve(REGISTRY, Layers::new().then(&layer)).expect_err("should fail");
        assert!(err.to_string().contains("config` leads nowhere"), "{err}");

        let plain = FileLayer::at(
            tree.0.join("never-made").join("hk.toml"),
            FileScope::Project,
        );
        let resolved = resolve(REGISTRY, Layers::new().then(&plain)).expect("should resolve");
        assert!(resolved.warnings.is_empty(), "{:?}", resolved.warnings);
    }

    #[cfg(all(unix, feature = "toml"))]
    #[test]
    fn a_step_back_out_of_a_link_goes_where_the_link_pointed() {
        // The thing string work cannot do: `link/..` is the parent of the link's *target*, not the
        // link's own directory. Resolving the `..` first sent the walk into a tree the user never
        // named — omitting the config that is there, or reading one that should have been out of
        // bounds.
        let tree = Tree::new("link_dots");
        let target = tree.0.join("a").join("target");
        std::fs::create_dir_all(&target).expect("dirs");
        let link = tree.0.join("link");
        std::os::unix::fs::symlink(&target, &link).expect("symlink");

        // `<tree>/link/../x` is `<tree>/a/x`, and `<tree>/x` if the `..` is applied to the link's
        // own name instead — two different trees, only one of which is inside the ceiling.
        let from = link.join("..").join("x");
        let ceiling = tree.0.join("a");
        let layer = FileLayer::find_up("hk.toml", &from, Some(&ceiling), FileScope::Project);
        let real = ceiling.canonicalize().expect("exists");
        assert_eq!(
            layer.paths(),
            [real.join("hk.toml"), real.join("x").join("hk.toml")],
            "the walk should be under {}",
            real.display()
        );
    }

    #[cfg(feature = "toml")]
    #[test]
    fn a_ceiling_that_does_not_exist_yet_is_still_a_boundary() {
        // A directory that is not there cannot be canonicalized, and comparing one normalized
        // side against one relative side made every comparison false: the walk stopped before
        // its first step and the layer read *no* files. A boundary is a boundary whether or not
        // the filesystem has caught up with it.
        let cwd = std::env::current_dir().expect("cwd");
        let root = format!("usage_config_absent_ceiling_{}", std::process::id());
        let from = cwd.join(&root).join("project").join("src");
        let ceiling = PathBuf::from(&root);
        assert!(!from.exists(), "the point is that it is not there");

        let layer = FileLayer::find_up("hk.toml", &from, Some(&ceiling), FileScope::Project);
        assert_eq!(
            layer.paths(),
            [
                cwd.join(&root).join("hk.toml"),
                cwd.join(&root).join("project").join("hk.toml"),
                from.join("hk.toml"),
            ],
            "farthest first, stopping at the ceiling"
        );
    }

    #[cfg(feature = "toml")]
    #[test]
    fn a_missing_file_is_not_a_failure_but_an_unreadable_one_is() {
        let tree = Tree::new("absent");
        // Most directories in a find-up chain have no config file in them.
        let layer = FileLayer::at(tree.0.join("nothing.toml"), FileScope::Project);
        let resolved = resolve(REGISTRY, Layers::new().then(&layer)).expect("should resolve");
        assert_eq!(resolved.get_key("jobs"), None);
        assert!(resolved.warnings.is_empty());

        // A file that exists and cannot be parsed is different in kind: the values the user
        // believes are in effect are not, and carrying on as though they had written nothing
        // is worse than saying so.
        let path = tree.write("broken.toml", "jobs = \n");
        let layer = FileLayer::at(&path, FileScope::Project);
        let err = resolve(REGISTRY, Layers::new().then(&layer)).expect_err("should fail");
        assert!(err.to_string().contains("broken.toml"), "{err}");
    }

    #[cfg(feature = "toml")]
    #[test]
    fn the_nearest_file_in_a_find_up_chain_wins() {
        // The bug every hand-written walk makes, and it is invisible until two files disagree.
        let tree = Tree::new("findup");
        tree.write("hk.toml", "jobs = 1\n");
        let deep = tree.0.join("a").join("b");
        std::fs::create_dir_all(&deep).expect("dirs");
        tree.write("a/hk.toml", "jobs = 2\n");
        tree.write("a/b/hk.toml", "jobs = 3\n");

        let layer = FileLayer::find_up("hk.toml", &deep, Some(&tree.0), FileScope::Project);
        assert_eq!(layer.paths().len(), 3, "root, a, a/b");
        let resolved = resolve(REGISTRY, Layers::new().then(&layer)).expect("should resolve");
        assert_eq!(
            resolved.get_key("jobs"),
            Some(&Value::Int(3)),
            "the nearest"
        );

        // Every file that contributed is recorded, farthest first — the order they were read.
        let contributors: Vec<String> = resolved
            .contributors_key("jobs")
            .iter()
            .map(|o| o.describe().to_string())
            .collect();
        assert_eq!(contributors.len(), 3, "{contributors:?}");
        assert!(contributors[0].contains("hk.toml"));
        assert!(
            contributors[2].contains("b"),
            "the nearest is last: {contributors:?}"
        );
    }

    #[cfg(feature = "toml")]
    #[test]
    fn the_ceiling_stops_the_walk() {
        // How a CLI avoids reading a stranger's file from above a checkout. Without it the walk
        // runs to the filesystem root, which on a shared machine is somebody else's business.
        let tree = Tree::new("ceiling");
        let deep = tree.0.join("a").join("b");
        std::fs::create_dir_all(&deep).expect("dirs");

        let bounded = FileLayer::find_up("hk.toml", &deep, Some(&tree.0), FileScope::Project);
        assert_eq!(bounded.paths().len(), 3);
        let unbounded = FileLayer::find_up("hk.toml", &deep, None, FileScope::Project);
        assert!(
            unbounded.paths().len() > 3,
            "without a ceiling the walk reaches the root: {:?}",
            unbounded.paths()
        );
    }

    #[cfg(feature = "toml")]
    #[test]
    fn settings_can_live_in_a_table_of_their_own() {
        // mise's file holds tools and tasks beside its `[settings]`; hk's settings are at the
        // top level. Without this a CLI would have to pre-extract the table, which means
        // parsing the file twice or teaching this layer the rest of the format.
        let tree = Tree::new("prefix");
        let path = tree.write(
            "mise.toml",
            "[tools]\nnode = \"20\"\n\n[settings]\njobs = 8\n\n[settings.task]\noutput = \"interleave\"\n",
        );
        let layer = FileLayer::at(&path, FileScope::Project).under("settings");
        let resolved = resolve(REGISTRY, Layers::new().then(&layer)).expect("should resolve");
        assert_eq!(resolved.get_key("jobs"), Some(&Value::Int(8)));
        assert_eq!(
            resolved.get_key("task.output"),
            Some(&Value::from("interleave"))
        );
        // `[tools]` is not a setting, and must not be reported as an unknown one: it is not
        // under the table this layer was pointed at.
        assert!(resolved.warnings.is_empty(), "{:?}", resolved.warnings);

        // A file with no settings table at all is not a broken file.
        let path = tree.write("empty.toml", "[tools]\nnode = \"20\"\n");
        let layer = FileLayer::at(&path, FileScope::Project).under("settings");
        let resolved = resolve(REGISTRY, Layers::new().then(&layer)).expect("should resolve");
        assert_eq!(resolved.get_key("jobs"), None);

        // A file where that key is present and is *not* a table is a different matter: reading
        // on from there produced an empty key, reported as an unknown setting called nothing,
        // while the settings that were really there went unread.
        let path = tree.write("wrong.toml", "settings = 4\njobs = 9\n");
        let layer = FileLayer::at(&path, FileScope::Project).under("settings");
        let err = resolve(REGISTRY, Layers::new().then(&layer)).expect_err("should fail");
        assert!(err.to_string().contains("should be a table"), "{err}");
        assert!(err.to_string().contains("wrong.toml"), "{err}");
    }

    #[cfg(feature = "toml")]
    #[test]
    fn a_key_nobody_knows_is_a_warning_and_the_rest_of_the_file_still_applies() {
        // A config file written for a newer binary read by an older one. Refusing the file
        // would make an upgrade a coin toss; ignoring the key in silence would make a typo
        // impossible to find.
        let tree = Tree::new("unknown");
        let path = tree.write("hk.toml", "jobs = 2\nfrom_the_future = true\n");
        let layer = FileLayer::at(&path, FileScope::Project);
        let resolved = resolve(REGISTRY, Layers::new().then(&layer)).expect("should resolve");
        assert_eq!(resolved.get_key("jobs"), Some(&Value::Int(2)));
        assert_eq!(resolved.warnings.len(), 1, "{:?}", resolved.warnings);
        assert!(
            resolved.warnings[0].message.contains("from_the_future"),
            "{:?}",
            resolved.warnings[0]
        );
    }

    #[cfg(feature = "toml")]
    #[test]
    fn a_value_of_the_wrong_type_costs_only_its_own_key() {
        // A typo in a system-wide file must not stop the CLI from starting for every user on
        // the machine.
        let tree = Tree::new("badvalue");
        let path = tree.write("hk.toml", "jobs = \"lots\"\n[task]\noutput = \"prefix\"\n");
        let layer = FileLayer::at(&path, FileScope::System);
        let resolved = resolve(REGISTRY, Layers::new().then(&layer)).expect("should resolve");
        assert_eq!(resolved.get_key("jobs"), None);
        assert_eq!(
            resolved.get_key("task.output"),
            Some(&Value::from("prefix")),
            "the rest of the file still applies"
        );
        assert!(
            resolved.warnings[0].message.contains("jobs expected"),
            "{:?}",
            resolved.warnings[0]
        );
    }

    #[cfg(feature = "toml")]
    #[test]
    fn a_project_file_cannot_set_what_the_spec_says_it_cannot() {
        // The scope is why `FileLayer` demands one rather than defaulting: a layer that forgot
        // to say where it read from would be trusted as the operator's by accident, and that
        // is the check `scope="global"` exists for.
        let tree = Tree::new("scope");
        let path = tree.write("hk.toml", "trusted = true\n");

        let project = FileLayer::at(&path, FileScope::Project);
        let resolved = resolve(REGISTRY, Layers::new().then(&project)).expect("should resolve");
        assert_eq!(resolved.get_key("trusted"), None);
        assert!(resolved.warnings[0]
            .message
            .contains("trusted cannot be set"));

        // The same file read as the user's own is accepted, which is what makes the refusal
        // above about trust rather than about files.
        let global = FileLayer::at(&path, FileScope::Global);
        let resolved = resolve(REGISTRY, Layers::new().then(&global)).expect("should resolve");
        assert_eq!(resolved.get_key("trusted"), Some(&Value::Bool(true)));
    }

    #[cfg(feature = "toml")]
    #[test]
    fn text_can_be_transformed_before_it_is_parsed() {
        // Where mise's tera templating goes, without this crate learning what a template is.
        let tree = Tree::new("preprocess");
        let path = tree.write("hk.toml", "jobs = {{ cores }}\n");
        let layer = FileLayer::at(&path, FileScope::Project)
            .preprocess(|text| Ok(text.replace("{{ cores }}", "6")));
        let resolved = resolve(REGISTRY, Layers::new().then(&layer)).expect("should resolve");
        assert_eq!(resolved.get_key("jobs"), Some(&Value::Int(6)));

        // A template that will not render is a read failure naming the file, not something to
        // carry on past — the values the user believes are in effect are not.
        let layer = FileLayer::at(&path, FileScope::Project)
            .preprocess(|_| Err("no such variable `cores`".to_string()));
        let err = resolve(REGISTRY, Layers::new().then(&layer)).expect_err("should fail");
        assert!(err.to_string().contains("no such variable"), "{err}");
        assert!(err.to_string().contains("hk.toml"), "{err}");
    }

    #[cfg(feature = "json")]
    #[test]
    fn a_json_null_is_a_key_that_is_not_there() {
        // `null` is how a JSON file says "no value". Reading it as text stored the *word*: a
        // string setting held `null`, and a list setting held one item of it — so an optional
        // field explicitly nulled set the setting rather than leaving it alone.
        let tree = Tree::new("nulls");
        let path = tree.write(
            "hk.json",
            "{\"task\": {\"output\": null}, \"plain_list\": null, \"jobs\": 3, \"url_replacements\": {\"a\": null, \"b\": \"c\"}}",
        );
        let layer = FileLayer::at(&path, FileScope::Project);
        let resolved = resolve(REGISTRY, Layers::new().then(&layer)).expect("should resolve");

        assert_eq!(resolved.get_key("task.output"), None);
        assert_eq!(resolved.get_key("plain_list"), None);
        assert!(resolved.warnings.is_empty(), "{:?}", resolved.warnings);
        // The rest of the file still applies, and a null *inside* a declared table is a key that
        // is not there either.
        assert_eq!(resolved.get_key("jobs"), Some(&Value::Int(3)));
        assert_eq!(
            resolved.get_key("url_replacements"),
            Some(&Value::Map(
                [("b".to_string(), Value::from("c"))].into_iter().collect()
            ))
        );

        // And the same rule where the settings live in a table of their own: a file may leave
        // that table out, so the null spelling of leaving it out cannot be the one that fails
        // the read.
        let path = tree.write(
            "mise.json",
            "{\"tools\": {\"node\": \"20\"}, \"settings\": null}",
        );
        let layer = FileLayer::at(&path, FileScope::Project).under("settings");
        let resolved = resolve(REGISTRY, Layers::new().then(&layer)).expect("should resolve");
        assert_eq!(resolved.get_key("jobs"), None);
        assert!(resolved.warnings.is_empty(), "{:?}", resolved.warnings);
    }

    #[cfg(feature = "json")]
    #[test]
    fn a_json_file_that_is_not_a_table_of_settings_says_so() {
        // TOML cannot get here — its root is always a table — so this is the shape of wrongness
        // only JSON has. Flattening a list or a bare scalar root pushed an entry keyed by the
        // empty string, which came out as ``unknown setting ` ` ``: a warning naming nothing,
        // for a file that is not a settings file at all.
        let tree = Tree::new("json_root");

        for (name, text) in [("list.json", "[1, 2]"), ("scalar.json", "\"text\"")] {
            let path = tree.write(name, text);
            let layer = FileLayer::at(&path, FileScope::Project);
            let err = resolve(REGISTRY, Layers::new().then(&layer)).expect_err("should fail");
            assert!(err.to_string().contains("should be a table"), "{err}");
            assert!(err.to_string().contains(name), "{err}");
        }

        // `null`, though, is a file that says nothing rather than a file that is wrong.
        let path = tree.write("null.json", "null");
        let layer = FileLayer::at(&path, FileScope::Project);
        let resolved = resolve(REGISTRY, Layers::new().then(&layer)).expect("should resolve");
        assert_eq!(resolved.get_key("jobs"), None);
        assert!(resolved.warnings.is_empty(), "{:?}", resolved.warnings);

        // The same file is the same wrong file when the layer reads a table within it. Asking
        // `get("settings")` about a list root answers `None` — the same answer as a file with no
        // settings table in it — so the shape that fails one way round was ignored the other.
        let path = tree.write("under.json", "[1, 2]");
        let layer = FileLayer::at(&path, FileScope::Project).under("settings");
        let err = resolve(REGISTRY, Layers::new().then(&layer)).expect_err("should fail");
        assert!(err.to_string().contains("should be a table"), "{err}");
        assert!(err.to_string().contains("under.json"), "{err}");
    }

    #[cfg(feature = "json")]
    #[test]
    fn json_reads_the_same_settings_as_toml() {
        // Two formats, one meaning: the flattening and the coercion are shared, so a CLI that
        // switches formats does not change what its settings mean.
        let tree = Tree::new("json");
        let path = tree.write(
            "hk.json",
            "{\"jobs\": 4, \"exclude\": [\"target\"], \"task\": {\"output\": \"prefix\"}}",
        );
        let layer = FileLayer::at(&path, FileScope::Project);
        let resolved = resolve(REGISTRY, Layers::new().then(&layer)).expect("should resolve");
        assert_eq!(resolved.get_key("jobs"), Some(&Value::Int(4)));
        assert_eq!(
            resolved.get_key("task.output"),
            Some(&Value::from("prefix"))
        );
        assert_eq!(
            resolved.get_key("exclude"),
            Some(&Value::List(vec![Value::from("target")]))
        );
    }

    #[cfg(feature = "yaml")]
    #[test]
    fn yaml_reads_the_same_settings_as_toml() {
        // Three formats, one meaning: the flattening and the coercion are shared, so a CLI that
        // switches formats does not change what its settings mean.
        let tree = Tree::new("yaml");
        let path = tree.write(
            "hk.yaml",
            "jobs: 4\nexclude:\n  - a,b\n  - c\ntask:\n  output: prefix\n",
        );
        let layer = FileLayer::at(&path, FileScope::Project);
        let resolved = resolve(REGISTRY, Layers::new().then(&layer)).expect("should resolve");
        assert_eq!(resolved.get_key("jobs"), Some(&Value::Int(4)));
        assert_eq!(
            resolved.get_key("task.output"),
            Some(&Value::from("prefix"))
        );
        // A sequence keeps the boundaries the file gave it, comma inside an item and all.
        assert_eq!(
            resolved.get_key("exclude"),
            Some(&Value::List(vec![Value::from("a,b"), Value::from("c")]))
        );
        let origin = resolved.origin_key("jobs").unwrap();
        assert!(origin.describe().ends_with("hk.yaml#jobs"), "{origin:?}");

        // `.yml` is the same format, and a settings table within the file works the same way.
        let path = tree.write("mise.yml", "tools:\n  node: '20'\nsettings:\n  jobs: 9\n");
        let layer = FileLayer::at(&path, FileScope::Project).under("settings");
        let resolved = resolve(REGISTRY, Layers::new().then(&layer)).expect("should resolve");
        assert_eq!(resolved.get_key("jobs"), Some(&Value::Int(9)));
        assert!(resolved.warnings.is_empty(), "{:?}", resolved.warnings);
    }

    #[cfg(feature = "yaml")]
    #[test]
    fn a_yaml_file_that_says_nothing_is_not_a_file_that_is_wrong() {
        // An empty file is what `touch` leaves and what a user who deleted every line has, and a
        // `---` with nothing under it is a document that says nothing. None of the three is a
        // file to refuse to start over.
        let tree = Tree::new("yaml_empty");
        for (name, text) in [
            ("empty.yaml", ""),
            ("comments.yaml", "# nothing to see\n"),
            ("marker.yaml", "---\n"),
            ("null.yaml", "null\n"),
        ] {
            let path = tree.write(name, text);
            let layer = FileLayer::at(&path, FileScope::Project);
            let resolved = resolve(REGISTRY, Layers::new().then(&layer)).expect(name);
            assert_eq!(resolved.get_key("jobs"), None, "{name}");
            assert!(
                resolved.warnings.is_empty(),
                "{name}: {:?}",
                resolved.warnings
            );
        }
    }

    #[cfg(feature = "yaml")]
    #[test]
    fn a_yaml_file_that_is_not_a_table_of_settings_says_so() {
        // The shapes only a non-TOML format can hold, and the same answer JSON gives them.
        let tree = Tree::new("yaml_root");
        for (name, text) in [("list.yaml", "- 1\n- 2\n"), ("scalar.yaml", "text\n")] {
            let path = tree.write(name, text);
            let layer = FileLayer::at(&path, FileScope::Project);
            let err = resolve(REGISTRY, Layers::new().then(&layer)).expect_err("should fail");
            assert!(err.to_string().contains("should be a table"), "{err}");
            assert!(err.to_string().contains(name), "{err}");
        }

        // And a settings table that is there and is something else.
        let path = tree.write("under.yaml", "settings: 4\n");
        let layer = FileLayer::at(&path, FileScope::Project).under("settings");
        let err = resolve(REGISTRY, Layers::new().then(&layer)).expect_err("should fail");
        assert!(
            err.to_string().contains("`settings` should be a table"),
            "{err}"
        );

        // A file with no settings table in it, though, is a file that says nothing.
        let path = tree.write("other.yaml", "tools:\n  node: '20'\n");
        let layer = FileLayer::at(&path, FileScope::Project).under("settings");
        let resolved = resolve(REGISTRY, Layers::new().then(&layer)).expect("should resolve");
        assert_eq!(resolved.get_key("jobs"), None);
        assert!(resolved.warnings.is_empty(), "{:?}", resolved.warnings);
    }

    #[cfg(feature = "yaml")]
    #[test]
    fn more_than_one_yaml_document_is_reported_as_the_file_it_is() {
        // The parser refuses this, in words about deserializing rather than about the file. A
        // user with a stray `---` in their config is being told to remove it, so that is what
        // the message says.
        let tree = Tree::new("yaml_docs");
        let path = tree.write("hk.yaml", "jobs: 4\n---\njobs: 8\n");
        let layer = FileLayer::at(&path, FileScope::Project);
        let err = resolve(REGISTRY, Layers::new().then(&layer)).expect_err("should fail");
        assert!(err.to_string().contains("one document"), "{err}");
        assert!(err.to_string().contains("hk.yaml"), "{err}");
    }

    #[cfg(feature = "yaml")]
    #[test]
    fn a_yaml_null_is_a_key_that_is_not_there() {
        // Three spellings of it, where JSON has one: an empty value, a `~` and the word.
        let tree = Tree::new("yaml_nulls");
        let path = tree.write(
            "hk.yaml",
            "jobs: 3\ntask:\n  output:\nplain_list: ~\nurl_replacements:\n  a: null\n  b: c\n",
        );
        let layer = FileLayer::at(&path, FileScope::Project);
        let resolved = resolve(REGISTRY, Layers::new().then(&layer)).expect("should resolve");

        assert_eq!(resolved.get_key("task.output"), None);
        assert_eq!(resolved.get_key("plain_list"), None);
        assert!(resolved.warnings.is_empty(), "{:?}", resolved.warnings);
        // The rest of the file still applies, and a null inside a declared table is a key that
        // is not there either.
        assert_eq!(resolved.get_key("jobs"), Some(&Value::Int(3)));
        assert_eq!(
            resolved.get_key("url_replacements"),
            Some(&Value::Map(
                [("b".to_string(), Value::from("c"))].into_iter().collect()
            ))
        );
    }

    #[cfg(feature = "yaml")]
    #[test]
    fn a_yaml_key_that_is_a_number_is_the_name_it_looks_like() {
        // `18:` in a `map<string, string>` is a key a user plainly meant as a name, and is the
        // string the same file in TOML or JSON would hold. Left as the integer YAML made of it,
        // the map could not be built at all.
        let tree = Tree::new("yaml_keys");
        let path = tree.write(
            "hk.yaml",
            "url_replacements:\n  18: eighteen\n  true: yes-really\n",
        );
        let layer = FileLayer::at(&path, FileScope::Project);
        let resolved = resolve(REGISTRY, Layers::new().then(&layer)).expect("should resolve");
        assert_eq!(
            resolved.get_key("url_replacements"),
            Some(&Value::Map(
                [
                    ("18".to_string(), Value::from("eighteen")),
                    ("true".to_string(), Value::from("yes-really")),
                ]
                .into_iter()
                .collect()
            ))
        );

        // A sequence or a mapping as a key is not a name, and a file keyed by those is not a
        // table of settings. The message says where, because a stray one is the sort of thing
        // that is hard to find by eye.
        let path = tree.write("complex.yaml", "task:\n  ? [a, b]\n  : value\n");
        let layer = FileLayer::at(&path, FileScope::Project);
        let err = resolve(REGISTRY, Layers::new().then(&layer)).expect_err("should fail");
        assert!(err.to_string().contains("`task` has a key"), "{err}");
        assert!(err.to_string().contains("complex.yaml"), "{err}");
    }

    #[cfg(feature = "yaml")]
    #[test]
    fn a_merge_key_brings_the_settings_it_points_at() {
        // The parser resolves the alias and leaves the `<<`, so without this the whole merge
        // arrived as an unknown setting called `<<` and every setting in it went unread.
        let tree = Tree::new("yaml_merge");
        let path = tree.write(
            "hk.yaml",
            "defaults: &defaults\n  jobs: 4\n  task:\n    output: prefix\nsettings:\n  <<: *defaults\n  jobs: 8\n",
        );
        let layer = FileLayer::at(&path, FileScope::Project).under("settings");
        let resolved = resolve(REGISTRY, Layers::new().then(&layer)).expect("should resolve");
        // Explicit wins over merged...
        assert_eq!(resolved.get_key("jobs"), Some(&Value::Int(8)));
        // ...and what only the merge supplies is there, nested keys and all.
        assert_eq!(
            resolved.get_key("task.output"),
            Some(&Value::from("prefix"))
        );
        assert!(resolved.warnings.is_empty(), "{:?}", resolved.warnings);

        // Several at once: the earlier item wins, which is what the merge key means.
        let path = tree.write(
            "many.yaml",
            "a: &a\n  jobs: 1\nb: &b\n  jobs: 2\n  trusted: true\nsettings:\n  <<: [*a, *b]\n",
        );
        let layer = FileLayer::at(&path, FileScope::System).under("settings");
        let resolved = resolve(REGISTRY, Layers::new().then(&layer)).expect("should resolve");
        assert_eq!(resolved.get_key("jobs"), Some(&Value::Int(1)));
        assert_eq!(resolved.get_key("trusted"), Some(&Value::Bool(true)));

        // A tag on the way to the merge key does not stop it being one. `!!merge <<` is the
        // spelling the YAML spec blesses and the parser resolves it away, so a tag only reaches
        // the merge check when it is an application's own — and that check is the one place
        // around here that asks about a key by its text rather than by its shape.
        let path = tree.write(
            "tagged.yaml",
            "a: &a\n  jobs: 7\nsettings:\n  !Merge <<: *a\n",
        );
        let layer = FileLayer::at(&path, FileScope::Project).under("settings");
        let resolved = resolve(REGISTRY, Layers::new().then(&layer)).expect("should resolve");
        assert_eq!(resolved.get_key("jobs"), Some(&Value::Int(7)));
        assert!(resolved.warnings.is_empty(), "{:?}", resolved.warnings);

        // And a `<<` that points at something there is nothing to merge from is a broken file,
        // not a setting called `<<`.
        let path = tree.write("bad.yaml", "settings:\n  <<: 4\n");
        let layer = FileLayer::at(&path, FileScope::Project).under("settings");
        let err = resolve(REGISTRY, Layers::new().then(&layer)).expect_err("should fail");
        assert!(err.to_string().contains("not a mapping"), "{err}");
    }

    #[cfg(feature = "yaml")]
    #[test]
    fn a_yaml_tag_does_not_change_what_a_value_is() {
        // A tag names a type for a reader that has types of its own. This one gets its types
        // from the spec, so the tag is stepped through: read as part of the value, every tagged
        // setting was the wrong type and every one of them warned about a good file.
        let tree = Tree::new("yaml_tags");
        let path = tree.write(
            "hk.yaml",
            "jobs: !!int 4\nurl_replacements: !Table\n  a: b\n",
        );
        let layer = FileLayer::at(&path, FileScope::Project);
        let resolved = resolve(REGISTRY, Layers::new().then(&layer)).expect("should resolve");
        assert_eq!(resolved.get_key("jobs"), Some(&Value::Int(4)));
        assert_eq!(
            resolved.get_key("url_replacements"),
            Some(&Value::Map(
                [("a".to_string(), Value::from("b"))].into_iter().collect()
            ))
        );
        assert!(resolved.warnings.is_empty(), "{:?}", resolved.warnings);
    }

    #[cfg(feature = "toml")]
    #[test]
    fn a_format_nothing_can_infer_is_named_rather_than_guessed() {
        // `.myclirc` is TOML in one CLI and YAML in another, and neither is discoverable from
        // the name.
        let tree = Tree::new("format");
        let path = tree.write(".hkrc", "jobs = 5\n");

        let layer = FileLayer::at(&path, FileScope::Project);
        let err = resolve(REGISTRY, Layers::new().then(&layer)).expect_err("should fail");
        assert!(err.to_string().contains("cannot tell what format"), "{err}");

        let layer = FileLayer::at(&path, FileScope::Project).as_format(Format::Toml);
        let resolved = resolve(REGISTRY, Layers::new().then(&layer)).expect("should resolve");
        assert_eq!(resolved.get_key("jobs"), Some(&Value::Int(5)));
    }
}
