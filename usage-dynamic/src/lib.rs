//! Opt-in runtime command catalogs for a derive-generated usage-rs host.
//!
//! Applications discover and cache plugin specs themselves, then attach them below a static
//! command that declares an external-subcommand variant. The catalog builds a separate merged
//! tree for navigable help and deep completion while generic parsing stays in `usage-lib` and
//! the derive-generated parse tables remain unchanged.

use std::borrow::Cow;
use std::collections::{HashMap, HashSet};
use std::ffi::OsString;
use std::fmt;

use usage_argv::complete::{self, Completions, Files, Shell, Split};
use usage_argv::spec::{Candidate, CandidateKind, CommandMeta, SpecView};
use usage_parser::error::UsageErr;
use usage_parser::Parser;

pub use usage_parser::parse::ParseOutput;
pub use usage_parser::Spec;

/// A validated collection of caller-supplied runtime command specs.
#[derive(Debug)]
pub struct Catalog<'a> {
    host: SpecView<'a>,
    entries: Vec<Entry>,
    merged: Spec,
    completion: Spec,
}

#[derive(Debug)]
struct Entry {
    parent: String,
    name: String,
    aliases: Vec<String>,
    spec: Spec,
}

/// Cold-path help and completion over the fully merged runtime tree.
#[derive(Debug, Clone, Copy)]
pub struct App<'a> {
    catalog: &'a Catalog<'a>,
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

    /// Cold-path help, completion, and spec access over the fully merged command tree.
    pub fn app(&self) -> App<'_> {
        App { catalog: self }
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

impl App<'_> {
    /// The fully merged portable spec used for dynamic help and completion.
    pub fn spec(&self) -> &Spec {
        &self.catalog.merged
    }

    /// Render help for a static or dynamic command path. The empty path is the root.
    pub fn help(self, path: &str, long: bool) -> Option<String> {
        let command = find_command(&self.catalog.merged, path)?;
        Some(usage_parser::docs::cli::render_help(
            &self.catalog.merged,
            command,
            long,
        ))
    }

    /// Use this merged tree to answer the usage-rs completion protocol.
    pub const fn completion_app(self) -> Self {
        self
    }

    /// Complete an already split command line without rendering a shell protocol.
    pub fn complete(self, split: &Split) -> Result<Completions<'static>, Error> {
        complete_merged(&self.catalog.completion, split)
    }

    /// Answer a hidden `__complete_word__` invocation, or return `None` for ordinary argv.
    pub async fn completion_request(self, argv: &[OsString]) -> Option<String> {
        let request = CompletionRequest::parse(argv)?;
        let answer = self.complete(&request.split).ok()?;
        Some(complete::render(&answer, request.shell))
    }
}

fn find_command<'a>(spec: &'a Spec, path: &str) -> Option<&'a usage_parser::SpecCommand> {
    let mut command = &spec.cmd;
    for component in path.split_ascii_whitespace() {
        command = command.find_subcommand(component)?;
    }
    Some(command)
}

