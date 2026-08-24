//! Runtime commands for a derive-generated usage-rs host.
//!
//! A CLI that has plugins knows what `host plugin-x` is only after it has read its own configuration,
//! which is long after the tables describing the rest of its CLI were compiled. Those tables
//! stay as they are: an application discovers and caches plugin specs itself, and a [`Catalog`]
//! attaches them beneath a static command that declared an
//! [`external_subcommand`](usage_parser::SpecCommand::external_subcommand) catch-all.
//!
//! What that buys is the three things the static tables cannot answer for a command they have
//! never seen: help that lists and navigates runtime commands, completion that descends into
//! them, and a parse of the argv the catch-all captured. Nothing here mutates the derived parse
//! tables or the KDL they emit, and nothing here runs a subprocess, reads a file, or calls back
//! into the application.

use std::borrow::Cow;
use std::collections::{HashMap, HashSet};
use std::ffi::OsString;
use std::fmt;
use std::sync::OnceLock;

use usage_argv::complete::{self, CompletionOverlay, CompletionRequest, Completions, Files, Split};
use usage_argv::spec::{Candidate, CandidateKind, CommandMeta, SpecView};
use usage_parser::error::UsageErr;
use usage_parser::Parser;

pub use usage_parser::parse::ParseOutput;
pub use usage_parser::Spec;

/// A validated collection of caller-supplied runtime command specs.
///
/// Building one validates; it does not assemble. The merged tree help and completion navigate
/// costs a KDL round trip of the whole host spec, and dispatching a plugin command needs none of
/// it — so it is built when something asks for it, and a host that only ever runs plugins never
/// pays. See [`app`](Self::app).
#[derive(Debug)]
pub struct Catalog<'a> {
    host: SpecView<'a>,
    entries: Vec<Entry>,
    overlays: &'a [CompletionOverlay<'a>],
    projection: Option<&'a str>,
    merged: OnceLock<Result<Spec, Error>>,
}

#[derive(Debug)]
struct Entry {
    parent: String,
    name: String,
    /// Answered to and advertised.
    aliases: Vec<String>,
    /// Answered to and never advertised — an old name kept working.
    hidden_aliases: Vec<String>,
    spec: Spec,
}

impl Entry {
    /// Whether this is what the user typed, by any name it answers to.
    fn answers_to(&self, word: &str) -> bool {
        self.name == word
            || self.aliases.iter().any(|alias| alias == word)
            || self.hidden_aliases.iter().any(|alias| alias == word)
    }
}

/// Help and completion over the whole command tree, static and runtime alike.
#[derive(Debug, Clone, Copy)]
pub struct App<'a> {
    catalog: &'a Catalog<'a>,
    merged: &'a Spec,
}

/// A catalog under construction.
#[derive(Debug)]
pub struct Builder<'a> {
    host: SpecView<'a>,
    pending: Vec<(String, Spec)>,
    overlays: &'a [CompletionOverlay<'a>],
    projection: Option<&'a str>,
}

impl<'a> Catalog<'a> {
    /// Begin a catalog over a derive-generated `Cli::app()`.
    pub fn builder(host: SpecView<'a>) -> Builder<'a> {
        Builder {
            host,
            pending: Vec::new(),
            overlays: &[],
            projection: None,
        }
    }

    /// Help and completion over the whole tree, static commands and runtime ones alike.
    ///
    /// The merged tree is assembled here, on the first call, and kept. Building it is a KDL
    /// round trip of the host spec — cheap next to rendering a page, and not something
    /// [`parse_external`](Self::parse_external) should ever pay for, which is why it waits until
    /// somebody asks.
    ///
    /// The error is a disagreement between the derived tables and the spec model that read
    /// them, which [`Builder::build`] cannot see because it has not lowered the host yet.
    pub fn app(&self) -> Result<App<'_>, &Error> {
        let merged = self.merged.get_or_init(|| self.merge()).as_ref()?;
        Ok(App {
            catalog: self,
            merged,
        })
    }

    /// Whether the merged tree has been assembled yet.
    ///
    /// [`app`](Self::app) builds it once and keeps it, so this answers whether the next call is
    /// the expensive one. A catalog that is only ever dispatched through never assembles.
    pub fn is_assembled(&self) -> bool {
        self.merged.get().is_some()
    }

    /// Lower the host tables into a spec and graft every entry into it.
    fn merge(&self) -> Result<Spec, Error> {
        let mut merged: Spec = self
            .host
            .clone()
            .to_kdl()
            .parse()
            .map_err(|error: UsageErr| Error::HostSpec(error.to_string()))?;
        for entry in &self.entries {
            insert_plugin(&mut merged, entry)?;
        }
        // A grafted command still carries the path it had in the spec it came from, where it was
        // the root — and so does its parent's usage line, which has just gained a subcommand.
        // Restamping is what makes the result indistinguishable from a spec that declared these
        // commands in KDL.
        merged.restamp();
        Ok(merged)
    }

