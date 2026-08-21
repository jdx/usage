//! What a flag *means*: how much the CLI should say, and whether to color it.
//!
//! Two declarations, both cold. A role changes nothing about where a token
//! lands — `-v` binds the same `u8` and `--no-color` the same `bool` whether or
//! not its meaning is declared — so nothing here is reachable from the hot
//! binding path, and [`crate::Flag`] does not grow a field.
//!
//! What the declaration buys is that the meaning stops being folklore. Help,
//! docs, an agent reading the emitted spec and the CLI's own logger all read one
//! statement instead of inferring from a spelling, and usage can finally honour
//! a CLI's own `--no-color` when it renders that CLI's help page.
//!
//! These types mirror `usage_lib::spec::policy`, the same way [`crate::spec::Effect`]
//! mirrors `SpecCommandEffect`: this crate has no dependencies, and that is worth
//! one small duplication that `conformance` holds to agreement.

use crate::spec::{FlagMeta, Spec};
use crate::{Command, Event, Flag, Parser};
use ::core::ffi::c_void;
use ::std::ffi::OsStr;

/// How much a CLI should say.
///
/// Ordered least to most, so where two flags pin a level the more restrictive
/// one wins.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, Default)]
pub enum Verbosity {
    /// Say nothing at all.
    Silent,
    /// Only what went wrong.
    Error,
    /// What went wrong, and what might.
    Warn,
    /// The default: what a user asked for and nothing else.
    #[default]
    Info,
    /// Enough to follow what the program decided.
    Debug,
    /// Everything, including what only a maintainer wants.
    Trace,
}

impl Verbosity {
    /// Where a CLI sits when nothing on the command line says otherwise.
    pub const BASELINE: Verbosity = Verbosity::Info;

    /// Least to most, which is the order steps move along.
    pub const SCALE: &'static [Verbosity] = &[
        Verbosity::Silent,
        Verbosity::Error,
        Verbosity::Warn,
        Verbosity::Info,
        Verbosity::Debug,
        Verbosity::Trace,
    ];

    /// The word for this level, as a spec spells it.
    ///
    /// This is the spelling the fleet uses and the one `verbosity=` takes, so it is what
    /// help, documentation and an emitted spec say. It is *not* what a logger reads —
    /// see [`Verbosity::log_filter`], which differs for exactly one level.
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::Silent => "silent",
            Self::Error => "error",
            Self::Warn => "warn",
            Self::Info => "info",
            Self::Debug => "debug",
            Self::Trace => "trace",
        }
    }

    /// The word `log`, `tracing` and `env_logger` all read as a filter.
    ///
    /// This is the whole integration with them: a CLI writes
    /// `.filter_level(level.log_filter().parse()?)` and usage needs no dependency on any
    /// of them.
    ///
    /// Identical to [`Verbosity::as_str`] for five of the six. The bottom of the scale is
    /// the exception, and the reason this is a second method rather than one: the fleet
    /// spells silence `silent` — mise's and hk's `--silent`, aube's `--loglevel silent` —
    /// while every logging crate spells it `off`, and `silent` is not a level to any of
    /// them. `env_logger` would read it as the name of a module to filter on, so a CLI
    /// handing it `as_str()` would answer `--silent` by logging *more*, not less.
    pub const fn log_filter(self) -> &'static str {
        match self {
            Self::Silent => "off",
            other => other.as_str(),
        }
    }

    /// The level a word names, or `None` if it names none.
    ///
    /// `warning` and `off`/`none` are accepted because mise and the fleet's
    /// config files already spell them that way. Case is ignored, since an
    /// environment variable carrying a level is as likely to shout.
    pub fn parse(word: &str) -> Option<Verbosity> {
        Some(match () {
            _ if word.eq_ignore_ascii_case("silent")
                || word.eq_ignore_ascii_case("off")
                || word.eq_ignore_ascii_case("none") =>
            {
                Self::Silent
            }
            _ if word.eq_ignore_ascii_case("error") => Self::Error,
            _ if word.eq_ignore_ascii_case("warn") || word.eq_ignore_ascii_case("warning") => {
                Self::Warn
            }
            _ if word.eq_ignore_ascii_case("info") => Self::Info,
            _ if word.eq_ignore_ascii_case("debug") => Self::Debug,
            _ if word.eq_ignore_ascii_case("trace") => Self::Trace,
            _ => return None,
        })
    }

    /// Move `by` steps along the scale, saturating at both ends.
    pub fn step(self, by: i32) -> Verbosity {
        let here = Self::SCALE.iter().position(|l| *l == self).unwrap_or(0) as i32;
        let there = (here + by).clamp(0, Self::SCALE.len() as i32 - 1);
        Self::SCALE[there as usize]
    }
}

