//! Cold-path metadata, and writing it out as a spec.
//!
//! The tables in the crate root are what a parse reads: names, shorts, whether a
//! flag takes a value. Everything *else* a CLI knows about itself — help text,
//! choices, defaults, what a command does to the world — lives here instead, in a
//! parallel tree that points at those tables.
//!
//! Two reasons for the split. A successful parse never touches any of this, so
//! keeping it out of the hot tables keeps them dense; and a CLI that wants only a
//! parser does not compile it at all, since this module is behind the `spec`
//! feature.
//!
//! What the metadata does *not* do is repeat the tables. [`FlagMeta`] borrows the
//! [`Flag`] it describes, so long and short forms have exactly one definition and
//! cannot drift from what the parser matches.
//!
//! # Emitting
//!
//! Everything here lowers into the spec without loss, which is deliberate: a
//! field the spec cannot express would make the emitted KDL a summary rather than
//! a definition. When something is missing the spec gains it first —
//! `help_heading` went in that way.
//!
use core::fmt::Write as _;

use crate::UnknownFlags;
use crate::{Arg, Command, DoubleDash, Flag};

/// The first key that appears twice anywhere in a command tree, if any.
///
/// Keys are what a parse dispatches on, and a derive assigns them without being able
/// to see other expansions — it hashes the type name to keep them apart. That makes
/// a collision astronomically unlikely rather than impossible, so it is checked
/// where a CLI is written out, which every adopter does in a test.
///
/// Checked *per command*, not across the tree. A key only ever decides between the flags of
/// the one command in scope, and `#[usage(flatten)]` makes sharing one across commands
/// ordinary rather than suspect: mise gives a single `ConfigLs` to both `config` and
/// `config ls`, so that declaration — and its key — is in both tables by design. An earlier
/// version of this check looked at the whole tree and called that an error.
fn duplicate_key(cmd: &Command<'_>) -> Option<u64> {
    let mut keys: std::vec::Vec<u64> = cmd
        .flags
        .iter()
        .map(|f| f.key)
        .chain(cmd.args.iter().map(|a| a.key))
        .collect();
    keys.sort_unstable();
    if let Some(pair) = keys.windows(2).find(|pair| pair[0] == pair[1]) {
        return Some(pair[0]);
    }
    cmd.subcommands.iter().find_map(|sub| duplicate_key(sub))
}

/// A long or short form that two flags on the same command both answer to, if any.
///
/// Within one struct the derive catches this, but `#[usage(flatten)]` joins declarations from
/// two expansions that cannot see each other — so `--quiet` on a parent and `--quiet` on the
/// struct it flattens compiles, and then only the first is ever reached. Checked here for the
/// same reason keys are: this is where the whole tree is visible, and writing a spec out is
/// something every adopter does in a test.
fn duplicate_flag_form(cmd: &Command<'_>) -> Option<std::string::String> {
    let mut forms: std::vec::Vec<std::string::String> = std::vec::Vec::new();
    for flag in cmd.flags {
        for long in flag.longs.iter().chain(flag.negate.iter()) {
            forms.push(std::format!("--{long}"));
        }
        for short in flag.shorts {
            forms.push(std::format!("-{}", *short as char));
        }
    }
    forms.sort_unstable();
    if let Some(pair) = forms.windows(2).find(|pair| pair[0] == pair[1]) {
        return Some(pair[0].clone());
    }
    cmd.subcommands
        .iter()
        .find_map(|sub| duplicate_flag_form(sub))
}

/// A group name that two declarations on the same command both claim, if any.
///
/// Within one struct the derive catches this, and `#[usage(flatten)]` joins declarations
/// from two expansions that cannot see each other — so a parent and the struct it
/// flattens can each declare `input`, and each then checks only its own members. One
/// member from either side would satisfy neither exclusion, and the emitted KDL would
/// carry two `group "input"` nodes saying different things.
///
/// Checked here for the same reason duplicate flag forms are: this is where the joined
/// tables are visible.
fn duplicate_group_name(meta: &CommandMeta<'_>) -> Option<std::string::String> {
    let mut names: std::vec::Vec<&str> = meta.groups.iter().map(|g| g.name).collect();
    names.sort_unstable();
    if let Some(pair) = names.windows(2).find(|pair| pair[0] == pair[1]) {
        return Some(pair[0].to_string());
    }
    meta.subcommands
        .iter()
        .find_map(|sub| duplicate_group_name(sub))
}

/// An argument that no word could ever reach, if any.
///
/// An unbounded variadic takes every remaining word, so what follows it can never be filled —
/// unless something stops the variadic. Two things do: an argument only fillable after a `--`,
/// because the separator ends the collecting, and a `var_max`, because a bounded variadic hands
/// over the words past its bound. mise relies on the first on `run`, `exec` and `git`.
///
/// The separator is good for one argument and only where it is still a separator. A variadic
/// that already claimed it has spent it, and one declaring `preserve` takes it as a *value* —
/// so in either case the argument waiting for a `--` waits forever.
///
/// `#[derive(Args)]` applies that rule to one struct's own arguments. It cannot apply it across
/// a `#[usage(flatten)]`: the variadic may be on one side of the boundary and the argument that
/// follows it on the other, and neither expansion can see the other's fields. Checked here for
/// the same reason the duplicate checks are — this is where the joined table is visible.
///
/// Returns the name of the unreachable argument.
fn unfillable_arg<'a>(cmd: &Command<'a>) -> Option<&'a str> {
    let mut variadic: Option<&Arg<'_>> = None;
    for arg in cmd.args {
        // A `--` stops the collecting, so an argument behind one is still reachable — but only
        // one separator exists, so nothing can follow *that*.
        let stopped_by_separator = arg.double_dash == DoubleDash::Required;
        if let Some(before) = variadic {
            let separator_is_gone = matches!(
                before.double_dash,
                // Spent on the variadic itself.
                DoubleDash::Required
                // Or never a separator at all, because the variadic keeps it as a value.
                    | DoubleDash::Preserve
            );
            if !stopped_by_separator || separator_is_gone {
                return Some(arg.name);
            }
        }
        if arg.var && arg.var_max.is_none() {
            variadic = Some(arg);
        }
    }
    cmd.subcommands.iter().find_map(|sub| unfillable_arg(sub))
}

/// What a completion callback is told about the cursor.
///
/// Everything a `run=` command is given through tera — the words, which one the cursor is in —
/// plus the prefix, which the reference makes each script filter on and this filters for the
/// callback. A completer that wants an earlier value on the line reads it from `words`; one that
/// wants it *typed* gets it in the next stage, where the derive can hand over the command's own
/// half-parsed struct.
#[derive(Debug, Clone, Copy)]
pub struct CompleteCtx<'a> {
    /// The words of the line, unquoted, including the one being completed.
    pub words: &'a [String],
    /// Which of `words` the cursor is in.
    pub cword: usize,
    /// What has been typed of that word. Candidates are filtered by it afterwards, so a
    /// completer may ignore it and answer with everything it knows.
    pub prefix: &'a str,
    /// Every command the words passed through, and the words each one was given.
    ///
    /// A completer is declared on some command, which is not always the deepest one the line
    /// reached — a global flag belongs to an ancestor — so a caller that wants its own command's
    /// words asks for them by that command.
    pub command_path: &'a [(&'a crate::Command<'a>, &'a [String])],
    /// The words the command in scope was given: after its own name, before the cursor's word.
    ///
    /// What a callback needs to reconstruct its command's half-parsed struct — `mise task ls
    /// --file other.toml ⌶` is `["--file", "other.toml"]` here, whatever the words before `ls`
    /// were.
    pub command_words: &'a [String],
}

impl<'a> CompleteCtx<'a> {
    /// The command in the active path that owns `declaration`, and the words it was given.
    ///
    /// Matched by key, not by address: a `Subcommands` variant builds its own table entry for
    /// the command it names — it may rename it, or give it aliases the type knows nothing about
    /// — so the entry the parse walked is a *copy* of the one the type declares. The key is a
    /// hash of the type it came from and survives the copying, which makes it the identity two
    /// tables of the same command agree on. Not the name, which a variant can change and two
    /// commands can share.
    ///
    /// A flattened group's command is not itself on the path: its fields were spliced into the
    /// parent's table. In that case the command containing all of the group's declarations is
    /// the owner. Returning that actual command matters as well as returning its words, because
    /// reparsing against the flattened table alone would stop at a parent subcommand name and
    /// miss a global flag written after it.
    pub fn command_for(
        &self,
        declaration: &crate::Command<'_>,
    ) -> Option<(&'a crate::Command<'a>, &'a [String])> {
        self.command_path
            .iter()
            .find(|(cmd, _)| cmd.key == declaration.key)
            .or_else(|| {
                let has_fields = !declaration.flags.is_empty() || !declaration.args.is_empty();
                self.command_path.iter().find(|(cmd, _)| {
                    has_fields
                        && declaration.flags.iter().all(|field| {
                            cmd.flags.iter().any(|candidate| candidate.key == field.key)
                        })
                        && declaration.args.iter().all(|field| {
                            cmd.args.iter().any(|candidate| candidate.key == field.key)
                        })
                })
            })
            .map(|(cmd, words)| (*cmd, *words))
    }

    /// The words `command` was given, or the deepest command's if it is not on the path.
    ///
    /// The fallback is for a completer reached by name rather than by a cursor position, where
    /// there may be no path at all.
    pub fn words_for(&self, command: &crate::Command<'_>) -> &'a [String] {
        self.command_for(command)
            .map(|(_, words)| words)
            .unwrap_or(self.command_words)
    }

    /// The words a parser should walk to find the command this request is about.
    ///
    /// After the program name, before the word being completed — the same slice `walk` is given
    /// for an ordinary request, so a `--candidates` request resolves the same command an
    /// ordinary one would.
    pub fn command_words_start(&self) -> &'a [String] {
        let start = 1.min(self.cword);
        self.words.get(start..self.cword).unwrap_or(&[])
    }

    /// The word before the one being completed, which is what mise's `{{words[PREV]}}` means.
    pub fn previous(&self) -> Option<&'a str> {
        self.cword
            .checked_sub(1)
            .and_then(|i| self.words.get(i))
            .map(String::as_str)
    }
}

/// A function that answers for one argument or flag value.
///
/// The Rust counterpart of a spec's `run=`, and the reason it is a plain `fn` rather than a
/// closure: it lives in a `&'static` table beside the parse tables, and a table entry cannot
/// capture anything.
pub type Completer = fn(&CompleteCtx<'_>) -> Vec<Candidate<'static>>;

/// Something a shell could offer at the cursor.
///
/// The description is what fish, zsh, nu and PowerShell show beside a candidate; bash shows
/// only the value. It is borrowed from the spec rather than built, because it is already
/// there — the help text a page would print for the same thing.
#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord)]
pub struct Candidate<'a> {
    pub value: String,
    /// Borrowed from the spec where it is already there, owned where a callback made it.
    pub description: Option<::std::borrow::Cow<'a, str>>,
}

impl Candidate<'_> {
    /// A candidate with nothing to say about itself.
    pub fn new(value: impl Into<String>) -> Self {
        Self {
            value: value.into(),
            description: None,
        }
    }

    /// A candidate and the line a shell shows beside it.
    pub fn described(value: impl Into<String>, description: impl Into<String>) -> Self {
        Self {
            value: value.into(),
            description: Some(::std::borrow::Cow::Owned(description.into())),
        }
    }
}

/// A whole CLI: the root command plus what describes the program itself.
#[derive(Debug, Clone, Copy)]
pub struct Spec<'a> {
    /// The program's name.
    pub name: &'a str,
    /// The binary as invoked, when it differs from `name`.
    pub bin: Option<&'a str>,
    pub version: Option<&'a str>,
    /// The oldest `usage` that can read this spec, when the CLI says.
    ///
    /// Written first, before anything a `usage` too old to understand would choke on — which is
    /// the whole point of it, and why it is declared rather than worked out here: it is the
    /// CLI's claim about which consumers it means to keep working.
    pub min_usage_version: Option<&'a str>,
    pub about: Option<&'a str>,
    pub long_about: Option<&'a str>,
    /// An exact usage synopsis, including the `Usage:` prefix, when the generated
    /// shape needs alternatives that cannot be inferred from one command grammar.
    pub usage: Option<&'a str>,
    /// Which command the root falls back to when a word matches no subcommand.
    /// mise uses this so `mise foo` completes as `mise run foo`.
    pub default_subcommand: Option<&'a str>,
    /// Whether argv[0]'s basename selects a subcommand (busybox-style applets).
    ///
    /// clap's `multicall`. The dispatcher names (`name` / `bin`) are skipped; any
    /// other basename is parsed as the first word. Path components and a trailing
    /// `.exe` are stripped. The parser itself does not see argv[0]; [`crate::multicall_applet`]
    /// is what a process entry applies before calling it.
    pub multicall: bool,
    /// The root command, and the home of everything a spec declares at its top level.
    ///
    /// A KDL spec has one place for surrounding text and examples — the top level — and the
    /// reference reads what is written there as the root's *and* as the default for every
    /// other page. So they live here, on the root's metadata, rather than in a second set of
    /// fields on the spec: two homes for one declaration is two answers to one question, and
    /// `to_kdl` and the renderer picked differently.
    pub root: &'a CommandMeta<'a>,
}

/// A command selected for a cold-path metadata override.
///
/// Paths are space-separated command names below the root (`"dist-tag rm"`). Keys are useful
/// when the command type is available and avoid coupling an overlay to its displayed spelling.
#[derive(Debug, Clone, Copy)]
pub enum CommandSelector<'a> {
    Any,
    Path(&'a str),
    Key(u64),
}

impl CommandSelector<'_> {
    pub(crate) fn matches(&self, meta: &CommandMeta<'_>, path: &[&str]) -> bool {
        match self {
            Self::Any => true,
            Self::Path(expected) => expected.split_ascii_whitespace().eq(path.iter().copied()),
            Self::Key(key) => meta.cmd.key == *key,
        }
    }
}

/// One sparse metadata override applied by [`SpecView`].
///
/// This deliberately contains only properties with real fleet users. Adding a property is
/// preferable to exposing a mutable command builder: the base derive tables remain immutable,
/// shareable and free to parse.
#[derive(Debug, Clone, Copy)]
pub struct CommandOverlay<'a> {
    pub command: CommandSelector<'a>,
    pub effect: Effect,
}