fn complete_merged(spec: &Spec, split: &Split) -> Result<Completions<'static>, Error> {
    let words: Vec<String> = split.words.iter().take(split.cword).cloned().collect();
    let parsed = usage_parser::parse::parse_partial(spec, &words)
        .map_err(|error| Error::Parse(error.to_string()))?;
    let prefix = &split.prefix;
    let flags_possible = !parsed.double_dash_seen;
    let mut candidates = Vec::new();
    let mut files = None;
    let mut declared_completion = false;

    if let Some(flag) = parsed.flag_awaiting_value.first() {
        if let Some(arg) = &flag.arg {
            candidates.extend(choice_candidates(arg, prefix));
            declared_completion = completion_for(spec, &parsed.cmd, &arg.name).is_some();
            files = completion_files(spec, &parsed.cmd, &arg.name);
        }
    } else if flags_possible && prefix.starts_with('-') {
        for (form, flag) in parsed.completion_flags() {
            if flag.hide || !form.starts_with(prefix) || hidden_flag_form(&form, &flag) {
                continue;
            }
            candidates.push(Candidate {
                value: form,
                kind: CandidateKind::Flag,
                display: None,
                description: flag.help.clone().map(Cow::Owned),
            });
        }
    } else {
        if let Some(arg) = parsed.next_arg.as_deref() {
            candidates.extend(choice_candidates(arg, prefix));
            declared_completion = completion_for(spec, &parsed.cmd, &arg.name).is_some();
            files = completion_files(spec, &parsed.cmd, &arg.name);
        }
        for command in parsed
            .cmd
            .subcommands
            .values()
            .filter(|command| !command.hide)
        {
            for name in std::iter::once(&command.name).chain(&command.aliases) {
                if name.starts_with(prefix) {
                    candidates.push(Candidate {
                        value: name.clone(),
                        kind: CandidateKind::Command,
                        display: None,
                        description: command.help.clone().map(Cow::Owned),
                    });
                }
            }
        }
    }
    candidates.sort_unstable();
    candidates.dedup_by(|a, b| a.value == b.value);
    if candidates.is_empty() && files.is_none() && !declared_completion && !prefix.starts_with('-')
    {
        files = Some(Files::Any);
    }
    Ok(Completions { candidates, files })
}

fn choice_candidates(arg: &usage_parser::SpecArg, prefix: &str) -> Vec<Candidate<'static>> {
    let Some(choices) = &arg.choices else {
        return Vec::new();
    };
    let details: HashMap<_, _> = choices
        .details
        .iter()
        .map(|choice| (choice.value.as_str(), choice))
        .collect();
    let mut candidates = Vec::new();
    for value in &choices.choices {
        let detail = details.get(value.as_str()).copied();
        if detail.is_some_and(|choice| choice.hide) {
            continue;
        }
        for form in std::iter::once(value.as_str()).chain(
            detail
                .into_iter()
                .flat_map(|choice| &choice.aliases)
                .filter(|alias| !alias.hide)
                .map(|alias| alias.value.as_str()),
        ) {
            if form.starts_with(prefix) {
                candidates.push(Candidate {
                    value: form.to_owned(),
                    kind: CandidateKind::Value,
                    display: None,
                    description: detail
                        .and_then(|choice| choice.help.clone())
                        .map(Cow::Owned),
                });
            }
        }
    }
    candidates
}

fn hidden_flag_form(form: &str, flag: &usage_parser::SpecFlag) -> bool {
    form.strip_prefix("--")
        .is_some_and(|long| flag.hidden_aliases.iter().any(|hidden| hidden == long))
        || form
            .strip_prefix('-')
            .filter(|short| short.len() == 1)
            .and_then(|short| short.chars().next())
            .is_some_and(|short| flag.hidden_short_aliases.contains(&short))
}

fn completion_files(spec: &Spec, command: &usage_parser::SpecCommand, name: &str) -> Option<Files> {
    let completion = completion_for(spec, command, name)?;
    let type_ = completion.type_.as_deref()?;
    let (kind, filter) = type_
        .split_once(':')
        .map_or((type_, None), |(kind, filter)| (kind, Some(filter)));
    match kind {
        "path" | "file" => match filter {
            Some(filter) => Some(Files::Extensions(
                filter
                    .split(',')
                    .map(|extension| extension.trim_start_matches('.').to_owned())
                    .collect(),
            )),
            None => Some(Files::Any),
        },
        "dir" | "directory" => Some(Files::Dirs),
        "executable" | "executable_path" => Some(Files::ExecutablePaths),
        "command" => Some(Files::Commands),
        _ => None,
    }
}

fn completion_for<'a>(
    spec: &'a Spec,
    command: &'a usage_parser::SpecCommand,
    name: &str,
) -> Option<&'a usage_parser::SpecComplete> {
    command
        .complete
        .get(name)
        .or_else(|| spec.complete.get(name))
}

struct CompletionRequest {
    shell: Shell,
    split: Split,
}