/// Whether output is colored.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Default)]
pub enum ColorChoice {
    /// Decide from the destination and the environment.
    #[default]
    Auto,
    /// Color, whatever the destination.
    Always,
    /// No color, whatever the destination.
    Never,
}

impl ColorChoice {
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::Auto => "auto",
            Self::Always => "always",
            Self::Never => "never",
        }
    }

    /// The choice a word names, or `None` if it names none.
    pub fn parse(word: &str) -> Option<ColorChoice> {
        Some(match () {
            _ if word.eq_ignore_ascii_case("auto") => Self::Auto,
            _ if word.eq_ignore_ascii_case("always") => Self::Always,
            _ if word.eq_ignore_ascii_case("never") => Self::Never,
            _ => return None,
        })
    }

    /// Combine two choices. A refusal beats a request, which is the convention
    /// `NO_COLOR` sets: saying "no color" once should be enough.
    pub const fn combine(self, other: ColorChoice) -> ColorChoice {
        match (self, other) {
            (Self::Never, _) | (_, Self::Never) => Self::Never,
            (Self::Always, _) | (_, Self::Always) => Self::Always,
            _ => Self::Auto,
        }
    }

    /// Whether to color output going to a destination that is, or is not, a terminal.
    ///
    /// `Auto` consults the environment first: `NO_COLOR` refuses, `CLICOLOR_FORCE`
    /// insists, and otherwise a terminal gets color and a pipe does not. An explicit
    /// choice skips all of that, which is the whole point of typing one.
    pub fn enabled_for(self, is_terminal: bool) -> bool {
        match self {
            Self::Always => true,
            Self::Never => false,
            Self::Auto => {
                let forced = ::std::env::var_os("CLICOLOR_FORCE").is_some_and(|v| v != "0");
                let refused = ::std::env::var_os("NO_COLOR").is_some_and(|v| !v.is_empty());
                !refused && (forced || is_terminal)
            }
        }
    }
}

/// What a flag means for verbosity. The spec's `verbosity=`.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum VerbosityRole {
    /// Each occurrence raises the level one step.
    Verbose,
    /// Each occurrence lowers the level one step.
    Quiet,
    /// The flag's value names the level.
    Level,
    /// This switch pins the level.
    Pin(Verbosity),
}

impl VerbosityRole {
    /// The spelling used in a spec.
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::Verbose => "verbose",
            Self::Quiet => "quiet",
            Self::Level => "level",
            Self::Pin(level) => level.as_str(),
        }
    }

    /// The level this role pins, for the roles that name one.
    pub const fn pinned(self) -> Option<Verbosity> {
        match self {
            Self::Pin(level) => Some(level),
            _ => None,
        }
    }

    /// How far one occurrence moves the level, for the roles that move it.
    pub const fn step(self) -> Option<i32> {
        match self {
            Self::Verbose => Some(1),
            Self::Quiet => Some(-1),
            _ => None,
        }
    }

    /// Whether the role reads the flag's own value.
    pub const fn takes_value(self) -> bool {
        matches!(self, Self::Level)
    }
}

/// What a flag means for color. The spec's `color=`.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum ColorRole {
    /// This switch forces color; its negation forbids it.
    Always,
    /// This switch forbids color; its negation forces it.
    Never,
    /// The flag's value is `auto`, `always` or `never`.
    Choice,
}

impl ColorRole {
    /// The spelling used in a spec.
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::Always => "always",
            Self::Never => "never",
            Self::Choice => "choice",
        }
    }

    /// What supplying this flag asks for, in its positive and negated spellings.
    pub const fn asks_for(self, negated: bool) -> Option<ColorChoice> {
        match (self, negated) {
            (Self::Always, false) | (Self::Never, true) => Some(ColorChoice::Always),
            (Self::Never, false) | (Self::Always, true) => Some(ColorChoice::Never),
            (Self::Choice, _) => None,
        }
    }

    /// Whether the role reads the flag's own value.
    pub const fn takes_value(self) -> bool {
        matches!(self, Self::Choice)
    }
}