impl<'a> CommandOverlay<'a> {
    pub const fn effect(path: &'a str, effect: Effect) -> Self {
        Self {
            command: CommandSelector::Path(path),
            effect,
        }
    }

    pub const fn effect_for(key: u64, effect: Effect) -> Self {
        Self {
            command: CommandSelector::Key(key),
            effect,
        }
    }
}

/// A borrowed, sparse view over a derive-generated [`Spec`].
///
/// Constructing a view copies no command tree and is never part of argv parsing. It exists for
/// cold paths—help, spec emission and completion—where a program may need runtime identity or a
/// centrally audited metadata policy without lowering the static tables through usage-lib.
#[derive(Debug, Clone)]
pub struct SpecView<'a> {
    base: &'a Spec<'a>,
    name: Option<&'a str>,
    bin: Option<&'a str>,
    version: Option<&'a str>,
    omit_version: bool,
    commands: Vec<CommandOverlay<'a>>,
}

impl<'a> SpecView<'a> {
    pub const fn new(base: &'a Spec<'a>) -> Self {
        Self {
            base,
            name: None,
            bin: None,
            version: None,
            omit_version: false,
            commands: Vec::new(),
        }
    }

    fn reborrow<'b>(self) -> SpecView<'b>
    where
        'a: 'b,
    {
        SpecView {
            base: self.base,
            name: self.name,
            bin: self.bin,
            version: self.version,
            omit_version: self.omit_version,
            commands: self.commands,
        }
    }

    pub fn name<'b, 'c>(self, name: &'b str) -> SpecView<'c>
    where
        'a: 'c,
        'b: 'c,
    {
        let mut view = self.reborrow();
        view.name = Some(name);
        view
    }

    pub fn bin<'b, 'c>(self, bin: &'b str) -> SpecView<'c>
    where
        'a: 'c,
        'b: 'c,
    {
        let mut view = self.reborrow();
        view.bin = Some(bin);
        view
    }

    pub fn version<'b, 'c>(self, version: &'b str) -> SpecView<'c>
    where
        'a: 'c,
        'b: 'c,
    {
        let mut view = self.reborrow();
        view.version = Some(version);
        view.omit_version = false;
        view
    }

    /// Leave the runtime version out of emitted specs and other metadata consumers.
    ///
    /// The parser's built-in version response is unchanged because parsing reads the base spec.
    /// This is for checked-in generated artifacts whose version is managed independently.
    pub fn omit_version(mut self) -> Self {
        self.version = None;
        self.omit_version = true;
        self
    }

    pub fn overlay<'b, 'c>(self, commands: &'b [CommandOverlay<'b>]) -> SpecView<'c>
    where
        'a: 'c,
        'b: 'c,
    {
        let mut view = self.reborrow();
        view.commands.extend_from_slice(commands);
        view
    }

    /// The shallow effective spec. Its command metadata still borrows the derive's static tree.
    pub const fn spec(&self) -> Spec<'a> {
        Spec {
            name: match self.name {
                Some(name) => name,
                None => self.base.name,
            },
            bin: match self.bin {
                Some(bin) => Some(bin),
                None => self.base.bin,
            },
            version: if self.omit_version {
                None
            } else {
                match self.version {
                    Some(version) => Some(version),
                    None => self.base.version,
                }
            },
            min_usage_version: self.base.min_usage_version,
            about: self.base.about,
            long_about: self.base.long_about,
            usage: self.base.usage,
            default_subcommand: self.base.default_subcommand,
            multicall: self.base.multicall,
            root: self.base.root,
        }
    }

    /// Emit the effective view without constructing or mutating a command graph.
    pub fn to_kdl(self) -> String {
        let spec = self.spec();
        spec.render_kdl_with(&self.commands)
    }
}

impl Spec<'_> {
    /// A spec with nothing declared but a root, for use with struct update syntax.
    ///
    /// Here so that gaining a field does not break every literal that builds one.
    pub const EMPTY: Spec<'static> = Spec {
        name: "",
        bin: None,
        version: None,
        min_usage_version: None,
        about: None,
        long_about: None,
        usage: None,
        default_subcommand: None,
        multicall: false,
        root: &CommandMeta::EMPTY,
    };

    /// Borrow this spec for runtime identity or sparse metadata overlays.
    pub const fn view(&self) -> SpecView<'_> {
        SpecView::new(self)
    }
}

/// Join groups of flag metadata into one, at compile time.
///
/// The metadata counterpart of [`concat_flags`](crate::concat_flags), for the same reason:
/// a flattened struct's flags belong in the parent's emitted spec, and the parent's macro
/// expansion has only a type to reach them through.
///
/// Metadata is held by value rather than by reference, so this copies — which is free at
/// compile time and produces a `static` the writer walks like any other.
///
/// `N` must be [`table_len`](crate::table_len) of the same groups.
pub const fn concat_flag_metas<const N: usize>(
    groups: &[&[FlagMeta<'static>]],
) -> [FlagMeta<'static>; N] {
    let mut out = [FlagMeta::EMPTY; N];
    let mut at = 0;
    let mut g = 0;
    while g < groups.len() {
        let group = groups[g];
        let mut i = 0;
        while i < group.len() {
            out[at] = group[i];
            at += 1;
            i += 1;
        }
        g += 1;
    }
    assert!(
        at == N,
        "`N` must be `table_len` of the same groups, or the metadata would describe a flag \
         that does not exist"
    );
    out
}

/// Join groups of argument metadata into one, at compile time.
///
/// See [`concat_flag_metas`]. Order is the same as the parse tables', because the two are
/// read together: a spec lists arguments in the order they are filled.
pub const fn concat_arg_metas<const N: usize>(
    groups: &[&[ArgMeta<'static>]],
) -> [ArgMeta<'static>; N] {
    let mut out = [ArgMeta::EMPTY; N];
    let mut at = 0;
    let mut g = 0;
    while g < groups.len() {
        let group = groups[g];
        let mut i = 0;
        while i < group.len() {
            out[at] = group[i];
            at += 1;
            i += 1;
        }
        g += 1;
    }
    assert!(
        at == N,
        "`N` must be `table_len` of the same groups, or the metadata would describe an \
         argument that does not exist"
    );
    out
}

/// Join command alias lists at compile time.
///
/// An `Args` struct can declare aliases belonging to the command itself, while the
/// `Subcommands` variant mounting it can add aliases belonging to that route. The derive joins
/// both without building a command at runtime.
pub const fn concat_aliases<const N: usize>(groups: &[&[&'static str]]) -> [&'static str; N] {
    let mut out = [""; N];
    let mut at = 0;
    let mut g = 0;
    while g < groups.len() {
        let group = groups[g];
        let mut i = 0;
        while i < group.len() {
            out[at] = group[i];
            at += 1;
            i += 1;
        }
        g += 1;
    }
    assert!(
        at == N,
        "`N` must be `table_len` of the same groups, or an alias would be empty"
    );
    out
}

/// A set of one command's arguments that relate to one another as a set.
///
/// Pairwise `conflicts` can say "at most one of these", once per pair; what it cannot
/// say is that one of them is *needed*, which is a statement about the set.
#[derive(Debug, Clone, Copy)]
pub struct GroupMeta<'a> {
    /// What the group is called. It appears in the message a failed check produces, and
    /// it is how a reader tells two groups on one command apart.
    pub name: &'a str,
    /// The entries in the group, as selectors — `--long` or `-s` for flags, and the
    /// bare argument name for a positional.
    pub members: &'a [&'a str],
    /// Whether at least one member has to be given.
    pub required: bool,
    /// Whether more than one member may be given. False is what makes a bare group
    /// mutual exclusion, as it does in clap.
    pub multiple: bool,
}

impl GroupMeta<'_> {
    /// A group with nothing in it, for the array initialiser a const concat needs.
    pub const EMPTY: GroupMeta<'static> = GroupMeta {
        name: "",
        members: &[],
        required: false,
        multiple: false,
    };
}

/// Join groups of group metadata into one, at compile time.
///
/// The same shape as [`concat_flag_metas`], and needed for the same reason: a flattened
/// struct's groups describe flags that are now in the parent's table, so they belong in
/// the parent's emitted spec. Without this a group declared on a flattened struct would
/// be enforced — the child's own `check` runs — and invisible to the KDL, which is
/// exactly the drift the spec-as-definition rule exists to prevent.
///
/// `N` must be [`table_len`](crate::table_len) of the same groups.
pub const fn concat_group_metas<const N: usize>(
    groups: &[&[GroupMeta<'static>]],
) -> [GroupMeta<'static>; N] {
    let mut out = [GroupMeta::EMPTY; N];
    let mut at = 0;
    let mut g = 0;
    while g < groups.len() {
        let group = groups[g];
        let mut i = 0;
        while i < group.len() {
            // This function initialises a generated `static`, so a collision across a parent
            // and a flattened child is rejected while the adopter compiles. Leaving this to
            // `to_kdl` let direct parsing enforce two independent groups with the same name.
            let mut seen = 0;
            while seen < at {
                assert!(
                    !crate::str_eq(out[seen].name, group[i].name),
                    "two flattened groups on one command have the same name"
                );
                seen += 1;
            }
            out[at] = group[i];
            at += 1;
            i += 1;
        }
        g += 1;
    }
    assert!(
        at == N,
        "`N` must be `table_len` of the same groups, or the metadata would describe a \
         group that does not exist"
    );
    out
}

/// What a command knows about itself beyond how it parses.
#[derive(Debug, Clone, Copy)]
pub struct CommandMeta<'a> {
    /// The parse table this describes. Names, aliases, and structure come from
    /// here rather than being repeated.
    pub cmd: &'a Command<'a>,
    pub about: Option<&'a str>,
    pub long_about: Option<&'a str>,
    /// Aliases that work but are not shown in help or completions. Everything in
    /// `cmd.aliases` and not here is visible.
    pub hidden_aliases: &'a [&'a str],
    /// Whether the command is hidden from help and completions.
    pub hide: bool,
    /// What running this does to the world, for a caller deciding whether to
    /// confirm first. clap cannot express this, which is why mise keeps a
    /// 330-entry table to bolt it on afterwards.
    pub effect: Option<Effect>,
    /// A command to run at parse time to discover further subcommands.
    ///
    /// Only meaningful on a subcommand. The spec accepts `mount` inside a `cmd`
    /// block and nowhere else, so setting this on the root is a mistake that
    /// [`Spec::to_kdl`] catches in debug builds.
    pub mount: Option<&'a str>,
    /// A token that starts a fresh invocation of this command, such as mise's
    /// `:::`.
    pub restart_token: Option<&'a str>,
    /// Whether this command cannot be run on its own: naming it and stopping is an
    /// error, and one of its subcommands has to follow.
    ///
    /// Cold metadata rather than a parse table, because it is not how a word binds —
    /// the derive already refuses the invocation from the type, a bare `T` subcommand
    /// field against an `Option<T>`. It is here so the emitted spec can say it, since
    /// everything reading that spec — help, docs, completions — otherwise describes a
    /// command as runnable when it is not.
    pub subcommand_required: bool,
    /// Heading for the list of this command's subcommands.
    pub subcommand_help_heading: Option<&'a str>,
    /// Placeholder used for a subcommand in the usage synopsis.
    pub subcommand_value_name: Option<&'a str>,
    /// Fixed help width. Zero disables wrapping.
    pub term_width: Option<usize>,
    /// Maximum detected terminal width when `term_width` is unset. Zero disables the cap.
    pub max_term_width: Option<usize>,
    /// Whether later single-valued occurrences replace earlier ones.
    pub args_override_self: bool,
    /// Text printed above the usage line, and below everything else.
    ///
    /// The spec's `before_help`/`after_help` and their long forms. mise puts an Examples
    /// section in `after_long_help` on 115 commands, which is where the reference renders it
    /// from — so a help page without these is missing the part a reader came for.
    pub before_help: Option<&'a str>,
    pub before_long_help: Option<&'a str>,
    pub after_help: Option<&'a str>,
    pub after_long_help: Option<&'a str>,
    pub examples: &'a [Example<'a>],
    /// Metadata for `cmd.flags`, in the same order.
    pub flags: &'a [FlagMeta<'a>],
    /// Metadata for `cmd.args`, in the same order.
    pub args: &'a [ArgMeta<'a>],
    /// Metadata for `cmd.subcommands`, in the same order.
    pub subcommands: &'a [&'a CommandMeta<'a>],
    /// Sets of this command's flags that relate to one another as a set.
    ///
    /// Cold like everything else here: a group is checked once the last token has been
    /// read, by code the derive generates, and a successful parse never reads this.
    pub groups: &'a [GroupMeta<'a>],
}

impl CommandMeta<'_> {
    /// Metadata for a command with nothing declared, for struct update syntax.
    pub const EMPTY: CommandMeta<'static> = CommandMeta {
        cmd: &Command::EMPTY,
        about: None,
        long_about: None,
        hidden_aliases: &[],
        hide: false,
        effect: None,
        mount: None,
        restart_token: None,
        subcommand_required: false,
        subcommand_help_heading: None,
        subcommand_value_name: None,
        term_width: None,
        max_term_width: None,
        args_override_self: true,
        before_help: None,
        before_long_help: None,
        after_help: None,
        after_long_help: None,
        examples: &[],
        groups: &[],
        flags: &[],
        args: &[],
        subcommands: &[],
    };
}

