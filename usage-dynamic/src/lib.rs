//! Opt-in runtime command catalogs for a derive-generated usage-rs host.
//!
//! Applications discover and cache plugin specs themselves, then attach them below a static
//! command that declares an external-subcommand variant. The catalog adds summaries to host
//! help and command-name completion while generic parsing stays isolated in `usage-lib`.

use std::collections::{HashMap, HashSet};
use std::ffi::OsString;
use std::fmt;

use usage_argv::spec::{CommandMeta, RuntimeCommand, SpecView};
use usage_parser::error::UsageErr;
use usage_parser::Parser;

pub use usage_parser::parse::ParseOutput;
pub use usage_parser::Spec;

/// A validated collection of caller-supplied runtime command specs.
#[derive(Debug)]
pub struct Catalog<'a> {
    host: SpecView<'a>,
    entries: Vec<Entry>,
}

#[derive(Debug)]
struct Entry {
    parent: String,
    name: String,
    aliases: Vec<String>,
    spec: Spec,
}

/// A catalog under construction.
#[derive(Debug)]
pub struct Builder<'a> {
    host: SpecView<'a>,
    pending: Vec<(String, Spec)>,
}

impl<'a> Catalog<'a> {
    /// Begin a catalog over a derive-generated `Cli::app()`.
    pub fn builder(host: SpecView<'a>) -> Builder<'a> {
        Builder {
            host,
            pending: Vec::new(),
        }
    }

    /// The host presentation view, including dynamic command summaries.
    pub fn app(&self) -> SpecView<'a> {
        self.host
            .clone()
            .runtime_commands(self.entries.iter().map(|entry| {
                let cmd = &entry.spec.cmd;
                RuntimeCommand {
                    parent: entry.parent.clone(),
                    name: entry.name.clone(),
                    aliases: cmd.aliases.clone(),
                    hidden_aliases: cmd.hidden_aliases.clone(),
                    about: cmd.help.clone().or_else(|| entry.spec.about.clone()),
                    long_about: cmd
                        .help_long
                        .clone()
                        .or_else(|| entry.spec.about_long.clone()),
                    help_heading: cmd.help_heading.clone(),
                    hide: cmd.hide,
                    display_order: cmd.display_order,
                }
            }))
    }

    /// Parse argv captured by an existing external-subcommand variant.
    ///
    /// `parent` is a static command path, with the empty string denoting the root. The first
    /// argv token is the external command name. Unknown names return `Ok(None)` so callers may
    /// retain arbitrary fallback dispatch.
    pub fn parse_external(
        &self,
        parent: &str,
        argv: &[OsString],
    ) -> Result<Option<Outcome>, Error> {
        let canonical_parent = canonical_parent(self.host.spec().root, parent)?;
        let Some(invoked) = argv.first() else {
            return Ok(None);
        };
        let invoked = invoked.to_str().ok_or(Error::NonUtf8 { index: 0 })?;
        let Some(entry) = self.entries.iter().find(|entry| {
            entry.parent == canonical_parent
                && (entry.name == invoked || entry.aliases.iter().any(|alias| alias == invoked))
        }) else {
            return Ok(None);
        };
        let mut input = Vec::with_capacity(argv.len());
        for (index, token) in argv.iter().enumerate() {
            input.push(token.to_str().ok_or(Error::NonUtf8 { index })?.to_owned());
        }
        let mut output = Parser::new(&entry.spec)
            .explain(&input)
            .map_err(|error| Error::Parse(error.to_string()))?;
        if let Some(index) = output
            .errors
            .iter()
            .position(|error| matches!(error, UsageErr::Help(_) | UsageErr::Version(_)))
        {
            return Ok(Some(match output.errors.swap_remove(index) {
                UsageErr::Help(page) => Outcome::Help(Help {
                    parent: entry.parent.clone(),
                    name: entry.name.clone(),
                    invoked_as: invoked.to_owned(),
                    page,
                }),
                UsageErr::Version(version) => Outcome::Version(Version {
                    parent: entry.parent.clone(),
                    name: entry.name.clone(),
                    invoked_as: invoked.to_owned(),
                    version,
                }),
                _ => unreachable!(),
            }));
        }
        if !output.errors.is_empty() {
            return Err(Error::InvalidArgv(
                output.errors.iter().map(ToString::to_string).collect(),
            ));
        }
        Ok(Some(Outcome::Parsed(Box::new(Parsed {
            parent: entry.parent.clone(),
            name: entry.name.clone(),
            invoked_as: invoked.to_owned(),
            output,
        }))))
    }
}

impl<'a> Builder<'a> {
    /// Attach a supplied plugin spec beneath the static root.
    pub fn root(mut self, spec: Spec) -> Self {
        self.pending.push((String::new(), spec));
        self
    }

    /// Attach a supplied plugin spec beneath a static command path.
    ///
    /// Components may use visible or hidden static aliases; construction stores the canonical
    /// path. The parent must declare an external-subcommand catch-all.
    pub fn under(mut self, parent: impl Into<String>, spec: Spec) -> Self {
        self.pending.push((parent.into(), spec));
        self
    }

