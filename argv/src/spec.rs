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
    /// The root command, and the home of everything a spec declares at its top level.
    ///
    /// A KDL spec has one place for surrounding text and examples — the top level — and the
    /// reference reads what is written there as the root's *and* as the default for every
    /// other page. So they live here, on the root's metadata, rather than in a second set of
    /// fields on the spec: two homes for one declaration is two answers to one question, and
    /// `to_kdl` and the renderer picked differently.
    pub root: &'a CommandMeta<'a>,
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
        root: &CommandMeta::EMPTY,
    };
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

/// A set of one command's flags that relate to one another as a set.
///
/// Pairwise `conflicts` can say "at most one of these", once per pair; what it cannot
/// say is that one of them is *needed*, which is a statement about the set.
#[derive(Debug, Clone, Copy)]
pub struct GroupMeta<'a> {
    /// What the group is called. It appears in the message a failed check produces, and
    /// it is how a reader tells two groups on one command apart.
    pub name: &'a str,
    /// The flags in the group, as selectors — `--long` or `-s`, the way every other
    /// relationship names a flag.
    pub members: &'a [&'a str],
    /// Whether at least one member has to be given.
    pub required: bool,
    /// Whether more than one member may be given. False is what makes a bare group
    /// mutual exclusion, as it does in clap.
    pub multiple: bool,
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
    /// Short help, shown by `-h`.
    pub help: Option<&'a str>,
    /// Long help, shown by `--help`.
    pub long_help: Option<&'a str>,
    /// The placeholder for the flag's value, such as `n` in `--jobs <n>`.
    pub value_name: Option<&'a str>,
    pub env: Option<&'a str>,
    pub default: &'a [&'a str],
    pub choices: &'a [&'a str],
    pub required: bool,
    /// Whether the flag's value may be left off, as in `--bump` or `--bump 5`.
    ///
    /// Help only, and deliberately: usage-lib's parser refuses a bare `--bump` exactly as it
    /// refuses a bare `--port`, so this changes no binding — it changes the brackets, `[BUMP]`
    /// rather than `<BUMP>`, which is what a spec's `arg "[BUMP]" required=#false` says. In
    /// [`FlagMeta`] and not in [`Flag`] for that reason: a parse never reads it.
    pub value_optional: bool,
    pub hide: bool,
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
    /// Flags this one displaces when both are given.
    pub overrides: &'a [&'a str],
    /// Flags that cannot be given alongside this one.
    ///
    /// Where [`overrides`](Self::overrides) resolves a collision by letting the last
    /// flag win, this reports it: the combination has no meaning, so honouring one
    /// side silently would hide a mistake.
    pub conflicts: &'a [&'a str],
    /// Flags that must also be given when this one is.
    ///
    /// The positive form of [`conflicts`](Self::conflicts), and the mirror image of
    /// [`required_if`](Self::required_if): the same rule written on the flag that
    /// imposes it rather than on the flag it lands on.
    pub requires: &'a [&'a str],
    /// Flags that make this one necessary.
    pub required_if: &'a [&'a str],
    /// Flags that make this one unnecessary.
    pub required_unless: &'a [&'a str],
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
        help: None,
        long_help: None,
        value_name: None,
        env: None,
        default: &[],
        choices: &[],
        required: false,
        value_optional: false,
        hide: false,
        count: false,
        repeatable: false,
        var_min: None,
        var_max: None,
        overrides: &[],
        conflicts: &[],
        requires: &[],
        required_if: &[],
        required_unless: &[],
        help_heading: None,
        effect: None,
    };
}

/// What a positional argument knows about itself beyond how it parses.
#[derive(Debug, Clone, Copy)]
pub struct ArgMeta<'a> {
    pub arg: &'a Arg<'a>,
    pub help: Option<&'a str>,
    pub long_help: Option<&'a str>,
    pub env: Option<&'a str>,
    pub default: &'a [&'a str],
    pub choices: &'a [&'a str],
    /// Whether the argument must be filled. The parser does not enforce this —
    /// it is checked once the last token has been read — but the spec has to say
    /// it, and help output has to show it.
    pub required: bool,
    pub hide: bool,
    pub var_min: Option<usize>,
    pub var_max: Option<usize>,
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
        help: None,
        long_help: None,
        env: None,
        default: &[],
        choices: &[],
        required: true,
        hide: false,
        var_min: None,
        var_max: None,
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
        let _ = self.write_kdl(&mut out);
        out
    }

    fn write_kdl(&self, out: &mut String) -> core::fmt::Result {
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
        write_body(
            out,
            self.root,
            0,
            UnknownFlags::Value,
            self.bin.unwrap_or(self.name),
        )
    }
}