/// What a flag knows about itself beyond how it parses.
#[derive(Debug, Clone, Copy)]
pub struct FlagMeta<'a> {
    pub flag: &'a Flag<'a>,
    /// Short forms accepted by the parser but omitted from help and completion.
    pub hidden_shorts: &'a [u8],
    /// Long forms accepted by the parser but omitted from help and completion.
    pub hidden_longs: &'a [&'a str],
    /// Short help, shown by `-h`.
    pub help: Option<&'a str>,
    /// Long help, shown by `--help`.
    pub long_help: Option<&'a str>,
    /// The placeholder for the flag's value, such as `n` in `--jobs <n>`.
    pub value_name: Option<&'a str>,
    /// Ordered placeholders for one fixed-arity occurrence.
    pub value_names: &'a [&'a str],
    pub env: Option<&'a str>,
    pub default: &'a [&'a str],
    /// Canonical choices plus aliases accepted by the value type.
    pub accepted_choices: &'a [&'a str],
    pub choices: &'a [&'a str],
    /// Canonical-to-alias pairs used when emitting a lossless spec.
    pub choice_aliases: &'a [(&'a str, &'a str)],
    /// Per-canonical presentation metadata used when emitting a lossless spec.
    pub choice_details: &'a [ChoiceMeta<'a>],
    pub ignore_case: bool,
    /// Portable expr expression evaluated for each raw value.
    pub validate: Option<&'a str>,
    /// Message reported when validation returns false.
    pub validate_error: Option<&'a str>,
    pub required: bool,
    /// Whether the flag's value may be left off, as in `--bump` or `--bump 5`.
    ///
    /// Help only, and deliberately: usage-lib's parser refuses a bare `--bump` exactly as it
    /// refuses a bare `--port`, so this changes no binding — it changes the brackets, `[BUMP]`
    /// rather than `<BUMP>`, which is what a spec's `arg "[BUMP]" required=#false` says. In
    /// [`FlagMeta`] and not in [`Flag`] for that reason: a parse never reads it.
    pub value_optional: bool,
    pub hide: bool,
    pub hide_default_value: bool,
    pub hide_env: bool,
    pub hide_env_values: bool,
    pub hide_possible_values: bool,
    pub hide_short_help: bool,
    pub hide_long_help: bool,
    /// Whether repetition is counted rather than collected, as in `-vvv`.
    pub count: bool,
    /// What answers for this flag's value when a shell asks.
    ///
    /// The Rust counterpart of a spec's `run=`: it is written into the emitted KDL as a command
    /// that asks *this binary*, so a spec stays complete for every other consumer while the
    /// binary answers itself.
    pub complete: Option<Completer>,
    /// A built-in completion class such as `path` or `dir`.
    pub complete_type: Option<&'a str>,
    /// Whether the flag may be given more than once. Distinct from
    /// [`Flag::variadic`], which is one occurrence taking several values.
    pub repeatable: bool,
    pub var_min: Option<usize>,
    pub var_max: Option<usize>,
    /// Bounds on values consumed by one occurrence, distinct from the
    /// flag-level occurrence bounds above.
    pub value_var_min: Option<usize>,
    pub value_var_max: Option<usize>,
    /// Flags this one displaces when both are given.
    pub overrides: &'a [&'a str],
    /// Flags that cannot be given alongside this one.
    ///
    /// Where [`overrides`](Self::overrides) resolves a collision by letting the last
    /// flag win, this reports it: the combination has no meaning, so honouring one
    /// side silently would hide a mistake.
    pub conflicts: &'a [&'a str],
    /// The character one word is split on to make several values, as clap's
    /// `value_delimiter` does. Only ever set where several values can land.
    pub delimiter: Option<char>,
    /// Whether this flag must be given on its own.
    ///
    /// The whole-command form of [`conflicts`](Self::conflicts): everything the command
    /// declares counts, positionals included.
    pub exclusive: bool,
    /// Flags that must also be given when this one is.
    ///
    /// The positive form of [`conflicts`](Self::conflicts), and the mirror image of
    /// [`required_if`](Self::required_if): the same rule written on the flag that
    /// imposes it rather than on the flag it lands on.
    pub requires: &'a [&'a str],
    /// Value-triggered requirements declared by this flag.
    pub requires_if: &'a [RequiresIf<'a>],
    /// Defaults that apply when another flag is given.
    pub default_if: &'a [DefaultIf<'a>],
    /// Flags that make this one necessary.
    pub required_if: &'a [&'a str],
    /// Selector/value conditions, any one of which makes this required.
    pub required_if_eq: &'a [RequiredIfEq<'a>],
    /// Selector/value conditions which must all match to make this required.
    pub required_if_eq_all: &'a [RequiredIfEq<'a>],
    /// Flags that make this one unnecessary.
    pub required_unless: &'a [&'a str],
    /// All selectors must be present to make this unnecessary.
    pub required_unless_all: &'a [&'a str],
    /// Heading to list this flag under in help output. Presentational: it groups
    /// a long flag list into sections and changes nothing about parsing.
    pub help_heading: Option<&'a str>,
    pub effect: Option<Effect>,
}

impl FlagMeta<'_> {
    /// Metadata for a flag with nothing declared, for struct update syntax.
    pub const EMPTY: FlagMeta<'static> = FlagMeta {
        complete: None,
        complete_type: None,
        flag: &Flag::BOOL,
        hidden_shorts: &[],
        hidden_longs: &[],
        help: None,
        long_help: None,
        value_name: None,
        value_names: &[],
        env: None,
        default: &[],
        accepted_choices: &[],
        choices: &[],
        choice_aliases: &[],
        choice_details: &[],
        ignore_case: false,
        validate: None,
        validate_error: None,
        required: false,
        value_optional: false,
        hide: false,
        hide_default_value: false,
        hide_env: false,
        hide_env_values: false,
        hide_possible_values: false,
        hide_short_help: false,
        hide_long_help: false,
        count: false,
        repeatable: false,
        var_min: None,
        var_max: None,
        value_var_min: None,
        value_var_max: None,
        overrides: &[],
        conflicts: &[],
        delimiter: None,
        exclusive: false,
        requires: &[],
        requires_if: &[],
        default_if: &[],
        required_if: &[],
        required_if_eq: &[],
        required_if_eq_all: &[],
        required_unless: &[],
        required_unless_all: &[],
        help_heading: None,
        effect: None,
    };
}

/// A flag required when the declaring flag is explicitly given `value`.
#[derive(Debug, Clone, Copy)]
pub struct RequiresIf<'a> {
    pub value: &'a str,
    pub requires: &'a str,
}

/// A selector/value comparison used by conditional requiredness.
#[derive(Debug, Clone, Copy)]
pub struct RequiredIfEq<'a> {
    pub selector: &'a str,
    pub value: &'a str,
}

/// A default that applies when another flag is given.
///
/// Two-argument form (`when` is `None`) is clap's `ArgPredicate::IsPresent`.
/// Three-argument form is `Equals`.
#[derive(Debug, Clone, Copy)]
pub struct DefaultIf<'a> {
    pub selector: &'a str,
    pub when: Option<&'a str>,
    pub value: &'a str,
}

/// What a positional argument knows about itself beyond how it parses.
#[derive(Debug, Clone, Copy)]
pub struct ArgMeta<'a> {
    pub arg: &'a Arg<'a>,
    /// Ordered placeholders for a fixed-arity positional.
    pub value_names: &'a [&'a str],
    pub help: Option<&'a str>,
    pub long_help: Option<&'a str>,
    pub env: Option<&'a str>,
    pub default: &'a [&'a str],
    /// Canonical choices plus aliases accepted by the value type.
    pub accepted_choices: &'a [&'a str],
    pub choices: &'a [&'a str],
    /// Canonical-to-alias pairs used when emitting a lossless spec.
    pub choice_aliases: &'a [(&'a str, &'a str)],
    /// Per-canonical presentation metadata used when emitting a lossless spec.
    pub choice_details: &'a [ChoiceMeta<'a>],
    pub ignore_case: bool,
    /// Portable expr expression evaluated for each raw value.
    pub validate: Option<&'a str>,
    /// Message reported when validation returns false.
    pub validate_error: Option<&'a str>,
    /// Whether the argument must be filled. The parser does not enforce this —
    /// it is checked once the last token has been read — but the spec has to say
    /// it, and help output has to show it.
    pub required: bool,
    pub hide: bool,
    pub hide_default_value: bool,
    pub hide_env: bool,
    pub hide_env_values: bool,
    pub hide_possible_values: bool,
    pub hide_short_help: bool,
    pub hide_long_help: bool,
    /// Entries that cannot be given alongside this positional.
    pub conflicts: &'a [&'a str],
    pub requires: &'a [&'a str],
    pub required_if: &'a [&'a str],
    pub required_if_eq: &'a [RequiredIfEq<'a>],
    pub required_if_eq_all: &'a [RequiredIfEq<'a>],
    pub required_unless: &'a [&'a str],
    pub required_unless_all: &'a [&'a str],
    pub var_min: Option<usize>,
    pub var_max: Option<usize>,
    /// The character one word is split on to make several positional values.
    pub delimiter: Option<char>,
    /// Heading to list this argument under in help output.
    pub help_heading: Option<&'a str>,
    /// What answers for this argument when a shell asks. See [`FlagMeta::complete`].
    pub complete: Option<Completer>,
    /// A built-in completion class such as `path` or `dir`.
    pub complete_type: Option<&'a str>,
}

impl ArgMeta<'_> {
    /// Metadata for an argument with nothing declared, for struct update syntax.
    pub const EMPTY: ArgMeta<'static> = ArgMeta {
        complete: None,
        complete_type: None,
        arg: &Arg::REQUIRED,
        value_names: &[],
        help: None,
        long_help: None,
        env: None,
        default: &[],
        accepted_choices: &[],
        choices: &[],
        choice_aliases: &[],
        choice_details: &[],
        ignore_case: false,
        validate: None,
        validate_error: None,
        required: true,
        hide: false,
        hide_default_value: false,
        hide_env: false,
        hide_env_values: false,
        hide_possible_values: false,
        hide_short_help: false,
        hide_long_help: false,
        conflicts: &[],
        requires: &[],
        required_if: &[],
        required_if_eq: &[],
        required_if_eq_all: &[],
        required_unless: &[],
        required_unless_all: &[],
        var_min: None,
        var_max: None,
        delimiter: None,
        help_heading: None,
    };
}

/// A worked example, for documentation.
#[derive(Debug, Clone, Copy)]
pub struct Example<'a> {
    pub code: &'a str,
    pub header: Option<&'a str>,
    pub help: Option<&'a str>,
}

/// What running a command does to the world.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Effect {
    /// Only inspects state.
    Read,
    /// Creates or modifies state, but removes nothing unrecoverable.
    Write,
    /// May delete or irreversibly overwrite something.
    Destructive,
}

impl Effect {
    /// The spelling used in a spec.
    pub fn as_str(self) -> &'static str {
        match self {
            Effect::Read => "read",
            Effect::Write => "write",
            Effect::Destructive => "destructive",
        }
    }
}

