//! Splitting a command line the way the shell that typed it would.
//!
//! A completion request arrives as a line and a cursor, not as an argv: the user has pressed
//! Tab in the middle of something the shell has not run and may never run. Four of the five
//! shells hand that over directly — bash's `COMP_LINE`/`COMP_POINT`, zsh's `$BUFFER`/`$CURSOR`,
//! fish's `commandline -cp`, PowerShell's `$commandAst.Extent.Text` with `$cursorPosition` —
//! and nushell, whose external completer only ever sees spans, re-quotes them into a line.
//!
//! Taking the line rather than the shell's own word split is what lets `mise use "my tool<TAB>`
//! complete inside a quoted word at all: a word split has already thrown away the quote that
//! says the space is part of the word. The cost is that the splitting is ours to get right,
//! which is what this module and its tests are.
//!
//! The words that come out are *unquoted*, because they are what the shell would have passed
//! as argv had the line been run — the parser downstream should see exactly what it would see
//! in a real invocation.

use crate::spec::{ArgMeta, CommandMeta, CommandSelector, FlagMeta, Spec, SpecView};
pub use crate::spec::{Candidate, CompleteCtx, Completer};
use crate::{Arg, Command, Error, Flag, Parser};
use core::future::Future;
use core::pin::Pin;
use std::ffi::OsString;

/// Where the cursor is, in the grammar rather than in the line.
///
/// What can be typed at a cursor is decided by everything to its left: which command the words
/// reached, whether a flag is still waiting for its value, which positional comes next, and
/// whether flag interpretation is still on at all. This is that state, read off a real parse
/// of the words before the cursor rather than guessed at — so what is offered and what would
/// be accepted are decided by the same tables.
#[derive(Debug, Clone)]
pub struct Position<'t> {
    /// The command in scope: the deepest one the words selected.
    pub cmd: &'t Command<'t>,
    /// Whether a dash-prefixed word here would still be read as a flag.
    ///
    /// False past a `--`, and past the first value of an `automatic` argument — a wrapper
    /// forwarding to another tool. There is no flag of *this* CLI to offer in either place.
    pub flags_possible: bool,
    /// A flag whose value the cursor is at, if the last word was one that takes a value.
    pub awaiting_value: Option<&'t Flag<'t>>,
    /// The positional a word here would fill, if any are left.
    pub next_arg: Option<&'t Arg<'t>>,
    /// Values already bound to [`next_arg`](Self::next_arg).
    ///
    /// Non-zero only for a variadic positional that is still collecting. Completion hints use
    /// this to distinguish the command word from the arguments in a forwarded argv vector.
    pub next_arg_values: u32,
    /// Whether a `--` has been typed.
    ///
    /// Narrower than `flags_possible`, which is also false past an `automatic` argument. An
    /// argument that requires a separator is asking for *this* one specifically.
    pub separator_seen: bool,
    /// Whether the word here names a command to *read about* rather than one to run.
    ///
    /// True after `help`, where the only thing that belongs is a command name — not the
    /// arguments of whatever the root would otherwise fall back to.
    pub help_topic: bool,
    /// Where the command in scope began, as an index into the words `walk` was given.
    pub command_start: usize,
    /// Every command the words passed through, and where each one's own words begin.
    ///
    /// The deepest is not always the one being asked about: a global flag is declared on an
    /// ancestor, and a completer for its value wants the words *that* command was given — the
    /// ones before the subcommand name included.
    pub path: Vec<(&'t Command<'t>, usize)>,
    /// The flags a word here could name: this command's own, then any ancestor's globals.
    ///
    /// Taken from the parser rather than gathered again, so what is offered is what would be
    /// accepted — shadowing included.
    pub flags: Vec<&'t Flag<'t>>,
}

/// Walk the words before the cursor, and report what the cursor is at.
///
/// Errors are not failures here. A line being completed is by definition unfinished — a flag
/// with no value yet, a word that names nothing yet — so a parse error means "the grammar runs
/// out here", which is exactly the position being asked about. The walk stops at the first one
/// and reports the state it reached, rather than discarding it the way a real parse must.
pub fn walk<'t>(root: &'t Command<'t>, words: &[String]) -> Position<'t> {
    walk_inner(root, words, None)
}

/// Walk a command line through an executable view's selected globals.
pub fn walk_view<'t>(
    root: &'t Command<'t>,
    words: &[String],
    view: &'t crate::spec::ViewMeta<'t>,
) -> Position<'t> {
    // Route through the promoted command without changing the line custom completers see.
    // Position offsets are translated back afterwards so `CompleteCtx` slices the user's
    // words, not this internal projection.
    let mut projected = words.to_vec();
    let route: Vec<String> = view
        .root
        .split_ascii_whitespace()
        .map(str::to_string)
        .collect();
    let count = route.len();
    projected.splice(0..0, route);
    let mut position = walk_inner(root, &projected, Some(view));
    let original_index = |index: usize| index.saturating_sub(count);
    position.command_start = original_index(position.command_start);
    for (_, start) in &mut position.path {
        *start = original_index(*start);
    }
    position
}

fn walk_inner<'t>(
    root: &'t Command<'t>,
    words: &[String],
    view: Option<&'t crate::spec::ViewMeta<'t>>,
) -> Position<'t> {
    let argv: Vec<&std::ffi::OsStr> = words.iter().map(std::ffi::OsStr::new).collect();
    let mut parser = Parser::for_completion(root, &argv);
    if let Some(view) = view {
        parser = parser.with_view(view);
    }
    let mut awaiting_value = None;
    let mut last_arg = None;
    let mut last_arg_values = 0u32;

    while let Some(event) = parser.next_event() {
        match event {
            Ok(crate::Event::Arg { arg, .. }) => {
                if last_arg.is_some_and(|prior| core::ptr::eq(prior, arg)) {
                    last_arg_values = last_arg_values.saturating_add(1);
                } else {
                    last_arg = Some(arg);
                    last_arg_values = 1;
                }
            }
            Ok(_) => {}
            // The one error that says something about the cursor rather than about the line:
            // the last word was a flag that takes a value, so the cursor is standing in it.
            Err(Error::MissingFlagValue { flag }) => {
                awaiting_value = Some(flag);
                break;
            }
            // `ex help config ⌶` is asking which command to read about, and the answer is a
            // command under `config` — the one the help request already resolved. The parser
            // never descended into it, on purpose (a topic is a question, not an invocation),
            // so the position has to be taken from the request rather than from the parser.
            //
            // Nothing else can be typed there: a topic takes no flags and fills no argument.
            Err(Error::Help { cmd, .. }) if parser.help_span() != (0, 0) => {
                return Position {
                    cmd,
                    flags_possible: false,
                    awaiting_value: None,
                    next_arg: None,
                    next_arg_values: 0,
                    path: Vec::new(),
                    separator_seen: false,
                    command_start: 0,
                    help_topic: true,
                    flags: Vec::new(),
                }
            }
            Err(_) => break,
        }
    }

    let next_arg = parser.pending_arg();
    Position {
        path: parser.command_path(),
        cmd: parser.command(),
        flags_possible: !parser.flags_stopped(),
        // A variadic flag still claiming words is standing in the same place a flag waiting
        // for its first value is: the next word belongs to it, not to the positional after it.
        awaiting_value: awaiting_value.or_else(|| parser.collecting()),
        next_arg,
        next_arg_values: next_arg
            .filter(|arg| last_arg.is_some_and(|prior| core::ptr::eq(prior, *arg)))
            .map_or(0, |_| last_arg_values),
        separator_seen: parser.double_dash_seen(),
        command_start: parser.command_start(),
        help_topic: false,
        flags: parser.flags_in_scope().collect(),
    }
}

/// Answer for the completer a spec names, wherever in the tree it was declared.
///
/// The other half of what `to_kdl` writes. A spec says `complete "tool" run="mise
/// __complete_word__ --candidates tool"`, and this is what answers that command — so the KDL is
/// complete for the usage CLI, for another shell's generator, for anything that reads a spec,
/// while the binary still answers itself when its own script asks.
///
/// Found by the name a spec uses, which is the lowercased argument name — the same rule the
/// reference resolves a `complete` block by, so the two agree about which completer a `run=`
/// belongs to. `None` when nothing of that name declares one.
pub fn for_name<'a>(
    spec: &'a Spec<'a>,
    name: &str,
    ctx: &CompleteCtx<'_>,
) -> Option<Vec<Candidate<'static>>> {
    let reached = walk(spec.root.cmd, ctx.command_words_start());
    for_name_at(spec, name, ctx, &reached, None)
}

/// Answer for a named completer through an executable view.
pub fn for_name_view<'a>(
    spec: &'a Spec<'a>,
    name: &str,
    ctx: &CompleteCtx<'_>,
    view: &'a crate::spec::ViewMeta<'a>,
) -> Option<Vec<Candidate<'static>>> {
    let reached = walk_view(spec.root.cmd, ctx.command_words_start(), view);
    for_name_at(spec, name, ctx, &reached, Some(view))
}

fn for_name_at<'a>(
    spec: &'a Spec<'a>,
    name: &str,
    ctx: &CompleteCtx<'_>,
    reached: &Position<'a>,
    view: Option<&'a crate::spec::ViewMeta<'a>>,
) -> Option<Vec<Candidate<'static>>> {
    fn on(meta: &CommandMeta<'_>, name: &str) -> Option<Completer> {
        for arg in meta.args {
            if arg.arg.name.eq_ignore_ascii_case(name) {
                if let Some(completer) = arg.complete {
                    return Some(completer);
                }
            }
        }
        for flag in meta.flags {
            let value = flag.value_name.unwrap_or(flag.flag.name);
            if value.eq_ignore_ascii_case(name) {
                if let Some(completer) = flag.complete {
                    return Some(completer);
                }
            }
        }
        None
    }

    fn find<'m, 's>(meta: &'m CommandMeta<'s>, name: &str) -> Option<&'m CommandMeta<'s>> {
        if on(meta, name).is_some() {
            return Some(meta);
        }
        meta.subcommands.iter().find_map(|sub| find(sub, name))
    }

    // A command can have two fields under one normalized completion name. Its emitted KDL has
    // one name-keyed block, so the line that block passes back is what distinguishes them. Use
    // the parser's position before falling back to declaration order for a stale or incomplete
    // request whose line does not identify either field.
    fn at_cursor(meta: &CommandMeta<'_>, name: &str, position: &Position<'_>) -> Option<Completer> {
        if let Some(wanted) = position.awaiting_value {
            let found = meta.flags.iter().find(|field| {
                let value = field.value_name.unwrap_or(field.flag.name);
                core::ptr::eq(field.flag, wanted) && value.eq_ignore_ascii_case(name)
            });
            if let Some(completer) = found.and_then(|field| field.complete) {
                return Some(completer);
            }
        }
        if let Some(wanted) = position.next_arg {
            let found = meta.args.iter().find(|field| {
                core::ptr::eq(field.arg, wanted) && field.arg.name.eq_ignore_ascii_case(name)
            });
            if let Some(completer) = found.and_then(|field| field.complete) {
                return Some(completer);
            }
        }
        None
    }

    // The root's own first, then the command the line reached, then anywhere in the tree.
    //
    // The first two in that order because the reference resolves a `complete` block that way —
    // `spec.complete.get(name).or(cmd.complete.get(name))` — and the root's completers are what
    // a spec writes at the top level, which is `spec.complete`. Answering in a different order
    // would mean this binary and the reference disagreed about a spec they both read.
    //
    // Tree order last, and only as a fallback: two sibling commands may take a `tool` and mean
    // different things by it, and the one the line reached is the one being asked about.
    let chain = metadata_chain_on_route(spec, reached).unwrap_or_default();
    // Cursor identity is stronger than name-keyed declaration order. In particular, an
    // executable view may promote a field whose completion name is also used by an omitted
    // host field. Only the promoted field's parser identity can match here.
    let at_cursor = chain
        .iter()
        .rev()
        .find_map(|meta| at_cursor(meta, name, reached));

    let fallback = if let Some(view) = view {
        let depth = view.root.split_ascii_whitespace().count();
        chain.get(depth).copied().and_then(|promoted| {
            let (flags, groups) = crate::help::view_root_fields(spec, promoted, view);
            let projected = CommandMeta {
                flags: &flags,
                groups: &groups,
                ..*promoted
            };
            on(&projected, name)
                .or_else(|| chain.last().copied().and_then(|meta| on(meta, name)))
                .or_else(|| find(promoted, name).and_then(|meta| on(meta, name)))
        })
    } else {
        on(spec.root, name)
            .or_else(|| chain.last().copied().and_then(|meta| on(meta, name)))
            .or_else(|| find(spec.root, name).and_then(|meta| on(meta, name)))
    };
    let completer = at_cursor.or(fallback)?;
    let mut found = completer(ctx);
    found.retain(|c| c.value.starts_with(ctx.prefix));
    Some(found)
}

/// The completers one command declares, as the names a `complete` block is written under.
///
/// One command's own, not the tree's: a spec writes the block inside the command that declares
/// it, so two siblings taking a `tool` and meaning different things by it each say so.
#[cfg(feature = "spec")]
pub fn completers_on(meta: &CommandMeta<'_>) -> Vec<String> {
    let mut out = Vec::new();
    for arg in meta.args {
        if arg.complete.is_some() {
            out.push(arg.arg.name.to_ascii_lowercase());
        }
    }
    for flag in meta.flags {
        if flag.complete.is_some() {
            out.push(
                flag.value_name
                    .unwrap_or(flag.flag.name)
                    .to_ascii_lowercase(),
            );
        }
    }
    out.sort();
    out.dedup();
    out
}

/// Paths a shell should offer, on top of whatever this crate knows about.
///
/// Listing them is the shell's job, not ours. It already does it better than a CLI can — the
/// user's own completion styles, colours, escaping, directory-aware widgets — and doing it here
/// would put a directory read in a binary whose whole claim is that it does not touch the
/// filesystem to parse a command line. So the answer says *that* paths belong here, and the
/// generated script hands the position to `_files`, `__fish_complete_path` or `compgen -f`.
///
/// This is a deliberate divergence from usage-lib, which reads the directory itself and returns
/// the names. The conformance comparison holds the two equivalent rather than equal: where the
/// reference answers with a listing, this answers with the marker.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Files {
    /// Anything: files, directories, whatever the shell shows for a path.
    Any,
    /// Directories only.
    Dirs,
    /// Executable files at a filesystem path.
    ExecutablePaths,
    /// Command names, including entries from the shell's command table and `PATH`.
    Commands,
    /// Files with one of these extensions, plus directories for continued traversal.
    Extensions(Vec<String>),
}

/// Everything a shell needs to answer one Tab.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct Completions<'a> {
    /// What this CLI knows the word could be.
    pub candidates: Vec<Candidate<'a>>,
    /// Paths the shell should add, if the position admits them.
    pub files: Option<Files>,
}

/// A completion future supplied by an embedding CLI.
///
/// usage-argv does not choose an executor. The application awaits this future on the runtime it
/// already owns; the allocation exists only after the shell asks for candidates.
pub type CompletionFuture<'a> = Pin<Box<dyn Future<Output = Vec<Candidate<'static>>> + 'a>>;

/// An async completion function. Taking the context by value lets the future borrow its line and
/// command tables without requiring global state or a runtime-specific callback trait.
pub type AsyncCompleter = for<'a> fn(CompleteCtx<'a>) -> CompletionFuture<'a>;