    /// The entry a word beneath a canonical parent path names, by any name it answers to.
    fn entry(&self, parent: &str, word: &str) -> Option<&Entry> {
        self.entries
            .iter()
            .find(|entry| entry.parent == parent && entry.answers_to(word))
    }

    /// The static tables' own completion surface, carrying whatever the host registered.
    fn host_app(&self) -> usage_argv::complete::App<'_> {
        let app = usage_argv::complete::App::new(self.host.clone()).completions(self.overlays);
        match self.projection {
            Some(path) => app.project(path),
            None => app,
        }
    }

    /// Parse the argv an external-subcommand variant captured.
    ///
    /// `parent` is the static command path the catch-all sits on, with the empty string meaning
    /// the root. The first token is the runtime command's name, by any spelling it answers to.
    ///
    /// Two of the outcomes here are the caller's to handle rather than errors:
    ///
    /// - `Ok(None)` — no catalogued command answers to that name. Whatever the host did with
    ///   unrecognized words before it had a catalog, it should still do.
    /// - `Err(Error::NonUtf8)` — a token the portable spec model cannot represent, because it
    ///   parses `String`s and this one is not one. The derive handed over `OsString`s, which
    ///   lose nothing, so the argv is still intact: dispatch it the same way as `Ok(None)`
    ///   rather than failing a command whose argument happens to be an unusual path.
    ///
    /// Defaults and environment fallbacks are applied by the plugin's own spec, and `--help` and
    /// `--version` come back as [`Outcome`]s rather than as errors.
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
        let Some(entry) = self.entry(&canonical_parent, invoked) else {
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

impl<'a> App<'a> {
    /// The merged tree: the host's commands with the catalogued ones grafted in.
    pub fn spec(&self) -> &'a Spec {
        self.merged
    }

    /// Render help for a static or runtime command path. The empty path is the root.
    ///
    /// `long` is the `--help` / `-h` distinction: the full page or the summary.
    pub fn help(self, path: &str, long: bool) -> Option<String> {
        let command = find_command(self.merged, path)?;
        Some(usage_parser::docs::cli::render_help(
            self.merged,
            command,
            long,
        ))
    }

    /// Answer a hidden `__complete_word__` invocation, or return `None` for ordinary argv.
    ///
    /// Two answerers, one line. Up to the catch-all the words are the host's, and the host's own
    /// tables answer for them — the same engine, so registered completers, multicall
    /// projections and a `--candidates` request all keep working, and the catalogued names are
    /// added where a subcommand belongs. Past it the words are a plugin's, and that plugin's
    /// spec answers alone. A name in neither is a program nobody here can describe, and the
    /// answer is nothing.
    pub async fn completion_request(self, argv: &[OsString]) -> Option<String> {
        let request = CompletionRequest::parse(argv)?;
        let answer = self.complete_request(&request).await;
        Some(complete::render(&answer, request.shell))
    }

    /// The answer to a parsed request, before it is written the way a shell reads it.
    ///
    /// For a caller holding a [`Split`] rather than a shell's argv, build the request with
    /// [`CompletionRequest::for_split`].
    pub async fn complete_request(self, request: &CompletionRequest) -> Completions<'static> {
        let host = self.catalog.host_app();
        let split = host.effective_split(&request.split);
        let position = complete::walk(self.catalog.host.spec().root.cmd, split.argv());
        match position.external {
            None => {
                let mut answer = own(host.complete_request(request).await);
                if request.candidates_for.is_none() {
                    self.add_catalogued_commands(&position, &split, &mut answer);
                }
                answer
            }
            // A spec's `run=` names a completer to *execute*, and executing one is the thing
            // this crate does not do. A stale script asking for one gets nothing rather than
            // the wrong list.
            Some(_) if request.candidates_for.is_some() => Completions {
                candidates: Vec::new(),
                files: None,
            },
            Some(index) => self.complete_plugin(&position, index, &split),
        }
    }

    /// Add the runtime commands catalogued beneath the command the cursor is at.
    ///
    /// The static tables have never heard of them, so this is the one place their names come
    /// from. Where they belong is wherever a subcommand of that parent would: not in a flag's
    /// value, not after a dash, and not where a `help` topic is being asked for.
    fn add_catalogued_commands(
        self,
        position: &usage_argv::complete::Position<'_>,
        split: &Split,
        answer: &mut Completions<'static>,
    ) {
        if position.help_topic
            || position.awaiting_value.is_some()
            || (position.flags_possible && split.prefix.starts_with('-'))
        {
            return;
        }
        let parent = position
            .path
            .iter()
            .skip(1)
            .map(|(command, _)| command.name)
            .collect::<Vec<_>>()
            .join(" ");
        let before = answer.candidates.len();
        for entry in self.catalog.entries.iter().filter(|e| e.parent == parent) {
            let description = entry
                .spec
                .cmd
                .help
                .clone()
                .or_else(|| entry.spec.about.clone());
            for name in std::iter::once(&entry.name).chain(&entry.aliases) {
                if name.starts_with(&split.prefix) {
                    answer.candidates.push(Candidate {
                        value: name.clone(),
                        kind: CandidateKind::Command,
                        display: None,
                        description: description.clone().map(Cow::Owned),
                    });
                }
            }
        }
        if answer.candidates.len() == before {
            return;
        }
        sort_and_dedup(&mut answer.candidates);

        // The static tables offered the working directory because *they* had nothing to say
        // here; a catalogued name is something to say, and the same rule that closes a position
        // with candidates in it closes this one. Only when no argument is at the cursor, since
        // an argument may have asked for paths outright — then both belong.
        if position.next_arg.is_none()
            && position.awaiting_value.is_none()
            && matches!(answer.files, Some(Files::Any))
        {
            answer.files = None;
        }
    }

    /// Answer from the plugin's own spec, for the words that belong to it.
    fn complete_plugin(
        self,
        position: &usage_argv::complete::Position<'_>,
        index: usize,
        split: &Split,
    ) -> Completions<'static> {
        let nothing = Completions {
            candidates: Vec::new(),
            files: None,
        };
        let parent = position
            .path
            .iter()
            .skip(1)
            .map(|(command, _)| command.name)
            .collect::<Vec<_>>()
            .join(" ");
        let words = split.argv();
        let Some(invoked) = words.get(index) else {
            return nothing;
        };
        let Some(entry) = self.catalog.entry(&parent, invoked) else {
            return nothing;
        };
        // The plugin's spec describes a program of its own, so the words it is given start with
        // its own name — its `bin`, not whichever alias was typed, so that a spec selecting an
        // applet or a view by argv0 selects the same one either way.
        let argv0 = if entry.spec.bin.trim().is_empty() {
            entry.name.clone()
        } else {
            entry.spec.bin.clone()
        };
        let mut input = vec![argv0];
        input.extend(words[index + 1..].iter().cloned());
        complete_subtree(&entry.spec, &input, &split.prefix).unwrap_or(nothing)
    }
}