impl Spec<'_> {
    /// Write this CLI as a usage spec, in KDL.
    pub fn to_kdl(&self) -> String {
        self.render_kdl_with(&[])
    }

    fn render_kdl_with(&self, overlays: &[CommandOverlay<'_>]) -> String {
        debug_assert!(
            duplicate_key(self.root.cmd).is_none(),
            "two things on the same command share a key ({:?}), so a parse would bind the \
             wrong one. A derive builds keys from a hash of the type they came from, so this \
             means two type names collided — or one struct was flattened into the same \
             command twice.",
            duplicate_key(self.root.cmd)
        );
        debug_assert!(
            duplicate_flag_form(self.root.cmd).is_none(),
            "two flags on the same command answer to {:?}, so only one of them could ever \
             be reached. With `flatten` this is the collision neither expansion can see: \
             the parent and the struct it flattens each declared it.",
            duplicate_flag_form(self.root.cmd)
        );
        assert!(
            duplicate_group_name(self.root).is_none(),
            "two groups on the same command are called {:?}, so each would enforce only \
             its own members and one from either side would satisfy neither. With \
             `flatten` this is the collision neither expansion can see: the parent and \
             the struct it flattens each declared it. Give one of them another name.",
            duplicate_group_name(self.root)
        );
        debug_assert!(
            unfillable_arg(self.root.cmd).is_none(),
            "no word could ever reach the argument {:?}, because an unbounded variadic before \
             it takes every remaining one. With `flatten` this is the arrangement neither \
             expansion can see: the variadic and the argument after it were declared in \
             different structs. Give the variadic a `var_max` — or, if the variadic leaves the \
             `--` alone, make the later argument fillable only after one. A variadic that \
             already requires a separator has spent it, and one declaring `preserve` takes it \
             as a value, so neither can be stopped that way.",
            unfillable_arg(self.root.cmd)
        );
        let mut out = String::new();
        // Unwrap-free: writing into a String cannot fail, and `write!` returning
        // Result is an artifact of the trait rather than a real outcome.
        let _ = self.write_kdl(&mut out, overlays);
        out
    }

    fn write_kdl(&self, out: &mut String, overlays: &[CommandOverlay<'_>]) -> core::fmt::Result {
        // First, so a `usage` too old to read the rest sees it before whatever it would choke on.
        if let Some(min) = self.min_usage_version {
            prop(out, "min_usage_version", min)?;
        }
        prop(out, "name", self.name)?;
        prop(out, "bin", self.bin.unwrap_or(self.name))?;
        if let Some(version) = self.version {
            prop(out, "version", version)?;
        }
        // A description may be given on the spec or on its root command — a derive
        // naturally has one doc comment and no reason to care which field it lands
        // in — so either is written, the spec's first.
        if let Some(about) = self.about.or(self.root.about) {
            prop(out, "about", about)?;
        }
        if let Some(long_about) = self.long_about.or(self.root.long_about) {
            prop(out, "long_about", long_about)?;
        }
        if let Some(usage) = self.usage {
            prop(out, "usage", usage)?;
        }
        // Written only when it is not the default, so an ordinary spec stays quiet
        // about it.
        if self.root.cmd.unknown_flags == Some(UnknownFlags::Error) {
            prop(out, "unknown_flags", "error")?;
        }
        if let Some(default_subcommand) = self.default_subcommand {
            prop(out, "default_subcommand", default_subcommand)?;
        }
        if self.multicall {
            writeln!(out, "multicall #true")?;
        }
        if self.root.cmd.external_subcommand {
            writeln!(out, "external_subcommand #true")?;
        }
        if self.root.cmd.arg_required_else_help {
            writeln!(out, "arg_required_else_help #true")?;
        }
        if self.root.cmd.dont_delimit_trailing_values {
            writeln!(out, "dont_delimit_trailing_values #true")?;
        }
        if !self.root.args_override_self {
            writeln!(out, "args_override_self #false")?;
        }
        if self.root.cmd.subcommand_negates_reqs {
            writeln!(out, "subcommand_negates_reqs #true")?;
        }
        if self.root.cmd.args_conflicts_with_subcommands {
            writeln!(out, "args_conflicts_with_subcommands #true")?;
        }
        if self.root.cmd.subcommand_precedence_over_arg {
            writeln!(out, "subcommand_precedence_over_arg #true")?;
        }
        if self.root.cmd.allow_missing_positional {
            writeln!(out, "allow_missing_positional #true")?;
        }
        // Root metadata is written separately from `write_command`, so keep the required
        // subcommand bit here as well. Without it a required typed enum still parsed correctly,
        // but its emitted portable spec rendered `[COMMAND]` and accepted an empty invocation.
        if self.root.subcommand_required && !self.root.cmd.subcommands.is_empty() {
            writeln!(out, "subcommand_required #true")?;
        }
        if let Some(heading) = self.root.subcommand_help_heading {
            prop(out, "subcommand_help_heading", heading)?;
        }
        if let Some(name) = self.root.subcommand_value_name {
            prop(out, "subcommand_value_name", name)?;
        }
        if let Some(width) = self.root.term_width {
            writeln!(out, "term_width {width}")?;
        }
        if let Some(width) = self.root.max_term_width {
            writeln!(out, "max_term_width {width}")?;
        }
        // A `complete` block for every completer this CLI declares, naming the command that
        // asks the binary itself. Written rather than declared, so there is one place a
        // completer is said to exist: the Rust function. Everything that reads a spec — the
        // usage CLI, another shell's generator, a doc page — gets a `run=` that works, and this
        // binary answers it without a second program in the way.

        // The text around the page. The root's nodes are written here rather than by
        // `write_body`, so these had to be repeated — and were not, which left a root's
        // preamble out of the spec that docs, manpages and completions read.
        for (node, text) in [
            ("before_help", self.root.before_help),
            ("before_long_help", self.root.before_long_help),
            ("after_help", self.root.after_help),
            ("after_long_help", self.root.after_long_help),
        ] {
            if let Some(text) = text {
                prop(out, node, text)?;
            }
        }
        // The root's own nodes sit at the top level rather than inside a `cmd`
        // block, so they are written here instead of by write_command.
        //
        // A mount is the exception: the spec only accepts one inside a `cmd`
        // block, so a root mount is not expressible. Emitting it anyway would
        // produce a document that does not parse, and dropping it quietly is the
        // lossiness this module claims not to have — so it fails loudly in debug
        // builds instead, and PLAN.md carries it as a possible spec extension.
        debug_assert!(
            self.root.mount.is_none(),
            "a mount on the root command cannot be written: the spec accepts \
             `mount` only inside a `cmd` block"
        );
        // The same is true of everything else that lives on a `cmd` node. Setting
        // one on the root is a mistake, and silently dropping it is the lossiness
        // this module claims not to have.
        debug_assert!(
            self.root.effect.is_none()
                && !self.root.hide
                && self.root.restart_token.is_none()
                && self.root.cmd.aliases.is_empty()
                && self.root.hidden_aliases.is_empty(),
            "the root command cannot carry an effect, hide, a restart token, or \
             aliases: the spec accepts those only inside a `cmd` block"
        );
        for example in self.root.examples {
            write_example(out, example, 0)?;
        }
        // Nothing above the root, so what it does not state is the default.
        let mut path = Vec::new();
        write_body(
            out,
            self.root,
            0,
            UnknownFlags::Value,
            self.bin.unwrap_or(self.name),
            overlays,
            &mut path,
        )
    }
}

/// Write a command's contents: its flags, arguments, and subcommands.
///
/// Separate from [`write_command`] because the root's contents sit at the top
/// level of the document rather than inside a `cmd` node.
fn write_body<'a>(
    out: &mut String,
    meta: &CommandMeta<'a>,
    depth: usize,
    inherited_unknown_flags: UnknownFlags,
    bin: &str,
    overlays: &[CommandOverlay<'_>],
    path: &mut Vec<&'a str>,
) -> core::fmt::Result {
    // The effective setting for everything inside, which is this command's if it stated one
    // and otherwise whatever it inherited.
    let enclosing_unknown_flags = meta.cmd.unknown_flags.unwrap_or(inherited_unknown_flags);
    // Indexing by metadata position below cannot see a table entry with no
    // metadata, which would be silently unwritten. Check the lengths first.
    debug_assert_eq!(
        meta.cmd.flags.len(),
        meta.flags.len(),
        "every flag in the parse table needs metadata, or it will not be written"
    );
    debug_assert_eq!(
        meta.cmd.args.len(),
        meta.args.len(),
        "every argument in the parse table needs metadata"
    );
    debug_assert_eq!(
        meta.cmd.subcommands.len(),
        meta.subcommands.len(),
        "every subcommand in the parse table needs metadata"
    );
    for (i, flag) in meta.flags.iter().enumerate() {
        // The two tables are written in the same order by construction, so a
        // mismatch means a table was edited without its metadata.
        debug_assert!(
            meta.cmd
                .flags
                .get(i)
                .is_some_and(|f| core::ptr::eq(*f, flag.flag)),
            "flag metadata is out of step with the parse table"
        );
        write_flag(out, flag, depth)?;
    }
    for (i, arg) in meta.args.iter().enumerate() {
        debug_assert!(
            meta.cmd
                .args
                .get(i)
                .is_some_and(|a| core::ptr::eq(*a, arg.arg)),
            "argument metadata is out of step with the parse table"
        );
        write_arg(out, arg, depth)?;
    }
    write_completion_types(out, meta, depth)?;
    // After the flags and arguments they name, so a reader meets the members before the
    // rule about them — the order usage-lib writes, so a round trip reads the same way.
    for group in meta.groups {
        write_group(out, group, depth)?;
    }
    #[cfg(feature = "complete")]
    write_completers(out, meta, bin, depth)?;
    for sub in meta.subcommands {
        write_command(
            out,
            sub,
            depth,
            enclosing_unknown_flags,
            bin,
            overlays,
            path,
        )?;
    }
    Ok(())
}

/// Built-in completion types declared by this command, written in the spec's vocabulary.
fn write_completion_types<'a>(
    out: &mut String,
    meta: &CommandMeta<'a>,
    depth: usize,
) -> core::fmt::Result {
    let mut written: Vec<(String, &'a str)> = Vec::new();
    for arg in meta.args {
        if let Some(type_) = arg.complete_type {
            write_completion_type(
                out,
                &mut written,
                arg.arg.name.to_ascii_lowercase(),
                type_,
                depth,
            )?;
        }
    }
    for flag in meta.flags {
        if let Some(type_) = flag.complete_type {
            let name = flag
                .value_name
                .unwrap_or(flag.flag.name)
                .to_ascii_lowercase();
            write_completion_type(out, &mut written, name, type_, depth)?;
        }
    }
    Ok(())
}

fn write_completion_type<'a>(
    out: &mut String,
    written: &mut Vec<(String, &'a str)>,
    name: String,
    type_: &'a str,
    depth: usize,
) -> core::fmt::Result {
    if written
        .iter()
        .any(|(written_name, written_type)| written_name == &name && *written_type == type_)
    {
        return Ok(());
    }
    indent(out, depth)?;
    writeln!(out, "complete {} type={}", quoted(&name), quoted(type_))?;
    written.push((name, type_));
    Ok(())
}

fn write_command<'a>(
    out: &mut String,
    meta: &CommandMeta<'a>,
    depth: usize,
    inherited_unknown_flags: UnknownFlags,
    bin: &str,
    overlays: &[CommandOverlay<'_>],
    path: &mut Vec<&'a str>,
) -> core::fmt::Result {
    path.push(meta.cmd.name);
    indent(out, depth)?;
    write!(out, "cmd {}", quoted(meta.cmd.name))?;
    if let Some(help) = meta.about {
        write!(out, " help={}", quoted(help))?;
    }
    if meta.hide {
        out.push_str(" hide=#true");
    }
    let effect = overlays
        .iter()
        .rev()
        .find(|overlay| overlay.command.matches(meta, path))
        .map(|overlay| overlay.effect)
        .or(meta.effect);
    if let Some(effect) = effect {
        write!(out, " effect={}", quoted(effect.as_str()))?;
    }
    // Written only where it changes, since the spec inherits it as the tables do: a command
    // that states nothing has nothing to write, and one that restates what it inherited would
    // be saying the same thing twice. A command that differs has to say so, or the setting is
    // lost on the way out.
    let effective_unknown_flags = meta.cmd.unknown_flags.unwrap_or(inherited_unknown_flags);
    if effective_unknown_flags != inherited_unknown_flags {
        write!(
            out,
            " unknown_flags={}",
            quoted(match effective_unknown_flags {
                UnknownFlags::Value => "value",
                UnknownFlags::Error => "error",
            })
        )?;
    }
    if let Some(token) = meta.restart_token {
        write!(out, " restart_token={}", quoted(token))?;
    }
    // Only where there is something to require. A command with no subcommands cannot
    // demand one, and the spec's own reader treats the pair as a mistake.
    if meta.subcommand_required && !meta.cmd.subcommands.is_empty() {
        out.push_str(" subcommand_required=#true");
    }
    if let Some(heading) = meta.subcommand_help_heading {
        write!(out, " subcommand_help_heading={}", quoted(heading))?;
    }
    if let Some(name) = meta.subcommand_value_name {
        write!(out, " subcommand_value_name={}", quoted(name))?;
    }
    if let Some(width) = meta.term_width {
        write!(out, " term_width={width}")?;
    }
    if let Some(width) = meta.max_term_width {
        write!(out, " max_term_width={width}")?;
    }
    if meta.cmd.external_subcommand {
        out.push_str(" external_subcommand=#true");
    }
    if meta.cmd.arg_required_else_help {
        out.push_str(" arg_required_else_help=#true");
    }
    if meta.cmd.dont_delimit_trailing_values {
        out.push_str(" dont_delimit_trailing_values=#true");
    }
    if !meta.args_override_self {
        out.push_str(" args_override_self=#false");
    }
    if meta.cmd.subcommand_negates_reqs {
        out.push_str(" subcommand_negates_reqs=#true");
    }
    if meta.cmd.args_conflicts_with_subcommands {
        out.push_str(" args_conflicts_with_subcommands=#true");
    }
    if meta.cmd.subcommand_precedence_over_arg {
        out.push_str(" subcommand_precedence_over_arg=#true");
    }
    if meta.cmd.allow_missing_positional {
        out.push_str(" allow_missing_positional=#true");
    }
    out.push_str(" {\n");

    let inner = depth + 1;
    for alias in meta.cmd.aliases {
        indent(out, inner)?;
        write!(out, "alias {}", quoted(alias))?;
        if meta.hidden_aliases.contains(alias) {
            out.push_str(" hide=#true");
        }
        out.push('\n');
    }
    if let Some(long_about) = meta.long_about {
        indent(out, inner)?;
        writeln!(out, "long_help {}", quoted(long_about))?;
    }
    // Text around the rest of the page. Written in the spec's order so a round trip reads the
    // same way it was written.
    for (node, text) in [
        ("before_help", meta.before_help),
        ("before_long_help", meta.before_long_help),
        ("after_help", meta.after_help),
        ("after_long_help", meta.after_long_help),
    ] {
        if let Some(text) = text {
            indent(out, inner)?;
            writeln!(out, "{node} {}", quoted(text))?;
        }
    }
    if let Some(mount) = meta.mount {
        indent(out, inner)?;
        writeln!(out, "mount run={}", quoted(mount))?;
    }
    for example in meta.examples {
        write_example(out, example, inner)?;
    }
    write_body(
        out,
        meta,
        inner,
        effective_unknown_flags,
        bin,
        overlays,
        path,
    )?;

    indent(out, depth)?;
    out.push_str("}\n");
    path.pop();
    Ok(())
}

fn write_group(out: &mut String, group: &GroupMeta<'_>, depth: usize) -> core::fmt::Result {
    indent(out, depth)?;
    write!(out, "group {}", quoted(group.name))?;
    for member in group.members {
        write!(out, " {}", quoted(member))?;
    }
    if group.required {
        out.push_str(" required=#true");
    }
    if group.multiple {
        out.push_str(" multiple=#true");
    }
    out.push('\n');
    Ok(())
}

fn write_example(out: &mut String, example: &Example<'_>, depth: usize) -> core::fmt::Result {
    indent(out, depth)?;
    write!(out, "example {}", quoted(example.code))?;
    if let Some(header) = example.header {
        write!(out, " header={}", quoted(header))?;
    }
    if let Some(help) = example.help {
        write!(out, " help={}", quoted(help))?;
    }
    out.push('\n');
    Ok(())
}

/// The `complete` blocks for one command, written where that command declares them.
///
/// Inside the `cmd` block rather than at the top level, because two sibling commands may take a
/// `tool` and mean different things by it — the reference looks a completer up on the command
/// first and only then on the spec, so this is what says which one a name belongs to.
///
/// The command line goes with the request. A caller that runs this is the *reference*, which
/// interpolates `words` through tera before running it, so a completer reading the line sees the
/// same line it would have seen natively — without it the answer would be computed against
/// nothing, which for a completer that reads an earlier flag is a wrong answer rather than a
/// missing one.
#[cfg(feature = "complete")]
fn write_completers(
    out: &mut String,
    meta: &CommandMeta<'_>,
    bin: &str,
    depth: usize,
) -> core::fmt::Result {
    for name in crate::complete::completers_on(meta) {
        indent(out, depth)?;
        write!(out, "complete {}", quoted(&name))?;
        write!(
            out,
            " run={}",
            quoted(&format!(
                "{bin} __complete_word__ --candidates {name} --line={{{{ words | shell_join | shell_quote }}}}"
            ))
        )?;
        // No `descriptions=#true`: that tells the reference to read a description after an
        // unescaped colon, and what this answers with is values. Claiming otherwise would split
        // a value containing a colon — which mise's task names are full of.
        writeln!(out)?;
    }
    Ok(())
}

fn write_flag(out: &mut String, meta: &FlagMeta<'_>, depth: usize) -> core::fmt::Result {
    indent(out, depth)?;
    write!(out, "flag {}", quoted(&flag_forms(meta)))?;

    if let Some(help) = meta.help {
        write!(out, " help={}", quoted(help))?;
    }
    if meta.required {
        out.push_str(" required=#true");
    }
    if meta.flag.global {
        out.push_str(" global=#true");
    }
    if meta.hide {
        out.push_str(" hide=#true");
    }
    write_help_hides(
        out,
        meta.hide_default_value,
        meta.hide_env,
        meta.hide_env_values,
        meta.hide_possible_values,
        meta.hide_short_help,
        meta.hide_long_help,
    )?;
    if meta.count {
        out.push_str(" count=#true");
    }
    if meta.repeatable {
        out.push_str(" var=#true");
    }
    if let Some(min) = meta.var_min {
        write!(out, " var_min={min}")?;
    }
    if let Some(max) = meta.var_max {
        write!(out, " var_max={max}")?;
    }
    if let Some(negate) = meta.flag.negate {
        // The spec writes the negation with its dashes; the table stores the bare
        // name, since that is what a token is matched against.
        write!(out, " negate={}", quoted(&format!("--{negate}")))?;
    }
    if let Some(heading) = meta.help_heading {
        write!(out, " help_heading={}", quoted(heading))?;
    }
    if let Some(effect) = meta.effect {
        write!(out, " effect={}", quoted(effect.as_str()))?;
    }
    if let Some(env) = meta.env {
        write!(out, " env={}", quoted(env))?;
    }
    write_single_default(out, meta.default)?;
    write_single_list(out, "overrides", meta.overrides)?;
    write_single_list(out, "conflicts", meta.conflicts)?;
    if meta.exclusive {
        out.push_str(" exclusive=#true");
    }
    if let Some(delimiter) = meta.delimiter {
        write!(out, " delimiter={}", quoted(&delimiter.to_string()))?;
    }
    if meta.flag.allow_hyphen_values {
        out.push_str(" allow_hyphen_values=#true");
    }
    if meta.flag.allow_negative_numbers {
        out.push_str(" allow_negative_numbers=#true");
    }
    if let Some(terminator) = meta.flag.value_terminator {
        write!(
            out,
            " value_terminator={}",
            quoted(::core::str::from_utf8(terminator).unwrap_or_default())
        )?;
    }
    if meta.flag.require_equals {
        out.push_str(" require_equals=#true");
    }
    if meta.flag.value_optional {
        out.push_str(" value_optional=#true");
    }
    if let Some(missing) = meta.flag.default_missing {
        write!(
            out,
            " default_missing={}",
            quoted(::core::str::from_utf8(missing).unwrap_or_default())
        )?;
    }
    write_single_list(out, "requires", meta.requires)?;
    write_single_list(out, "required_if", meta.required_if)?;
    write_single_list(out, "required_unless", meta.required_unless)?;
    write_single_list(out, "required_unless_all", meta.required_unless_all)?;

    let has_children = meta.long_help.is_some()
        || !meta.hidden_shorts.is_empty()
        || !meta.hidden_longs.is_empty()
        || meta.flag.takes_value
        || !meta.choices.is_empty()
        || meta.default.len() > 1
        || meta.overrides.len() > 1
        || meta.conflicts.len() > 1
        || meta.requires.len() > 1
        || !meta.requires_if.is_empty()
        || !meta.default_if.is_empty()
        || !meta.required_if_eq.is_empty()
        || !meta.required_if_eq_all.is_empty()
        || meta.required_if.len() > 1
        || meta.required_unless.len() > 1
        || meta.required_unless_all.len() > 1;
    if !has_children {
        out.push('\n');
        return Ok(());
    }

    out.push_str(" {\n");
    let inner = depth + 1;
    if let Some(long_help) = meta.long_help {
        indent(out, inner)?;
        writeln!(out, "long_help {}", quoted(long_help))?;
    }
    if !meta.hidden_shorts.is_empty() || !meta.hidden_longs.is_empty() {
        indent(out, inner)?;
        out.push_str("alias");
        for alias in meta.hidden_shorts {
            write!(out, " {}", quoted(&format!("-{}", *alias as char)))?;
        }
        for alias in meta.hidden_longs {
            write!(out, " {}", quoted(&format!("--{alias}")))?;
        }
        out.push_str(" hide=#true\n");
    }
    write_many_defaults(out, meta.default, inner)?;
    write_many_list(out, "overrides", meta.overrides, inner)?;
    write_many_list(out, "conflicts", meta.conflicts, inner)?;
    write_many_list(out, "requires", meta.requires, inner)?;
    for condition in meta.requires_if {
        indent(out, inner)?;
        writeln!(
            out,
            "requires_if {} {}",
            quoted(condition.value),
            quoted(condition.requires)
        )?;
    }
    for condition in meta.default_if {
        indent(out, inner)?;
        match condition.when {
            None => writeln!(
                out,
                "default_if {} {}",
                quoted(condition.selector),
                quoted(condition.value)
            )?,
            Some(when) => writeln!(
                out,
                "default_if {} {} {}",
                quoted(condition.selector),
                quoted(when),
                quoted(condition.value)
            )?,
        }
    }
    for condition in meta.required_if_eq {
        indent(out, inner)?;
        writeln!(
            out,
            "required_if_eq {} {}",
            quoted(condition.selector),
            quoted(condition.value)
        )?;
    }
    if !meta.required_if_eq_all.is_empty() {
        indent(out, inner)?;
        out.push_str("required_if_eq_all");
        for condition in meta.required_if_eq_all {
            write!(
                out,
                " {} {}",
                quoted(condition.selector),
                quoted(condition.value)
            )?;
        }
        out.push('\n');
    }
    write_many_list(out, "required_if", meta.required_if, inner)?;
    write_many_list(out, "required_unless", meta.required_unless, inner)?;
    write_many_list(out, "required_unless_all", meta.required_unless_all, inner)?;
    if meta.flag.takes_value {
        indent(out, inner)?;
        let exact = exact_arity(meta.value_var_min, meta.value_var_max);
        let rendered = if meta.value_names.len() <= 1 && exact.is_some_and(|n| n > 1) {
            let name = meta
                .value_names
                .first()
                .copied()
                .or(meta.value_name)
                .unwrap_or(meta.flag.name);
            (0..exact.unwrap())
                .map(|_| placeholder(name, false, meta.value_optional))
                .collect::<Vec<_>>()
                .join(" ")
        } else if meta.value_names.len() <= 1 {
            let name = meta
                .value_names
                .first()
                .copied()
                .or(meta.value_name)
                .unwrap_or(meta.flag.name);
            placeholder(name, meta.flag.variadic, meta.value_optional)
        } else {
            meta.value_names
                .iter()
                .map(|name| placeholder(name, false, meta.value_optional))
                .collect::<Vec<_>>()
                .join(" ")
        };
        write!(out, "arg {}", quoted(&rendered))?;
        if let Some(min) = meta.value_var_min {
            write!(out, " var_min={min}")?;
        }
        if let Some(max) = meta.value_var_max {
            write!(out, " var_max={max}")?;
        }
        // Square brackets alone would round-trip as required, since usage-lib reads the
        // brackets *and* the attribute: `[BUMP]` without it comes back `required=#true`.
        if meta.value_optional {
            out.push_str(" required=#false");
        }
        if let Some(validate) = meta.validate {
            write!(out, " validate={}", quoted(validate))?;
        }
        if meta.validate.is_some() {
            if let Some(error) = meta.validate_error {
                write!(out, " validate_error={}", quoted(error))?;
            }
        }
        if meta.choices.is_empty()
            && meta.accepted_choices.is_empty()
            && meta.choice_details.is_empty()
        {
            out.push('\n');
        } else {
            out.push_str(" {\n");
            write_choices(
                out,
                meta.choices,
                meta.accepted_choices,
                meta.choice_aliases,
                meta.choice_details,
                meta.ignore_case,
                inner + 1,
            )?;
            indent(out, inner)?;
            out.push_str("}\n");
        }
    } else if !meta.choices.is_empty()
        || !meta.accepted_choices.is_empty()
        || !meta.choice_details.is_empty()
    {
        write_choices(
            out,
            meta.choices,
            meta.accepted_choices,
            meta.choice_aliases,
            meta.choice_details,
            meta.ignore_case,
            inner,
        )?;
    }
    indent(out, depth)?;
    out.push_str("}\n");
    Ok(())
}

fn write_help_hides(
    out: &mut String,
    hide_default_value: bool,
    hide_env: bool,
    hide_env_values: bool,
    hide_possible_values: bool,
    hide_short_help: bool,
    hide_long_help: bool,
) -> core::fmt::Result {
    for (name, hidden) in [
        ("hide_default_value", hide_default_value),
        ("hide_env", hide_env),
        ("hide_env_values", hide_env_values),
        ("hide_possible_values", hide_possible_values),
        ("hide_short_help", hide_short_help),
        ("hide_long_help", hide_long_help),
    ] {
        if hidden {
            write!(out, " {name}=#true")?;
        }
    }
    Ok(())
}

fn write_arg(out: &mut String, meta: &ArgMeta<'_>, depth: usize) -> core::fmt::Result {
    indent(out, depth)?;
    let name = if meta.arg.name.is_empty() {
        "arg"
    } else {
        meta.arg.name
    };
    write!(out, "arg {}", quoted(&arg_placeholder(name, meta)))?;

    if let Some(help) = meta.help {
        write!(out, " help={}", quoted(help))?;
    }
    if meta.hide {
        out.push_str(" hide=#true");
    }
    write_help_hides(
        out,
        meta.hide_default_value,
        meta.hide_env,
        meta.hide_env_values,
        meta.hide_possible_values,
        meta.hide_short_help,
        meta.hide_long_help,
    )?;
    if meta.conflicts.len() == 1 {
        write!(out, " conflicts={}", quoted(meta.conflicts[0]))?;
    }
    if let Some(min) = meta.var_min {
        write!(out, " var_min={min}")?;
    }
    if let Some(max) = meta.var_max {
        write!(out, " var_max={max}")?;
    }
    if let Some(delimiter) = meta.delimiter {
        write!(out, " delimiter={}", quoted(&delimiter.to_string()))?;
    }
    if meta.arg.allow_negative_numbers {
        out.push_str(" allow_negative_numbers=#true");
    }
    if let Some(terminator) = meta.arg.value_terminator {
        write!(
            out,
            " value_terminator={}",
            quoted(::core::str::from_utf8(terminator).unwrap_or_default())
        )?;
    }
    if meta.arg.double_dash != DoubleDash::Optional {
        let mode = match meta.arg.double_dash {
            DoubleDash::Required => "required",
            DoubleDash::Preserve => "preserve",
            DoubleDash::Automatic => "automatic",
            DoubleDash::Optional => unreachable!("excluded by the branch above"),
        };
        write!(out, " double_dash={}", quoted(mode))?;
    }
    if let Some(heading) = meta.help_heading {
        write!(out, " help_heading={}", quoted(heading))?;
    }
    if let Some(env) = meta.env {
        write!(out, " env={}", quoted(env))?;
    }
    if let Some(validate) = meta.validate {
        write!(out, " validate={}", quoted(validate))?;
    }
    if meta.validate.is_some() {
        if let Some(error) = meta.validate_error {
            write!(out, " validate_error={}", quoted(error))?;
        }
    }
    write_single_default(out, meta.default)?;
    write_single_list(out, "requires", meta.requires)?;
    write_single_list(out, "required_if", meta.required_if)?;
    write_single_list(out, "required_unless", meta.required_unless)?;
    write_single_list(out, "required_unless_all", meta.required_unless_all)?;

    let has_children = meta.long_help.is_some()
        || !meta.choices.is_empty()
        || !meta.accepted_choices.is_empty()
        || !meta.choice_details.is_empty()
        || meta.default.len() > 1
        || meta.conflicts.len() > 1
        || meta.requires.len() > 1
        || meta.required_if.len() > 1
        || !meta.required_if_eq.is_empty()
        || !meta.required_if_eq_all.is_empty()
        || meta.required_unless.len() > 1
        || meta.required_unless_all.len() > 1;
    if !has_children {
        out.push('\n');
        return Ok(());
    }

    out.push_str(" {\n");
    let inner = depth + 1;
    if let Some(long_help) = meta.long_help {
        indent(out, inner)?;
        writeln!(out, "long_help {}", quoted(long_help))?;
    }
    if meta.conflicts.len() > 1 {
        indent(out, inner)?;
        out.push_str("conflicts");
        for conflict in meta.conflicts {
            write!(out, " {}", quoted(conflict))?;
        }
        out.push('\n');
    }
    write_many_list(out, "requires", meta.requires, inner)?;
    write_many_list(out, "required_if", meta.required_if, inner)?;
    for condition in meta.required_if_eq {
        indent(out, inner)?;
        writeln!(
            out,
            "required_if_eq {} {}",
            quoted(condition.selector),
            quoted(condition.value)
        )?;
    }
    if !meta.required_if_eq_all.is_empty() {
        indent(out, inner)?;
        out.push_str("required_if_eq_all");
        for condition in meta.required_if_eq_all {
            write!(
                out,
                " {} {}",
                quoted(condition.selector),
                quoted(condition.value)
            )?;
        }
        out.push('\n');
    }
    write_many_list(out, "required_unless", meta.required_unless, inner)?;
    write_many_list(out, "required_unless_all", meta.required_unless_all, inner)?;
    write_many_defaults(out, meta.default, inner)?;
    write_choices(
        out,
        meta.choices,
        meta.accepted_choices,
        meta.choice_aliases,
        meta.choice_details,
        meta.ignore_case,
        inner,
    )?;
    indent(out, depth)?;
    out.push_str("}\n");
    Ok(())
}

/// A lone value goes on the node as a property; see [`write_many_list`] for why
/// several cannot.
fn write_single_list(out: &mut String, key: &str, values: &[&str]) -> core::fmt::Result {
    if let [only] = values {
        write!(out, " {key}={}", quoted(only))?;
    }
    Ok(())
}

/// Several values, as `overrides "a" "b"`.
///
/// The same trap as defaults: `overrides="a" overrides="b"` is one node with a
/// property set twice, and only the last one survives.
fn write_many_list(
    out: &mut String,
    key: &str,
    values: &[&str],
    depth: usize,
) -> core::fmt::Result {
    if values.len() < 2 {
        return Ok(());
    }
    indent(out, depth)?;
    write!(out, "{key}")?;
    for value in values {
        write!(out, " {}", quoted(value))?;
    }
    out.push('\n');
    Ok(())
}

/// A lone default goes on the node as a property.
///
/// Several cannot: KDL properties are unique per node, so `default="a"
/// default="b"` keeps only the last one. Those go in a child block instead, which
/// is what [`write_many_defaults`] emits.
fn write_single_default(out: &mut String, defaults: &[&str]) -> core::fmt::Result {
    if let [only] = defaults {
        write!(out, " default={}", quoted(only))?;
    }
    Ok(())
}

/// Several defaults, as `default { "a"; "b" }`.
fn write_many_defaults(out: &mut String, defaults: &[&str], depth: usize) -> core::fmt::Result {
    if defaults.len() < 2 {
        return Ok(());
    }
    indent(out, depth)?;
    out.push_str("default {\n");
    for value in defaults {
        indent(out, depth + 1)?;
        writeln!(out, "{}", quoted(value))?;
    }
    indent(out, depth)?;
    out.push_str("}\n");
    Ok(())
}

fn write_choices(
    out: &mut String,
    choices: &[&str],
    accepted_choices: &[&str],
    aliases: &[(&str, &str)],
    details: &[ChoiceMeta<'_>],
    ignore_case: bool,
    depth: usize,
) -> core::fmt::Result {
    if choices.is_empty() && accepted_choices.is_empty() && details.is_empty() {
        return Ok(());
    }
    indent(out, depth)?;
    out.push_str("choices");
    if ignore_case {
        out.push_str(" ignore_case=#true");
    }

    if !details.is_empty() {
        out.push_str(" {\n");
        let is_alias = |value: &str| {
            aliases.iter().any(|(_, alias)| *alias == value)
                || details
                    .iter()
                    .flat_map(|choice| choice.aliases)
                    .any(|alias| alias.value == value)
        };
        let mut canonicals = std::vec::Vec::new();
        for value in choices
            .iter()
            .chain(aliases.iter().map(|(canonical, _)| canonical))
            .chain(accepted_choices.iter())
            .chain(details.iter().map(|choice| &choice.value))
        {
            if !is_alias(value) && !canonicals.contains(value) {
                canonicals.push(*value);
            }
        }
        for value in canonicals {
            let detail = details.iter().find(|choice| choice.value == value);
            indent(out, depth + 1)?;
            write!(out, "choice {}", quoted(value))?;
            if let Some(help) = detail.and_then(|choice| choice.help) {
                write!(out, " help={}", quoted(help))?;
            }
            if detail.is_some_and(|choice| choice.hide) || !choices.contains(&value) {
                out.push_str(" hide=#true");
            }
            let fallback_aliases: std::vec::Vec<ChoiceAliasMeta<'_>> = aliases
                .iter()
                .filter_map(|(canonical, alias)| {
                    (*canonical == value).then_some(ChoiceAliasMeta {
                        value: alias,
                        hide: !choices.contains(alias),
                    })
                })
                .collect();
            let choice_aliases = detail
                .map(|choice| choice.aliases)
                .unwrap_or(&fallback_aliases);
            if choice_aliases.is_empty() {
                out.push('\n');
                continue;
            }
            out.push_str(" {\n");
            for alias in choice_aliases {
                indent(out, depth + 2)?;
                write!(out, "alias {}", quoted(alias.value))?;
                if alias.hide {
                    out.push_str(" hide=#true");
                }
                out.push('\n');
            }
            indent(out, depth + 1)?;
            out.push_str("}\n");
        }
        indent(out, depth)?;
        out.push_str("}\n");
        return Ok(());
    }

    let has_hidden_accepted = accepted_choices
        .iter()
        .any(|value| !choices.contains(value));
    if aliases.is_empty() && !has_hidden_accepted {
        for choice in choices {
            write!(out, " {}", quoted(choice))?;
        }
        out.push('\n');
        return Ok(());
    }

    out.push_str(" {\n");
    // `choices` is the advertised set, so it contains visible aliases as well as canonical
    // values. `accepted_choices` adds everything hidden. The alias pairs let emission recover
    // which is which without putting presentation-only visibility into the parse table.
    let is_alias = |value: &str| aliases.iter().any(|(_, alias)| *alias == value);
    let mut canonicals = std::vec::Vec::new();
    for value in choices
        .iter()
        .chain(aliases.iter().map(|(canonical, _)| canonical))
        .chain(accepted_choices.iter())
    {
        if !is_alias(value) && !canonicals.contains(value) {
            canonicals.push(*value);
        }
    }
    for choice in canonicals {
        indent(out, depth + 1)?;
        write!(out, "choice {}", quoted(choice))?;
        if !choices.contains(&choice) {
            out.push_str(" hide=#true");
        }
        let choice_aliases: std::vec::Vec<&str> = aliases
            .iter()
            .filter_map(|(canonical, alias)| (*canonical == choice).then_some(*alias))
            .collect();
        if choice_aliases.is_empty() {
            out.push('\n');
            continue;
        }
        out.push_str(" {\n");
        for alias in choice_aliases {
            indent(out, depth + 2)?;
            write!(out, "alias {}", quoted(alias))?;
            if !choices.contains(&alias) {
                out.push_str(" hide=#true");
            }
            out.push('\n');
        }
        indent(out, depth + 1)?;
        out.push_str("}\n");
    }
    indent(out, depth)?;
    out.push_str("}\n");
    Ok(())
}

/// The `-s --long` form a spec uses to declare a flag.
fn flag_forms(meta: &FlagMeta<'_>) -> String {
    let flag = meta.flag;
    let mut forms = String::new();
    for short in flag.shorts {
        if meta.hidden_shorts.contains(short) {
            continue;
        }
        if !forms.is_empty() {
            forms.push(' ');
        }
        // A short is one byte, and a non-ASCII one could not have been matched
        // against a token in the first place.
        forms.push('-');
        forms.push(*short as char);
    }
    for long in flag.longs {
        if meta.hidden_longs.contains(long) {
            continue;
        }
        if !forms.is_empty() {
            forms.push(' ');
        }
        forms.push_str("--");
        forms.push_str(long);
    }
    forms
}

/// `<name>` or `<name>...`, the spec's way of writing a value placeholder.
fn placeholder(name: &str, variadic: bool, optional: bool) -> String {
    let ellipsis = if variadic { "..." } else { "" };
    let (open, close) = if optional { ('[', ']') } else { ('<', '>') };
    format!("{open}{name}{close}{ellipsis}")
}

fn exact_arity(min: Option<usize>, max: Option<usize>) -> Option<usize> {
    match (min, max) {
        (Some(min), Some(max)) if min == max => Some(min),
        _ => None,
    }
}

/// A positional's placeholder: angle brackets when required, square when not.
fn arg_placeholder(name: &str, meta: &ArgMeta<'_>) -> String {
    if let Some(arity) =
        exact_arity(meta.var_min, meta.var_max).filter(|n| *n > 1 && meta.value_names.len() <= 1)
    {
        let values = (0..arity)
            .map(|_| placeholder(name, false, !meta.required))
            .collect::<Vec<_>>()
            .join(" ");
        return values;
    }
    if meta.value_names.len() > 1 {
        let (open, close) = if meta.required {
            ('<', '>')
        } else {
            ('[', ']')
        };
        let values = meta
            .value_names
            .iter()
            .map(|name| format!("{open}{name}{close}"))
            .collect::<Vec<_>>()
            .join(" ");
        return if meta.arg.double_dash == DoubleDash::Required {
            format!("-- {values}")
        } else {
            values
        };
    }
    let ellipsis = if meta.arg.var { "..." } else { "" };
    if meta.required {
        format!("<{name}>{ellipsis}")
    } else {
        format!("[{name}]{ellipsis}")
    }
}

fn indent(out: &mut String, depth: usize) -> core::fmt::Result {
    for _ in 0..depth {
        out.push_str("    ");
    }
    Ok(())
}

fn prop(out: &mut String, key: &str, value: &str) -> core::fmt::Result {
    writeln!(out, "{key} {}", quoted(value))
}

/// Render a string value in KDL's canonical form.
///
/// KDL 2 writes a string as a plain identifier whenever that cannot be confused
/// with a number or keyword, and quotes it otherwise. Keeping this dependency-free
/// copy of the identifier predicate makes derive output byte-for-byte stable across
/// a usage-lib parse/serialize round trip.
fn quoted(value: &str) -> String {
    if !value.is_empty() && is_plain_kdl_identifier(value) {
        return value.to_owned();
    }
    let mut out = String::with_capacity(value.len() + 2);
    out.push('"');
    for ch in value.chars() {
        match ch {
            '"' => out.push_str("\\\""),
            '\\' => out.push_str("\\\\"),
            '\n' => out.push_str("\\n"),
            '\r' => out.push_str("\\r"),
            '\t' => out.push_str("\\t"),
            // KDL forbids other literal control characters in a quoted string, and
            // help text really does contain them: a CLI that colors its help with
            // ANSI codes has an escape character in the middle of it.
            c if c.is_control() => {
                let _ = write!(out, "\\u{{{:x}}}", c as u32);
            }
            c => out.push(c),
        }
    }
    out.push('"');
    out
}

fn is_plain_kdl_identifier(value: &str) -> bool {
    value.chars().all(|c| !is_disallowed_kdl_identifier_char(c))
        && !starts_like_kdl_number(value)
        && !matches!(value, "inf" | "-inf" | "nan" | "true" | "false" | "null")
}

fn starts_like_kdl_number(value: &str) -> bool {
    let unsigned = value.strip_prefix(['-', '+']).unwrap_or(value).as_bytes();
    unsigned.first().is_some_and(u8::is_ascii_digit)
        || (unsigned.first() == Some(&b'.') && unsigned.get(1).is_some_and(u8::is_ascii_digit))
}

fn is_disallowed_kdl_identifier_char(c: char) -> bool {
    matches!(
        c,
        '\\' | '/' | '(' | ')' | '{' | '}' | '[' | ']' | ';' | '"' | '#' | '='
    ) || matches!(
        c,
        '\u{0000}'..='\u{0008}'
            | '\u{000A}'..='\u{001F}'
            | '\u{0085}'
            | '\u{00A0}'
            | '\u{1680}'
            | '\u{2000}'..='\u{200A}'
            | '\u{200E}'..='\u{200F}'
            | '\u{2028}'..='\u{202F}'
            | '\u{205F}'
            | '\u{2066}'..='\u{2069}'
            | '\u{3000}'
            | '\u{FEFF}'
    ) || matches!(c, '\u{0009}' | '\u{0020}')
}

/// A struct that describes one command's flags and arguments.
///
/// Implemented by the derive on a subcommand's argument struct. The root's
/// generated parse needs to name the type that accumulates a subcommand's values
/// while parsing, and cannot know which module the derive put it in — an
/// associated type is how it names it regardless.
/// A type whose values are a fixed set of words.
///
/// What a CLI calls an enum: `--shell bash`. The words are what the spec lists as
/// `choices`, so declaring them once on the type keeps help, completions and the check that
/// rejects a wrong value reading from the same place — rather than a list in an attribute
/// that has to be kept in step with the type by hand.
pub trait ValueEnum: Sized {
    /// Every word this type accepts, in the order it declared them.
    const CHOICES: &'static [&'static str];
    /// Canonical words plus aliases accepted by the type.
    const ACCEPTED_CHOICES: &'static [&'static str] = Self::CHOICES;
    /// Canonical-to-alias pairs, used by spec emission to retain the distinction.
    const ALIASES: &'static [(&'static str, &'static str)] = &[];
    /// Presentation metadata for canonical values and aliases.
    const DETAILS: &'static [ChoiceMeta<'static>] = &[];
    const IGNORE_CASE: bool = false;
    /// Convert one canonical word or alias into its enum variant.
    fn from_choice(value: &str) -> Option<Self>;
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct ChoiceMeta<'a> {
    pub value: &'a str,
    pub help: Option<&'a str>,
    pub hide: bool,
    pub aliases: &'a [ChoiceAliasMeta<'a>],
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct ChoiceAliasMeta<'a> {
    pub value: &'a str,
    pub hide: bool,
}

