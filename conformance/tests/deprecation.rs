//! What a parse says about the deprecated declarations it used, in both implementations.
//!
//! The compiled parser reads static tables and the reference implementation interprets a spec, so
//! nothing makes them agree except a test that asks them the same question. The rule they are both
//! held to is on <https://usage.jdx.dev/spec/argv>.
//!
//! Its own test binary, for the reason `post_binding_env.rs` gives: one case sets an environment
//! variable, and a variable is process-wide.

use std::ffi::OsStr;

use usage::warn::WarningKind as LibWarningKind;
use usage::Spec;
use usage_argv::warn::WarningKind;
use usage_derive::{Args, Cli, Subcommands};

/// The same CLI the spec below describes.
#[derive(Cli)]
#[usage(
    bin = "ex",
    version = "2.0.0",
    view("ex-compile", root = "compile"),
    view("ex-inner", root = "compile inner")
)]
struct Ex {
    /// Where to write the result
    #[usage(long, deprecated = "use --out", deprecated_remove_at = "3.0.0")]
    output: Option<String>,
    #[usage(long)]
    out: Option<String>,
    /// Deprecated in a release this CLI has not reached
    #[usage(long, deprecated = "use --out", deprecated_warn_at = "9.0.0")]
    outfile: Option<String>,
    #[usage(
        long,
        env = "EX_DEP_TOKEN",
        deprecated_env = "EX_DEP_OLD_TOKEN",
        deprecated = "use --out"
    )]
    token: Option<String>,
    /// A default fills this in, which is nobody's request
    #[usage(long, default = "quiet", deprecated = "use --out")]
    mode: Option<String>,
    #[usage(subcommand)]
    command: Option<ExCommand>,
}

#[derive(Subcommands)]
// The variants exist to be selected, not read: what this file checks is what a selection
// reports.
#[allow(dead_code)]
enum ExCommand {
    #[usage(deprecated = "use build")]
    Compile(ExCompile),
    Build(ExBuild),
}

#[derive(Args)]
struct ExCompile {
    #[usage(long, deprecated = "no longer read")]
    incremental: bool,
    #[usage(subcommand)]
    command: Option<ExCompileCommand>,
}

#[derive(Subcommands)]
#[allow(dead_code)]
enum ExCompileCommand {
    /// Deprecated, and promoted by a view two words deep
    #[usage(deprecated = "use build")]
    Inner(ExInner),
}

#[derive(Args)]
struct ExInner {
    #[usage(long, deprecated = "no longer read")]
    fast: bool,
}

#[derive(Args)]
struct ExBuild {
    #[usage(long)]
    release: bool,
}

const SPEC: &str = r#"
name "ex"
bin "ex"
version "2.0.0"
flag "--output <output>" help="Where to write the result" deprecated="use --out" deprecated_remove_at="3.0.0"
flag "--out <out>"
flag "--outfile <outfile>" deprecated="use --out" deprecated_warn_at="9.0.0"
flag "--token <token>" env="EX_DEP_TOKEN" deprecated="use --out" {
    deprecated_env "EX_DEP_OLD_TOKEN"
}
flag "--mode <mode>" default="quiet" deprecated="use --out"
cmd "compile" deprecated="use build" {
    flag "--incremental" deprecated="no longer read"
    cmd "inner" deprecated="use build" {
        flag "--fast" deprecated="no longer read"
    }
}
cmd "build" {
    flag "--release"
}
view "ex-compile" root="compile"
view "ex-inner" root="compile inner"
"#;

/// What the compiled parser reports, as kinds and names.
fn compiled(words: &[&str]) -> Vec<(WarningKind, String)> {
    let argv: Vec<&OsStr> = words.iter().map(OsStr::new).collect();
    let mut warnings = Vec::new();
    Ex::parse_from_with_warnings(&argv, &mut warnings).expect("a valid command line");
    warnings
        .iter()
        .map(|warning| (warning.kind, warning.name.to_string()))
        .collect()
}

/// What the reference implementation reports, in the same shape.
fn interpreted(words: &[&str]) -> Vec<(LibWarningKind, String)> {
    let spec: Spec = SPEC.parse().expect("the spec should parse");
    let mut input = vec!["ex".to_string()];
    input.extend(words.iter().map(|word| word.to_string()));
    let out = usage::parse(&spec, &input).expect("a valid command line");
    out.warnings
        .iter()
        .map(|warning| (warning.kind, warning.name.clone()))
        .collect()
}