/// A runtime completion callback added to a derive-generated field.
#[derive(Clone, Copy)]
pub enum CompletionHandler {
    Sync(Completer),
    Async(AsyncCompleter),
}

impl core::fmt::Debug for CompletionHandler {
    fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        f.write_str(match self {
            Self::Sync(_) => "Sync(..)",
            Self::Async(_) => "Async(..)",
        })
    }
}

/// One sparse completion override, selected by declaring command and normalized value name.
#[derive(Debug, Clone, Copy)]
pub struct CompletionOverlay<'a> {
    pub command: CommandSelector<'a>,
    pub value: &'a str,
    pub handler: CompletionHandler,
}

impl<'a> CompletionOverlay<'a> {
    pub const fn sync_any(value: &'a str, completer: Completer) -> Self {
        Self {
            command: CommandSelector::Any,
            value,
            handler: CompletionHandler::Sync(completer),
        }
    }

    pub const fn async_any(value: &'a str, completer: AsyncCompleter) -> Self {
        Self {
            command: CommandSelector::Any,
            value,
            handler: CompletionHandler::Async(completer),
        }
    }

    pub const fn sync(path: &'a str, value: &'a str, completer: Completer) -> Self {
        Self {
            command: CommandSelector::Path(path),
            value,
            handler: CompletionHandler::Sync(completer),
        }
    }

    pub const fn asynchronous(path: &'a str, value: &'a str, completer: AsyncCompleter) -> Self {
        Self {
            command: CommandSelector::Path(path),
            value,
            handler: CompletionHandler::Async(completer),
        }
    }
}

/// A self-contained completion surface over a borrowed [`SpecView`].
///
/// A projection inserts a command path after argv0 before walking the base tables. That lets a
/// multicall binary such as `aubr` expose `aube run` without cloning a command tree or losing the
/// root's global flags.
#[derive(Debug, Clone)]
pub struct App<'a> {
    view: SpecView<'a>,
    overlays: &'a [CompletionOverlay<'a>],
    projection: Option<&'a str>,
}

impl<'a> App<'a> {
    pub const fn new(view: SpecView<'a>) -> Self {
        Self {
            view,
            overlays: &[],
            projection: None,
        }
    }

    pub const fn completions(mut self, overlays: &'a [CompletionOverlay<'a>]) -> Self {
        self.overlays = overlays;
        self
    }

    pub const fn project(mut self, command_path: &'a str) -> Self {
        self.projection = Some(command_path);
        self
    }

    pub fn completion_script(self, shell: Shell) -> String {
        let spec = self.view.spec();
        crate::script::script(spec.bin.unwrap_or(spec.name), shell)
    }

    /// Register completion under `alias` while invoking this view's real binary.
    pub fn completion_script_for_alias(self, alias: &str, shell: Shell) -> String {
        let spec = self.view.spec();
        crate::script::script_for(spec.bin.unwrap_or(spec.name), alias, shell)
    }

    /// Where this view's completion script goes, and what else the user has to do.
    ///
    /// Touches no filesystem, so this is the answer a preview prints. The binary is resolved the
    /// same way [`App::completion_script`] resolves it, which is what keeps a view or a multicall
    /// projection from needing a second idea of its own name.
    pub fn completion_install_plan(
        self,
        shell: Shell,
        env: &crate::install::Env,
    ) -> Result<crate::install::Plan, crate::install::Error> {
        let spec = self.view.spec();
        crate::install::plan(spec.bin.unwrap_or(spec.name), shell, env)
    }

    /// Where an alias's script would go. The preview half of [`App::install_completion_for_alias`].
    pub fn completion_install_plan_for_alias(
        self,
        alias: &str,
        shell: Shell,
        env: &crate::install::Env,
    ) -> Result<crate::install::Plan, crate::install::Error> {
        let spec = self.view.spec();
        crate::install::plan_for(spec.bin.unwrap_or(spec.name), alias, shell, env)
    }

    /// Write this view's completion script where the described environment says it goes.
    pub fn install_completion(
        self,
        shell: Shell,
        env: &crate::install::Env,
        on_foreign: crate::install::OnForeign,
    ) -> Result<crate::install::Installed, crate::install::Error> {
        let spec = self.view.spec();
        crate::install::install(spec.bin.unwrap_or(spec.name), shell, env, on_foreign)
    }

    /// Install under a shell alias while still asking this view's real binary for answers.
    pub fn install_completion_for_alias(
        self,
        alias: &str,
        shell: Shell,
        env: &crate::install::Env,
        on_foreign: crate::install::OnForeign,
    ) -> Result<crate::install::Installed, crate::install::Error> {
        let spec = self.view.spec();
        crate::install::install_for(spec.bin.unwrap_or(spec.name), alias, shell, env, on_foreign)
    }

    /// Answer a hidden completion invocation, or return `None` for ordinary argv.
    pub async fn completion_request(self, argv: &[OsString]) -> Option<String> {
        let request = Request::parse(argv)?;
        let mut split = request.split;
        if let Some(path) = self.projection {
            let projected: Vec<String> =
                path.split_ascii_whitespace().map(str::to_string).collect();
            let count = projected.len();
            split.words.splice(1..1, projected);
            // A projection lives after argv0. When the cursor is still editing
            // argv0, inserting that path must not move the cursor into it.
            if split.cword > 0 {
                split.cword += count;
            }
        }
        let spec = self.view.spec();
        let answer = if let Some(name) = request.candidates_for {
            complete_named_with(&spec, &split, self.overlays, &name).await
        } else {
            complete_with(&spec, &split, self.overlays).await
        };
        Some(render(&answer, request.shell))
    }
}

impl<'a> SpecView<'a> {
    /// Add compiled completion callbacks and optional multicall projections to this view.
    pub const fn completion_app(self) -> App<'a> {
        App::new(self)
    }
}

struct Request {
    shell: Shell,
    split: Split,
    candidates_for: Option<String>,
}

impl Request {
    fn parse(argv: &[OsString]) -> Option<Self> {
        if argv.first()?.to_str()? != "__complete_word__" {
            return None;
        }
        let mut shell = Shell::Bash;
        let mut line = String::new();
        let mut cursor = None;
        let mut candidates_for = None;
        let mut rest = argv[1..].iter();
        while let Some(arg) = rest.next() {
            match arg.to_str().unwrap_or_default() {
                "--shell" => {
                    if let Some(found) = rest
                        .next()
                        .and_then(|name| Shell::from_name(&name.to_string_lossy()))
                    {
                        shell = found;
                    }
                }
                "--line" => {
                    if let Some(value) = rest.next() {
                        line = value.to_string_lossy().into_owned();
                    }
                }
                "--cursor" => {
                    cursor = rest
                        .next()
                        .and_then(|value| value.to_str().and_then(|v| v.parse().ok()));
                }
                "--candidates" => {
                    candidates_for = rest.next().map(|v| v.to_string_lossy().into_owned());
                }
                _ => {}
            }
        }
        let cursor = cursor.unwrap_or(line.len());
        Some(Self {
            shell,
            split: split(&line, cursor, shell),
            candidates_for,
        })
    }
}

/// The line a shell reads to mean "paths belong here too".
///
/// A whole line rather than a flag on the protocol, because every one of the five shells can
/// already split output into lines and look at the last one. `\x01` opens it because no
/// candidate this crate produces can contain a control character, so it cannot be mistaken for
/// one.
pub const FILES_MARKER: &str = "\u{1}files";
/// See [`FILES_MARKER`]. Directories only.
pub const DIRS_MARKER: &str = "\u{1}dirs";
/// See [`FILES_MARKER`]. Executable filesystem paths only.
pub const EXECUTABLE_PATHS_MARKER: &str = "\u{1}executables";
/// See [`FILES_MARKER`]. Command names from the shell and `PATH` only.
pub const COMMANDS_MARKER: &str = "\u{1}commands";

/// Write an answer the way `shell` reads it.
///
/// One line per candidate, in the shape the shell's own completion machinery expects — which is
/// where the five differ. bash reads values only; fish and nu take a description after a tab;
/// PowerShell takes value, description and display text; zsh takes display text, description and
/// the quoted text to insert.
///
/// A trailing [`FILES_MARKER`] says the generated script should hand the position to the
/// shell's own path completion afterwards.
pub fn render(answer: &Completions<'_>, shell: Shell) -> String {
    let mut out = String::new();
    // Descriptions are all-or-nothing per answer: a column that appears on some rows and not
    // others reads as missing data rather than as an absent description, which is the reason
    // the reference decides this per answer too.
    let described = answer.candidates.iter().any(|c| c.description.is_some());

    for candidate in &answer.candidates {
        let description = one_line(candidate.description.as_deref().unwrap_or_default());
        let description = description.as_str();
        match shell {
            Shell::Bash => out.push_str(&candidate.value),
            Shell::Zsh => {
                // Display, then description, then what to type: a candidate containing a space
                // or a quote has to reach the command line intact.
                out.push_str(&one_line(
                    candidate.display.as_deref().unwrap_or(&candidate.value),
                ));
                out.push('\t');
                out.push_str(description);
                out.push('\t');
                out.push_str(&zsh_quote(&candidate.value));
            }
            Shell::PowerShell => {
                out.push_str(&candidate.value);
                out.push('\t');
                out.push_str(description);
                out.push('\t');
                out.push_str(&one_line(
                    candidate.display.as_deref().unwrap_or(&candidate.value),
                ));
            }
            Shell::Fish | Shell::Nu => {
                out.push_str(&candidate.value);
                if described {
                    out.push('\t');
                    out.push_str(description);
                }
            }
        }
        out.push('\n');
    }

    match &answer.files {
        Some(Files::Any) => {
            out.push_str(FILES_MARKER);
            out.push('\n');
        }
        Some(Files::Dirs) => {
            out.push_str(DIRS_MARKER);
            out.push('\n');
        }
        Some(Files::ExecutablePaths) => {
            out.push_str(EXECUTABLE_PATHS_MARKER);
            out.push('\n');
        }
        Some(Files::Commands) => {
            out.push_str(COMMANDS_MARKER);
            out.push('\n');
        }
        Some(Files::Extensions(extensions)) => {
            out.push_str("\u{1}extensions");
            for extension in extensions {
                out.push('\t');
                out.push_str(extension);
            }
            out.push('\n');
        }
        None => {}
    }
    out
}

/// A description with nothing in it that a line-based protocol would read as structure.
///
/// One line per candidate, fields separated by tabs — so a description containing either splits
/// one candidate into several rows, or invents a field. mise declares multi-line `help` on 37 of
/// its commands and flags, so this is the common case rather than a defensive one.
///
/// Collapsed rather than truncated at the first line: a description written across two lines is
/// one sentence to a reader, and showing half of it is worse than showing it spaced.
fn one_line(text: &str) -> String {
    let mut out = String::with_capacity(text.len());
    let mut spaced = false;
    for c in text.chars() {
        if matches!(c, '\n' | '\r' | '\t') {
            // One space for a run of them, and never a leading one.
            if !spaced && !out.is_empty() {
                out.push(' ');
                spaced = true;
            }
        } else {
            out.push(c);
            spaced = false;
        }
    }
    // A trailing break became a trailing space.
    while out.ends_with(' ') {
        out.pop();
    }
    out
}

/// A candidate as zsh would have to see it typed.
///
/// The same rule the reference uses: left alone when every character is safe, and otherwise
/// single-quoted with the close-open dance around any apostrophe.
fn zsh_quote(value: &str) -> String {
    let safe = |c: char| {
        c.is_ascii_alphanumeric()
            || matches!(c, '_' | '-' | '.' | '/' | ':' | '@' | '+' | '=' | '%' | ',')
    };
    if !value.is_empty() && value.chars().all(safe) {
        return value.to_string();
    }
    format!("'{}'", value.replace('\'', "'\\''"))
}

/// The paths an argument or value name asks for, by the name itself.
///
/// The reference resolves a completer by the *lowercased name* and falls back to treating that
/// name as the type, so an argument called `<FILE>` completes files and `<DIR>` directories
/// without a spec saying so. Reimplemented rather than modelled as new vocabulary, because it
/// is the same rule read off the same names.
fn files_for(name: &str) -> Option<Files> {
    // Compared without allocating a lowercased copy: a name is short, and this is a parser.
    let matches = |want: &str| name.eq_ignore_ascii_case(want);
    if matches("file") || matches("path") || matches("config_file") {
        Some(Files::Any)
    } else if matches("dir") || matches("directory") {
        Some(Files::Dirs)
    } else if matches("executable") {
        Some(Files::ExecutablePaths)
    } else if matches("command") {
        Some(Files::Commands)
    } else {
        None
    }
}

fn declared_files(type_: &str, next_arg_values: u32) -> Option<Files> {
    if let Some(extensions) = type_
        .strip_prefix("path:")
        .or_else(|| type_.strip_prefix("file:"))
    {
        let extensions = extensions
            .split(',')
            .filter(|extension| !extension.is_empty())
            .map(|extension| extension.trim_start_matches('.').to_string())
            .collect::<Vec<_>>();
        if !extensions.is_empty() {
            return Some(Files::Extensions(extensions));
        }
    }
    if type_.eq_ignore_ascii_case("command_args") {
        return Some(if next_arg_values == 0 {
            Files::Commands
        } else {
            Files::Any
        });
    }
    files_for(type_)
}

fn declared_files_at_cursor(
    spec: &Spec<'_>,
    split: &Split,
    position: &Position<'_>,
) -> Option<Files> {
    if split.cword == 0
        || (position.awaiting_value.is_none()
            && position.flags_possible
            && split.prefix.starts_with('-'))
    {
        return None;
    }
    let meta = metadata_chain_on_route(spec, position).and_then(|chain| chain.last().copied());
    let after_restart = restarted(meta, split);
    let at_cursor = if after_restart {
        meta.and_then(|m| m.args.first()).map(|m| m.arg)
    } else {
        position
            .next_arg
            .or_else(|| default_subcommand_arg(spec, split, position).map(|(_, field)| field.arg))
    };
    if position.awaiting_value.is_none()
        && at_cursor.is_some_and(|arg| arg.double_dash == crate::DoubleDash::Required)
        && !position.separator_seen
    {
        return None;
    }
    let (name, complete_type) = if let Some(flag) = position.awaiting_value {
        let meta = flag_meta(spec.root, flag);
        (
            meta.and_then(|m| m.value_name).or(Some(flag.name)),
            meta.and_then(|m| m.complete_type),
        )
    } else if let Some(arg) = at_cursor {
        let meta = arg_meta(spec.root, arg);
        (Some(arg.name), meta.and_then(|m| m.complete_type))
    } else {
        (None, None)
    };
    match complete_type {
        Some(type_) => declared_files(
            type_,
            if after_restart {
                0
            } else {
                position.next_arg_values
            },
        )
        .or_else(|| {
            type_
                .eq_ignore_ascii_case("unknown")
                .then(|| name.and_then(files_for))
                .flatten()
        }),
        None => name.and_then(files_for),
    }
}

/// What could be typed at the cursor, given a spec and a split line.
///
/// The rules are usage-lib's, because a CLI's completions should not change with the
/// implementation that answers them:
///
/// - a lone `-` offers both forms, `--…` offers longs, `-…` offers shorts;
/// - a flag waiting for its value offers that flag's choices, and nothing else;
/// - otherwise the next positional's choices and the subcommands, aliases included.
///
/// Hidden things are never offered, in any branch. What is *not* here yet: the file fallback
/// for a word nothing is known about, and the `run=` completions a spec can declare.
pub fn complete<'a>(spec: &'a Spec<'a>, split: &Split) -> Completions<'a> {
    complete_inner(spec, split, None)
}