/// One flag's contribution to the level, as the bound struct holds it.
///
/// `count` is how many times the flag was given — one for a switch, the count
/// for a counted flag, zero for a flag that was not given at all — and `value`
/// is the word a [`VerbosityRole::Level`] flag carried.
#[derive(Debug, Clone, Copy)]
pub struct VerbosityInput<'a> {
    pub role: VerbosityRole,
    pub count: usize,
    pub value: Option<&'a str>,
}

/// One flag's contribution to the color choice.
#[derive(Debug, Clone, Copy)]
pub struct ColorInput<'a> {
    pub role: ColorRole,
    /// Whether the flag arrived through its negated spelling.
    pub negated: bool,
    /// Whether the flag was given at all. A switch that was not given says nothing.
    pub given: bool,
    /// The word a [`ColorRole::Choice`] flag carried.
    pub value: Option<&'a str>,
}

/// Resolve a level from what a command line supplied.
///
/// An explicit level value pins; otherwise the most restrictive pinning switch
/// wins; otherwise the baseline stands. Then the stepping roles apply,
/// saturating at both ends.
///
/// This never sees argv order, and does not need to: `overrides` has usually
/// settled the question already — mise and hk declare their verbosity flags as
/// a mutual override lattice, so at most one of them survives the parse — and
/// where it has not, an order-independent rule is the only kind whose answer a
/// reader can predict.
pub fn resolve_verbosity<'a>(supplied: impl IntoIterator<Item = VerbosityInput<'a>>) -> Verbosity {
    resolve_verbosity_from(Verbosity::BASELINE, supplied)
}

/// The same, from a baseline the caller chose.
///
/// A flattened group resolves first and hands its answer along as the base, so a
/// parent's own `-v` moves whatever the group settled on.
pub fn resolve_verbosity_from<'a>(
    base: Verbosity,
    supplied: impl IntoIterator<Item = VerbosityInput<'a>>,
) -> Verbosity {
    let mut from_value: Option<Verbosity> = None;
    let mut pinned: Option<Verbosity> = None;
    let mut steps = 0i32;
    for input in supplied {
        if input.count == 0 {
            continue;
        }
        if input.role.takes_value() {
            if let Some(level) = input.value.and_then(Verbosity::parse) {
                from_value = Some(match from_value {
                    Some(held) if held < level => held,
                    _ => level,
                });
            }
            continue;
        }
        if let Some(level) = input.role.pinned() {
            pinned = Some(match pinned {
                Some(held) if held < level => held,
                _ => level,
            });
        }
        if let Some(step) = input.role.step() {
            steps += step * input.count as i32;
        }
    }
    let base = from_value.or(pinned).unwrap_or(base);
    base.step(steps)
}

/// Resolve a color choice from what a command line supplied.
pub fn resolve_color<'a>(supplied: impl IntoIterator<Item = ColorInput<'a>>) -> ColorChoice {
    let mut choice = ColorChoice::Auto;
    for input in supplied {
        if !input.given {
            continue;
        }
        let asked = match input.role {
            ColorRole::Choice => input.value.and_then(ColorChoice::parse),
            role => role.asks_for(input.negated),
        };
        if let Some(asked) = asked {
            choice = choice.combine(asked);
        }
    }
    choice
}

/// The level a CLI was asked for, for a type whose declaration says which flags mean it.
///
/// A trait rather than an inherent method, deliberately: a CLI adopting this may
/// well already have its own `fn verbosity`, and an inherent method would win
/// over ours and change what its own code does. Here the CLI keeps its method
/// and reaches this one as `VerbosityPolicy::verbosity(&cli)`.
pub trait VerbosityPolicy {
    /// The level this command line asked for, from `base` when it asked for nothing.
    fn verbosity_from(&self, base: Verbosity) -> Verbosity;

    /// The level this command line asked for.
    fn verbosity(&self) -> Verbosity {
        self.verbosity_from(Verbosity::BASELINE)
    }
}

/// The color choice a CLI was asked for. See [`VerbosityPolicy`] on why it is a trait.
pub trait ColorPolicy {
    /// The color choice this command line asked for.
    fn color(&self) -> ColorChoice;
}