/// The two vocabularies are separate types on purpose — one borrows static tables, the other owns
/// its strings — so agreement is checked by name.
fn same_kind(compiled: WarningKind, interpreted: LibWarningKind) -> bool {
    matches!(
        (compiled, interpreted),
        (WarningKind::DeprecatedFlag, LibWarningKind::DeprecatedFlag)
            | (
                WarningKind::DeprecatedCommand,
                LibWarningKind::DeprecatedCommand
            )
            | (WarningKind::DeprecatedEnv, LibWarningKind::DeprecatedEnv)
            | (WarningKind::Other, LibWarningKind::Other)
    )
}

/// Both implementations report the same warnings for the same command line.
///
/// Compared in order, which is stricter than the grammar promises — it specifies the set and
/// leaves the arrangement to the implementation. Every case here is one the two happen to agree
/// on, and holding them to it catches a change in either; a case where they diverge in order
/// would be compared as a set instead, not treated as a failure.
#[track_caller]
fn both_report(words: &[&str], expected: &[(WarningKind, &str)]) {
    let compiled = compiled(words);
    let interpreted = interpreted(words);
    let expected: Vec<(WarningKind, String)> = expected
        .iter()
        .map(|(kind, name)| (*kind, (*name).to_string()))
        .collect();
    assert_eq!(compiled, expected, "compiled parser, for {words:?}");
    assert_eq!(
        interpreted.len(),
        expected.len(),
        "reference implementation, for {words:?}: {interpreted:?}"
    );
    for (found, want) in interpreted.iter().zip(&expected) {
        assert!(
            same_kind(want.0, found.0) && found.1 == want.1,
            "reference implementation, for {words:?}: {found:?} is not {want:?}"
        );
    }
}