pub fn choice_matches(choices: &[&str], value: &str, ignore_case: bool) -> bool {
    choices
        .iter()
        .any(|choice| *choice == value || ignore_case && choice.eq_ignore_ascii_case(value))
}

/// One value a flag was given, in a vocabulary this crate can hold.
///
/// A settings layer is `usage-config`'s idea, and this crate does not know that crate exists —
/// but a group of flags declared once and flattened into several commands is *this* crate's
/// idea, and its values have to reach the command that owns them somehow. So a flattened group
/// hands its parent what it was given in these terms, and the parent, which is the one place
/// that knows what a setting is, turns them into the layer.
///
/// Deliberately small: what a flag can be given, and nothing about types. The registry decides
/// what `"8"` means, and a second opinion here would be the first thing to disagree with it.
#[derive(Debug, Clone, PartialEq)]
pub enum SettingGiven {
    /// A switch, `true` for the flag and `false` for its negation. Given either way.
    Bool(bool),
    /// A count: `-vvv` is three.
    Int(i64),
    /// A flag's argument, as text.
    Text(String),
    /// A repeated flag, item by item — joining them would lose an item holding the separator.
    List(Vec<String>),
    /// A value that is not text at all: bytes an argument can hold and a setting cannot. Said
    /// rather than rendered, because rendering it lossily sets a setting to a value nobody typed.
    NotText,
}