/// Complete through an executable view, omitting root globals it does not carry.
pub fn complete_view<'a>(
    spec: &'a Spec<'a>,
    split: &Split,
    view: &'a crate::spec::ViewMeta<'a>,
) -> Completions<'a> {
    complete_inner(spec, split, Some(view))
}

fn complete_inner<'a>(
    spec: &'a Spec<'a>,
    split: &Split,
    view: Option<&'a crate::spec::ViewMeta<'a>>,
) -> Completions<'a> {
    let position = match view {
        Some(view) => walk_view(spec.root.cmd, split.argv(), view),
        None => walk(spec.root.cmd, split.argv()),
    };
    let meta = metadata_chain_on_route(spec, &position).and_then(|chain| chain.last().copied());
    let token = split.prefix.as_str();
    let candidates = candidates_inner(spec, split, view);

    // Which argument the cursor is at — the same question `candidates` answers, asked once so
    // that the two halves cannot disagree. Past a restart token it is the command's *first*
    // argument, whatever the words before the token filled, and everything below follows from
    // that: whether paths belong, whether the set is declared, whether a separator is owed.
    let after_restart = restarted(meta, split);
    let at_cursor = if after_restart {
        meta.and_then(|m| m.args.first()).map(|m| m.arg)
    } else {
        position.next_arg
    };

    // A dash-prefixed word is a flag or nothing: no path starts with one, and the reference
    // suppresses its own listing there for the same reason.
    let flag_like = position.flags_possible && token.starts_with('-');

    // The name a value here would have, which is what says whether paths belong, and whether
    // that value declares its own set.
    let (named, declares_choices, complete_type) = if let Some(flag) = position.awaiting_value {
        let meta = flag_meta(spec.root, flag);
        (
            meta.and_then(|m| m.value_name).or(Some(flag.name)),
            meta.is_some_and(|m| !m.choices.is_empty() || !m.accepted_choices.is_empty()),
            meta.and_then(|m| m.complete_type),
        )
    } else if let Some(arg) = at_cursor {
        let meta = arg_meta(spec.root, arg);
        (
            Some(arg.name),
            meta.is_some_and(|m| !m.choices.is_empty() || !m.accepted_choices.is_empty()),
            meta.and_then(|m| m.complete_type),
        )
    } else {
        (None, false, None)
    };
    let asked_for = match complete_type {
        Some(type_) => declared_files(
            type_,
            if after_restart {
                0
            } else {
                position.next_arg_values
            },
        )
        .or_else(|| {
            type_
                .eq_ignore_ascii_case("unknown")
                .then(|| named.and_then(files_for))
                .flatten()
        }),
        None => named.and_then(files_for),
    };

    // An argument that requires a separator is not fillable yet, so nothing else belongs here —
    // not even a path, which the parser would reject exactly as it rejects a value.
    //
    // Only when the cursor is *at* that argument, though. A flag waiting for its value is a
    // different position that happens to have an unfilled positional behind it, and a rule about
    // the positional has nothing to say about the flag: `ex --from ⌶` takes a path whatever the
    // argument after it needs.
    let needs_separator = position.awaiting_value.is_none()
        && at_cursor.is_some_and(|arg| arg.double_dash == crate::DoubleDash::Required)
        && !position.separator_seen;

    // Two questions, and the reference asks both. Was anything found — because a position that
    // answered does not need help. And does the position *declare* its set — because then an
    // unmatched prefix means no matches rather than "ask somebody else", which is the difference
    // between "there is nothing else this can be" and "nothing matched what you typed".
    // Offering the working directory for a mistyped choice answers the second as though it were
    // the first.
    let declared_non_file_type = complete_type
        .is_some_and(|type_| !type_.eq_ignore_ascii_case("unknown") && asked_for.is_none());
    let closed =
        !candidates.is_empty() || declares_choices || declared_non_file_type || position.help_topic;

    let files = if flag_like || needs_separator {
        None
    } else if asked_for.is_some() {
        asked_for
    } else if closed {
        None
    } else {
        Some(Files::Any)
    };

    Completions { candidates, files }
}

/// Complete from the static tables plus sparse sync or async runtime callbacks.
///
/// No future or callback is created until a completion request reaches a field with an overlay.
pub async fn complete_with<'a>(
    spec: &'a Spec<'a>,
    split: &Split,
    overlays: &[CompletionOverlay<'_>],
) -> Completions<'a> {
    let position = walk(spec.root.cmd, split.argv());
    let Some(overlay) = overlay_at_cursor(spec, split, &position, overlays) else {
        return complete(spec, split);
    };

    let words = split.argv();
    let command_path: Vec<(&Command<'_>, &[String])> = position
        .path
        .iter()
        .map(|(cmd, start)| (*cmd, words.get(*start..).unwrap_or(&[])))
        .collect();
    let ctx = CompleteCtx {
        words: &split.words,
        cword: split.cword,
        prefix: &split.prefix,
        command_words: command_words(split, &position),
        command_path: &command_path,
    };
    let mut dynamic = match overlay.handler {
        CompletionHandler::Sync(completer) => completer(&ctx),
        CompletionHandler::Async(completer) => completer(ctx).await,
    };
    dynamic.retain(|candidate| candidate.value.starts_with(&split.prefix));

    let mut answer = complete(spec, split);
    answer.candidates.extend(dynamic);
    sort_and_dedup_candidates(&mut answer.candidates);
    // Even an empty callback suppresses the cwd fallback, but a field that
    // explicitly declares files or directories keeps that shell completion.
    answer.files = declared_files_at_cursor(spec, split, &position);
    answer
}

async fn complete_named_with<'a>(
    spec: &'a Spec<'a>,
    split: &Split,
    overlays: &[CompletionOverlay<'_>],
    name: &str,
) -> Completions<'a> {
    let position = walk(spec.root.cmd, split.argv());
    let words = split.argv();
    let command_path: Vec<(&Command<'_>, &[String])> = position
        .path
        .iter()
        .map(|(cmd, start)| (*cmd, words.get(*start..).unwrap_or(&[])))
        .collect();
    let ctx = CompleteCtx {
        words: &split.words,
        cword: split.cword,
        prefix: &split.prefix,
        command_words: command_words(split, &position),
        command_path: &command_path,
    };
    let mut candidates = for_name(spec, name, &ctx).unwrap_or_default();
    if let Some(overlay) = overlay_for_name(spec, split, &position, overlays, name) {
        let mut dynamic = match overlay.handler {
            CompletionHandler::Sync(completer) => completer(&ctx),
            CompletionHandler::Async(completer) => completer(ctx).await,
        };
        candidates.append(&mut dynamic);
    }
    candidates.retain(|candidate| candidate.value.starts_with(&split.prefix));
    sort_and_dedup_candidates(&mut candidates);
    Completions {
        candidates,
        files: None,
    }
}

/// Sort candidates by insertion value and merge presentation metadata from duplicates.
///
/// Static metadata and a runtime overlay can legitimately offer the same value. Keeping the
/// first candidate after derived sorting would prefer `None` over `Some`, discarding the richer
/// display label or description.
fn sort_and_dedup_candidates(candidates: &mut Vec<Candidate<'_>>) {
    candidates.sort();
    let mut deduped: Vec<Candidate<'_>> = Vec::with_capacity(candidates.len());
    for mut candidate in candidates.drain(..) {
        if let Some(existing) = deduped
            .last_mut()
            .filter(|existing| existing.value == candidate.value)
        {
            if existing.display.is_none() {
                existing.display = candidate.display.take();
            }
            if existing.description.is_none() {
                existing.description = candidate.description.take();
            }
        } else {
            deduped.push(candidate);
        }
    }
    *candidates = deduped;
}

fn overlay_for_name<'o>(
    spec: &Spec<'_>,
    split: &Split,
    position: &Position<'_>,
    overlays: &'o [CompletionOverlay<'_>],
    name: &str,
) -> Option<&'o CompletionOverlay<'o>> {
    if let Some(overlay) = overlay_at_cursor(spec, split, position, overlays)
        .filter(|overlay| overlay.value.eq_ignore_ascii_case(name))
    {
        return Some(overlay);
    }
    let chain = metadata_chain_on_route(spec, position)?;
    let owner = chain.last()?;
    let path: Vec<&str> = chain.iter().skip(1).map(|meta| meta.cmd.name).collect();
    overlays.iter().rev().find(|overlay| {
        overlay.value.eq_ignore_ascii_case(name) && overlay.command.matches(owner, &path)
    })
}

fn overlay_at_cursor<'o>(
    spec: &Spec<'_>,
    split: &Split,
    position: &Position<'_>,
    overlays: &'o [CompletionOverlay<'_>],
) -> Option<&'o CompletionOverlay<'o>> {
    if split.cword == 0
        || (position.awaiting_value.is_none()
            && position.flags_possible
            && split.prefix.starts_with('-'))
    {
        return None;
    }
    let meta = metadata_chain_on_route(spec, position).and_then(|chain| chain.last().copied());
    let target = if restarted(meta, split) {
        meta.and_then(|owner| {
            owner.args.first().map(|field| {
                (
                    owner,
                    field.arg.name,
                    field.arg.double_dash == crate::DoubleDash::Required,
                )
            })
        })
    } else if let Some(flag) = position.awaiting_value {
        flag_meta_owner_on_route(spec, position, flag)
            .map(|(owner, field)| (owner, field.value_name.unwrap_or(field.flag.name), false))
    } else {
        position
            .next_arg
            .and_then(|arg| arg_meta_owner_on_route(spec, position, arg))
            .map(|(owner, field)| {
                (
                    owner,
                    field.arg.name,
                    field.arg.double_dash == crate::DoubleDash::Required,
                )
            })
            .or_else(|| {
                default_subcommand_arg(spec, split, position).map(|(owner, field)| {
                    (
                        owner,
                        field.arg.name,
                        field.arg.double_dash == crate::DoubleDash::Required,
                    )
                })
            })
    };
    let (owner, value, needs_separator) = target?;
    if needs_separator && !position.separator_seen {
        return None;
    }
    let chain = metadata_chain_on_route(spec, position)?;
    let path: Vec<&str> = if let Some(owner_at) = chain
        .iter()
        .position(|candidate| core::ptr::eq(*candidate, owner))
    {
        chain[..=owner_at]
            .iter()
            .skip(1)
            .map(|meta| meta.cmd.name)
            .collect()
    } else if core::ptr::eq(position.cmd, spec.root.cmd)
        && spec
            .root
            .subcommands
            .iter()
            .any(|candidate| core::ptr::eq(*candidate, owner))
    {
        // A default subcommand can supply the root cursor's first argument without the
        // parser descending into it. Its metadata identity is already unambiguous here;
        // preserve that implied route instead of falling back to a tree-wide pointer search.
        vec![owner.cmd.name]
    } else {
        return None;
    };
    overlays.iter().rev().find(|overlay| {
        overlay.value.eq_ignore_ascii_case(value) && overlay.command.matches(owner, &path)
    })
}

fn flag_meta_owner_on_route<'a>(
    spec: &'a Spec<'a>,
    position: &Position<'_>,
    flag: &Flag<'_>,
) -> Option<(&'a CommandMeta<'a>, &'a FlagMeta<'a>)> {
    let chain = metadata_chain_on_route(spec, position)?;
    chain.iter().rev().find_map(|owner| {
        owner
            .flags
            .iter()
            .find(|field| core::ptr::eq(field.flag, flag))
            .map(|field| (*owner, field))
    })
}

fn arg_meta_owner_on_route<'a>(
    spec: &'a Spec<'a>,
    position: &Position<'_>,
    arg: &Arg<'_>,
) -> Option<(&'a CommandMeta<'a>, &'a ArgMeta<'a>)> {
    let chain = metadata_chain_on_route(spec, position)?;
    chain.iter().rev().find_map(|owner| {
        owner
            .args
            .iter()
            .find(|field| core::ptr::eq(field.arg, arg))
            .map(|field| (*owner, field))
    })
}

fn default_subcommand_arg<'a>(
    spec: &'a Spec<'a>,
    split: &Split,
    position: &Position<'_>,
) -> Option<(&'a CommandMeta<'a>, &'a ArgMeta<'a>)> {
    if !core::ptr::eq(position.cmd, spec.root.cmd) || position.help_topic || split.cword == 0 {
        return None;
    }
    let default = spec.default_subcommand?;
    let subcommands = || spec.root.subcommands.iter().copied();
    subcommands()
        .find(|sub| sub.cmd.name == default)
        .or_else(|| subcommands().find(|sub| sub.cmd.aliases.contains(&default)))
        .and_then(|sub| sub.args.first().map(|field| (sub, field)))
}

/// The metadata route selected by the parser, preserving parent identity even when two
/// wrappers reuse the same nested command tables.
fn metadata_chain_on_route<'a>(
    spec: &'a Spec<'a>,
    position: &Position<'_>,
) -> Option<Vec<&'a CommandMeta<'a>>> {
    if position.path.is_empty() {
        return crate::help::find(spec, position.cmd).map(|(_, chain)| chain);
    }
    let mut chain = vec![spec.root];
    let mut current = spec.root;
    for (command, _) in position.path.iter().skip(1) {
        current = current
            .subcommands
            .iter()
            .copied()
            .find(|meta| core::ptr::eq(meta.cmd, *command))?;
        chain.push(current);
    }
    Some(chain)
}

/// Just the candidates this CLI knows about, without the question of paths.
pub fn candidates<'a>(spec: &'a Spec<'a>, split: &Split) -> Vec<Candidate<'a>> {
    candidates_inner(spec, split, None)
}

fn candidates_inner<'a>(
    spec: &'a Spec<'a>,
    split: &Split,
    view: Option<&'a crate::spec::ViewMeta<'a>>,
) -> Vec<Candidate<'a>> {
    let position = match view {
        Some(view) => walk_view(spec.root.cmd, split.argv(), view),
        None => walk(spec.root.cmd, split.argv()),
    };
    let meta = metadata_chain_on_route(spec, &position).and_then(|chain| chain.last().copied());
    let token = split.prefix.as_str();

    let mut out = if position.flags_possible && token == "-" {
        // Both forms, because a lone dash says nothing about which was meant.
        let mut both = short_flags(spec, &position, "");
        both.extend(long_flags(spec, &position, ""));
        both
    } else if position.flags_possible && token.starts_with("--") {
        long_flags(spec, &position, token)
    } else if position.flags_possible && token.starts_with('-') {
        short_flags(spec, &position, token)
    } else if restarted(meta, split) {
        // Past a restart token — mise's `:::`, which starts a fresh invocation of the same
        // command — the cursor is at that command's *first* argument again, whatever the
        // words before the token filled.
        meta.and_then(|m| m.args.first())
            .map(|m| positional(m, &position, split, token))
            .unwrap_or_default()
    } else if let Some(flag) = position.awaiting_value {
        flag_meta(spec.root, flag)
            .map(|m| {
                declared(
                    m.choices,
                    m.choice_details,
                    m.complete,
                    split,
                    &position,
                    token,
                )
            })
            .unwrap_or_default()
    } else {
        let mut found = Vec::new();
        if let Some(arg) = position.next_arg {
            if let Some(m) = arg_meta(spec.root, arg) {
                found.extend(positional(m, &position, split, token));
            }
        }
        if let Some(meta) = meta {
            found.extend(subcommands(meta, token));
        }
        // At the root, a word may also be meant for the command the root falls back to —
        // `mise build` is `mise run build` — so what that command's first argument accepts is
        // a candidate here too. Only its first: the words that would fill the rest have not
        // been typed, since the subcommand name itself was elided.
        if let Some((_, arg)) = default_subcommand_arg(spec, split, &position) {
            found.extend(positional(arg, &position, split, token));
        }
        found
    };

    sort_and_dedup_candidates(&mut out);
    out
}