#[test]
fn what_a_command_line_used_is_reported_by_both_implementations() {
    // One test, in one process. A variable is process-wide, so the alias case below would race
    // every other case in this binary that parses a CLI reading `EX_DEP_TOKEN` — which is all of
    // them. `post_binding_env.rs` has the same shape for the same reason.

    // A deprecated flag that was typed.
    both_report(
        &["--output", "a.txt"],
        &[(WarningKind::DeprecatedFlag, "--output")],
    );

    // Its replacement says nothing.
    both_report(&["--out", "a.txt"], &[]);

    // `--mode` is deprecated and has a default, so a bare command line fills it without anybody
    // having asked for it. A default is not a use.
    both_report(&[], &[]);

    // A milestone this CLI's version has not reached is an author saying *not yet*.
    both_report(&["--outfile", "a.txt"], &[]);

    // A deprecated command reports itself, and then whatever of its own was used.
    both_report(
        &["compile", "--incremental"],
        &[
            (WarningKind::DeprecatedCommand, "compile"),
            (WarningKind::DeprecatedFlag, "--incremental"),
        ],
    );
    both_report(&["build", "--release"], &[]);

    // And both word it the same way.
    let mut warnings = Vec::new();
    let argv = [OsStr::new("--output"), OsStr::new("a.txt")];
    Ex::parse_from_with_warnings(&argv, &mut warnings).expect("a valid command line");
    let spec: Spec = SPEC.parse().expect("the spec should parse");
    let out = usage::parse(
        &spec,
        &["ex".to_string(), "--output".into(), "a.txt".into()],
    )
    .expect("a valid command line");
    assert_eq!(
        usage_argv::warn::render_warnings(&warnings),
        usage::warn::render(&out.warnings),
    );
    assert_eq!(
        usage_argv::warn::render_warnings(&warnings),
        "warning: --output is deprecated, removed at 3.0.0: use --out\n",
    );

    // Invoked through an executable view, the promoted command is not a selection anybody made
    // — `ex-compile` *is* `ex compile` — so it is not reported, while a deprecated flag the user
    // typed on it still is. Both implementations agree, from opposite directions: the compiled
    // parser injects the view's words into argv and skips them, and the reference implementation
    // parses a spec whose root already *is* the promoted command.
    let mut warnings = Vec::new();
    let argv = [OsStr::new("ex-compile"), OsStr::new("--incremental")];
    Ex::parse_from_argv_with_warnings(&argv, &mut warnings).expect("the view should dispatch");
    assert_eq!(
        warnings
            .iter()
            .map(|w| (w.kind, w.name))
            .collect::<Vec<_>>(),
        [(WarningKind::DeprecatedFlag, "--incremental")],
        "{warnings:?}",
    );

    let promoted = spec
        .for_view("ex-compile")
        .expect("the view should materialize");
    let out = usage::parse(
        &promoted,
        &["ex-compile".to_string(), "--incremental".to_string()],
    )
    .expect("the view should parse");
    assert_eq!(out.warnings.len(), 1, "{:?}", out.warnings);
    assert_eq!(out.warnings[0].kind, LibWarningKind::DeprecatedFlag);
    assert_eq!(out.warnings[0].name, "--incremental");

    // A view two commands deep: neither the command it routes through nor the promoted one is
    // reported, which is the branch that decrements rather than the one that stops.
    let mut warnings = Vec::new();
    let argv = [OsStr::new("ex-inner"), OsStr::new("--fast")];
    Ex::parse_from_argv_with_warnings(&argv, &mut warnings).expect("the view should dispatch");
    assert_eq!(
        warnings
            .iter()
            .map(|w| (w.kind, w.name))
            .collect::<Vec<_>>(),
        [(WarningKind::DeprecatedFlag, "--fast")],
        "{warnings:?}",
    );

    let promoted = spec
        .for_view("ex-inner")
        .expect("the nested view should materialize");
    let out = usage::parse(&promoted, &["ex-inner".to_string(), "--fast".to_string()])
        .expect("the view should parse");
    assert_eq!(out.warnings.len(), 1, "{:?}", out.warnings);
    assert_eq!(out.warnings[0].name, "--fast");

    // Reached as ordinary subcommands, the same commands *are* selections, and say so.
    both_report(
        &["compile", "inner", "--fast"],
        &[
            (WarningKind::DeprecatedCommand, "compile"),
            (WarningKind::DeprecatedCommand, "inner"),
            (WarningKind::DeprecatedFlag, "--fast"),
        ],
    );

    // Reached as an ordinary subcommand, the same command *is* a selection, and says so.
    both_report(
        &["compile", "--incremental"],
        &[
            (WarningKind::DeprecatedCommand, "compile"),
            (WarningKind::DeprecatedFlag, "--incremental"),
        ],
    );

    // A value that arrived through a deprecated alias, with the name to use instead. Last,
    // because it is the case that has to touch the environment.
    unsafe { std::env::set_var("EX_DEP_OLD_TOKEN", "secret") };

    // `--token` is deprecated *and* reached through a deprecated alias, so both are reported —
    // the flag first, then the variable, on both sides.
    let mut warnings = Vec::new();
    let empty: Vec<&OsStr> = Vec::new();
    let ex = Ex::parse_from_with_warnings(&empty, &mut warnings).expect("the alias still supplies");
    assert_eq!(ex.token.as_deref(), Some("secret"));
    assert_eq!(warnings.len(), 2, "{warnings:?}");
    assert_eq!(warnings[0].kind, WarningKind::DeprecatedFlag);
    assert_eq!(warnings[0].name, "--token");
    assert_eq!(warnings[1].kind, WarningKind::DeprecatedEnv);
    assert_eq!(warnings[1].name, "EX_DEP_OLD_TOKEN");
    assert_eq!(warnings[1].replacement, Some("EX_DEP_TOKEN"));

    let out = usage::parse(&spec, &["ex".to_string()]).expect("the alias still supplies");
    assert_eq!(out.warnings.len(), 2, "{:?}", out.warnings);
    assert_eq!(out.warnings[0].kind, LibWarningKind::DeprecatedFlag);
    assert_eq!(out.warnings[0].name, "--token");
    assert_eq!(out.warnings[1].kind, LibWarningKind::DeprecatedEnv);
    assert_eq!(out.warnings[1].name, "EX_DEP_OLD_TOKEN");
    assert_eq!(out.warnings[1].replacement.as_deref(), Some("EX_DEP_TOKEN"));

    // A current name is not a deprecated one — but supplying a deprecated flag through it is
    // still using the flag, so that much is still reported.
    unsafe { std::env::remove_var("EX_DEP_OLD_TOKEN") };
    unsafe { std::env::set_var("EX_DEP_TOKEN", "secret") };
    both_report(&[], &[(WarningKind::DeprecatedFlag, "--token")]);
    unsafe { std::env::remove_var("EX_DEP_TOKEN") };
}
