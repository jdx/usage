use serde::Serialize;
use strum::{Display as StrumDisplay, EnumString};

/// How much a CLI should say.
///
/// Every CLI in the fleet declares this and none of them can say so in a spec:
/// mise has six flags for it, hk three, aube spells quiet as a value of
/// `--loglevel`, fnox has a bare `-v`. What they have in common is not the
/// spelling but the scale, so the scale is what the spec records, and a flag
/// says which point on it — or which direction along it — it means.
///
/// Ordered least to most, so `Ord` gives the combining rule: where two flags
/// pin a level, the more restrictive one wins.
#[derive(
    Debug, Copy, Clone, PartialEq, Eq, PartialOrd, Ord, Hash, EnumString, StrumDisplay, Serialize,
)]
#[strum(serialize_all = "snake_case")]
#[serde(rename_all = "snake_case")]
pub enum Verbosity {
    /// Say nothing at all, not even errors the program can survive.
    Silent,
    /// Only what went wrong.
    Error,
    /// What went wrong, and what might.
    Warn,
    /// The default: what a user asked for and nothing else.
    Info,
    /// Enough to follow what the program decided.
    Debug,
    /// Everything, including what only a maintainer wants.
    Trace,
}

impl Default for Verbosity {
    fn default() -> Self {
        Self::BASELINE
    }
}

impl Verbosity {
    /// Where a CLI sits when nothing on the command line says otherwise.
    ///
    /// `info`, which is what `examples/config.usage.kdl` and hk's own
    /// `log_level` prop both already default to.
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
    /// Not what a logger reads: see [`Verbosity::log_filter`].
    pub fn as_str(&self) -> &'static str {
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
    /// The same as [`Verbosity::as_str`] for five of the six levels. The fleet spells
    /// silence `silent` and every logging crate spells it `off`; handing one of them
    /// `silent` gets it read as a module name rather than as a level.
    pub fn log_filter(&self) -> &'static str {
        match self {
            Self::Silent => "off",
            other => other.as_str(),
        }
    }

    /// The level a word names, or `None` if it names none.
    ///
    /// Two alias families are accepted, each because a fleet CLI spells it that
    /// way: `warning` (mise's `--log-level warning`) and `off`/`none` for
    /// `silent`. Matching ignores ASCII case, because an environment variable
    /// carrying a level is as likely to say `DEBUG` as `debug`. Nothing else is
    /// accepted — `fatal`, `notice` and `verbose` are guesses, and this table
    /// grows when a real CLI needs one.
    pub fn parse(word: &str) -> Option<Verbosity> {
        match word.to_ascii_lowercase().as_str() {
            "silent" | "off" | "none" => Some(Self::Silent),
            "error" => Some(Self::Error),
            "warn" | "warning" => Some(Self::Warn),
            "info" => Some(Self::Info),
            "debug" => Some(Self::Debug),
            "trace" => Some(Self::Trace),
            _ => None,
        }
    }

    /// Move `by` steps along the scale, saturating at both ends.
    ///
    /// `-vvv` on a CLI whose scale tops out is `trace`, not an error: a user
    /// asking for more than there is has still asked for the most there is.
    pub fn step(self, by: i32) -> Verbosity {
        let here = Self::SCALE.iter().position(|l| *l == self).unwrap_or(0) as i32;
        // Saturating, so a step count large enough to wrap lands at the end of the scale
        // rather than at the other end of it: `-v` repeated past `i32` is still "louder".
        let there = here
            .saturating_add(by)
            .clamp(0, Self::SCALE.len() as i32 - 1);
        Self::SCALE[there as usize]
    }
}

/// Whether output is colored.
///
/// `Auto` is the answer until something says otherwise, and it is resolved
/// against the terminal and the environment where the writing happens rather
/// than here — the same split `Style::auto` already makes.
#[derive(Debug, Copy, Clone, PartialEq, Eq, Hash, Default, EnumString, StrumDisplay, Serialize)]
#[strum(serialize_all = "snake_case")]
#[serde(rename_all = "snake_case")]
pub enum ColorChoice {
    /// Decide from the destination: a terminal gets color, a pipe does not.
    #[default]
    Auto,
    /// Color, whatever the destination.
    Always,
    /// No color, whatever the destination.
    Never,
}