/// The bindings of several commands, joined into one array.
///
/// `const` because a binding table is a `static` a test reads without running anything, and a
/// slice cannot be concatenated in a const initializer without somewhere to put the result. `N`
/// is the sum of the parts' lengths, which a generated caller computes from the same consts.
pub const fn concat_bindings<const N: usize>(
    parts: &[&'static [(&'static str, &'static str)]],
) -> [(&'static str, &'static str); N] {
    let mut joined = [("", ""); N];
    let mut at = 0;
    let mut part = 0;
    while part < parts.len() {
        let mut i = 0;
        while i < parts[part].len() {
            joined[at] = parts[part][i];
            at += 1;
            i += 1;
        }
        part += 1;
    }
    joined
}

/// Presence information exposed across a flattened [`CommandArgs`] boundary.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct ArgumentState {
    /// The canonical name used in diagnostics.
    pub name: &'static str,
    /// Whether argv or an environment fallback supplied the argument.
    pub given: bool,
    /// Whether the argument is satisfied, including an unconditional default.
    pub satisfied: bool,
}

pub trait CommandArgs: Sized {
    /// Values collected so far. Partly-filled by construction, since a parse can
    /// stop early.
    ///
    /// No `Default` bound: [`CommandArgs::start`] is what produces a fresh one,
    /// because a command with subcommands of its own has nested state that a derived
    /// `Default` cannot set up.
    type Partial;