/// Whether the word before the cursor is this command's restart token.
///
/// mise writes `:::` between two runs of the same command, and what follows one is a fresh
/// invocation — so the cursor is back at the first argument rather than wherever the previous
/// words had reached.
fn restarted(meta: Option<&CommandMeta<'_>>, split: &Split) -> bool {
    let Some(token) = meta.and_then(|m| m.restart_token) else {
        return false;
    };
    split.cword > 0 && split.words[split.cword - 1] == token
}

/// The subcommands a word here could name, each under every name it answers to.
///
/// Aliases are offered beside canonical names — someone who types `mise ls<TAB>` means `list`
/// and should see it — except the hidden ones, which the parser answers to but nothing
/// advertises. A hidden command is offered under none of its names at all.
fn subcommands<'a>(meta: &'a CommandMeta<'a>, token: &str) -> Vec<Candidate<'a>> {
    let mut out = Vec::new();
    for sub in meta.subcommands {
        if sub.hide {
            continue;
        }
        for name in core::iter::once(&sub.cmd.name).chain(sub.cmd.aliases.iter()) {
            // A hidden alias is one the parser answers to but nothing advertises — usually an
            // old name kept working after a rename. The parse table holds it beside the
            // visible ones because both must be *accepted*; only the metadata says which are
            // meant to be *offered*.
            if sub.hidden_aliases.contains(name) {
                continue;
            }
            if name.starts_with(token) {
                out.push(Candidate {
                    value: (*name).to_string(),
                    display: None,
                    description: deprecated_description(
                        sub.about,
                        sub.deprecated,
                        sub.deprecated_warn_at,
                        sub.deprecated_remove_at,
                    ),
                });
            }
        }
    }
    out
}

/// The long forms of every flag in scope, and the negations of those that have one.
fn long_flags<'a>(spec: &'a Spec<'a>, position: &Position<'_>, token: &str) -> Vec<Candidate<'a>> {
    let mut out = Vec::new();
    for flag in &position.flags {
        let meta = flag_meta(spec.root, flag);
        if meta.is_some_and(|m| m.hide) {
            continue;
        }
        let description = meta.and_then(|m| {
            deprecated_description(
                m.help,
                m.deprecated,
                m.deprecated_warn_at,
                m.deprecated_remove_at,
            )
        });
        for long in flag.longs {
            if meta.is_some_and(|m| m.hidden_longs.contains(long)) {
                continue;
            }
            let value = format!("--{long}");
            if value.starts_with(token) {
                out.push(Candidate {
                    value,
                    display: None,
                    description: description.clone(),
                });
            }
        }
        // A negation is a way to write the same flag, so it carries the same help — the
        // reference leaves it bare, which reads as a flag nobody documented.
        if let Some(negate) = flag.negate {
            // The table holds it the way the parser matches it — with the dashes already
            // taken off — so a candidate has to put them back. Offered bare, it never matched
            // a `--` the user had typed, and a lone `-` offered a word no shell would accept.
            let value = format!("--{negate}");
            if value.starts_with(token) {
                out.push(Candidate {
                    value,
                    display: None,
                    description: description.clone(),
                });
            }
        }
    }
    out
}

/// The short forms of every flag in scope.
///
/// A token of `-x` is asking about the letter `x`, so only that letter's flag is offered:
/// bundling means anything else would be a candidate for a different position in the token.
fn short_flags<'a>(spec: &'a Spec<'a>, position: &Position<'_>, token: &str) -> Vec<Candidate<'a>> {
    let wanted = token.as_bytes().get(1).copied();
    let mut out = Vec::new();
    for flag in &position.flags {
        let meta = flag_meta(spec.root, flag);
        if meta.is_some_and(|m| m.hide) {
            continue;
        }
        for &short in flag.shorts {
            if meta.is_some_and(|m| m.hidden_shorts.contains(&short)) {
                continue;
            }
            // Written out rather than with `is_none_or`, which this crate's MSRV predates.
            let asked_about = match wanted {
                None => true,
                Some(letter) => letter == short,
            };
            if asked_about {
                out.push(Candidate {
                    value: format!("-{}", short as char),
                    display: None,
                    description: meta.and_then(|m| {
                        deprecated_description(
                            m.help,
                            m.deprecated,
                            m.deprecated_warn_at,
                            m.deprecated_remove_at,
                        )
                    }),
                });
            }
        }
    }
    out
}

fn deprecated_description<'a>(
    base: Option<&'a str>,
    message: Option<&'a str>,
    warn_at: Option<&'a str>,
    remove_at: Option<&'a str>,
) -> Option<std::borrow::Cow<'a, str>> {
    if message.is_none() && warn_at.is_none() && remove_at.is_none() {
        return base.map(std::borrow::Cow::Borrowed);
    }
    let mut parts = Vec::new();
    if let Some(message) = message {
        parts.push(message.to_string());
    }
    if let Some(at) = warn_at {
        parts.push(format!("warns at {at}"));
    }
    if let Some(at) = remove_at {
        parts.push(format!("removed at {at}"));
    }
    let label = format!("[deprecated: {}]", parts.join("; "));
    Some(std::borrow::Cow::Owned(match base {
        Some(base) if !base.is_empty() => format!("{base} {label}"),
        _ => label,
    }))
}

/// What a positional accepts here — its choices, or the separator that has to come first.
///
/// An argument declared `double_dash = "required"` is not fillable until a `--` has been
/// typed, so its values are not candidates yet: the parser would reject every one of them. The
/// only useful thing to offer is the separator itself, and only when nothing has been typed to
/// filter it away.
fn positional<'a>(
    meta: &'a ArgMeta<'a>,
    position: &Position<'_>,
    split: &Split,
    token: &str,
) -> Vec<Candidate<'a>> {
    if meta.arg.double_dash == crate::DoubleDash::Required && !position.separator_seen {
        if token.is_empty() {
            return vec![Candidate {
                value: "--".to_string(),
                display: None,
                description: None,
            }];
        }
        return Vec::new();
    }
    declared(
        meta.choices,
        meta.choice_details,
        meta.complete,
        split,
        position,
        token,
    )
}

/// What a position declares it takes: its choices, or the completer that answers for it.
///
/// Choices first, and a completer only where there are none — the order the reference reads a
/// spec in, where a `run=` is what an argument has *instead of* a fixed set rather than beside
/// one. The answer is filtered here so a completer may return everything it knows, which is what
/// the reference does with a `run=` command's output.
fn declared<'a>(
    choices_declared: &'a [&'a str],
    choice_details: &'a [crate::spec::ChoiceMeta<'a>],
    completer: Option<Completer>,
    split: &Split,
    position: &Position<'_>,
    token: &str,
) -> Vec<Candidate<'a>> {
    if !choices_declared.is_empty() {
        return choices(choices_declared, choice_details, token);
    }
    let Some(completer) = completer else {
        return Vec::new();
    };
    // The words each command on the path was given, so a completer declared on an ancestor —
    // which is what a global flag is — reads its own command's rather than the deepest one's.
    let words = split.argv();
    let path: Vec<(&Command<'_>, &[String])> = position
        .path
        .iter()
        .map(|(cmd, start)| (*cmd, words.get(*start..).unwrap_or(&[])))
        .collect();
    let ctx = CompleteCtx {
        words: &split.words,
        cword: split.cword,
        prefix: token,
        command_words: command_words(split, position),
        command_path: &path,
    };
    let mut found = completer(&ctx);
    found.retain(|c| c.value.starts_with(token));
    found
}

/// The words the command in scope was given, out of the whole line.
///
/// `walk` was handed the words after the program name and before the cursor's, and reported
/// where the command in scope began within them — so this is the tail of that, which is what
/// that command would have been given had the line been run.
fn command_words<'s>(split: &'s Split, position: &Position<'_>) -> &'s [String] {
    let words = split.argv();
    words.get(position.command_start..).unwrap_or(&[])
}

/// The declared values of a flag or argument, filtered by what has been typed.
fn choices<'a>(
    declared: &'a [&'a str],
    details: &'a [crate::spec::ChoiceMeta<'a>],
    token: &str,
) -> Vec<Candidate<'a>> {
    declared
        .iter()
        .filter(|c| c.starts_with(token))
        .map(|c| Candidate {
            value: (*c).to_string(),
            display: None,
            description: details
                .iter()
                .find(|detail| {
                    detail.value == *c || detail.aliases.iter().any(|alias| alias.value == *c)
                })
                .and_then(|detail| detail.help)
                .map(::std::borrow::Cow::Borrowed),
        })
        .collect()
}

/// The metadata for a flag, wherever in the tree it was declared.
///
/// By identity rather than by name: a global flag's metadata sits on the command that declared
/// it, which is an ancestor of the one being completed, and two commands may declare different
/// flags under one name.
fn flag_meta<'a>(meta: &'a CommandMeta<'a>, flag: &Flag<'_>) -> Option<&'a FlagMeta<'a>> {
    meta.flags
        .iter()
        .find(|m| core::ptr::eq(m.flag, flag))
        .or_else(|| meta.subcommands.iter().find_map(|sub| flag_meta(sub, flag)))
}

/// The metadata for an argument, wherever in the tree it was declared.
fn arg_meta<'a>(meta: &'a CommandMeta<'a>, arg: &Arg<'_>) -> Option<&'a ArgMeta<'a>> {
    meta.args
        .iter()
        .find(|m| core::ptr::eq(m.arg, arg))
        .or_else(|| meta.subcommands.iter().find_map(|sub| arg_meta(sub, arg)))
}

/// Which shell's quoting rules a line follows.
///
/// Only two rule sets, not five: bash, zsh, fish and nushell all follow the POSIX shape
/// closely enough that a completion request cannot tell them apart, while PowerShell escapes
/// with a backtick and doubles a quote to escape it. The distinction is kept per *shell*
/// rather than per rule set so that a shell whose rules turn out to differ can be given its
/// own without changing this type's public shape.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[non_exhaustive]
pub enum Shell {
    Bash,
    Zsh,
    Fish,
    Nu,
    PowerShell,
}

impl Shell {
    /// The spelling a shell is named by, on the command line and in a spec.
    pub fn as_str(self) -> &'static str {
        match self {
            Shell::Bash => "bash",
            Shell::Zsh => "zsh",
            Shell::Fish => "fish",
            Shell::Nu => "nu",
            Shell::PowerShell => "powershell",
        }
    }

    /// Read a shell by name, as it is spelled on the command line.
    pub fn from_name(name: &str) -> Option<Self> {
        match name {
            "bash" => Some(Shell::Bash),
            "zsh" => Some(Shell::Zsh),
            "fish" => Some(Shell::Fish),
            "nu" | "nushell" => Some(Shell::Nu),
            "powershell" | "pwsh" => Some(Shell::PowerShell),
            _ => None,
        }
    }

    /// Whether an escape is written with a backtick rather than a backslash.
    fn backtick_escapes(self) -> bool {
        matches!(self, Shell::PowerShell)
    }

    /// Whether a quote inside a quoted string is written by doubling it.
    fn doubles_quotes(self) -> bool {
        matches!(self, Shell::PowerShell)
    }
}

/// A command line as the shell would have passed it, plus where the cursor was.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Split {
    /// The words, unquoted — what argv would hold had the line been run.
    ///
    /// Always at least one: a cursor sitting after a space is completing a word that does not
    /// exist yet, and an empty word is how that is said. Candidates for "anything at all" and
    /// candidates for "something starting with `no`" are the same question with a different
    /// prefix, and a caller should not have to special-case the empty one.
    pub words: Vec<String>,
    /// Which of `words` the cursor is in.
    pub cword: usize,
    /// The part of that word before the cursor, unquoted — what a candidate must start with.
    pub prefix: String,
}

impl Split {
    /// The words up to and including the one being completed.
    ///
    /// What the parser should walk: everything after the cursor describes a command line the
    /// user has not finished thinking about, and a half-typed tail behind the cursor should
    /// not decide what can be typed at it.
    pub fn walked(&self) -> &[String] {
        &self.words[..=self.cword]
    }

    /// The words a parser should walk: after the program name, before the cursor's word.
    ///
    /// Two things dropped for two reasons. The program name, because argv does not contain it
    /// and the parse tables describe what comes *after* it. The word being completed, because
    /// it is half-typed by definition — feeding it in would ask what can follow a word the
    /// user has not finished, when the question is what that word could be.
    pub fn argv(&self) -> &[String] {
        let start = 1.min(self.cword);
        &self.words[start..self.cword]
    }
}

/// Split a line at a byte cursor, the way `shell` would have split it.
///
/// `cursor` is a byte offset into `line`; anything past the end is treated as the end, and an
/// offset landing inside a multi-byte character is moved back to that character's start rather
/// than panicking — a completion request is not a place to be strict about a shell's arithmetic.
pub fn split(line: &str, cursor: usize, shell: Shell) -> Split {
    let cursor = floor_char_boundary(line, cursor.min(line.len()));

    let mut words: Vec<String> = Vec::new();
    let mut word = String::new();
    // Whether anything has been written into `word` — including a quote that so far contains
    // nothing, so that `mise ""` is a word and not a gap between two.
    let mut started = false;
    let mut cword = None;
    let mut prefix = None;
    // Whether the cursor sat inside a word rather than in the gap before one. A gap is a word
    // the user is about to type, so one has to be made for them.
    let mut cursor_in_word = false;

    let mut chars = line.char_indices().peekable();
    let mut quote: Option<char> = None;

    // The cursor is reached before the character it sits in front of is read, so the word in
    // hand is the word being completed and what is in it is the prefix.
    //
    // A macro rather than a closure because it writes to four locals the loop also writes to,
    // and it has to run at *two* places: at the top of each character, and again before an
    // escape swallows the one after it. Only checking the top meant a cursor sitting on an
    // escaped character was never noticed, and the split described the last word of the line
    // instead of the one being typed.
    macro_rules! reached {
        ($idx:expr) => {
            if $idx == cursor && prefix.is_none() {
                cword = Some(words.len());
                prefix = Some(word.clone());
                cursor_in_word = started;
            }
        };
    }

    while let Some((i, c)) = chars.next() {
        reached!(i);

        match quote {
            Some('\'') => {
                if c == '\'' {
                    // PowerShell writes a quote inside a quoted string by doubling it; the
                    // POSIX-shaped shells have no such rule, and there a second quote always
                    // ends the string.
                    if shell.doubles_quotes() && chars.peek().map(|&(_, n)| n) == Some('\'') {
                        reached!(chars.peek().expect("peeked just above").0);
                        word.push('\'');
                        chars.next();
                    } else {
                        quote = None;
                    }
                } else {
                    word.push(c);
                }
            }
            Some(q) => {
                if c == q {
                    if shell.doubles_quotes() && chars.peek().map(|&(_, n)| n) == Some(q) {
                        reached!(chars.peek().expect("peeked just above").0);
                        word.push(q);
                        chars.next();
                    } else {
                        quote = None;
                    }
                } else if is_escape(c, shell) {
                    // Inside double quotes an escape is only an escape before a character it
                    // could mean something to; before anything else it is a literal, which is
                    // why a Windows path in double quotes survives.
                    match chars.peek() {
                        Some(&(j, next)) if escapable_in_quotes(next, shell) => {
                            reached!(j);
                            word.push(next);
                            chars.next();
                        }
                        _ => word.push(c),
                    }
                } else {
                    word.push(c);
                }
            }
            None => {
                if c == '\'' || c == '"' {
                    quote = Some(c);
                    started = true;
                } else if is_escape(c, shell) {
                    // Before the check, because the escape has already started the word: a
                    // cursor on the escaped character is inside it, not in the gap before it.
                    started = true;
                    if let Some(&(j, next)) = chars.peek() {
                        reached!(j);
                        word.push(next);
                        chars.next();
                    } else {
                        // A trailing escape is a line the user is still typing, not a mistake
                        // to report: it escapes the character they have not typed yet.
                        started = true;
                    }
                } else if c.is_whitespace() {
                    if started {
                        words.push(core::mem::take(&mut word));
                        started = false;
                    }
                } else {
                    word.push(c);
                    started = true;
                }
            }
        }
    }

    // The cursor at the very end of the line: the loop above only sees positions it reads a
    // character at, and there is no character there.
    if prefix.is_none() {
        cword = Some(words.len());
        prefix = Some(word.clone());
        cursor_in_word = started;
    }

    if started {
        words.push(word);
    }

    let cword = cword.unwrap_or(0);
    // A cursor in a gap is completing a word that is not in the line yet — at the end of it,
    // or between two that are already there. `mise ⌶use` is asking what can go *before* `use`,
    // and answering about `use` itself would complete the wrong word.
    if !cursor_in_word {
        words.insert(cword, String::new());
    }
    Split {
        words,
        cword,
        prefix: prefix.unwrap_or_default(),
    }
}

