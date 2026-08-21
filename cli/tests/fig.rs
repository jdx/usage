//! `usage generate fig`: which of several completion declarations reaches an argument.
//!
//! Fig has two ways to say what a value is — a `template` it answers itself, and a
//! `generator` that runs a command — and an argument carrying both leaves the reader of
//! the spec to guess which one wins. These cases pin the rule: the nearest declaration,
//! which is what the generator side already did.

use assert_cmd::Command;

/// Half of the cases below assert that something is *absent* from the output, and an
/// empty stdout satisfies every one of them — so the exit status is checked here rather
/// than leaving each case to pass for the wrong reason.
fn fig_of(spec: &str) -> String {
    let output = Command::new(assert_cmd::cargo::cargo_bin!("usage"))
        .args(["generate", "fig", "--spec", spec, "--out-file", "-"])
        .output()
        .unwrap();
    assert!(
        output.status.success(),
        "usage generate fig failed: {}",
        String::from_utf8_lossy(&output.stderr)
    );
    let fig = String::from_utf8(output.stdout).unwrap();
    assert!(fig.contains("completionSpec"), "not a Fig spec: {fig}");
    fig
}

#[test]
fn a_typed_completer_becomes_the_template_fig_has_a_name_for() {
    // A `complete` inside a `cmd` block used to reach nothing at all: only the root
    // spec's completers were applied, so an argument declaring a path hint was emitted
    // with no template unless its *name* happened to contain "file", "dir" or "path".
    let fig = fig_of(
        r#"
name "ex"
bin "ex"
cmd "d" help="d" {
    arg "<old>" help="old"
    complete "old" type="path"
}
        "#,
    );
    assert!(fig.contains(r#""template": "filepaths""#), "{fig}");
    assert!(!fig.contains("generators"), "{fig}");
}

#[test]
fn a_directory_hint_is_folders_and_an_unmapped_kind_is_nothing() {
    let dirs = fig_of(
        r#"
name "ex"
bin "ex"
cmd "d" help="d" {
    arg "<where>" help="where"
    complete "where" type="dir"
}
        "#,
    );
    assert!(dirs.contains(r#""template": "folders""#), "{dirs}");

    // `none` says the shell should offer nothing. Approximating it with the wrong
    // template would be worse than leaving it out, and it has no command to run — which
    // used to produce a generator whose script was the empty string.
    let none = fig_of(
        r#"
name "ex"
bin "ex"
cmd "d" help="d" {
    arg "<opaque>" help="opaque"
    complete "opaque" type="none"
}
        "#,
    );
    assert!(!none.contains("template"), "{none}");
    assert!(!none.contains("generators"), "{none}");
}

#[test]
fn the_nearest_declaration_wins_and_never_both() {
    // `d` declares its own `run=` completer and the root declares a typed one for the
    // same argument name. The command's own is nearer, so it stands alone — and `e`,
    // which declares nothing, still gets the root's.
    let fig = fig_of(
        r#"
name "ex"
bin "ex"
complete "old" type="path"
cmd "d" help="d" {
    arg "<old>" help="old"
    complete "old" run="echo a"
}
cmd "e" help="e" {
    arg "<old>" help="old"
}
        "#,
    );
    let d = &fig[fig.find(r#""name": "d""#).unwrap()..fig.find(r#""name": "e""#).unwrap()];
    assert!(d.contains("generators"), "{d}");
    assert!(!d.contains("template"), "{d}");

    let e = &fig[fig.find(r#""name": "e""#).unwrap()..];
    assert!(e.contains(r#""template": "filepaths""#), "{e}");
    assert!(!e.contains("generators"), "{e}");
}

#[test]
fn a_name_inferred_template_is_still_a_guess_a_declaration_can_replace() {
    // `get_template` reads the argument's name: anything containing "file" gets paths.
    // That guess predates `type=`, and a spec that says what its value is should win.
    let fig = fig_of(
        r#"
name "ex"
bin "ex"
cmd "d" help="d" {
    arg "<config_file>" help="config"
    complete "config_file" type="dir"
}
        "#,
    );
    assert!(fig.contains(r#""template": "folders""#), "{fig}");
    assert!(!fig.contains(r#""template": "filepaths""#), "{fig}");
}

#[test]
fn a_declaration_that_offers_nothing_is_honoured_not_ignored() {
    // `none` means offer nothing. The argument's name says "file", which the guess reads
    // as paths — and a declaration, including one declaring there is nothing to offer,
    // replaces a guess.
    let fig = fig_of(
        r#"
name "ex"
bin "ex"
complete "secret_file" type="path"
cmd "d" help="d" {
    arg "<secret_file>" help="secret"
    complete "secret_file" type="none"
}
        "#,
    );
    assert!(!fig.contains("template"), "{fig}");
    assert!(!fig.contains("generators"), "{fig}");
}

#[test]
fn a_declaration_replaces_a_guessed_generator_too() {
    // `get_generator` reads the name as well: anything containing "env_var" gets the
    // environment-variable generator. Treating that guess as a prior declaration let it
    // outrank a real one.
    let fig = fig_of(
        r#"
name "ex"
bin "ex"
cmd "d" help="d" {
    arg "<env_var>" help="var"
    complete "env_var" type="path"
}
        "#,
    );
    assert!(fig.contains(r#""template": "filepaths""#), "{fig}");
    assert!(!fig.contains("generators"), "{fig}");
}

#[test]
fn a_command_to_run_leaves_no_guessed_template_beside_it() {
    // The reverse of the case above: a `run=` declaration on an argument whose name
    // infers a template used to emit both.
    let fig = fig_of(
        r#"
name "ex"
bin "ex"
cmd "d" help="d" {
    arg "<config_file>" help="config"
    complete "config_file" run="echo a"
}
        "#,
    );
    assert!(fig.contains("generators"), "{fig}");
    assert!(!fig.contains("template"), "{fig}");
}

#[test]
fn diff_offers_paths_for_both_of_its_specs() {
    // The command this file was written for: two arguments that are always spec files.
    let fig = fig_of(
        r#"
name "ex"
bin "ex"
cmd "diff" help="diff" {
    arg "<old>" help="the old spec"
    arg "<new>" help="the new spec"
    complete "old" type="path"
    complete "new" type="path"
}
        "#,
    );
    assert_eq!(
        fig.matches(r#""template": "filepaths""#).count(),
        2,
        "{fig}"
    );
    assert!(!fig.contains("generators"), "{fig}");
}