    /// The parse tables for this command.
    ///
    /// A const rather than a method, because a parent splices it into its own
    /// `static` tables and a method call is not allowed there. That is the whole
    /// reason this trait exists: the tables stay static all the way down, so
    /// nothing is built at run time to start a parse.
    const COMMAND: &'static Command<'static>;

    /// The metadata for this command.
    const META: &'static CommandMeta<'static>;

    /// A partial with any declared defaults already in place.
    ///
    /// Not `Default::default()`, because a default has to be there before parsing
    /// starts: nothing afterwards distinguishes it from what the user typed.
    fn start() -> Self::Partial;

    /// Take one event, and say whether it belonged to this command.
    ///
    /// Keys are unique across a CLI, so an event that is not this command's is left
    /// for whoever owns it rather than mistaken for a local field.
    fn apply(partial: &mut Self::Partial, event: &crate::Event<'_, '_, '_>) -> bool;

    /// Every flag this command reads into a setting, and the setting it sets.
    ///
    /// Empty by default, so a command that binds nothing implements nothing and a parent can ask
    /// any command without knowing which kind it got. What a parent joins into its own table, and
    /// what `usage_config::Registry::drift` is held against.
    const SETTINGS_BINDINGS: &'static [(&'static str, &'static str)] = &[];

    /// The settings this command line gave values for.
    ///
    /// From the partial rather than from the built struct, and only for flags that were actually
    /// given: a `bool` field is `false` whether the flag was left off or negated, and the command
    /// line outranks every file on the machine.
    ///
    /// Empty by default, for the same reason as [`CommandArgs::SETTINGS_BINDINGS`].
    fn settings_given(partial: &Self::Partial) -> Vec<(&'static str, SettingGiven)> {
        let _ = partial;
        Vec::new()
    }

    /// One declaration in this command that ended up being given, if any.
    ///
    /// Used to enforce relationships across a flattened `CommandArgs` boundary, where the
    /// parent can see the nested partial only through this trait.
    fn any_given(partial: &Self::Partial) -> Option<&'static str> {
        let _ = partial;
        None
    }

    /// One exclusive flag in this command that was given, if any.
    ///
    /// Like [`CommandArgs::any_given`], this is the composition point for flattened argument
    /// groups and selected subcommands. Parents need the latter to apply whole-invocation
    /// exclusivity and its requiredness escape across a command boundary.
    fn exclusive_given(partial: &Self::Partial) -> Option<&'static str> {
        let _ = partial;
        None
    }

    /// Find an argument by any selector it accepts.
    ///
    /// Parents use this to enforce a relationship declared beside a flattened
    /// argument group. The default keeps hand-written implementations source compatible.
    fn argument_state(partial: &Self::Partial, selector: &str) -> Option<ArgumentState> {
        let _ = (partial, selector);
        None
    }

    /// Whether a selected argument has an explicitly supplied value.
    ///
    /// This is the value-aware half needed by conditional defaults and requirements.
    fn argument_matches(partial: &Self::Partial, selector: &str, value: &[u8]) -> Option<bool> {
        let _ = (partial, selector, value);
        None
    }

    /// Reset the flag named by `selector` after an overriding token wins.
    fn displace(partial: &mut Self::Partial, selector: &str) -> bool {
        let _ = (partial, selector);
        false
    }

    /// Whether this event binds the flag named by `selector`.
    fn event_matches(event: &crate::Event<'_, '_, '_>, selector: &str) -> bool {
        let _ = (event, selector);
        false
    }

    /// Fill fields in this command from their declared defaults.
    ///
    /// Kept separate from [`CommandArgs::check`] so a parent can preserve defaults in a
    /// flattened argument group while an unrelated exclusive flag suppresses only that
    /// group's missing-value checks.
    fn apply_defaults(partial: &mut Self::Partial) {
        let _ = partial;
    }

    /// Fill fields in this command from their declared environment variables.
    ///
    /// A parent calls this before relationships that cross a flattened `CommandArgs`
    /// boundary, so those relationships see the same values as the nested command's own
    /// checks. Empty by default for hand-written implementations.
    fn apply_env(partial: &mut Self::Partial) {
        let _ = partial;
    }

    /// Everything this command decides after the last token: required-ness,
    /// choices, and how many values a variadic got.
    ///
    /// Separate from [`CommandArgs::build`] because only the command that was
    /// actually reached is judged — a flag that `install` requires says nothing
    /// about an invocation that ran `run`.
    /// Defaulted, so a hand-written implementation with nothing to check is not
    /// forced to say so — and adding a check to the derive does not break one.
    fn check<'t, 'v>(partial: &mut Self::Partial) -> Result<(), crate::Error<'t, 'v>> {
        let _ = partial;
        Ok(())
    }

    /// Run checks under the repeat policy of the command that composed these args.
    ///
    /// A flattened argument group is part of its parent's command, so the parent's
    /// `args_override_self` setting governs repeats in the group as well. Hand-written
    /// implementations keep their existing behavior through this default.
    fn check_with_args_override_self<'t, 'v>(
        partial: &mut Self::Partial,
        args_override_self: bool,
    ) -> Result<(), crate::Error<'t, 'v>> {
        let _ = args_override_self;
        Self::check(partial)
    }

    /// Build the struct from what was collected.
    ///
    /// Fallible because a command can require a subcommand of its own, and "none was
    /// given" is only knowable here — at the point where the value has to exist.
    fn build<'t, 'v>(partial: Self::Partial) -> Result<Self, crate::Error<'t, 'v>>;
}

/// How many flags on `command` accept `selector`.
///
/// Used by derives after flattened tables are composed, where neither expansion could validate
/// a cross-boundary relationship on its own.
pub const fn flag_selector_count(command: &Command<'_>, selector: &str) -> usize {
    let selector = selector.as_bytes();
    let mut count = 0;
    let mut i = 0;
    while i < command.flags.len() {
        let flag = command.flags[i];
        let mut matched = false;
        if selector.len() == 2 && selector[0] == b'-' && selector[1] != b'-' {
            let mut short = 0;
            while short < flag.shorts.len() {
                if flag.shorts[short] == selector[1] {
                    matched = true;
                }
                short += 1;
            }
        } else if selector.len() > 2 && selector[0] == b'-' && selector[1] == b'-' {
            let mut long = 0;
            while long < flag.longs.len() {
                if long_selector_equal(selector, flag.longs[long].as_bytes()) {
                    matched = true;
                }
                long += 1;
            }
            if let Some(negate) = flag.negate {
                if long_selector_equal(selector, negate.as_bytes()) {
                    matched = true;
                }
            }
        }
        if matched {
            count += 1;
        }
        i += 1;
    }
    count
}

const fn long_selector_equal(selector: &[u8], name: &[u8]) -> bool {
    if selector.len() != name.len() + 2 {
        return false;
    }
    let mut i = 0;
    while i < name.len() {
        if selector[i + 2] != name[i] {
            return false;
        }
        i += 1;
    }
    true
}

/// An enum whose variants are a command's subcommands.
///
/// Implemented by the derive on the enum a `subcommand` field holds.
pub trait Subcommands: Sized {
    /// Values collected for whichever variant is being filled.
    type Partial: Default;

    /// The parse tables for the variants, to splice into the parent command's
    /// `static` tables — hence a const. See [`CommandArgs::COMMAND`].
    const COMMANDS: &'static [&'static Command<'static>];

    /// The metadata for the variants, in the same order.
    const METAS: &'static [&'static CommandMeta<'static>];

    /// Whether one of these variants is an [`external_subcommand`](Command::external_subcommand).
    const HAS_EXTERNAL: bool = false;

    /// The variant index of the catch-all, if any. Not a position in [`COMMANDS`]:
    /// an external variant is not a named command.
    const EXTERNAL: Option<usize> = None;

    /// Maps a position in [`COMMANDS`] to a variant index.
    ///
    /// Identity when there is no catch-all. When [`HAS_EXTERNAL`] is set, the named
    /// commands sit in `COMMANDS` without the external variant, so a table position
    /// is not a variant index and this is how the two are tied together.
    const VARIANT_OF: &'static [usize] = &[];

    /// Take one event, and say whether it belonged to the selected command.
    ///
    /// `selected` is a variant index, or `None` before any of them has been reached —
    /// in which case the event cannot be theirs and nothing is asked.
    ///
    /// Only the selected one is asked, which is both cheaper and *necessary*. Cheaper because
    /// a CLI with a hundred subcommands would otherwise offer every event to all hundred.
    /// Necessary because two commands can legitimately hold the same declaration once
    /// `#[usage(flatten)]` exists: mise gives one `ConfigLs` to both `config` and `config ls`,
    /// so the same key is in both tables, and whichever was asked first would claim the event
    /// — including a command the user never named.
    fn apply(
        partial: &mut Self::Partial,
        selected: Option<usize>,
        event: &crate::Event<'_, '_, '_>,
    ) -> bool;

    /// Make room for the variant at `selected`, if it is not already the one being filled.
    ///
    /// Called when a command word selects a variant, before any event reaches it.
    /// [`Subcommands::Partial`] holds *one* command's values rather than every command's, so
    /// the storage for a variant comes into being here instead of at the start of the parse:
    /// an invocation that reached `mise use` never materialises the other 209.
    ///
    /// Idempotent by contract. A second call naming the variant already being filled must
    /// leave what has been collected alone — a restart token can re-announce the command
    /// that is already selected, and re-starting it there would discard the parse so far.
    ///
    /// The default does nothing, which is correct for a `Partial` that has room for every
    /// variant from the start.
    fn begin(partial: &mut Self::Partial, selected: usize) {
        let _ = (partial, selected);
    }

    /// Every flag any of these commands reads into a setting, and the setting it sets.
    ///
    /// Every variant's, not the selected one's: a binding table says what the CLI *can* do, and
    /// is compared against a spec that documents all of them.
    const SETTINGS_BINDINGS: &'static [(&'static str, &'static str)] = &[];

    /// The settings the selected command was given values for, and no other command's.
    ///
    /// `None` selected is nothing given, which is also what a CLI that reached no subcommand
    /// contributed. Unlike the bindings, this is about one invocation.
    fn settings_given(
        partial: &Self::Partial,
        selected: Option<usize>,
    ) -> Vec<(&'static str, SettingGiven)> {
        let _ = (partial, selected);
        Vec::new()
    }