impl CompletionRequest {
    fn parse(argv: &[OsString]) -> Option<Self> {
        if argv.first()?.to_str()? != "__complete_word__" {
            return None;
        }
        let mut shell = Shell::Bash;
        let mut line = String::new();
        let mut cursor = None;
        let mut words: Option<Vec<String>> = None;
        let mut rest = argv[1..].iter();
        while let Some(arg) = rest.next() {
            match arg.to_str().unwrap_or_default() {
                "--shell" => {
                    shell = rest
                        .next()
                        .and_then(|name| Shell::from_name(&name.to_string_lossy()))
                        .unwrap_or(shell);
                }
                "--line" => {
                    line = rest.next()?.to_string_lossy().into_owned();
                }
                "--cursor" => {
                    cursor = rest.next()?.to_str()?.parse().ok();
                }
                "--words" => {
                    words = Some(
                        rest.map(|word| word.to_string_lossy().into_owned())
                            .collect(),
                    );
                    break;
                }
                _ => {}
            }
        }
        let split = if let Some(mut words) = words {
            if words.is_empty() {
                words.push(String::new());
            }
            let cword = words.len() - 1;
            Split {
                prefix: words[cword].clone(),
                words,
                cword,
            }
        } else {
            complete::split(&line, cursor.unwrap_or(line.len()), shell)
        };
        Some(Self { shell, split })
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
        let mut merged: Spec = self
            .host
            .clone()
            .to_kdl()
            .parse()
            .map_err(|error: UsageErr| Error::HostSpec(error.to_string()))?;
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
            let mut aliases = spec.cmd.aliases.clone();
            for alias in &spec.cmd.hidden_aliases {
                if !aliases.contains(alias) {
                    aliases.push(alias.clone());
                }
            }
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
                static_forms.extend(command.hidden_aliases.iter().copied());
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
            insert_plugin(&mut merged, &parent, &name, &spec)?;
            entries.push(Entry {
                parent,
                name,
                aliases,
                spec,
            });
        }
        // Re-reading recalculates every `full_cmd`, usage line, and command lookup cache after
        // insertion. This is cold construction work and keeps the merged tree internally
        // indistinguishable from a spec that declared the commands in KDL.
        merged = merged
            .to_string()
            .parse()
            .map_err(|error: UsageErr| Error::HostSpec(error.to_string()))?;
        let mut completion = merged.clone();
        clear_mounts(&mut completion.cmd);
        Ok(Catalog {
            host: self.host,
            entries,
            merged,
            completion,
        })
    }
}

fn insert_plugin(merged: &mut Spec, parent: &str, name: &str, plugin: &Spec) -> Result<(), Error> {
    let mut command = plugin.cmd.clone();
    command.name = name.to_owned();
    command.help = command.help.or_else(|| plugin.about.clone());
    command.help_long = command.help_long.or_else(|| plugin.about_long.clone());
    command.before_help = command.before_help.or_else(|| plugin.before_help.clone());
    command.before_help_long = command
        .before_help_long
        .or_else(|| plugin.before_help_long.clone());
    command.after_help = command.after_help.or_else(|| plugin.after_help.clone());
    command.after_help_long = command
        .after_help_long
        .or_else(|| plugin.after_help_long.clone());
    for (key, complete) in &plugin.complete {
        command.complete.insert(key.clone(), complete.clone());
    }
    let mut current = &mut merged.cmd;
    for component in parent.split_ascii_whitespace() {
        current = current
            .subcommands
            .get_mut(component)
            .ok_or_else(|| Error::MissingParent(parent.to_owned()))?;
    }
    current.subcommands.insert(name.to_owned(), command);
    Ok(())
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

fn clear_mounts(command: &mut usage_parser::SpecCommand) {
    command.mounts.clear();
    for child in command.subcommands.values_mut() {
        clear_mounts(child);
    }
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
    HostSpec(String),
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
            Self::HostSpec(error) => write!(f, "could not construct merged host spec: {error}"),
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