/// The color choice a command line asks for, found without binding a struct.
///
/// Help and errors are rendered on paths where the struct was never built — a
/// `--help` request and a parse failure both come back as an `Err` — so the
/// answer has to come from argv. It comes from a real parse rather than a scan
/// for the word: `mycli --message --no-color` gives `--message` its value, and
/// a `--no-color` after `--` is somebody's argument.
///
/// `None` means the command line said nothing about color, which is not the
/// same as asking for `Auto`: a caller can tell "no opinion" from "decide from
/// the terminal" and fall back accordingly.
///
/// Parsing stops at the first thing that does not parse, since after that the
/// tokens no longer mean what they appear to. A color flag before the mistake
/// is honoured; one after it is not, and the environment decides instead. That
/// is the conservative direction: the cost is a help page painted the way it
/// would have been painted anyway.
pub fn color_from_argv(spec: &Spec<'_>, argv: &[&OsStr]) -> Option<ColorChoice> {
    let root = spec.root;
    // The commands argv descended through, outermost first. A flag in scope was
    // declared by one of them.
    let mut scope: ::std::vec::Vec<&crate::spec::CommandMeta<'_>> = ::std::vec![root];
    // What each flag ended up saying, in the order the flags were first seen. Kept per
    // flag rather than as one running answer, because that is the shape the bound struct
    // has and the two must not disagree: a second occurrence of *one* flag replaces its
    // value, which is `args_override_self`, while two *different* flags both hold theirs
    // and are combined below, where a refusal beats a request.
    let mut said: ::std::vec::Vec<(&Flag<'_>, ColorChoice)> = ::std::vec::Vec::new();
    let mut parser = Parser::new(root.cmd, argv);
    while let Some(Ok(event)) = parser.next_event() {
        match event {
            Event::Command(cmd) => {
                if let Some(meta) = child_meta(scope.last().copied(), cmd) {
                    scope.push(meta);
                }
            }
            Event::Flag {
                flag,
                value,
                negated,
            } => {
                let Some(role) = role_of(&scope, flag) else {
                    continue;
                };
                let asked = match role {
                    ColorRole::Choice => value
                        .and_then(|bytes| ::core::str::from_utf8(bytes).ok())
                        .and_then(ColorChoice::parse),
                    role => role.asks_for(negated),
                };
                if let Some(asked) = asked {
                    match said.iter_mut().find(|(seen, _)| same_flag(seen, flag)) {
                        Some((_, held)) => *held = asked,
                        None => said.push((flag, asked)),
                    }
                }
            }
            _ => {}
        }
    }
    said.into_iter()
        .map(|(_, asked)| asked)
        .reduce(ColorChoice::combine)
}

/// The color role declared for `flag`, looked for only among the commands on the
/// route argv took. A flag in scope was declared by one of them — a global by an
/// ancestor, anything else by the command that owns it.
fn role_of(scope: &[&crate::spec::CommandMeta<'_>], flag: &Flag<'_>) -> Option<ColorRole> {
    scope.iter().rev().find_map(|meta| {
        meta.flags
            .iter()
            .find(|candidate: &&FlagMeta<'_>| same_flag(candidate.flag, flag))
            .and_then(|meta| meta.color)
    })
}

/// Metadata for a child the parser descended into.
fn child_meta<'a>(
    parent: Option<&'a crate::spec::CommandMeta<'a>>,
    cmd: &Command<'_>,
) -> Option<&'a crate::spec::CommandMeta<'a>> {
    let parent = parent?;
    let index = parent
        .cmd
        .subcommands
        .iter()
        .position(|candidate| same_command(candidate, cmd))?;
    parent.subcommands.get(index).copied()
}

/// Whether two references name the same table entry.
///
/// Metadata borrows the parse-table entry it describes, so identity is the
/// address rather than anything compared field by field — two flags may share
/// every field and still be different flags.
fn same_flag(a: &Flag<'_>, b: &Flag<'_>) -> bool {
    ::core::ptr::eq(
        a as *const _ as *const c_void,
        b as *const _ as *const c_void,
    )
}

fn same_command(a: &Command<'_>, b: &Command<'_>) -> bool {
    ::core::ptr::eq(
        a as *const _ as *const c_void,
        b as *const _ as *const c_void,
    )
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_scale_runs_least_to_most() {
        assert!(Verbosity::Silent < Verbosity::Error);
        assert!(Verbosity::Info < Verbosity::Debug);
        assert!(Verbosity::Debug < Verbosity::Trace);
        assert_eq!(Verbosity::default(), Verbosity::Info);
    }

    #[test]
    fn a_word_names_a_level() {
        assert_eq!(Verbosity::parse("warning"), Some(Verbosity::Warn));
        assert_eq!(Verbosity::parse("off"), Some(Verbosity::Silent));
        assert_eq!(Verbosity::parse("DEBUG"), Some(Verbosity::Debug));
        assert_eq!(Verbosity::parse("fatal"), None);
    }

    #[test]
    fn the_bottom_of_the_scale_has_two_spellings() {
        // The fleet says `silent`; every logging crate says `off`. Both are this level,
        // and which word comes out depends on who is being told.
        assert_eq!(Verbosity::Silent.as_str(), "silent");
        assert_eq!(Verbosity::Silent.log_filter(), "off");
        assert_eq!(Verbosity::parse("off"), Some(Verbosity::Silent));
        // Everything else is the same word to both.
        for level in Verbosity::SCALE.iter().skip(1) {
            assert_eq!(level.as_str(), level.log_filter());
        }
    }

    #[test]
    fn steps_saturate_at_both_ends() {
        assert_eq!(Verbosity::Info.step(1), Verbosity::Debug);
        assert_eq!(Verbosity::Trace.step(4), Verbosity::Trace);
        assert_eq!(Verbosity::Silent.step(-4), Verbosity::Silent);
    }

    fn verbose(count: usize) -> VerbosityInput<'static> {
        VerbosityInput {
            role: VerbosityRole::Verbose,
            count,
            value: None,
        }
    }

    fn pin(level: Verbosity, given: bool) -> VerbosityInput<'static> {
        VerbosityInput {
            role: VerbosityRole::Pin(level),
            count: usize::from(given),
            value: None,
        }
    }

    #[test]
    fn nothing_supplied_is_the_baseline() {
        assert_eq!(resolve_verbosity([]), Verbosity::Info);
        assert_eq!(resolve_color([]), ColorChoice::Auto);
    }

    #[test]
    fn a_count_steps_once_per_occurrence() {
        assert_eq!(resolve_verbosity([verbose(0)]), Verbosity::Info);
        assert_eq!(resolve_verbosity([verbose(1)]), Verbosity::Debug);
        assert_eq!(resolve_verbosity([verbose(3)]), Verbosity::Trace);
    }

    #[test]
    fn the_most_restrictive_switch_wins_whatever_the_order() {
        let forward = [pin(Verbosity::Trace, true), pin(Verbosity::Silent, true)];
        let backward = [pin(Verbosity::Silent, true), pin(Verbosity::Trace, true)];
        assert_eq!(resolve_verbosity(forward), Verbosity::Silent);
        assert_eq!(resolve_verbosity(backward), Verbosity::Silent);
    }

    #[test]
    fn an_explicit_value_pins_over_a_switch() {
        let supplied = [
            pin(Verbosity::Debug, true),
            VerbosityInput {
                role: VerbosityRole::Level,
                count: 1,
                value: Some("trace"),
            },
        ];
        assert_eq!(resolve_verbosity(supplied), Verbosity::Trace);
    }

    #[test]
    fn steps_apply_to_whatever_was_pinned() {
        let supplied = [pin(Verbosity::Error, true), verbose(2)];
        assert_eq!(resolve_verbosity(supplied), Verbosity::Info);
    }

    fn color(role: ColorRole, negated: bool) -> ColorInput<'static> {
        ColorInput {
            role,
            negated,
            given: true,
            value: None,
        }
    }

    #[test]
    fn a_refusal_of_color_beats_a_request() {
        let both = [
            color(ColorRole::Always, false),
            color(ColorRole::Never, false),
        ];
        assert_eq!(resolve_color(both), ColorChoice::Never);
    }

    #[test]
    fn a_negated_color_switch_means_the_other_answer() {
        assert_eq!(
            resolve_color([color(ColorRole::Always, true)]),
            ColorChoice::Never
        );
        assert_eq!(
            resolve_color([color(ColorRole::Never, true)]),
            ColorChoice::Always
        );
    }

    #[test]
    fn a_flag_that_was_not_given_says_nothing() {
        let absent = [ColorInput {
            role: ColorRole::Never,
            negated: false,
            given: false,
            value: None,
        }];
        assert_eq!(resolve_color(absent), ColorChoice::Auto);
    }

    #[test]
    fn an_explicit_choice_ignores_the_environment() {
        assert!(ColorChoice::Always.enabled_for(false));
        assert!(!ColorChoice::Never.enabled_for(true));
    }
}