    /// One declaration in the selected command that was given, if any.
    fn any_given(partial: &Self::Partial, selected: Option<usize>) -> Option<&'static str> {
        let _ = (partial, selected);
        None
    }

    /// One exclusive flag in the selected command that was given, if any.
    ///
    /// This lets the parent compare its own fields with the selected child's without
    /// exposing the child's generated partial type.
    fn exclusive_given(partial: &Self::Partial, selected: Option<usize>) -> Option<&'static str> {
        let _ = (partial, selected);
        None
    }

    /// Fill fields in the selected command from their declared environment variables.
    ///
    /// A parent calls this before relationships that cross the subcommand boundary, just as
    /// [`CommandArgs::apply_env`] prepares a flattened argument group.
    fn apply_env(partial: &mut Self::Partial, selected: Option<usize>) {
        let _ = (partial, selected);
    }

    /// Check the selected command's requirements, and nothing else's.
    ///
    /// A flag that `install` requires says nothing about an invocation that ran
    /// `run`, so only the command that was actually reached is judged.
    ///
    /// `selected` is a variant index, the same value [`apply`] is given — mapped
    /// through [`VARIANT_OF`] when [`HAS_EXTERNAL`] is set, so it is not a
    /// position in [`COMMANDS`]. Two commands whose keys happen to collide still
    /// cannot be confused for one another: the index names the variant, not a
    /// table slot.
    fn check<'t, 'v>(
        partial: &mut Self::Partial,
        selected: usize,
    ) -> Result<(), crate::Error<'t, 'v>>;

    /// Build the variant at `selected`, a variant index.
    ///
    /// The same index [`apply`] and [`check`] take — mapped through [`VARIANT_OF`]
    /// when [`HAS_EXTERNAL`] is set, so it is not a position in [`COMMANDS`]. An
    /// external variant is not in that table, and a caller that indexed `COMMANDS`
    /// by this value would read the wrong command or go out of bounds.
    ///
    /// `None` when no variant was selected, which a caller reads as "no subcommand
    /// was given". An `Err` comes from building the variant that *was* selected.
    fn select<'t, 'v>(
        partial: Self::Partial,
        selected: usize,
    ) -> Result<Option<Self>, crate::Error<'t, 'v>>;
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn composed_relationship_selectors_count_flag_spellings() {
        static FIRST: Flag = Flag {
            longs: &["first", "alias"],
            shorts: b"f",
            negate: Some("no-first"),
            ..Flag::BOOL
        };
        static SECOND: Flag = Flag {
            longs: &["second"],
            ..Flag::BOOL
        };
        static COMMAND: Command = Command {
            flags: &[&FIRST, &SECOND],
            ..Command::EMPTY
        };
        assert_eq!(flag_selector_count(&COMMAND, "--first"), 1);
        assert_eq!(flag_selector_count(&COMMAND, "--alias"), 1);
        assert_eq!(flag_selector_count(&COMMAND, "--no-first"), 1);
        assert_eq!(flag_selector_count(&COMMAND, "-f"), 1);
        assert_eq!(flag_selector_count(&COMMAND, "first"), 0);
        assert_eq!(flag_selector_count(&COMMAND, "--missing"), 0);
    }

    #[test]
    fn quoting_escapes_what_would_break_a_document() {
        assert_eq!(quoted("plain"), "plain");
        assert_eq!(quoted("true"), r#""true""#);
        assert_eq!(quoted("12"), r#""12""#);
        assert_eq!(quoted("-12"), r#""-12""#);
        assert_eq!(quoted(".5"), r#"".5""#);
        assert_eq!(quoted("-.5"), r#""-.5""#);
        assert_eq!(quoted("+.5"), r#""+.5""#);
        assert_eq!(quoted("with space"), r#""with space""#);
        assert_eq!(quoted(r#"say "hi""#), r#""say \"hi\"""#);
        assert_eq!(quoted("a\\b"), r#""a\\b""#);
        assert_eq!(quoted("one\ntwo"), r#""one\ntwo""#);
    }

    fn test_completer(_: &CompleteCtx<'_>) -> Vec<Candidate<'static>> {
        Vec::new()
    }

    #[test]
    fn generated_completer_bridges_preserve_the_argv_vector() {
        static ARG: Arg = Arg {
            name: "ARG",
            ..Arg::REQUIRED
        };
        static ARG_META: ArgMeta = ArgMeta {
            arg: &ARG,
            complete: Some(test_completer),
            ..ArgMeta::EMPTY
        };
        static COMMAND: Command = Command {
            args: &[&ARG],
            ..Command::EMPTY
        };
        static META: CommandMeta = CommandMeta {
            cmd: &COMMAND,
            args: &[ARG_META],
            ..CommandMeta::EMPTY
        };
        static SPEC: Spec = Spec {
            name: "ex",
            bin: Some("ex"),
            root: &META,
            ..Spec::EMPTY
        };

        let kdl = SPEC.to_kdl();
        assert!(
            kdl.contains(
                "__complete_word__ --candidates arg --line={{ words | shell_join | shell_quote }}"
            ),
            "{kdl}"
        );
        assert!(!kdl.contains("replace(from="), "{kdl}");
    }

    #[test]
    fn choice_emission_preserves_visible_and_hidden_aliases() {
        let mut out = String::new();
        write_choices(
            &mut out,
            &["shown", "short"],
            &["shown", "secret", "short", "secret-short"],
            &[("shown", "short"), ("shown", "secret-short")],
            &[],
            false,
            0,
        )
        .unwrap();
        assert_eq!(
            out,
            "choices {\n    choice shown {\n        alias short\n        alias secret-short hide=#true\n    }\n    choice secret hide=#true\n}\n"
        );

        out.clear();
        write_choices(&mut out, &[], &["secret"], &[], &[], false, 0).unwrap();
        assert_eq!(
            out, "choices {\n    choice secret hide=#true\n}\n",
            "an entirely hidden set must still be emitted"
        );

        out.clear();
        write_choices(
            &mut out,
            &["plain", "shown", "short"],
            &["plain", "shown", "short", "secret"],
            &[("shown", "short")],
            &[
                ChoiceMeta {
                    value: "shown",
                    help: Some("Shown value"),
                    hide: false,
                    aliases: &[ChoiceAliasMeta {
                        value: "short",
                        hide: false,
                    }],
                },
                ChoiceMeta {
                    value: "secret",
                    help: None,
                    hide: true,
                    aliases: &[],
                },
            ],
            false,
            0,
        )
        .unwrap();
        assert_eq!(
            out,
            "choices {\n    choice plain\n    choice shown help=\"Shown value\" {\n        alias short\n    }\n    choice secret hide=#true\n}\n"
        );
    }

    #[test]
    fn an_argument_no_word_can_reach_is_caught_where_the_table_is_joined() {
        // The arrangement `flatten` makes possible and neither expansion can see: an
        // unbounded variadic declared in one struct and an argument after it in another. The
        // derive checks this within a struct; across the boundary it has no way to.
        static FILES: Arg = Arg {
            name: "files",
            ..Arg::VAR
        };
        static AFTER: Arg = Arg {
            name: "after",
            ..Arg::REQUIRED
        };
        static BROKEN: Command = Command {
            name: "ex",
            args: &[&FILES, &AFTER],
            ..Command::EMPTY
        };
        assert_eq!(unfillable_arg(&BROKEN), Some("after"));

        // A bound stops the variadic, so what follows is reachable.
        static BOUNDED: Arg = Arg {
            name: "files",
            var_max: Some(2),
            ..Arg::VAR
        };
        static WITH_BOUND: Command = Command {
            name: "ex",
            args: &[&BOUNDED, &AFTER],
            ..Command::EMPTY
        };
        assert_eq!(unfillable_arg(&WITH_BOUND), None);

        // So does a `--`, which is what mise's `run`, `exec` and `git` rely on.
        static PAST_SEPARATOR: Arg = Arg {
            name: "args_last",
            double_dash: DoubleDash::Required,
            ..Arg::VAR
        };
        static WITH_SEPARATOR: Command = Command {
            name: "ex",
            args: &[&FILES, &PAST_SEPARATOR],
            ..Command::EMPTY
        };
        assert_eq!(unfillable_arg(&WITH_SEPARATOR), None);

        // But only one separator exists, so nothing can follow the variadic behind it.
        static AFTER_THE_SEPARATOR: Command = Command {
            name: "ex",
            args: &[&PAST_SEPARATOR, &AFTER],
            ..Command::EMPTY
        };
        assert_eq!(unfillable_arg(&AFTER_THE_SEPARATOR), Some("after"));

        // Nor a variadic that keeps the separator as one of its values: `preserve` means the
        // `--` never ends anything, so the argument waiting for one waits forever. The derive
        // refuses this within one struct; here is where the same layout is caught when the two
        // halves are on either side of a `#[usage(flatten)]` and neither expansion can see it.
        static KEEPS_SEPARATOR: Arg = Arg {
            name: "kept",
            double_dash: DoubleDash::Preserve,
            ..Arg::VAR
        };
        static SEPARATOR_KEPT: Command = Command {
            name: "ex",
            args: &[&KEEPS_SEPARATOR, &PAST_SEPARATOR],
            ..Command::EMPTY
        };
        assert_eq!(unfillable_arg(&SEPARATOR_KEPT), Some("args_last"));

        // And a bound puts it back, `preserve` or not: the variadic hands over once it is full,
        // so what follows is reachable without needing a separator at all.
        static KEEPS_SEPARATOR_BOUNDED: Arg = Arg {
            name: "kept",
            double_dash: DoubleDash::Preserve,
            var_max: Some(2),
            ..Arg::VAR
        };
        static BOUNDED_KEEPER: Command = Command {
            name: "ex",
            args: &[&KEEPS_SEPARATOR_BOUNDED, &PAST_SEPARATOR],
            ..Command::EMPTY
        };
        assert_eq!(unfillable_arg(&BOUNDED_KEEPER), None);

        // Found at any depth, since a flattened struct can be used by a nested command.
        static NESTED: Command = Command {
            name: "outer",
            subcommands: &[&BROKEN],
            ..Command::EMPTY
        };
        assert_eq!(unfillable_arg(&NESTED), Some("after"));
    }

    #[test]
    fn a_subcommand_writes_unknown_flags_only_where_it_differs() {
        // A command that restates what it inherited says nothing — and one that says
        // nothing at all has nothing to write either — but a command that differs has to
        // say so, or the setting never reaches the spec.
        static STRICT_SUB: Command = Command {
            name: "build",
            unknown_flags: Some(UnknownFlags::Error),
            ..Command::EMPTY
        };
        static SILENT_SUB: Command = Command {
            name: "test",
            ..Command::EMPTY
        };
        static LENIENT_SUB: Command = Command {
            name: "exec",
            unknown_flags: Some(UnknownFlags::Value),
            ..Command::EMPTY
        };
        static ROOT: Command = Command {
            name: "ex",
            subcommands: &[&STRICT_SUB, &SILENT_SUB, &LENIENT_SUB],
            unknown_flags: Some(UnknownFlags::Error),
            ..Command::EMPTY
        };
        static STRICT_META: CommandMeta = CommandMeta {
            cmd: &STRICT_SUB,
            ..CommandMeta::EMPTY
        };
        static SILENT_META: CommandMeta = CommandMeta {
            cmd: &SILENT_SUB,
            ..CommandMeta::EMPTY
        };
        static LENIENT_META: CommandMeta = CommandMeta {
            cmd: &LENIENT_SUB,
            ..CommandMeta::EMPTY
        };
        static ROOT_META: CommandMeta = CommandMeta {
            cmd: &ROOT,
            subcommands: &[&STRICT_META, &SILENT_META, &LENIENT_META],
            ..CommandMeta::EMPTY
        };

        let mut out = String::new();
        write_body(
            &mut out,
            &ROOT_META,
            0,
            UnknownFlags::Value,
            "ex",
            &[],
            &mut Vec::new(),
        )
        .unwrap();

        // Counted rather than checked with `contains`, which is how a duplicated
        // write survived review: `unknown_flags="value" unknown_flags="value"` contains
        // the string it was checked for. A KDL node carrying the same property twice
        // keeps only the last, so once is the whole point.
        let line = |name: &str| -> String {
            out.lines()
                .map(str::trim)
                .find(|l| l.starts_with(&format!("cmd {name}")))
                .unwrap_or_else(|| panic!("no `{name}` command was written:\n{out}"))
                .to_string()
        };

        let build = line("build");
        assert!(
            !build.contains("unknown_flags"),
            "a subcommand matching the enclosing command should not repeat it: {build}"
        );

        let test = line("test");
        assert!(
            !test.contains("unknown_flags"),
            "a subcommand that declares nothing inherits, and writes nothing: {test}"
        );

        let exec = line("exec");
        assert_eq!(
            exec.matches("unknown_flags=value").count(),
            1,
            "a differing subcommand declares it exactly once: {exec}"
        );
    }

    #[test]
    fn a_spec_view_applies_identity_and_sparse_effects_without_mutating_the_base() {
        static RM: Command = Command {
            name: "rm",
            key: 12,
            ..Command::EMPTY
        };
        static DIST_TAG: Command = Command {
            name: "dist-tag",
            subcommands: &[&RM],
            ..Command::EMPTY
        };
        static LIST: Command = Command {
            name: "list",
            key: 13,
            ..Command::EMPTY
        };
        static ROOT: Command = Command {
            name: "ex",
            subcommands: &[&DIST_TAG, &LIST],
            ..Command::EMPTY
        };
        static RM_META: CommandMeta = CommandMeta {
            cmd: &RM,
            ..CommandMeta::EMPTY
        };
        static DIST_TAG_META: CommandMeta = CommandMeta {
            cmd: &DIST_TAG,
            subcommands: &[&RM_META],
            ..CommandMeta::EMPTY
        };
        static LIST_META: CommandMeta = CommandMeta {
            cmd: &LIST,
            ..CommandMeta::EMPTY
        };
        static ROOT_META: CommandMeta = CommandMeta {
            cmd: &ROOT,
            subcommands: &[&DIST_TAG_META, &LIST_META],
            ..CommandMeta::EMPTY
        };
        static SPEC: Spec = Spec {
            name: "ex",
            bin: Some("ex"),
            version: Some("1.0.0"),
            root: &ROOT_META,
            ..Spec::EMPTY
        };
        static OVERLAY: [CommandOverlay<'static>; 2] = [
            CommandOverlay::effect("dist-tag rm", Effect::Destructive),
            CommandOverlay::effect_for(13, Effect::Read),
        ];

        let view = SPEC
            .view()
            .name("embedded")
            .bin("embedded")
            .version("2.0.0")
            .overlay(&OVERLAY);
        let effective = view.spec();
        assert_eq!(effective.name, "embedded");
        assert_eq!(effective.bin, Some("embedded"));
        assert_eq!(effective.version, Some("2.0.0"));

        let kdl = view.to_kdl();
        assert!(kdl.contains("name embedded"), "{kdl}");
        assert!(kdl.contains("cmd rm effect=destructive"), "{kdl}");
        assert!(kdl.contains("cmd list effect=read"), "{kdl}");

        let base = SPEC.to_kdl();
        assert!(base.contains("name ex"), "{base}");
        assert!(base.contains("version \"1.0.0\""), "{base}");
        assert!(!base.contains("effect="), "{base}");

        let without_version = SPEC.view().version("2.0.0").omit_version();
        assert_eq!(without_version.spec().version, None);
        assert!(!without_version.to_kdl().contains("version "));
        assert_eq!(
            SPEC.view().omit_version().version("3.0.0").spec().version,
            Some("3.0.0")
        );

        let runtime_name = String::from("runtime");
        let runtime_path = String::from("list");
        let runtime_overlays = vec![CommandOverlay::effect(&runtime_path, Effect::Write)];
        let runtime = SPEC
            .view()
            .name(&runtime_name)
            // Longer-lived identity and policy values remain safe after the
            // view has already narrowed to a runtime borrow.
            .bin("embedded")
            .version("2.0.0")
            .overlay(&OVERLAY)
            .overlay(&runtime_overlays)
            .to_kdl();
        assert!(runtime.contains("name runtime"), "{runtime}");
        assert!(runtime.contains("cmd list effect=write"), "{runtime}");
        assert!(runtime.contains("cmd rm effect=destructive"), "{runtime}");
    }

    #[test]
    fn flag_forms_lists_shorts_then_longs() {
        static F: Flag = Flag {
            longs: &["jobs", "workers"],
            shorts: b"jw",
            ..Flag::VALUE
        };
        assert_eq!(
            flag_forms(&FlagMeta {
                flag: &F,
                hidden_shorts: b"w",
                hidden_longs: &["workers"],
                ..FlagMeta::EMPTY
            }),
            "-j --jobs"
        );
    }

    #[test]
    fn placeholders_show_arity_and_optionality() {
        static REQ: Arg = Arg {
            name: "file",
            ..Arg::REQUIRED
        };
        static VAR: Arg = Arg {
            name: "rest",
            ..Arg::VAR
        };
        let required = ArgMeta {
            arg: &REQ,
            ..ArgMeta::EMPTY
        };
        let optional_var = ArgMeta {
            arg: &VAR,
            required: false,
            ..ArgMeta::EMPTY
        };
        assert_eq!(arg_placeholder("file", &required), "<file>");
        assert_eq!(arg_placeholder("rest", &optional_var), "[rest]...");
        assert_eq!(placeholder("n", false, false), "<n>");
        assert_eq!(placeholder("pattern", true, false), "<pattern>...");
        assert_eq!(placeholder("BUMP", false, true), "[BUMP]");
    }
}
#[test]
#[should_panic(expected = "two flattened groups on one command have the same name")]
fn concatenating_group_metadata_rejects_duplicate_names() {
    static LEFT: [GroupMeta; 1] = [GroupMeta {
        name: "input",
        members: &["--file", "--url"],
        required: false,
        multiple: false,
    }];
    static RIGHT: [GroupMeta; 1] = [GroupMeta {
        name: "input",
        members: &["--json", "--yaml"],
        required: false,
        multiple: false,
    }];
    let _ = concat_group_metas::<2>(&[&LEFT, &RIGHT]);
}