impl ColorChoice {
    pub fn as_str(&self) -> &'static str {
        match self {
            Self::Auto => "auto",
            Self::Always => "always",
            Self::Never => "never",
        }
    }

    /// The choice a word names, or `None` if it names none.
    pub fn parse(word: &str) -> Option<ColorChoice> {
        match word.to_ascii_lowercase().as_str() {
            "auto" => Some(Self::Auto),
            "always" => Some(Self::Always),
            "never" => Some(Self::Never),
            _ => None,
        }
    }

    /// Combine two choices, most restrictive first.
    ///
    /// A refusal beats a request, which is the same convention `NO_COLOR` sets:
    /// somebody who has said "no color" once should not have to say it twice.
    pub fn combine(self, other: ColorChoice) -> ColorChoice {
        match (self, other) {
            (Self::Never, _) | (_, Self::Never) => Self::Never,
            (Self::Always, _) | (_, Self::Always) => Self::Always,
            _ => Self::Auto,
        }
    }
}

/// What a flag means for verbosity.
///
/// Declared with `verbosity=` on a flag the CLI already has. It never adds an
/// `overrides` or a `conflicts` and never changes how anything parses: it says
/// what the flag *means*, so that help, docs, an agent reading the spec, and
/// the CLI's own logger can all read one declaration instead of guessing from
/// a spelling.
#[derive(
    Debug, Copy, Clone, PartialEq, Eq, Hash, EnumString, StrumDisplay, Serialize, PartialOrd, Ord,
)]
#[strum(serialize_all = "snake_case")]
#[serde(rename_all = "snake_case")]
pub enum SpecVerbosityRole {
    /// Each occurrence raises the level one step. mise's and hk's `-v`.
    Verbose,
    /// Each occurrence lowers the level one step.
    Quiet,
    /// The flag's value names the level. mise's `--log-level`, aube's
    /// `--loglevel`, and the reason `silent` is a point on the scale rather
    /// than a separate concept: aube spells it `--loglevel silent`.
    Level,
    /// This switch pins the level to `silent`. mise's and hk's `--silent`.
    Silent,
    /// This switch pins the level to `error`. mise's `-q`, which suppresses
    /// everything that is not a failure — two steps down, but the flag means
    /// the level and not the distance.
    Error,
    /// This switch pins the level to `warn`.
    Warn,
    /// This switch pins the level to `info`.
    Info,
    /// This switch pins the level to `debug`. mise's `--debug`, aube's `-v`.
    Debug,
    /// This switch pins the level to `trace`. mise's `--trace`.
    Trace,
}

impl SpecVerbosityRole {
    pub fn as_str(&self) -> &'static str {
        match self {
            Self::Verbose => "verbose",
            Self::Quiet => "quiet",
            Self::Level => "level",
            Self::Silent => "silent",
            Self::Error => "error",
            Self::Warn => "warn",
            Self::Info => "info",
            Self::Debug => "debug",
            Self::Trace => "trace",
        }
    }

    /// The level this role pins, for the roles that name one.
    pub fn pinned(&self) -> Option<Verbosity> {
        match self {
            Self::Silent => Some(Verbosity::Silent),
            Self::Error => Some(Verbosity::Error),
            Self::Warn => Some(Verbosity::Warn),
            Self::Info => Some(Verbosity::Info),
            Self::Debug => Some(Verbosity::Debug),
            Self::Trace => Some(Verbosity::Trace),
            Self::Verbose | Self::Quiet | Self::Level => None,
        }
    }

    /// How far one occurrence moves the level, for the roles that move it.
    pub fn step(&self) -> Option<i32> {
        match self {
            Self::Verbose => Some(1),
            Self::Quiet => Some(-1),
            _ => None,
        }
    }

    /// Whether the role reads the flag's own value, and so requires one.
    pub fn takes_value(&self) -> bool {
        matches!(self, Self::Level)
    }
}