/// Sorted and deduplicated the way the static engine does it, so an answer with catalogued
/// names in it reads as one list rather than two concatenated.
fn sort_and_dedup(candidates: &mut Vec<Candidate<'static>>) {
    candidates.sort_unstable();
    candidates.dedup_by(|a, b| a.value == b.value);
}

/// Take ownership of an answer borrowed from the static tables.
///
/// The tables are `'static` in a real program, but a caller's are not required to be, and this
/// crate's answers say nothing about where they came from.
fn own(answer: Completions<'_>) -> Completions<'static> {
    Completions {
        candidates: answer
            .candidates
            .into_iter()
            .map(|candidate| Candidate {
                value: candidate.value,
                kind: candidate.kind,
                display: candidate.display.map(|text| Cow::Owned(text.into_owned())),
                description: candidate
                    .description
                    .map(|text| Cow::Owned(text.into_owned())),
            })
            .collect(),
        files: answer.files,
    }
}

/// The command a space-separated path names, static or runtime alike.
fn find_command<'a>(spec: &'a Spec, path: &str) -> Option<&'a usage_parser::SpecCommand> {
    let mut command = &spec.cmd;
    for component in path.split_ascii_whitespace() {
        command = command.find_subcommand(component)?;
    }
    Some(command)
}

/// What could go at the cursor, asked of one plugin's own spec.
///
/// `input` is the plugin's argv as the plugin would see it — its own name first — and `prefix`
/// is the word being typed, which is not in `input`: a half-typed word constrains the answer
/// without being part of the line yet.
fn complete_subtree(
    spec: &Spec,
    input: &[String],
    prefix: &str,
) -> Result<Completions<'static>, Error> {
    let parsed = usage_parser::parse::parse_partial(spec, input)
        .map_err(|error| Error::Parse(error.to_string()))?;
    // A plugin may itself forward to a program of its own. One level down, the same rule: these
    // words describe nothing this spec knows about.
    if parsed.external.is_some() {
        return Ok(Completions {
            candidates: Vec::new(),
            files: None,
        });
    }
    let flags_possible = !parsed.double_dash_seen;
    // A dash-prefixed word is a flag or nothing: no path starts with one.
    let flag_like = flags_possible && prefix.starts_with('-');
    let mut candidates = Vec::new();
    let mut files = None;
    // Whether the position names its own answers. An unmatched prefix against a declared set
    // means "nothing matched what you typed", not "ask the filesystem" — offering the working
    // directory for a mistyped choice answers a question nobody asked.
    let mut closed = false;
    let mut at_cursor = None;

    if let Some(flag) = parsed.flag_awaiting_value.first() {
        if let Some(arg) = &flag.arg {
            candidates.extend(choice_candidates(arg, prefix));
            closed |= declares_its_own(spec, &parsed.cmd, arg);
            files = completion_files(spec, &parsed.cmd, &arg.name);
        }
    } else if flag_like {
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
            at_cursor = Some(arg);
            candidates.extend(choice_candidates(arg, prefix));
            closed |= declares_its_own(spec, &parsed.cmd, arg);
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
    sort_and_dedup(&mut candidates);

    // An argument that requires a `--` is asking for that one word specifically; nothing else
    // belongs at the cursor until it is there.
    let needs_separator = parsed.flag_awaiting_value.is_empty()
        && at_cursor
            .is_some_and(|arg| arg.double_dash == usage_parser::SpecDoubleDashChoices::Required)
        && !parsed.double_dash_seen;

    if flag_like || needs_separator {
        files = None;
    } else if files.is_none() && candidates.is_empty() && !closed {
        files = Some(Files::Any);
    }
    Ok(Completions { candidates, files })
}