/// Write a command's contents: its flags, arguments, and subcommands.
///
/// Separate from [`write_command`] because the root's contents sit at the top
/// level of the document rather than inside a `cmd` node.
fn write_body(
    out: &mut String,
    meta: &CommandMeta<'_>,
    depth: usize,
    inherited_unknown_flags: UnknownFlags,
    bin: &str,
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
        write_command(out, sub, depth, enclosing_unknown_flags, bin)?;
    }
    Ok(())
}

/// Built-in completion types declared by this command, written in the spec's vocabulary.
fn write_completion_types(
    out: &mut String,
    meta: &CommandMeta<'_>,
    depth: usize,
) -> core::fmt::Result {
    for arg in meta.args {
        if let Some(type_) = arg.complete_type {
            indent(out, depth)?;
            writeln!(
                out,
                "complete {} type={}",
                quoted(&arg.arg.name.to_ascii_lowercase()),
                quoted(type_)
            )?;
        }
    }
    for flag in meta.flags {
        if let Some(type_) = flag.complete_type {
            let name = flag
                .value_name
                .unwrap_or(flag.flag.name)
                .to_ascii_lowercase();
            indent(out, depth)?;
            writeln!(out, "complete {} type={}", quoted(&name), quoted(type_))?;
        }
    }
    Ok(())
}