/// What a flag means for color.
///
/// Declared with `color=`. Two shapes are in the fleet and both are supported:
/// aube's independent `--color` / `--no-color` pair held apart by `conflicts`,
/// and a single negatable flag whose negation means the other answer.
#[derive(
    Debug, Copy, Clone, PartialEq, Eq, Hash, EnumString, StrumDisplay, Serialize, PartialOrd, Ord,
)]
#[strum(serialize_all = "snake_case")]
#[serde(rename_all = "snake_case")]
pub enum SpecColorRole {
    /// This switch forces color; its negation, if it has one, forbids it.
    Always,
    /// This switch forbids color; its negation, if it has one, forces it.
    Never,
    /// The flag's value is `auto`, `always` or `never`.
    Choice,
}

impl SpecColorRole {
    pub fn as_str(&self) -> &'static str {
        match self {
            Self::Always => "always",
            Self::Never => "never",
            Self::Choice => "choice",
        }
    }

    /// What supplying this flag asks for, in its positive and negated spellings.
    pub fn asks_for(&self, negated: bool) -> Option<ColorChoice> {
        match (self, negated) {
            (Self::Always, false) | (Self::Never, true) => Some(ColorChoice::Always),
            (Self::Never, false) | (Self::Always, true) => Some(ColorChoice::Never),
            (Self::Choice, _) => None,
        }
    }

    /// Whether the role reads the flag's own value, and so requires one.
    pub fn takes_value(&self) -> bool {
        matches!(self, Self::Choice)
    }
}

/// The set of values accepted by `verbosity=`, for error messages.
pub(crate) const VERBOSITY_VALUES: &str =
    "verbose, quiet, level, silent, error, warn, info, debug, trace";

/// The set of values accepted by `color=`, for error messages.
pub(crate) const COLOR_VALUES: &str = "always, never, choice";

/// Resolve a level from the roles a command line supplied.
///
/// `supplied` is every role that was given, paired with how many times it was
/// given and, for a [`SpecVerbosityRole::Level`] flag, the word it carried.
///
/// The rule, in order: an explicit level value pins; otherwise the most
/// restrictive pinning switch wins; otherwise the baseline stands. Then the
/// stepping roles apply, saturating.
///
/// Note that `overrides` has usually already settled this. mise and hk declare
/// their verbosity flags as a mutual override lattice, so at most one of them
/// survives the parse and every branch below but the first is unreachable for
/// them. The rules exist for a CLI that declared no lattice, where several
/// roles really can arrive together, and they are order-independent so that the
/// answer does not depend on which end of the command line a word was typed at.
pub fn resolve_verbosity<'a>(
    supplied: impl IntoIterator<Item = (SpecVerbosityRole, usize, Option<&'a str>)>,
) -> Verbosity {
    let mut from_value: Option<Verbosity> = None;
    let mut pinned: Option<Verbosity> = None;
    let mut steps = 0i64;
    for (role, count, value) in supplied {
        if count == 0 {
            continue;
        }
        if role.takes_value() {
            if let Some(level) = value.and_then(Verbosity::parse) {
                from_value = Some(from_value.map_or(level, |held| held.min(level)));
            }
            continue;
        }
        if let Some(level) = role.pinned() {
            pinned = Some(pinned.map_or(level, |held| held.min(level)));
        }
        if let Some(step) = role.step() {
            // In `i64` and saturating; see the note on the compiled side. A count is a
            // `usize`, and `as i32` on a large one changes its sign.
            let by = i64::try_from(count).unwrap_or(i64::MAX);
            steps = steps.saturating_add(by.saturating_mul(i64::from(step)));
        }
    }
    let base = from_value.or(pinned).unwrap_or(Verbosity::BASELINE);
    base.step(steps.clamp(i32::MIN as i64, i32::MAX as i64) as i32)
}