    /// Validate all parents and namespaces and finish the catalog.
    pub fn build(self) -> Result<Catalog<'a>, Error> {
        let host = self.host.spec();
        let mut entries = Vec::with_capacity(self.pending.len());
        let mut claimed: HashMap<String, HashSet<String>> = HashMap::new();
        for (requested_parent, spec) in self.pending {
            reject_mounts(&spec.cmd)?;
            let (parent, parent_meta) = resolve_parent(host.root, &requested_parent)?;
            if !parent_meta.cmd.external_subcommand {
                return Err(Error::ParentNotExternal(parent));
            }
            let name = if spec.name.trim().is_empty() {
                spec.cmd.name.trim().to_owned()
            } else {
                spec.name.trim().to_owned()
            };
            if name.is_empty() {
                return Err(Error::EmptyName);
            }
            let aliases = spec.cmd.aliases.clone();
            let mut forms = Vec::with_capacity(1 + aliases.len());
            forms.push(name.clone());
            forms.extend(aliases.iter().cloned());
            if forms.iter().any(|form| form.is_empty()) {
                return Err(Error::EmptyName);
            }
            let mut static_forms = HashSet::new();
            for command in parent_meta.subcommands {
                static_forms.insert(command.cmd.name);
                static_forms.extend(command.cmd.aliases.iter().copied());
            }
            if !parent_meta.cmd.disable_help_subcommand {
                static_forms.insert("help");
            }
            let parent_claimed = claimed.entry(parent.clone()).or_default();
            for form in &forms {
                if static_forms.contains(form.as_str()) || !parent_claimed.insert(form.clone()) {
                    return Err(Error::Collision {
                        parent,
                        name: form.clone(),
                    });
                }
            }
            entries.push(Entry {
                parent,
                name,
                aliases,
                spec,
            });
        }
        Ok(Catalog {
            host: self.host,
            entries,
        })
    }
}

fn canonical_parent(root: &CommandMeta<'_>, requested: &str) -> Result<String, Error> {
    resolve_parent(root, requested).map(|(path, _)| path)
}

fn resolve_parent<'a>(
    root: &'a CommandMeta<'a>,
    requested: &str,
) -> Result<(String, &'a CommandMeta<'a>), Error> {
    let mut current = root;
    let mut canonical = Vec::new();
    for component in requested.split_ascii_whitespace() {
        let Some(next) = current.subcommands.iter().copied().find(|candidate| {
            candidate.cmd.name == component || candidate.cmd.aliases.contains(&component)
        }) else {
            return Err(Error::MissingParent(requested.to_owned()));
        };
        canonical.push(next.cmd.name);
        current = next;
    }
    Ok((canonical.join(" "), current))
}

fn reject_mounts(command: &usage_parser::SpecCommand) -> Result<(), Error> {
    if !command.mounts.is_empty() {
        return Err(Error::UnresolvedMount(command.name.clone()));
    }
    for child in command.subcommands.values() {
        reject_mounts(child)?;
    }
    Ok(())
}

/// The result of parsing a catalogued external command.
#[derive(Debug)]
#[non_exhaustive]
pub enum Outcome {
    Parsed(Box<Parsed>),
    Help(Help),
    Version(Version),
}

#[derive(Debug)]
pub struct Parsed {
    pub parent: String,
    pub name: String,
    pub invoked_as: String,
    pub output: ParseOutput,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Help {
    pub parent: String,
    pub name: String,
    pub invoked_as: String,
    pub page: String,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Version {
    pub parent: String,
    pub name: String,
    pub invoked_as: String,
    pub version: String,
}

/// A catalog construction or dynamic parse failure.
#[derive(Debug)]
#[non_exhaustive]
pub enum Error {
    MissingParent(String),
    ParentNotExternal(String),
    EmptyName,
    Collision { parent: String, name: String },
    UnresolvedMount(String),
    NonUtf8 { index: usize },
    InvalidArgv(Vec<String>),
    Parse(String),
}

impl fmt::Display for Error {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::MissingParent(parent) => write!(f, "static parent `{parent}` does not exist"),
            Self::ParentNotExternal(parent) => {
                write!(
                    f,
                    "static parent `{parent}` has no external-subcommand catch-all"
                )
            }
            Self::EmptyName => f.write_str("dynamic command name cannot be empty"),
            Self::Collision { parent, name } => {
                write!(
                    f,
                    "dynamic command form `{name}` collides beneath `{parent}`"
                )
            }
            Self::UnresolvedMount(command) => {
                write!(
                    f,
                    "dynamic spec contains an unresolved mount beneath `{command}`"
                )
            }
            Self::NonUtf8 { index } => {
                write!(f, "dynamic argv token {index} is not valid UTF-8")
            }
            Self::InvalidArgv(errors) => f.write_str(&errors.join("\n")),
            Self::Parse(error) => f.write_str(error),
        }
    }
}

impl std::error::Error for Error {}