/// Whether a character starts an escape in this shell.
fn is_escape(c: char, shell: Shell) -> bool {
    if shell.backtick_escapes() {
        c == '`'
    } else {
        c == '\\'
    }
}

/// Whether an escape inside double quotes applies to the character after it.
fn escapable_in_quotes(c: char, shell: Shell) -> bool {
    if shell.backtick_escapes() {
        matches!(c, '"' | '`' | '$')
    } else {
        matches!(c, '"' | '\\' | '$' | '`')
    }
}

/// The start of the character containing `index`.
///
/// `str::floor_char_boundary` is still unstable, and this crate takes no dependencies.
fn floor_char_boundary(s: &str, index: usize) -> usize {
    let mut i = index;
    while i > 0 && !s.is_char_boundary(i) {
        i -= 1;
    }
    i
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::task::{Context, Poll, Waker};

    fn run_ready<F: Future>(future: F) -> F::Output {
        let waker = Waker::noop();
        let mut cx = Context::from_waker(waker);
        let mut future = std::pin::pin!(future);
        match future.as_mut().poll(&mut cx) {
            Poll::Ready(output) => output,
            Poll::Pending => panic!("test completion unexpectedly needed an executor wakeup"),
        }
    }

    /// A small tree with the shapes that make a cursor's position non-obvious: a global flag,
    /// a flag that takes a value, a subcommand of a subcommand, and a wrapper that forwards.
    static GLOBAL: Flag = Flag {
        key: 1,
        name: "verbose",
        longs: &["verbose"],
        shorts: b"v",
        global: true,
        // Held without its dashes, which is how the parser matches it.
        negate: Some("quiet"),
        ..Flag::BOOL
    };
    static JOBS: Flag = Flag {
        key: 2,
        name: "jobs",
        longs: &["jobs"],
        ..Flag::VALUE
    };
    /// A flag that keeps claiming words once it has one.
    static TOOLS: Flag = Flag {
        key: 8,
        name: "tools",
        longs: &["tools"],
        variadic: true,
        ..Flag::VALUE
    };
    static TOOL: Arg = Arg {
        key: 3,
        name: "TOOL",
        ..Arg::REQUIRED
    };
    static USE: Command = Command {
        name: "use",
        aliases: &["u"],
        flags: &[&JOBS, &TOOLS],
        args: &[&TOOL],
        ..Command::EMPTY
    };
    static FORWARDED: Arg = Arg {
        key: 4,
        name: "ARGS",
        double_dash: crate::DoubleDash::Automatic,
        ..Arg::VAR
    };
    static EXEC: Command = Command {
        name: "exec",
        args: &[&FORWARDED],
        ..Command::EMPTY
    };
    static LS: Command = Command {
        name: "ls",
        ..Command::EMPTY
    };
    /// Two arguments and a restart token, so that "back at the first" is a different answer
    /// from "wherever the words reached".
    static FIRST: Arg = Arg {
        key: 5,
        name: "FIRST",
        ..Arg::REQUIRED
    };
    static SECOND: Arg = Arg {
        key: 6,
        name: "SECOND",
        ..Arg::REQUIRED
    };
    static TASK: Command = Command {
        name: "task",
        args: &[&FIRST, &SECOND],
        ..Command::EMPTY
    };
    /// A restarting command whose first argument takes paths and whose second does not, so that
    /// "which argument is the cursor at" has two different answers.
    static SCRIPT_ARG: Arg = Arg {
        key: 13,
        name: "FILE",
        ..Arg::REQUIRED
    };
    static MODE: Arg = Arg {
        key: 14,
        name: "MODE",
        ..Arg::REQUIRED
    };
    static SHIP: Command = Command {
        name: "ship",
        // The choices-bearing one first, so that "which argument is the cursor at" has two
        // different *answers* rather than two routes to the same one.
        args: &[&MODE, &SCRIPT_ARG],
        ..Command::EMPTY
    };
    static FILE: Arg = Arg {
        key: 9,
        name: "FILE",
        ..Arg::REQUIRED
    };
    /// A path-named argument that is not fillable until a `--` is typed.
    static PIPED: Arg = Arg {
        key: 11,
        name: "PATH",
        double_dash: crate::DoubleDash::Required,
        ..Arg::REQUIRED
    };
    static FROM: Flag = Flag {
        key: 12,
        name: "from",
        longs: &["from"],
        ..Flag::VALUE
    };
    static PIPE: Command = Command {
        name: "pipe",
        flags: &[&FROM],
        args: &[&PIPED],
        ..Command::EMPTY
    };
    static EDIT: Command = Command {
        name: "edit",
        flags: &[&INTO],
        args: &[&FILE],
        ..Command::EMPTY
    };
    static INTO: Flag = Flag {
        key: 10,
        name: "into",
        longs: &["into"],
        ..Flag::VALUE
    };
    static AFTER: Arg = Arg {
        key: 7,
        name: "AFTER",
        double_dash: crate::DoubleDash::Required,
        ..Arg::REQUIRED
    };
    static WRAP: Command = Command {
        name: "wrap",
        args: &[&AFTER],
        ..Command::EMPTY
    };
    static PLUGINS: Command = Command {
        name: "plugins",
        subcommands: &[&LS],
        ..Command::EMPTY
    };
    static ROOT: Command = Command {
        name: "mise",
        flags: &[&GLOBAL],
        subcommands: &[&USE, &EXEC, &PLUGINS],
        ..Command::EMPTY
    };

    /// The same tree's metadata: help text, choices, and what is hidden.
    static SECRET: Command = Command {
        name: "secret",
        ..Command::EMPTY
    };
    static LIST: Command = Command {
        name: "list",
        // `l` is answered to but never advertised — an old name kept working.
        aliases: &["ls", "l"],
        ..Command::EMPTY
    };
    static META_LS: CommandMeta = CommandMeta {
        cmd: &LS,
        about: Some("List them"),
        ..CommandMeta::EMPTY
    };
    static META_PLUGINS: CommandMeta = CommandMeta {
        cmd: &PLUGINS,
        about: Some("Manage plugins"),
        subcommands: &[&META_LS],
        ..CommandMeta::EMPTY
    };
    static META_USE: CommandMeta = CommandMeta {
        cmd: &USE,
        about: Some("Use a tool"),
        flags: &[FlagMeta {
            flag: &JOBS,
            help: Some("How many at once"),
            choices: &["1", "2", "4"],
            choice_details: &[crate::spec::ChoiceMeta {
                value: "2",
                help: Some("Two workers"),
                hide: false,
                aliases: &[],
            }],
            ..FlagMeta::EMPTY
        }],
        args: &[ArgMeta {
            arg: &TOOL,
            help: Some("Which tool"),
            choices: &["node", "python"],
            ..ArgMeta::EMPTY
        }],
        ..CommandMeta::EMPTY
    };
    /// What answers for `mise install <TOOL>`: everything it knows, prefix and all, because the
    /// filtering is not its job.
    fn tools(ctx: &CompleteCtx<'_>) -> Vec<Candidate<'static>> {
        // Reads the line, which is what a `run=` gets through tera and what half of mise's
        // completers use — `{{words[PREV]}}` is `ctx.previous()`.
        if ctx.previous() == Some("--only") {
            return vec![Candidate::new("node")];
        }
        vec![
            Candidate::described("node", "JavaScript"),
            Candidate::described("python", "Snakes"),
            Candidate::new("ruby"),
            Candidate::new("ruby").displayed("Ruby runtime"),
        ]
    }

    static TOOL_ARG: Arg = Arg {
        key: 15,
        name: "TOOL",
        ..Arg::REQUIRED
    };
    static ONLY: Flag = Flag {
        key: 16,
        name: "only",
        longs: &["only"],
        ..Flag::VALUE
    };
    /// A flag whose completer is *not* the argument's, so which one answered is visible.
    fn sources(_ctx: &CompleteCtx<'_>) -> Vec<Candidate<'static>> {
        vec![Candidate::new("upstream")]
    }
    static SOURCE: Flag = Flag {
        key: 17,
        name: "source",
        longs: &["source"],
        ..Flag::VALUE
    };
    static INSTALL: Command = Command {
        name: "install",
        flags: &[&ONLY, &SOURCE],
        args: &[&TOOL_ARG],
        ..Command::EMPTY
    };
    static META_INSTALL: CommandMeta = CommandMeta {
        cmd: &INSTALL,
        about: Some("Install a tool"),
        flags: &[
            FlagMeta {
                flag: &ONLY,
                help: Some("Just this one"),
                complete: Some(tools),
                ..FlagMeta::EMPTY
            },
            FlagMeta {
                flag: &SOURCE,
                help: Some("Where from"),
                complete: Some(sources),
                ..FlagMeta::EMPTY
            },
        ],
        args: &[ArgMeta {
            arg: &TOOL_ARG,
            help: Some("Which tool"),
            complete: Some(tools),
            ..ArgMeta::EMPTY
        }],
        ..CommandMeta::EMPTY
    };

    static META_SHIP: CommandMeta = CommandMeta {
        cmd: &SHIP,
        about: Some("Ship a file"),
        restart_token: Some(":::"),
        args: &[
            ArgMeta {
                arg: &MODE,
                choices: &["fast", "slow"],
                ..ArgMeta::EMPTY
            },
            ArgMeta {
                arg: &SCRIPT_ARG,
                help: Some("Which file"),
                ..ArgMeta::EMPTY
            },
        ],
        ..CommandMeta::EMPTY
    };
    static META_PIPE: CommandMeta = CommandMeta {
        cmd: &PIPE,
        about: Some("Pipe a file"),
        flags: &[FlagMeta {
            flag: &FROM,
            help: Some("Read from"),
            value_name: Some("FILE"),
            ..FlagMeta::EMPTY
        }],
        args: &[ArgMeta {
            arg: &PIPED,
            help: Some("Where from"),
            ..ArgMeta::EMPTY
        }],
        ..CommandMeta::EMPTY
    };
    static META_EDIT: CommandMeta = CommandMeta {
        cmd: &EDIT,
        about: Some("Edit a file"),
        flags: &[FlagMeta {
            flag: &INTO,
            help: Some("Where to write it"),
            value_name: Some("DIR"),
            ..FlagMeta::EMPTY
        }],
        args: &[ArgMeta {
            arg: &FILE,
            help: Some("Which file"),
            // Two well-known ones, so the position has candidates *and* wants paths — which is
            // what makes the name the deciding rule rather than the emptiness.
            choices: &["mise.toml", "mise.local.toml"],
            ..ArgMeta::EMPTY
        }],
        ..CommandMeta::EMPTY
    };
    static META_WRAP: CommandMeta = CommandMeta {
        cmd: &WRAP,
        about: Some("Wrap something"),
        args: &[ArgMeta {
            arg: &AFTER,
            choices: &["red", "blue"],
            ..ArgMeta::EMPTY
        }],
        ..CommandMeta::EMPTY
    };
    static META_TASK: CommandMeta = CommandMeta {
        cmd: &TASK,
        about: Some("Do two things"),
        restart_token: Some(":::"),
        args: &[
            ArgMeta {
                arg: &FIRST,
                choices: &["one", "two"],
                ..ArgMeta::EMPTY
            },
            ArgMeta {
                arg: &SECOND,
                choices: &["alpha", "beta"],
                ..ArgMeta::EMPTY
            },
        ],
        ..CommandMeta::EMPTY
    };
    static META_EXEC: CommandMeta = CommandMeta {
        cmd: &EXEC,
        about: Some("Run something"),
        restart_token: Some(":::"),
        args: &[ArgMeta {
            arg: &FORWARDED,
            help: Some("What to run"),
            choices: &["one", "two"],
            complete_type: Some("command_args"),
            ..ArgMeta::EMPTY
        }],
        ..CommandMeta::EMPTY
    };
    static META_SECRET: CommandMeta = CommandMeta {
        cmd: &SECRET,
        about: Some("Not for you"),
        hide: true,
        ..CommandMeta::EMPTY
    };
    static META_LIST: CommandMeta = CommandMeta {
        cmd: &LIST,
        about: Some("List everything"),
        hidden_aliases: &["l"],
        ..CommandMeta::EMPTY
    };
    static META_ROOT: CommandMeta = CommandMeta {
        cmd: &ROOT_WITH_META,
        flags: &[FlagMeta {
            flag: &GLOBAL,
            help: Some("Say more"),
            ..FlagMeta::EMPTY
        }],
        subcommands: &[
            &META_USE,
            &META_EXEC,
            &META_PLUGINS,
            &META_SECRET,
            &META_LIST,
            &META_TASK,
            &META_WRAP,
            &META_EDIT,
            &META_PIPE,
            &META_SHIP,
            &META_INSTALL,
        ],
        ..CommandMeta::EMPTY
    };
    static ROOT_WITH_META: Command = Command {
        name: "mise",
        flags: &[&GLOBAL],
        subcommands: &[
            &USE, &EXEC, &PLUGINS, &SECRET, &LIST, &TASK, &WRAP, &EDIT, &PIPE, &SHIP, &INSTALL,
        ],
        ..Command::EMPTY
    };
    static SPEC: Spec = Spec {
        name: "mise",
        bin: Some("mise"),
        root: &META_ROOT,
        // What a word at the root falls back to, as mise's `run` does.
        default_subcommand: Some("u"),
        ..Spec::EMPTY
    };

    /// What a shell would be offered at the end of a line.
    fn offered(line: &str) -> Vec<String> {
        candidates(&SPEC, &at_end(line))
            .into_iter()
            .map(|c| c.value)
            .collect()
    }

    fn runtime_tools(ctx: CompleteCtx<'_>) -> CompletionFuture<'_> {
        Box::pin(async move {
            vec![Candidate::described(
                format!("{}uby", ctx.prefix),
                "from async runtime state",
            )]
        })
    }

    fn labeled_ruby(_ctx: &CompleteCtx<'_>) -> Vec<Candidate<'static>> {
        vec![Candidate::new("ruby").displayed("Ruby runtime")]
    }

    static RUNTIME_COMPLETIONS: [CompletionOverlay<'static>; 1] =
        [CompletionOverlay::asynchronous(
            "use",
            "tool",
            runtime_tools,
        )];
    static LABELED_COMPLETIONS: [CompletionOverlay<'static>; 1] =
        [CompletionOverlay::sync("install", "tool", labeled_ruby)];
    static GLOBAL_RUNTIME_COMPLETIONS: [CompletionOverlay<'static>; 1] =
        [CompletionOverlay::async_any("tool", runtime_tools)];
    static FILE_RUNTIME_COMPLETIONS: [CompletionOverlay<'static>; 1] =
        [CompletionOverlay::asynchronous(
            "edit",
            "file",
            runtime_tools,
        )];
    static PIPE_RUNTIME_COMPLETIONS: [CompletionOverlay<'static>; 1] =
        [CompletionOverlay::asynchronous(
            "pipe",
            "path",
            runtime_tools,
        )];
    static PLUGIN_RUNTIME_COMPLETIONS: [CompletionOverlay<'static>; 1] =
        [CompletionOverlay::asynchronous(
            "plugins",
            "tool",
            runtime_tools,
        )];

    #[test]
    fn a_failed_view_name_fallback_keeps_the_cursor_selected_completer() {
        static DEEP_VIEW: crate::spec::ViewMeta = crate::spec::ViewMeta {
            id: "installer",
            name: "installer",
            bin: "installer",
            // Deliberately deeper than the metadata route below. A stale named-completion
            // request may have enough cursor identity to answer even when its view path cannot
            // be recovered, and the weaker name fallback must not erase that answer.
            root: "install nested",
            all_globals: false,
            globals: &[],
        };
        let split = at_end("mise install r");
        let position = walk(SPEC.root.cmd, split.argv());
        let words = split.argv();
        let command_path: Vec<(&Command<'_>, &[String])> = position
            .path
            .iter()
            .map(|(cmd, start)| (*cmd, words.get(*start..).unwrap_or(&[])))
            .collect();
        let ctx = CompleteCtx {
            words: &split.words,
            cword: split.cword,
            prefix: &split.prefix,
            command_words: command_words(&split, &position),
            command_path: &command_path,
        };

        let found = for_name_at(&SPEC, "tool", &ctx, &position, Some(&DEEP_VIEW))
            .expect("the cursor-selected completer should survive fallback failure");
        assert!(
            found.iter().any(|candidate| candidate.value == "ruby"),
            "{found:?}"
        );
    }

    #[test]
    fn async_overlays_run_only_for_the_field_and_projection_at_the_cursor() {
        let split = at_end("mise use r");
        let answer = run_ready(complete_with(&SPEC, &split, &RUNTIME_COMPLETIONS));
        assert_eq!(
            answer
                .candidates
                .iter()
                .map(|candidate| candidate.value.as_str())
                .collect::<Vec<_>>(),
            ["ruby"]
        );
        assert_eq!(answer.files, None);

        let file = run_ready(complete_with(
            &SPEC,
            &at_end("mise edit "),
            &FILE_RUNTIME_COMPLETIONS,
        ));
        assert_eq!(file.files, Some(Files::Any));

        let before_separator = run_ready(complete_with(
            &SPEC,
            &at_end("mise pipe "),
            &PIPE_RUNTIME_COMPLETIONS,
        ));
        assert!(
            before_separator
                .candidates
                .iter()
                .all(|candidate| candidate.value != "uby"),
            "{before_separator:?}"
        );
        assert_eq!(before_separator.files, None);

        let after_separator = run_ready(complete_with(
            &SPEC,
            &at_end("mise pipe -- "),
            &PIPE_RUNTIME_COMPLETIONS,
        ));
        assert!(
            after_separator
                .candidates
                .iter()
                .any(|candidate| candidate.value == "uby"),
            "{after_separator:?}"
        );
        assert_eq!(after_separator.files, Some(Files::Any));

        let flag = run_ready(complete_with(
            &SPEC,
            &at_end("mise use -"),
            &RUNTIME_COMPLETIONS,
        ));
        assert!(
            flag.candidates
                .iter()
                .all(|candidate| candidate.value != "-uby"),
            "{flag:?}"
        );

        let implied = run_ready(complete_with(
            &SPEC,
            &at_end("mise r"),
            &RUNTIME_COMPLETIONS,
        ));
        assert!(
            implied
                .candidates
                .iter()
                .any(|candidate| candidate.value == "ruby"),
            "{implied:?}"
        );

        static FILE_DEFAULT_SPEC: Spec = Spec {
            name: "mise",
            bin: Some("mise"),
            root: &META_ROOT,
            default_subcommand: Some("edit"),
            ..Spec::EMPTY
        };
        let implied_file = run_ready(complete_with(
            &FILE_DEFAULT_SPEC,
            &at_end("mise m"),
            &FILE_RUNTIME_COMPLETIONS,
        ));
        assert_eq!(implied_file.files, Some(Files::Any));

        let named = run_ready(complete_named_with(
            &SPEC,
            &at_end("mise plugins "),
            &GLOBAL_RUNTIME_COMPLETIONS,
            "tool",
        ));
        assert!(
            named
                .candidates
                .iter()
                .any(|candidate| candidate.value == "uby"),
            "{named:?}"
        );
        let named_at_root = run_ready(complete_named_with(
            &SPEC,
            &at_end("mise "),
            &GLOBAL_RUNTIME_COMPLETIONS,
            "tool",
        ));
        assert!(
            named_at_root
                .candidates
                .iter()
                .any(|candidate| candidate.value == "uby"),
            "{named_at_root:?}"
        );

        let named_on_owner = run_ready(complete_named_with(
            &SPEC,
            &at_end("mise plugins "),
            &PLUGIN_RUNTIME_COMPLETIONS,
            "tool",
        ));
        assert!(
            named_on_owner
                .candidates
                .iter()
                .any(|candidate| candidate.value == "uby"),
            "{named_on_owner:?}"
        );
        let named_on_descendant = run_ready(complete_named_with(
            &SPEC,
            &at_end("mise plugins ls "),
            &PLUGIN_RUNTIME_COMPLETIONS,
            "tool",
        ));
        assert!(
            named_on_descendant
                .candidates
                .iter()
                .all(|candidate| candidate.value != "uby"),
            "{named_on_descendant:?}"
        );

        let argv = [
            OsString::from("__complete_word__"),
            OsString::from("--shell"),
            OsString::from("bash"),
            OsString::from("--line"),
            OsString::from("miser r"),
        ];
        let rendered = run_ready(
            SPEC.view()
                .name("miser")
                .bin("miser")
                .completion_app()
                .completions(&RUNTIME_COMPLETIONS)
                .project("use")
                .completion_request(&argv),
        );
        assert_eq!(rendered.as_deref(), Some("ruby\n"));

        let binary_argv = [
            OsString::from("__complete_word__"),
            OsString::from("--shell"),
            OsString::from("bash"),
            OsString::from("--line"),
            OsString::from("mis"),
        ];
        let without_projection = run_ready(
            SPEC.view()
                .completion_app()
                .completion_request(&binary_argv),
        );
        let with_projection = run_ready(
            SPEC.view()
                .completion_app()
                .project("use")
                .completion_request(&binary_argv),
        );
        assert_eq!(with_projection, without_projection);

        static ROOT_ARG: Command = Command {
            name: "root-arg",
            args: &[&TOOL],
            ..Command::EMPTY
        };
        static ROOT_ARG_META: CommandMeta = CommandMeta {
            cmd: &ROOT_ARG,
            args: &[ArgMeta {
                arg: &TOOL,
                ..ArgMeta::EMPTY
            }],
            ..CommandMeta::EMPTY
        };
        static ROOT_ARG_SPEC: Spec = Spec {
            name: "root-arg",
            root: &ROOT_ARG_META,
            ..Spec::EMPTY
        };
        let argv0 = run_ready(complete_with(
            &ROOT_ARG_SPEC,
            &at_end("root"),
            &GLOBAL_RUNTIME_COMPLETIONS,
        ));
        assert!(
            argv0
                .candidates
                .iter()
                .all(|candidate| candidate.value != "rootuby"),
            "{argv0:?}"
        );

        let unrelated = at_end("mise plugins ");
        let answer = run_ready(complete_with(&SPEC, &unrelated, &RUNTIME_COMPLETIONS));
        assert_eq!(answer.candidates[0].value, "ls");
    }

    #[test]
    fn cursor_completion_keeps_presentation_metadata_from_duplicate_values() {
        let answer = run_ready(complete_with(
            &SPEC,
            &at_end("mise install r"),
            &LABELED_COMPLETIONS,
        ));
        assert_eq!(answer.candidates.len(), 1, "{answer:?}");
        assert_eq!(answer.candidates[0].value, "ruby");
        assert_eq!(
            answer.candidates[0].display.as_deref(),
            Some("Ruby runtime")
        );
    }

    #[test]
    fn named_completion_keeps_presentation_metadata_from_duplicate_values() {
        let answer = run_ready(complete_named_with(
            &SPEC,
            &at_end("mise install r"),
            &LABELED_COMPLETIONS,
            "tool",
        ));
        assert_eq!(answer.candidates.len(), 1, "{answer:?}");
        assert_eq!(answer.candidates[0].value, "ruby");
        assert_eq!(
            answer.candidates[0].display.as_deref(),
            Some("Ruby runtime")
        );
    }

    /// The position at the cursor of a line, which is what a completion asks about.
    fn position_at(line: &str) -> Position<'static> {
        let s = at_end(line);
        walk(&ROOT, s.argv())
    }

    #[test]
    fn the_cursor_is_in_the_command_the_words_reached() {
        assert_eq!(position_at("mise ").cmd.name, "mise");
        assert_eq!(position_at("mise plugins ").cmd.name, "plugins");
        assert_eq!(position_at("mise plugins ls ").cmd.name, "ls");
        // A half-typed word names nothing yet, so it is not descended into: the cursor is
        // still in the command before it.
        assert_eq!(position_at("mise plug").cmd.name, "mise");
    }

    #[test]
    fn a_flag_that_takes_a_value_puts_the_cursor_in_it() {
        // The parser calls this a missing value; to a completion it is the question being
        // asked, and the flag is what decides the answer.
        let p = position_at("mise use --jobs ");
        assert_eq!(p.awaiting_value.map(|f| f.name), Some("jobs"));

        // A flag that takes none does not, and neither does one already given its value.
        assert!(position_at("mise use --jobs 4 ").awaiting_value.is_none());
        assert!(position_at("mise --verbose ").awaiting_value.is_none());
    }

    #[test]
    fn declared_builtin_actions_do_not_turn_completion_into_a_help_topic() {
        static ASSIST: Flag = Flag {
            key: 100,
            name: "assist",
            longs: &["assist"],
            action: crate::ArgAction::Help,
            ..Flag::BOOL
        };
        static REVISION: Flag = Flag {
            key: 101,
            name: "revision",
            longs: &["revision"],
            action: crate::ArgAction::Version,
            ..Flag::BOOL
        };
        static TARGET: Arg = Arg {
            key: 102,
            name: "TARGET",
            ..Arg::REQUIRED
        };
        static APP: Command = Command {
            name: "app",
            flags: &[&ASSIST, &REVISION],
            args: &[&TARGET],
            ..Command::EMPTY
        };

        for action in ["--assist", "--revision"] {
            let position = walk(&APP, &[action.to_string(), "filled".to_string()]);
            assert!(!position.help_topic, "{action} became a help topic");
            assert_eq!(position.next_arg, None, "{action} stopped the walk early");
            assert!(position.flags_possible);
        }
    }

    #[test]
    fn a_variadic_flag_still_claiming_words_holds_the_cursor() {
        // `mise use --tools node ⌶` — the next word is another tool, because the parser would
        // bind it to the flag rather than to the positional after it. The state saying so is
        // cleared by the very call that ends the walk, so it has to be read on the way.
        let p = position_at("mise use --tools node ");
        assert_eq!(p.awaiting_value.map(|f| f.name), Some("tools"));

        // Until something that could not be a value comes along, which is when the parser
        // stops claiming too.
        let p = position_at("mise use --tools node -- ");
        assert!(p.awaiting_value.is_none());
    }

    #[test]
    fn the_next_positional_is_the_one_a_word_would_fill() {
        assert_eq!(
            position_at("mise use ").next_arg.map(|a| a.name),
            Some("TOOL")
        );
        // Filled, so there is nothing left for a second word to go into.
        assert!(position_at("mise use node ").next_arg.is_none());
        // A variadic keeps taking values, so it is still the answer.
        assert_eq!(
            position_at("mise exec a b ").next_arg.map(|a| a.name),
            Some("ARGS")
        );
    }

    #[test]
    fn flags_stop_being_possible_where_the_parser_stops_reading_them() {
        assert!(position_at("mise use ").flags_possible);
        assert!(!position_at("mise use -- ").flags_possible);
        // And past an `automatic` argument's first value, which is a wrapper forwarding to
        // another tool: the flags there are the other tool's, and this CLI has none to offer.
        assert!(!position_at("mise exec node ").flags_possible);
        assert!(position_at("mise exec ").flags_possible);
    }

    #[test]
    fn a_help_topic_puts_the_cursor_under_the_command_it_named() {
        // `mise help plugins ⌶` asks which command to read about, and the candidates are
        // `plugins`'s own — not the root's, which is where the parser stayed, since a topic is
        // resolved without being descended into.
        let p = position_at("mise help plugins ");
        assert_eq!(p.cmd.name, "plugins");
        assert!(!p.flags_possible, "a topic takes no flags");
        assert!(p.next_arg.is_none(), "and fills no argument");

        // And with no topic yet, the cursor is under the command `help` was typed in.
        assert_eq!(position_at("mise help ").cmd.name, "mise");
    }

    #[test]
    fn a_global_flag_is_in_scope_inside_a_subcommand() {
        // What the parser would accept there is what should be offered there — one rule, read
        // from the same tables, rather than two that can disagree.
        let names: Vec<_> = {
            let argv = [std::ffi::OsStr::new("plugins")];
            let mut parser = Parser::new(&ROOT, &argv);
            while parser.next_event().is_some() {}
            parser.flags_in_scope().map(|f| f.name).collect()
        };
        assert!(names.contains(&"verbose"), "{names:?}");
    }

    fn at_end(line: &str) -> Split {
        split(line, line.len(), Shell::Bash)
    }

    #[test]
    fn a_line_splits_into_the_words_the_shell_would_have_passed() {
        let s = at_end("mise use node");
        assert_eq!(s.words, ["mise", "use", "node"]);
        assert_eq!(s.cword, 2);
        assert_eq!(s.prefix, "node");
    }

    #[test]
    fn a_cursor_after_a_space_is_completing_a_word_that_does_not_exist_yet() {
        // The common case — Tab pressed to ask "what can go here?" — and the one an empty
        // word exists for: without it a caller cannot tell "anything" from "nothing".
        let s = at_end("mise use ");
        assert_eq!(s.words, ["mise", "use", ""]);
        assert_eq!(s.cword, 2);
        assert_eq!(s.prefix, "");
    }

    #[test]
    fn a_cursor_inside_a_word_completes_that_word_and_keeps_the_rest_of_the_line() {
        // `mise us|e node` — the tail is still part of the line, because the parser walking
        // left to right may need it, but the word being completed is `us`.
        let line = "mise use node";
        let s = split(line, 7, Shell::Bash);
        assert_eq!(s.words, ["mise", "use", "node"]);
        assert_eq!(s.cword, 1);
        assert_eq!(s.prefix, "us");
        assert_eq!(s.walked(), ["mise", "use"]);
    }

    #[test]
    fn a_quoted_space_stays_inside_its_word() {
        // The reason for taking a line rather than the shell's own split: by the time a shell
        // has split, the quote that made this one word is gone.
        let s = at_end(r#"mise run "my task"#);
        assert_eq!(s.words, ["mise", "run", "my task"]);
        assert_eq!(s.prefix, "my task");

        let s = at_end("mise run 'my task");
        assert_eq!(s.words, ["mise", "run", "my task"]);
        assert_eq!(s.prefix, "my task");
    }

    #[test]
    fn an_empty_quote_is_a_word() {
        let s = at_end(r#"mise run "" "#);
        assert_eq!(s.words, ["mise", "run", "", ""]);
        assert_eq!(s.cword, 3);
    }

    #[test]
    fn a_backslash_escapes_the_character_after_it() {
        let s = at_end(r"mise run my\ task");
        assert_eq!(s.words, ["mise", "run", "my task"]);

        // And a trailing one escapes the character not yet typed, rather than being an error:
        // the user is mid-word, which is the only state this function is ever called in.
        let s = at_end(r"mise run my\");
        assert_eq!(s.words, ["mise", "run", "my"]);
        assert_eq!(s.prefix, "my");
    }

    #[test]
    fn a_single_quote_keeps_a_backslash_literal() {
        // Which is what makes a Windows path survive being completed.
        let s = at_end(r"mise use 'C:\Users\me");
        assert_eq!(s.words, ["mise", "use", r"C:\Users\me"]);

        // In double quotes a backslash escapes only what it could mean something to, so the
        // same path survives there too.
        let s = at_end(r#"mise use "C:\Users\me"#);
        assert_eq!(s.words, ["mise", "use", r"C:\Users\me"]);

        let s = at_end(r#"mise use "say \"hi"#);
        assert_eq!(s.words, ["mise", "use", r#"say "hi"#]);
    }

    #[test]
    fn powershell_escapes_with_a_backtick() {
        let s = split("mise run my` task", 17, Shell::PowerShell);
        assert_eq!(s.words, ["mise", "run", "my task"]);

        // And a backslash is an ordinary character there, which is the point: a path typed in
        // PowerShell is full of them.
        let s = split(r"mise use C:\Users\me", 20, Shell::PowerShell);
        assert_eq!(s.words, ["mise", "use", r"C:\Users\me"]);
    }

    #[test]
    fn a_cursor_in_a_gap_completes_a_word_that_is_not_there_yet() {
        // `mise ⌶use` asks what can go *before* `use` — a word the line does not contain. It
        // has to be made, or the question is answered about `use`, which is a different one.
        let s = split("mise use", 5, Shell::Bash);
        assert_eq!(s.words, ["mise", "", "use"]);
        assert_eq!(s.cword, 1);
        assert_eq!(s.prefix, "");
        assert_eq!(s.walked(), ["mise", ""]);

        // Same in a run of spaces, where there is no character at the cursor either way.
        let s = split("mise   use", 6, Shell::Bash);
        assert_eq!(s.words, ["mise", "", "use"]);
        assert_eq!(s.cword, 1);
    }

    #[test]
    fn a_cursor_on_an_escaped_character_is_still_in_its_word() {
        // `mise run my\ ⌶task` — the cursor sits on the character the escape swallowed, which
        // the loop never stops at on its own. Missing it made the split describe the last word
        // of the line rather than the one being typed.
        // The backslash is at 11 and the space it escapes at 12, which is where the cursor is.
        let line = r"mise run my\ task and more";
        let s = split(line, 12, Shell::Bash);
        assert_eq!(s.cword, 2);
        assert_eq!(s.prefix, "my");
        assert_eq!(s.words, ["mise", "run", "my task", "and", "more"]);

        // The same inside double quotes, where the escape applies to the quote it precedes.
        let line = r#"mise run "say \"hi" then"#;
        let s = split(line, 15, Shell::Bash);
        assert_eq!(s.cword, 2);
        assert_eq!(s.prefix, "say ");
    }

    #[test]
    fn powershell_writes_a_quote_by_doubling_it() {
        // Documented as PowerShell's rule, and now done: `''` inside a quoted string is one
        // quote rather than the end of the string and the start of another.
        let s = split("mise run 'it''s here", 20, Shell::PowerShell);
        assert_eq!(s.words, ["mise", "run", "it's here"]);
        assert_eq!(s.prefix, "it's here");

        let s = split(r#"mise run "say ""hi"#, 18, Shell::PowerShell);
        assert_eq!(s.words, ["mise", "run", r#"say "hi"#]);

        // And not POSIX's rule, in either quote: there a second quote ends the string, so
        // `'it''s` is two pieces of one word rather than an escaped quote.
        let s = split("mise run 'it''s", 15, Shell::Bash);
        assert_eq!(s.words, ["mise", "run", "its"]);
        let s = split(r#"mise run "it""s"#, 15, Shell::Bash);
        assert_eq!(s.words, ["mise", "run", "its"]);
    }

    #[test]
    fn an_empty_line_is_still_completing_something() {
        let s = at_end("");
        assert_eq!(s.words, [""]);
        assert_eq!(s.cword, 0);
        assert_eq!(s.prefix, "");
        assert_eq!(s.walked(), [""]);
    }

    #[test]
    fn a_cursor_off_the_end_or_inside_a_character_lands_somewhere_sensible() {
        // A shell's idea of a byte offset is not always ours — nushell counts spans, and a
        // reconstructed line can be a byte or two off. Being wrong is better than panicking in
        // a keystroke handler.
        let s = split("mise use", 999, Shell::Bash);
        assert_eq!(s.prefix, "use");

        // `ü` is two bytes, at 5 and 6, so 7 is the boundary after it and 6 is inside it.
        let line = "mise ünicode";
        let s = split(line, 7, Shell::Bash);
        assert_eq!(s.cword, 1);
        assert_eq!(s.prefix, "ü");
        // Mid-character, floored to the start of the `ü` rather than splitting it.
        let s = split(line, 6, Shell::Bash);
        assert_eq!(s.cword, 1);
        assert_eq!(s.prefix, "");
    }
    #[test]
    fn a_word_that_could_name_a_command_offers_the_commands() {
        assert_eq!(
            offered("mise "),
            // `node` and `python` are the fallback command's, which the root offers too — see
            // `the_root_offers_what_the_command_it_falls_back_to_accepts`.
            [
                "edit", "exec", "install", "list", "ls", "node", "pipe", "plugins", "python",
                "ship", "task", "u", "use", "wrap",
            ],
            "sorted, and a hidden command is offered under none of its names"
        );
        assert_eq!(offered("mise pl"), ["plugins"]);
        // An alias is a name someone types meaning the command, so it is offered too — but
        // not a hidden one, which the parser answers to and nothing advertises.
        assert_eq!(offered("mise l"), ["list", "ls"]);
        assert_eq!(offered("mise plugins "), ["ls"]);
    }

    #[test]
    fn a_dash_offers_the_flags_that_would_be_accepted_there() {
        // Both forms for a lone dash, since it says nothing about which was meant.
        assert_eq!(offered("mise -"), ["--quiet", "--verbose", "-v"]);
        assert_eq!(offered("mise --"), ["--quiet", "--verbose"]);

        // Inside a subcommand: its own flags and the globals it inherits, which is exactly
        // what the parser would accept there.
        assert_eq!(
            offered("mise use --"),
            ["--jobs", "--quiet", "--tools", "--verbose"]
        );
        // A short token asks about one letter, because bundling means any other letter would
        // be a candidate for a different position in the token.
        assert_eq!(offered("mise use -v"), ["-v"]);
        assert!(
            offered("mise use -j").is_empty(),
            "--jobs has no short form"
        );
    }

    #[test]
    fn a_flag_waiting_for_its_value_offers_that_flags_choices() {
        assert_eq!(offered("mise use --jobs "), ["1", "2", "4"]);
        assert_eq!(offered("mise use --jobs 2"), ["2"]);
        // And nothing else: the subcommands and the positional are not candidates for a
        // value that a flag has already claimed.
        assert!(!offered("mise use --jobs ").contains(&"node".to_string()));
    }

    #[test]
    fn a_positional_offers_its_choices() {
        assert_eq!(offered("mise use "), ["node", "python"]);
        assert_eq!(offered("mise use p"), ["python"]);
    }

    #[test]
    fn past_a_separator_a_dash_is_not_a_flag() {
        // `--` stops flag interpretation, so there is no flag to offer — the word there is a
        // value, and the only candidates are what a value could be.
        assert!(offered("mise use -- -").is_empty());
        // The fallback command's values are still offered past a separator: `--` says the
        // word is not a flag, not that it is not a word.
        assert_eq!(
            offered("mise -- "),
            [
                "edit", "exec", "install", "list", "ls", "node", "pipe", "plugins", "python",
                "ship", "task", "u", "use", "wrap",
            ]
        );
    }

    #[test]
    fn a_candidate_carries_the_help_a_page_would_print() {
        let found = candidates(&SPEC, &at_end("mise use --"));
        let jobs = found.iter().find(|c| c.value == "--jobs").expect("--jobs");
        assert_eq!(jobs.description.as_deref(), Some("How many at once"));

        let found = candidates(&SPEC, &at_end("mise pl"));
        assert_eq!(found[0].description.as_deref(), Some("Manage plugins"));

        let found = candidates(&SPEC, &at_end("mise use --jobs "));
        let two = found.iter().find(|c| c.value == "2").expect("choice 2");
        assert_eq!(two.description.as_deref(), Some("Two workers"));
    }
    #[test]
    fn a_help_topic_offers_the_commands_under_it() {
        // The candidate half of the same rule: after `mise help plugins ⌶` the useful answer
        // is what can be read about under `plugins`, and nothing else belongs there.
        assert_eq!(offered("mise help plugins "), ["ls"]);
        // And nothing the root would fall back to: `mise help ⌶` is asking for a command to
        // read about, not for a word to run.
        assert_eq!(
            offered("mise help "),
            [
                "edit", "exec", "install", "list", "ls", "pipe", "plugins", "ship", "task", "u",
                "use", "wrap"
            ]
        );
    }
    #[test]
    fn a_negation_is_offered_the_way_it_is_typed() {
        // The table holds it without dashes, because that is what the parser matches once a
        // token's `--` has been taken off. A candidate is what the user types, so it needs
        // them back — offered bare it matched no `--` prefix and no shell would accept it.
        assert!(offered("mise --").contains(&"--quiet".to_string()));
        assert_eq!(offered("mise --q"), ["--quiet"]);
    }

    #[test]
    fn a_restart_token_puts_the_cursor_back_at_the_first_argument() {
        // `mise task one ::: ⌶` starts a fresh invocation of the same command, so the words
        // before the token do not decide what comes after it: the cursor is back at the first
        // argument, not at the second one the words had reached.
        assert_eq!(offered("mise task one "), ["alpha", "beta"]);
        assert_eq!(offered("mise task one ::: "), ["one", "two"]);
        assert_eq!(offered("mise task one ::: t"), ["two"]);
    }

    #[test]
    fn the_root_offers_what_the_command_it_falls_back_to_accepts() {
        // `mise build` means `mise run build`, so a word at the root may be meant for the
        // default subcommand — and what its first argument accepts belongs here too, beside
        // the subcommand names themselves.
        let found = offered("mise ");
        assert!(found.contains(&"use".to_string()), "{found:?}");
        assert!(found.contains(&"node".to_string()), "{found:?}");
        assert!(found.contains(&"python".to_string()), "{found:?}");

        // Filtered like everything else, and not offered inside a subcommand: the fallback is
        // the root's rule.
        assert_eq!(offered("mise n"), ["node"]);
        assert!(!offered("mise plugins ").contains(&"node".to_string()));
    }
    #[test]
    fn an_argument_that_needs_a_separator_offers_the_separator() {
        // `mise wrap ⌶` cannot take `red` yet — the parser rejects every value until a `--`
        // has been typed — so offering the choices would be offering words that do not work.
        assert_eq!(offered("mise wrap "), ["--"]);
        // Nothing once something has been typed: the separator no longer matches, and the
        // values are still not fillable.
        assert!(offered("mise wrap r").is_empty());
        // Past the separator they are.
        assert_eq!(offered("mise wrap -- "), ["blue", "red"]);
        assert_eq!(offered("mise wrap -- r"), ["red"]);
    }

    #[test]
    fn the_fallback_command_is_found_by_any_name_it_answers_to() {
        // `default_subcommand` names a command, and a spec may name it the way its author
        // refers to it — here `u` rather than `use`. Resolving only canonical names silently
        // dropped the fallback's values.
        let found = offered("mise ");
        assert!(found.contains(&"node".to_string()), "{found:?}");
    }

    #[test]
    fn the_fallback_command_prefers_a_name_to_another_commands_alias() {
        // Resolved the way a typed word is, so what a shell offers at the root is the first
        // argument of the command the parser would actually route the word to. Matching name
        // and alias in one pass offered `alpha`'s values for a line the parse sends to `run`.
        static A_ARG: Arg = Arg {
            key: 90,
            name: "a",
            ..Arg::REQUIRED
        };
        static B_ARG: Arg = Arg {
            key: 91,
            name: "b",
            ..Arg::REQUIRED
        };
        static ALPHA: Command = Command {
            name: "alpha",
            aliases: &["run"],
            args: &[&A_ARG],
            ..Command::EMPTY
        };
        static PLAIN_RUN: Command = Command {
            name: "run",
            args: &[&B_ARG],
            ..Command::EMPTY
        };
        static META_ALPHA: CommandMeta = CommandMeta {
            cmd: &ALPHA,
            args: &[ArgMeta {
                arg: &A_ARG,
                choices: &["from-alpha"],
                ..ArgMeta::EMPTY
            }],
            ..CommandMeta::EMPTY
        };
        static META_PLAIN_RUN: CommandMeta = CommandMeta {
            cmd: &PLAIN_RUN,
            args: &[ArgMeta {
                arg: &B_ARG,
                choices: &["from-run"],
                ..ArgMeta::EMPTY
            }],
            ..CommandMeta::EMPTY
        };
        static EX: Command = Command {
            name: "ex",
            subcommands: &[&ALPHA, &PLAIN_RUN],
            ..Command::EMPTY
        };
        static META_EX: CommandMeta = CommandMeta {
            cmd: &EX,
            subcommands: &[&META_ALPHA, &META_PLAIN_RUN],
            ..CommandMeta::EMPTY
        };
        static EX_SPEC: Spec = Spec {
            name: "ex",
            bin: Some("ex"),
            root: &META_EX,
            default_subcommand: Some("run"),
            ..Spec::EMPTY
        };

        let found: Vec<String> = candidates(&EX_SPEC, &at_end("ex "))
            .into_iter()
            .map(|c| c.value)
            .collect();
        assert!(found.contains(&"from-run".to_string()), "{found:?}");
        assert!(!found.contains(&"from-alpha".to_string()), "{found:?}");
    }

    /// The whole answer for a line: what this CLI knows, and whether paths belong.
    fn answer(line: &str) -> Completions<'static> {
        complete(&SPEC, &at_end(line))
    }

    #[test]
    fn a_word_named_like_a_path_asks_the_shell_for_paths() {
        // The reference resolves a completer by the lowercased name and treats that name as the
        // type when nothing else says otherwise, so `<FILE>` completes files without the spec
        // mentioning it. Same rule, read off the same name.
        assert_eq!(answer("mise edit ").files, Some(Files::Any));
        // Even though it has choices of its own, which would otherwise close the position: a
        // path argument that names two well-known files still takes any other path.
        assert_eq!(offered("mise edit "), ["mise.local.toml", "mise.toml"]);
        // And a flag's value is named by its placeholder, not by the flag.
        assert_eq!(answer("mise edit --into ").files, Some(Files::Dirs));
    }

    #[test]
    fn extension_types_preserve_the_filter_for_the_shell() {
        assert_eq!(
            declared_files("path:toml,.yaml", 0),
            Some(Files::Extensions(vec![
                "toml".to_string(),
                "yaml".to_string()
            ]))
        );
        let answer = Completions {
            candidates: Vec::new(),
            files: declared_files("path:toml,yaml", 0),
        };
        assert_eq!(
            render(&answer, Shell::Bash),
            "\u{1}extensions\ttoml\tyaml\n"
        );
    }

    #[test]
    fn executable_paths_and_command_names_are_distinct_shell_requests() {
        assert_eq!(files_for("executable"), Some(Files::ExecutablePaths));
        assert_eq!(files_for("command"), Some(Files::Commands));
    }

    #[test]
    fn open_ended_value_hints_suppress_path_fallback() {
        static URL: Arg = Arg {
            key: 80,
            name: "URL",
            ..Arg::REQUIRED
        };
        static ROOT: Command = Command {
            name: "ex",
            args: &[&URL],
            ..Command::EMPTY
        };
        static URL_META: CommandMeta = CommandMeta {
            cmd: &ROOT,
            args: &[ArgMeta {
                arg: &URL,
                complete_type: Some("url"),
                ..ArgMeta::EMPTY
            }],
            ..CommandMeta::EMPTY
        };
        static URL_SPEC: Spec = Spec {
            name: "ex",
            bin: Some("ex"),
            root: &URL_META,
            ..Spec::EMPTY
        };
        static UNKNOWN_META: CommandMeta = CommandMeta {
            cmd: &ROOT,
            args: &[ArgMeta {
                arg: &URL,
                complete_type: Some("unknown"),
                ..ArgMeta::EMPTY
            }],
            ..CommandMeta::EMPTY
        };
        static UNKNOWN_SPEC: Spec = Spec {
            name: "ex",
            bin: Some("ex"),
            root: &UNKNOWN_META,
            ..Spec::EMPTY
        };

        assert_eq!(complete(&URL_SPEC, &at_end("ex ")).files, None);
        assert_eq!(
            complete(&UNKNOWN_SPEC, &at_end("ex ")).files,
            Some(Files::Any)
        );
    }

    #[test]
    fn a_position_that_knows_its_answers_does_not_ask_for_paths() {
        // Offering the working directory beside a known set is how a mistyped choice completes
        // to whatever happened to be lying around.
        assert_eq!(answer("mise use ").files, None);
        assert_eq!(answer("mise plugins ").files, None);
        // Nor for a flag, which no path starts with — including one that matches nothing, where
        // there is otherwise no candidate to suppress the fallback.
        assert_eq!(answer("mise use --").files, None);
        assert_eq!(answer("mise -").files, None);
        // And a prefix that matches none of a declared set: the set is still the whole answer,
        // so "nothing matched what you typed" must not become "here is the working directory".
        let a = answer("mise use nodx");
        assert!(a.candidates.is_empty());
        assert_eq!(a.files, None);
        // A mistyped *command*, though, does fall back — nothing declared a set there, and the
        // reference offers paths for the same reason.
        assert_eq!(answer("mise plugni").files, Some(Files::Any));

        let a = answer("mise use --zzz");
        assert!(a.candidates.is_empty());
        assert_eq!(a.files, None);
        // Nor for a help topic, which is a command name and nothing else.
        assert_eq!(answer("mise help ").files, None);
    }

    #[test]
    fn hidden_only_choices_still_close_the_position() {
        static VALUE: Arg = Arg {
            key: 90,
            name: "VALUE",
            ..Arg::REQUIRED
        };
        static ROOT: Command = Command {
            name: "hidden",
            args: &[&VALUE],
            ..Command::EMPTY
        };
        static META: CommandMeta = CommandMeta {
            cmd: &ROOT,
            args: &[ArgMeta {
                arg: &VALUE,
                accepted_choices: &["secret"],
                ..ArgMeta::EMPTY
            }],
            ..CommandMeta::EMPTY
        };
        static HIDDEN_SPEC: Spec = Spec {
            name: "hidden",
            bin: Some("hidden"),
            root: &META,
            ..Spec::EMPTY
        };

        let answer = complete(&HIDDEN_SPEC, &at_end("hidden sec"));
        assert!(answer.candidates.is_empty());
        assert_eq!(answer.files, None);
    }

    #[test]
    fn a_position_with_nothing_to_say_lets_the_shell_answer() {
        // `mise edit some-file ⌶` has filled its only argument and has no subcommands, so this
        // CLI has nothing to say — and a path is the shell's best guess, which is the
        // reference's fallback too.
        let a = answer("mise edit some-file ");
        assert!(a.candidates.is_empty(), "{:?}", a.candidates);
        assert_eq!(a.files, Some(Files::Any));
    }

    #[test]
    fn a_path_that_needs_a_separator_is_still_not_a_path_yet() {
        // Named `<PATH>`, so paths are what it takes — but not until the separator is typed,
        // because until then the parser rejects a path exactly as it rejects any other value.
        let a = answer("mise pipe ");
        assert_eq!(a.files, None);
        assert_eq!(a.candidates.len(), 1, "the separator, and nothing else");
        assert_eq!(a.candidates[0].value, "--");

        // Past it, they are.
        assert_eq!(answer("mise pipe -- ").files, Some(Files::Any));

        // And a flag waiting for its value is a different position that happens to have that
        // argument behind it: `--from ⌶` takes a path whatever the positional after it needs.
        assert_eq!(answer("mise pipe --from ").files, Some(Files::Any));
    }

    #[test]
    fn an_argument_that_needs_a_separator_asks_for_nothing_else() {
        // The separator is the only useful candidate, and a path is not one: the parser would
        // reject it until the `--` is typed.
        let a = answer("mise wrap ");
        assert_eq!(a.candidates.len(), 1);
        assert_eq!(a.files, None);
    }
    #[test]
    fn a_restart_asks_about_the_first_argument_for_paths_too() {
        // Both halves of an answer have to agree about which argument the cursor is at. `ship`
        // takes a `MODE` and then a `FILE`, so its two arguments want different things: the first
        // declares its whole set, the second takes any path.
        let first = answer("mise ship ");
        assert_eq!(first.files, None, "MODE declares its set");
        assert_eq!(
            first
                .candidates
                .iter()
                .map(|c| c.value.as_str())
                .collect::<Vec<_>>(),
            ["fast", "slow"]
        );
        assert_eq!(
            answer("mise ship fast ").files,
            Some(Files::Any),
            "FILE takes paths"
        );

        // Past the restart token the cursor is back at the first argument, whatever the words
        // before it filled — so a prefix matching none of `MODE`'s set means no matches, not
        // "here is the working directory".
        let after = answer("mise ship fast ::: ");
        assert_eq!(after.files, None, "back at MODE, which declares its set");
        assert_eq!(
            after
                .candidates
                .iter()
                .map(|c| c.value.as_str())
                .collect::<Vec<_>>(),
            ["fast", "slow"]
        );
        let mistyped = answer("mise ship fast ::: zzz");
        assert!(mistyped.candidates.is_empty());
        assert_eq!(mistyped.files, None, "a mistyped choice is still a choice");
    }

    #[test]
    fn a_restart_makes_command_args_expect_a_command_again() {
        assert_eq!(answer("mise exec ").files, Some(Files::Commands));
        assert_eq!(answer("mise exec one ").files, Some(Files::Any));
        assert_eq!(answer("mise exec one ::: ").files, Some(Files::Commands));
    }

    #[test]
    fn each_shell_is_written_the_way_it_reads() {
        let answer = complete(&SPEC, &at_end("mise pl"));

        // bash shows values and nothing else.
        assert_eq!(render(&answer, Shell::Bash), "plugins\n");

        // fish and nu take a description after a tab.
        assert_eq!(render(&answer, Shell::Fish), "plugins\tManage plugins\n");

        // PowerShell also receives its separately selectable display text.
        assert_eq!(
            render(&answer, Shell::PowerShell),
            "plugins\tManage plugins\tplugins\n"
        );

        // zsh takes a third field: what to type, which is not always what is shown.
        assert_eq!(
            render(&answer, Shell::Zsh),
            "plugins\tManage plugins\tplugins\n"
        );
    }

    #[test]
    fn display_text_does_not_change_what_the_shell_inserts() {
        let answer = Completions {
            candidates: vec![Candidate::described("iad", "US East").displayed("IAD · Virginia")],
            files: None,
        };

        assert_eq!(render(&answer, Shell::Bash), "iad\n");
        assert_eq!(render(&answer, Shell::Fish), "iad\tUS East\n");
        assert_eq!(
            render(&answer, Shell::PowerShell),
            "iad\tUS East\tIAD · Virginia\n"
        );
        assert_eq!(
            render(&answer, Shell::Zsh),
            "IAD · Virginia\tUS East\tiad\n"
        );
    }

    #[test]
    fn a_candidate_a_shell_could_not_read_is_quoted_for_zsh() {
        static ODD: Command = Command {
            name: "with space",
            ..Command::EMPTY
        };
        static ODD_META: CommandMeta = CommandMeta {
            cmd: &ODD,
            about: Some("Odd"),
            ..CommandMeta::EMPTY
        };
        static ODD_ROOT: Command = Command {
            name: "ex",
            subcommands: &[&ODD],
            ..Command::EMPTY
        };
        static ODD_ROOT_META: CommandMeta = CommandMeta {
            cmd: &ODD_ROOT,
            subcommands: &[&ODD_META],
            ..CommandMeta::EMPTY
        };
        static ODD_SPEC: Spec = Spec {
            name: "ex",
            bin: Some("ex"),
            root: &ODD_ROOT_META,
            ..Spec::EMPTY
        };

        let answer = complete(&ODD_SPEC, &split("ex ", 3, Shell::Zsh));
        let line = render(&answer, Shell::Zsh);
        // Shown as it is, typed as the shell needs it.
        assert!(
            line.starts_with("with space\tOdd\t'with space'"),
            "{line:?}"
        );
    }

    #[test]
    fn the_marker_is_the_last_line_when_paths_belong() {
        let answer = complete(&SPEC, &at_end("mise edit "));
        let out = render(&answer, Shell::Bash);
        assert_eq!(out.lines().last(), Some(FILES_MARKER));
        // And the candidates are still there in front of it.
        assert!(out.starts_with("mise.local.toml\nmise.toml\n"), "{out:?}");

        let answer = complete(&SPEC, &at_end("mise edit --into "));
        assert_eq!(
            render(&answer, Shell::Fish).lines().last(),
            Some(DIRS_MARKER)
        );

        // And absent where paths do not belong, rather than saying "none".
        let answer = complete(&SPEC, &at_end("mise use "));
        let out = render(&answer, Shell::Bash);
        assert!(!out.contains('\u{1}'), "{out:?}");
    }
    #[test]
    fn a_description_written_across_lines_stays_one_row() {
        // One line per candidate and tabs between fields, so a description containing either
        // would split one candidate into several rows or invent a field. mise writes multi-line
        // `help` on 37 of its commands and flags, so this is the ordinary case.
        static WORDY: Command = Command {
            name: "wordy",
            ..Command::EMPTY
        };
        static WORDY_META: CommandMeta = CommandMeta {
            cmd: &WORDY,
            about: Some("First line\nsecond line\n\nand a\ttab"),
            ..CommandMeta::EMPTY
        };
        static WORDY_ROOT: Command = Command {
            name: "ex",
            subcommands: &[&WORDY],
            ..Command::EMPTY
        };
        static WORDY_ROOT_META: CommandMeta = CommandMeta {
            cmd: &WORDY_ROOT,
            subcommands: &[&WORDY_META],
            ..CommandMeta::EMPTY
        };
        static WORDY_SPEC: Spec = Spec {
            name: "ex",
            bin: Some("ex"),
            root: &WORDY_ROOT_META,
            ..Spec::EMPTY
        };

        let answer = complete(&WORDY_SPEC, &split("ex w", 4, Shell::Fish));
        let out = render(&answer, Shell::Fish);
        assert_eq!(out, "wordy\tFirst line second line and a tab\n");
        assert_eq!(out.lines().count(), 1, "one candidate, one row");

        // zsh keeps its three fields, and no more.
        let out = render(&answer, Shell::Zsh);
        assert_eq!(out.matches('\t').count(), 2, "{out:?}");
    }
    #[test]
    fn a_declared_completer_answers_for_its_value() {
        // The Rust counterpart of a spec's `run=`, and the same shape of answer: values with
        // descriptions where it has them.
        assert_eq!(offered("mise install "), ["node", "python", "ruby"]);
        let found = candidates(&SPEC, &at_end("mise install n"));
        assert_eq!(found.len(), 1);
        assert_eq!(found[0].value, "node");
        assert_eq!(found[0].description.as_deref(), Some("JavaScript"));

        let ruby = candidates(&SPEC, &at_end("mise install ru"));
        assert_eq!(ruby.len(), 1, "{ruby:?}");
        assert_eq!(ruby[0].display.as_deref(), Some("Ruby runtime"));

        // For a flag's value as well as an argument's.
        assert_eq!(offered("mise install --only "), ["node"]);
    }

    #[test]
    fn a_completer_is_filtered_for_rather_than_by() {
        // The reference filters a `run=` command's output by the typed prefix rather than making
        // every script do it, so a callback may answer with everything it knows.
        assert_eq!(offered("mise install ru"), ["ruby"]);
        assert!(offered("mise install zzz").is_empty());
    }

    #[test]
    fn a_completer_can_read_the_line_it_was_called_about() {
        // `{{words[PREV]}}` is what two of mise's nine completers use, so a callback has to be
        // able to see at least that much: here the answer narrows after `--only`.
        assert_eq!(offered("mise install --only "), ["node"]);
        assert_eq!(offered("mise install "), ["node", "python", "ruby"]);
    }

    #[test]
    fn a_completer_that_says_nothing_leaves_the_position_open() {
        // "A script that prints nothing may simply have had nothing to say about this prefix",
        // as the reference puts it — so paths still follow, rather than the position claiming
        // to know its whole set.
        let a = answer("mise install zzz");
        assert!(a.candidates.is_empty());
        assert_eq!(a.files, Some(Files::Any));
    }
    #[test]
    fn an_attached_value_is_not_answered_by_the_positional() {
        // `--source=⌶` is a dash-prefixed token, so it is a *flag* position: the flag branches
        // come first and the positional's completer is never reached. Worth pinning, because the
        // word being completed is excluded from the walk — so the flag is not `awaiting_value`
        // either, and the position could look like the argument's if the order changed.
        assert!(!offered("mise install --source=").contains(&"node".to_string()));
        assert!(!offered("mise install --source=").contains(&"upstream".to_string()));
        assert!(!offered("mise install -s").contains(&"node".to_string()));

        // Detached, the flag's own completer answers — which is the case that works.
        assert_eq!(offered("mise install --source "), ["upstream"]);
        assert_eq!(offered("mise install "), ["node", "python", "ruby"]);
    }
}
