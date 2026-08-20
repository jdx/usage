//! The four ways an argument can relate to `--`, declared rather than patched in.
//!
//! The parser has understood all four since it was written; the derive could say only one of
//! them, so a CLI wanting the others had to reach into the generated spec afterwards. mise does
//! exactly that today — `src/cli/root_grammar.rs` sets `automatic` on the root's task by hand,
//! because clap has nowhere to put it and the derive had nowhere either.
//!
//! `optional` and `required` are covered where they were built — the usage line, the completion
//! walk, `only_one_argument_can_live_after_the_separator`. What is new here is that `preserve`
//! and `automatic` can be *written*, so this file is about those two and about the one rule that
//! distinguishes them from `required`.

use std::ffi::OsStr;

use usage::parse::ParseValue;
use usage::Spec as LibSpec;
use usage_derive::Cli;

/// A wrapper: the task's own flags are the task's
///
/// mise's shape, and the reason `automatic` exists. `mise run build --watch` gives `--watch` to
/// the task; `mise run --dry-run build` is still mise's flag, because nothing had filled the
/// argument yet.
#[derive(Cli)]
#[usage(bin = "ex")]
struct Wrapper {
    /// Do not actually run anything
    #[usage(long)]
    dry_run: bool,
    /// The task to run
    ///
    /// It carries the mode too, and that is not redundant: the mode is per-argument, and it is
    /// *filling this one* that ends mise's half of the line. mise's `root_grammar.rs` sets it on
    /// both for the same reason.
    #[usage(arg, name = "TASK", double_dash = "automatic")]
    task: Option<String>,
    /// Task and arguments to run
    #[usage(arg, name = "ARGS", double_dash = "automatic")]
    args: Vec<String>,
}

/// A tool that hands its argument the separator too
#[derive(Cli)]
#[usage(bin = "keep")]
struct Keeper {
    #[usage(long)]
    verbose: bool,
    /// Everything, `--` included
    #[usage(arg, name = "REST", double_dash = "preserve")]
    rest: Vec<String>,
}

/// A task runner with ordinary task args before `--` and passthrough args after it.
#[derive(Cli)]
#[usage(bin = "split")]
struct Split {
    #[usage(arg, name = "TASK", double_dash = "automatic")]
    task: String,
    #[usage(arg, name = "ARGS")]
    args: Vec<String>,
    #[usage(arg, name = "TRAILING", double_dash = "required")]
    trailing: Vec<String>,
}

#[test]
fn a_mode_survives_the_round_trip_to_kdl() {
    // The spec is the interface, so the mode has to be *written* as well as obeyed — a derive
    // that parsed correctly and emitted nothing would leave `usage complete-word` and every
    // other spec reader with the default.
    let spec: LibSpec = Wrapper::to_kdl().parse().expect("valid spec");
    let args = &spec.cmd.args;
    assert_eq!(
        args[1].double_dash,
        usage::spec::arg::SpecDoubleDashChoices::Automatic
    );
    assert_eq!(
        args[0].double_dash,
        usage::spec::arg::SpecDoubleDashChoices::Automatic
    );

    let spec: LibSpec = Keeper::to_kdl().parse().expect("valid spec");
    assert_eq!(
        spec.cmd.args[0].double_dash,
        usage::spec::arg::SpecDoubleDashChoices::Preserve
    );

    // And only where declared: a flag's own value argument is untouched by any of this.
    assert!(spec.cmd.flags.iter().all(|f| f
        .arg
        .as_ref()
        .is_none_or(|a| a.double_dash == usage::spec::arg::SpecDoubleDashChoices::Optional)));
}

#[test]
fn automatic_stops_flags_where_the_argument_starts_filling() {
    // Before: mise's flag, read by mise.
    let argv = [OsStr::new("--dry-run"), OsStr::new("build")];
    let parsed = Wrapper::parse_from(&argv).expect("should parse");
    assert!(parsed.dry_run);
    assert_eq!(parsed.task.as_deref(), Some("build"));
    assert!(parsed.args.is_empty());

    // After: the task's, handed over untouched — and *not* an unknown-flag error, which is the
    // whole point of the mode.
    let argv = [
        OsStr::new("build"),
        OsStr::new("--dry-run"),
        OsStr::new("-x"),
    ];
    let parsed = Wrapper::parse_from(&argv).expect("should parse");
    assert!(
        !parsed.dry_run,
        "a flag after the argument belongs to whatever the argument names"
    );
    assert_eq!(parsed.task.as_deref(), Some("build"));
    assert_eq!(parsed.args, ["--dry-run", "-x"]);
}

#[test]
fn preserve_gives_the_separator_to_the_argument() {
    // `required` consumes the `--`; `preserve` keeps it, because the argument's holder wants the
    // command line as it was typed. The two are opposite ends of the same question, and the
    // derive could previously ask neither.
    let argv = [
        OsStr::new("--verbose"),
        OsStr::new("a"),
        OsStr::new("--"),
        OsStr::new("b"),
    ];
    let parsed = Keeper::parse_from(&argv).expect("should parse");
    assert!(parsed.verbose);
    assert_eq!(parsed.rest, ["a", "--", "b"]);
}

#[test]
fn an_explicit_separator_still_ends_an_automatic_argument() {
    let argv = [
        OsStr::new("task"),
        OsStr::new("--"),
        OsStr::new("mise"),
        OsStr::new("x"),
        OsStr::new("--"),
        OsStr::new("command"),
    ];
    let parsed = Split::parse_from(&argv).expect("the separator unlocks the trailing argument");
    assert_eq!(parsed.task, "task");
    assert!(parsed.args.is_empty());
    assert_eq!(parsed.trailing, ["mise", "x", "--", "command"]);

    // The emitted spec is also consumed by usage-lib for completion and shell integrations.
    // Keep its interpretation identical to the typed parser: the first explicit separator is
    // syntax even though `automatic` has already stopped flags, while the second is data.
    let spec: LibSpec = Split::to_kdl().parse().expect("valid spec");
    let argv = ["split", "task", "--", "mise", "x", "--", "command"].map(str::to_string);
    let interpreted = usage::Parser::new(&spec)
        .parse(&argv)
        .expect("the emitted spec accepts the same command line");
    let args = interpreted
        .args
        .iter()
        .find(|(arg, _)| arg.name == "ARGS")
        .map(|(_, value)| value);
    assert!(args
        .is_none_or(|value| matches!(value, ParseValue::MultiString(values) if values.is_empty())));
    let trailing = interpreted
        .args
        .iter()
        .find(|(arg, _)| arg.name == "TRAILING")
        .map(|(_, value)| value)
        .expect("TRAILING is present");
    assert!(
        matches!(trailing, ParseValue::MultiString(values) if values == &["mise", "x", "--", "command"])
    );
}

#[test]
fn only_required_stops_a_variadic() {
    // The rule the ordering check turns on, and the one that is easy to get wrong: an argument
    // after an unbounded variadic is reachable only if something ends the variadic, and only
    // `required` does. `automatic` stops *flags*, which is a different thing, and `preserve`
    // changes what a `--` means without ending anything — so a declaration of either behind an
    // unbounded variadic is refused exactly as a plain one is.
    //
    // Checked here by parsing rather than by asserting on the compiler's message, which
    // `derive::model`'s own tests do: this is the behaviour the rule protects.
    let argv = [OsStr::new("build"), OsStr::new("one"), OsStr::new("two")];
    let parsed = Wrapper::parse_from(&argv).expect("should parse");
    assert_eq!(
        parsed.args,
        ["one", "two"],
        "the unbounded variadic takes the rest, `automatic` or not"
    );
}