/// Resolve a color choice from the roles a command line supplied.
///
/// `supplied` pairs each role with the spelling it arrived in — negated or not
/// — and, for a [`SpecColorRole::Choice`] flag, the word it carried. A refusal
/// beats a request; nothing supplied is [`ColorChoice::Auto`].
pub fn resolve_color<'a>(
    supplied: impl IntoIterator<Item = (SpecColorRole, bool, Option<&'a str>)>,
) -> ColorChoice {
    let mut choice = ColorChoice::Auto;
    for (role, negated, value) in supplied {
        let asked = match role {
            SpecColorRole::Choice => value.and_then(ColorChoice::parse),
            _ => role.asks_for(negated),
        };
        if let Some(asked) = asked {
            choice = choice.combine(asked);
        }
    }
    choice
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_scale_runs_least_to_most() {
        assert!(Verbosity::Silent < Verbosity::Error);
        assert!(Verbosity::Error < Verbosity::Warn);
        assert!(Verbosity::Warn < Verbosity::Info);
        assert!(Verbosity::Info < Verbosity::Debug);
        assert!(Verbosity::Debug < Verbosity::Trace);
        assert_eq!(Verbosity::default(), Verbosity::Info);
    }

    #[test]
    fn a_word_names_a_level() {
        assert_eq!(Verbosity::parse("debug"), Some(Verbosity::Debug));
        // mise spells it `warning`, the scale spells it `warn`.
        assert_eq!(Verbosity::parse("warning"), Some(Verbosity::Warn));
        assert_eq!(Verbosity::parse("warn"), Some(Verbosity::Warn));
        // aube spells silence `silent`; `off` and `none` are the other two
        // spellings a logger config is likely to already carry.
        assert_eq!(Verbosity::parse("off"), Some(Verbosity::Silent));
        assert_eq!(Verbosity::parse("none"), Some(Verbosity::Silent));
        assert_eq!(Verbosity::parse("silent"), Some(Verbosity::Silent));
        // An environment variable is as likely to shout.
        assert_eq!(Verbosity::parse("DEBUG"), Some(Verbosity::Debug));
        // Not accepted, deliberately: they are guesses rather than fleet spellings.
        assert_eq!(Verbosity::parse("fatal"), None);
        assert_eq!(Verbosity::parse("verbose"), None);
        assert_eq!(Verbosity::parse(""), None);
    }

    #[test]
    fn the_bottom_of_the_scale_has_two_spellings() {
        assert_eq!(Verbosity::Silent.as_str(), "silent");
        assert_eq!(Verbosity::Silent.log_filter(), "off");
        for level in Verbosity::SCALE.iter().skip(1) {
            assert_eq!(level.as_str(), level.log_filter());
        }
    }

    #[test]
    fn steps_saturate_at_both_ends() {
        assert_eq!(Verbosity::Info.step(1), Verbosity::Debug);
        assert_eq!(Verbosity::Info.step(-1), Verbosity::Warn);
        assert_eq!(Verbosity::Info.step(0), Verbosity::Info);
        assert_eq!(Verbosity::Trace.step(9), Verbosity::Trace);
        assert_eq!(Verbosity::Silent.step(-9), Verbosity::Silent);
        assert_eq!(Verbosity::Silent.step(99), Verbosity::Trace);
    }

    #[test]
    fn roles_round_trip_through_their_words() {
        for (word, role) in [
            ("verbose", SpecVerbosityRole::Verbose),
            ("quiet", SpecVerbosityRole::Quiet),
            ("level", SpecVerbosityRole::Level),
            ("silent", SpecVerbosityRole::Silent),
            ("trace", SpecVerbosityRole::Trace),
        ] {
            assert_eq!(word.parse::<SpecVerbosityRole>().unwrap(), role);
            assert_eq!(role.as_str(), word);
            assert_eq!(role.to_string(), word);
        }
        for (word, role) in [
            ("always", SpecColorRole::Always),
            ("never", SpecColorRole::Never),
            ("choice", SpecColorRole::Choice),
        ] {
            assert_eq!(word.parse::<SpecColorRole>().unwrap(), role);
            assert_eq!(role.as_str(), word);
        }
        assert!("loud".parse::<SpecVerbosityRole>().is_err());
        assert!("auto".parse::<SpecColorRole>().is_err());
    }

    #[test]
    fn nothing_supplied_is_the_baseline() {
        assert_eq!(resolve_verbosity([]), Verbosity::Info);
        assert_eq!(resolve_color([]), ColorChoice::Auto);
    }

    #[test]
    fn a_count_steps_once_per_occurrence() {
        let three = [(SpecVerbosityRole::Verbose, 3, None)];
        assert_eq!(resolve_verbosity(three), Verbosity::Trace);
        let one = [(SpecVerbosityRole::Verbose, 1, None)];
        assert_eq!(resolve_verbosity(one), Verbosity::Debug);
        let down = [(SpecVerbosityRole::Quiet, 2, None)];
        assert_eq!(resolve_verbosity(down), Verbosity::Error);
        // Given zero times is not given.
        let none = [(SpecVerbosityRole::Verbose, 0, None)];
        assert_eq!(resolve_verbosity(none), Verbosity::Info);
    }

    #[test]
    fn a_pinning_switch_names_the_level_rather_than_a_distance() {
        // mise's `-q` is `error`, which is two steps down, but the flag says
        // which level it means and not how far to move.
        let quiet = [(SpecVerbosityRole::Error, 1, None)];
        assert_eq!(resolve_verbosity(quiet), Verbosity::Error);
        let silent = [(SpecVerbosityRole::Silent, 1, None)];
        assert_eq!(resolve_verbosity(silent), Verbosity::Silent);
    }

    #[test]
    fn an_explicit_value_pins_over_a_switch() {
        // aube: `--loglevel` beats `-v`, which is a shortcut for `debug`.
        let both = [
            (SpecVerbosityRole::Debug, 1, None),
            (SpecVerbosityRole::Level, 1, Some("trace")),
        ];
        assert_eq!(resolve_verbosity(both), Verbosity::Trace);
        // A word the scale does not know leaves the rest of the resolution alone.
        let unknown = [
            (SpecVerbosityRole::Debug, 1, None),
            (SpecVerbosityRole::Level, 1, Some("chatty")),
        ];
        assert_eq!(resolve_verbosity(unknown), Verbosity::Debug);
    }

    #[test]
    fn the_most_restrictive_switch_wins() {
        // Only reachable for a CLI that declared no `overrides` lattice. Order
        // must not matter, so both orderings are asserted.
        let forward = [
            (SpecVerbosityRole::Trace, 1, None),
            (SpecVerbosityRole::Silent, 1, None),
        ];
        let backward = [
            (SpecVerbosityRole::Silent, 1, None),
            (SpecVerbosityRole::Trace, 1, None),
        ];
        assert_eq!(resolve_verbosity(forward), Verbosity::Silent);
        assert_eq!(resolve_verbosity(backward), Verbosity::Silent);
    }

    #[test]
    fn steps_apply_to_whatever_was_pinned() {
        let pinned_then_raised = [
            (SpecVerbosityRole::Error, 1, None),
            (SpecVerbosityRole::Verbose, 2, None),
        ];
        assert_eq!(resolve_verbosity(pinned_then_raised), Verbosity::Info);
        let valued_then_lowered = [
            (SpecVerbosityRole::Level, 1, Some("trace")),
            (SpecVerbosityRole::Quiet, 1, None),
        ];
        assert_eq!(resolve_verbosity(valued_then_lowered), Verbosity::Debug);
    }

    #[test]
    fn a_refusal_of_color_beats_a_request() {
        let both = [
            (SpecColorRole::Always, false, None),
            (SpecColorRole::Never, false, None),
        ];
        assert_eq!(resolve_color(both), ColorChoice::Never);
        let one = [(SpecColorRole::Always, false, None)];
        assert_eq!(resolve_color(one), ColorChoice::Always);
    }

    #[test]
    fn a_negated_color_switch_means_the_other_answer() {
        let negated = [(SpecColorRole::Always, true, None)];
        assert_eq!(resolve_color(negated), ColorChoice::Never);
        let negated_never = [(SpecColorRole::Never, true, None)];
        assert_eq!(resolve_color(negated_never), ColorChoice::Always);
    }

    #[test]
    fn a_choice_flag_reads_its_value() {
        let never = [(SpecColorRole::Choice, false, Some("never"))];
        assert_eq!(resolve_color(never), ColorChoice::Never);
        let always = [(SpecColorRole::Choice, false, Some("always"))];
        assert_eq!(resolve_color(always), ColorChoice::Always);
        let auto = [(SpecColorRole::Choice, false, Some("auto"))];
        assert_eq!(resolve_color(auto), ColorChoice::Auto);
        let nonsense = [(SpecColorRole::Choice, false, Some("chartreuse"))];
        assert_eq!(resolve_color(nonsense), ColorChoice::Auto);
    }
}