/// Whether this position states what it accepts, so an unmatched prefix means "no matches".
///
/// Choices are the obvious case. A declared `complete` block is one too — unless it says its
/// values are paths, which is a way of saying the filesystem *is* the answer, or says nothing
/// at all with `unknown`.
fn declares_its_own(
    spec: &Spec,
    command: &usage_parser::SpecCommand,
    arg: &usage_parser::SpecArg,
) -> bool {
    if arg.choices.is_some() {
        return true;
    }
    completion_for(spec, command, &arg.name).is_some_and(|completion| {
        completion.type_.as_deref().is_some_and(|type_| {
            !type_.eq_ignore_ascii_case("unknown") && files_kind(type_).is_none()
        })
    })
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
    files_kind(completion_for(spec, command, name)?.type_.as_deref()?)
}

/// The path fallback a declared `complete` type asks for, if it asks for one at all.
fn files_kind(type_: &str) -> Option<Files> {
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

    /// Register the host's own completion callbacks, as
    /// [`usage_argv::complete::App::completions`] would.
    ///
    /// Words before a runtime command are the host's, and the host's engine answers for them —
    /// so whatever it was given, it keeps.
    pub const fn completions(mut self, overlays: &'a [CompletionOverlay<'a>]) -> Self {
        self.overlays = overlays;
        self
    }

    /// Answer as though this command path came after argv0, for a multicall binary.
    ///
    /// The same projection [`usage_argv::complete::App::project`] applies, applied once, before
    /// anything asks where the words are.
    pub const fn project(mut self, command_path: &'a str) -> Self {
        self.projection = Some(command_path);
        self
    }

    /// Validate every parent and name, and finish the catalog.
    ///
    /// What is checked here is what the static tables can answer on their own: that the parent
    /// exists, that it invited runtime commands, and that no name collides. Assembling the
    /// merged tree is left to [`Catalog::app`] — dispatching a plugin command never needs it.
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
            let hidden_aliases: Vec<String> = spec
                .cmd
                .hidden_aliases
                .iter()
                .filter(|alias| !aliases.contains(alias))
                .cloned()
                .collect();
            let mut forms = Vec::with_capacity(1 + aliases.len() + hidden_aliases.len());
            forms.push(name.clone());
            forms.extend(aliases.iter().cloned());
            forms.extend(hidden_aliases.iter().cloned());
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
            entries.push(Entry {
                parent,
                name,
                aliases,
                hidden_aliases,
                spec,
            });
        }
        Ok(Catalog {
            host: self.host,
            entries,
            overlays: self.overlays,
            projection: self.projection,
            merged: OnceLock::new(),
        })
    }
}

/// Graft one entry's spec into the merged tree as a command of its parent.
///
/// A spec describes a program, and a command is one thing a program can be. What the two models
/// keep in different places — a program's `about` is a command's `help` — moves across here.
fn insert_plugin(merged: &mut Spec, entry: &Entry) -> Result<(), Error> {
    let plugin = &entry.spec;
    let mut command = plugin.cmd.clone();
    command.name = entry.name.clone();
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
    for component in entry.parent.split_ascii_whitespace() {
        current = current
            .subcommands
            .get_mut(component)
            .ok_or_else(|| Error::MissingParent(entry.parent.clone()))?;
    }
    current.subcommands.insert(entry.name.clone(), command);
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
            candidate.cmd.name == component
                || candidate.cmd.aliases.contains(&component)
                // A hidden alias is one the CLI answers to, and an application naming its own
                // static command is not "a user" being kept from an old spelling.
                || candidate.hidden_aliases.contains(&component)
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