fn write_command(
    out: &mut String,
    meta: &CommandMeta<'_>,
    depth: usize,
    inherited_unknown_flags: UnknownFlags,
    bin: &str,
) -> core::fmt::Result {
    indent(out, depth)?;
    write!(out, "cmd {}", quoted(meta.cmd.name))?;
    if let Some(help) = meta.about {
        write!(out, " help={}", quoted(help))?;
    }
    if meta.hide {
        out.push_str(" hide=#true");
    }
    if let Some(effect) = meta.effect {
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
    write_body(out, meta, inner, effective_unknown_flags, bin)?;

    indent(out, depth)?;
    out.push_str("}\n");
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
                "{bin} __complete_word__ --candidates {name} --line '{{{{ words | join(sep=\" \") | replace(from=\"'\", to=\"'\\\"'\\\"'\") }}}}'"
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
    write!(out, "flag {}", quoted(&flag_forms(meta.flag)))?;

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
    write_single_list(out, "requires", meta.requires)?;
    write_single_list(out, "required_if", meta.required_if)?;
    write_single_list(out, "required_unless", meta.required_unless)?;

    let has_children = meta.long_help.is_some()
        || meta.flag.takes_value
        || !meta.choices.is_empty()
        || meta.default.len() > 1
        || meta.overrides.len() > 1
        || meta.conflicts.len() > 1
        || meta.requires.len() > 1
        || meta.required_if.len() > 1
        || meta.required_unless.len() > 1;
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
    write_many_defaults(out, meta.default, inner)?;
    write_many_list(out, "overrides", meta.overrides, inner)?;
    write_many_list(out, "conflicts", meta.conflicts, inner)?;
    write_many_list(out, "requires", meta.requires, inner)?;
    write_many_list(out, "required_if", meta.required_if, inner)?;
    write_many_list(out, "required_unless", meta.required_unless, inner)?;
    if meta.flag.takes_value {
        indent(out, inner)?;
        let name = meta.value_name.unwrap_or(meta.flag.name);
        write!(
            out,
            "arg {}",
            quoted(&placeholder(name, meta.flag.variadic, meta.value_optional))
        )?;
        // Square brackets alone would round-trip as required, since usage-lib reads the
        // brackets *and* the attribute: `[BUMP]` without it comes back `required=#true`.
        if meta.value_optional {
            out.push_str(" required=#false");
        }
        if meta.choices.is_empty() {
            out.push('\n');
        } else {
            out.push_str(" {\n");
            write_choices(out, meta.choices, inner + 1)?;
            indent(out, inner)?;
            out.push_str("}\n");
        }
    } else if !meta.choices.is_empty() {
        write_choices(out, meta.choices, inner)?;
    }
    indent(out, depth)?;
    out.push_str("}\n");
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
    if let Some(min) = meta.var_min {
        write!(out, " var_min={min}")?;
    }
    if let Some(max) = meta.var_max {
        write!(out, " var_max={max}")?;
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
    write_single_default(out, meta.default)?;

    let has_children =
        meta.long_help.is_some() || !meta.choices.is_empty() || meta.default.len() > 1;
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
    write_many_defaults(out, meta.default, inner)?;
    write_choices(out, meta.choices, inner)?;
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

fn write_choices(out: &mut String, choices: &[&str], depth: usize) -> core::fmt::Result {
    if choices.is_empty() {
        return Ok(());
    }
    indent(out, depth)?;
    out.push_str("choices");
    for choice in choices {
        write!(out, " {}", quoted(choice))?;
    }
    out.push('\n');
    Ok(())
}

/// The `-s --long` form a spec uses to declare a flag.
fn flag_forms(flag: &Flag<'_>) -> String {
    let mut forms = String::new();
    for short in flag.shorts {
        if !forms.is_empty() {
            forms.push(' ');
        }
        // A short is one byte, and a non-ASCII one could not have been matched
        // against a token in the first place.
        forms.push('-');
        forms.push(*short as char);
    }
    for long in flag.longs {
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

/// A positional's placeholder: angle brackets when required, square when not.
fn arg_placeholder(name: &str, meta: &ArgMeta<'_>) -> String {
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

/// Quote a value as a KDL string.
///
/// Always quoted, even where KDL would accept a bare identifier: deciding when a
/// string is bare-safe means encoding KDL's identifier rules, and getting that
/// subtly wrong produces a spec that parses as something else. Quoting always is
/// less pretty and cannot be wrong.
fn quoted(value: &str) -> String {
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
    fn apply(partial: &mut Self::Partial, event: &crate::Event<'_, '_>) -> bool;

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

    /// Build the struct from what was collected.
    ///
    /// Fallible because a command can require a subcommand of its own, and "none was
    /// given" is only knowable here — at the point where the value has to exist.
    fn build<'t, 'v>(partial: Self::Partial) -> Result<Self, crate::Error<'t, 'v>>;
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

    /// Take one event, and say whether it belonged to the selected command.
    ///
    /// `selected` is a position in [`Subcommands::COMMANDS`], or `None` before any of them
    /// has been reached — in which case the event cannot be theirs and nothing is asked.
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
        event: &crate::Event<'_, '_>,
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

    /// Check the selected command's requirements, and nothing else's.
    ///
    /// A flag that `install` requires says nothing about an invocation that ran
    /// `run`, so only the command that was actually reached is judged.
    ///
    /// Identified by its position in [`Subcommands::COMMANDS`] rather than by its
    /// key: the position is found from the table's own address, so two commands whose
    /// keys happen to collide still cannot be confused for one another.
    fn check<'t, 'v>(
        partial: &mut Self::Partial,
        selected: usize,
    ) -> Result<(), crate::Error<'t, 'v>>;

    /// Build the variant at `selected`, a position in [`Subcommands::COMMANDS`].
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
    fn quoting_escapes_what_would_break_a_document() {
        assert_eq!(quoted("plain"), r#""plain""#);
        assert_eq!(quoted(r#"say "hi""#), r#""say \"hi\"""#);
        assert_eq!(quoted("a\\b"), r#""a\\b""#);
        assert_eq!(quoted("one\ntwo"), r#""one\ntwo""#);
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
        write_body(&mut out, &ROOT_META, 0, UnknownFlags::Value, "ex").unwrap();

        // Counted rather than checked with `contains`, which is how a duplicated
        // write survived review: `unknown_flags="value" unknown_flags="value"` contains
        // the string it was checked for. A KDL node carrying the same property twice
        // keeps only the last, so once is the whole point.
        let line = |name: &str| -> String {
            out.lines()
                .map(str::trim)
                .find(|l| l.starts_with(&format!(r#"cmd "{name}""#)))
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
            exec.matches(r#"unknown_flags="value""#).count(),
            1,
            "a differing subcommand declares it exactly once: {exec}"
        );
    }

    #[test]
    fn flag_forms_lists_shorts_then_longs() {
        static F: Flag = Flag {
            longs: &["jobs", "workers"],
            shorts: b"jw",
            ..Flag::VALUE
        };
        assert_eq!(flag_forms(&F), "-j -w --jobs --workers");
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
