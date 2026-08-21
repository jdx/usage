use assert_cmd::Command;
use predicates::str::contains;

fn usage_cmd() -> Command {
    Command::new(assert_cmd::cargo::cargo_bin!("usage"))
}

fn example_path(name: &str) -> String {
    format!("{}/../examples/{}", env!("CARGO_MANIFEST_DIR"), name)
}

/// Stdout, once the run is known to have succeeded.
///
/// Asserted here rather than at each call site: a failed run has empty stdout, and a test
/// comparing it reports a confusing string difference instead of the real failure.
fn stdout_of(cmd: &mut Command) -> String {
    let output = cmd.output().unwrap();
    assert!(output.status.success(), "{output:?}");
    String::from_utf8(output.stdout).unwrap()
}

/// The fixture, the whole environment, and whatever argv the test is about.
///
/// The environment goes through `--env` rather than through the child process: `Parser`
/// reads the real environment when it is given no map, so a test that set `MYCLI_COLOR` on
/// the child would still be at the mercy of whatever else the machine exports.
fn explain(argv: &[&str]) -> String {
    let mut cmd = usage_cmd();
    cmd.args(["explain", "-f", &example_path("explain.usage.kdl")]);
    cmd.args(["-e", "MYCLI_COLOR=never", "-e", "MYCLI_PROFILE=prod"]);
    cmd.arg("--");
    cmd.args(argv);
    stdout_of(&mut cmd)
}

#[test]
fn explains_the_worked_example() {
    insta::assert_snapshot!(explain(&[
        "mycli",
        "-j8",
        "--env=prod",
        "build",
        "a",
        "b",
        "--",
        "--raw"
    ]));
}

/// The example on the grammar page, so the page cannot drift from the tool.
///
/// `docs/spec/argv.md` claims this output; a documented example nothing checks is doc rot
/// with a delay on it.
#[test]
fn explains_the_documented_example() {
    insta::assert_snapshot!(explain(&[
        "mycli",
        "-j8",
        "--env=prod",
        "build",
        "a",
        "--",
        "--raw"
    ]));
}

#[test]
fn a_default_on_a_flags_argument_is_shadowed_like_any_other() {
    let mut cmd = usage_cmd();
    cmd.args([
        "explain",
        "-s",
        "name \"ex\"\nbin \"ex\"\nflag \"--jobs <n>\" {\n    arg \"<n>\" default=\"1\"\n}\n",
        "--",
        "ex",
        "--jobs",
        "8",
    ]);

    // A flag can declare its default on itself or on its argument, and the parser prefers
    // them in that order. Reading only the first called a shadowed default no default at all.
    cmd.assert()
        .success()
        .stdout(contains("--jobs  default 1  lost to argv [2]"));
}

#[test]
fn env_wants_a_key() {
    let mut cmd = usage_cmd();
    cmd.args(["explain", "-f", &example_path("explain.usage.kdl")]);
    cmd.args(["-e", "=never", "--", "mycli"]);

    // No variable can be named "", so accepting it would describe an environment nothing
    // could produce.
    cmd.assert().failure().stderr(contains("KEY=VALUE"));
}

#[test]
fn env_wants_a_separator() {
    let mut cmd = usage_cmd();
    cmd.args(["explain", "-f", &example_path("explain.usage.kdl")]);
    cmd.args(["-e", "MYCLI_COLOR", "--", "mycli"]);

    // The other half of the same contract as `env_wants_a_key`: a word with no `=` names no
    // value, and guessing one would put a variable in the report the caller never set.
    cmd.assert().failure().stderr(contains("KEY=VALUE"));
}

#[test]
fn binds_an_attached_long_flag() {
    // jdx/mise discussion #8883: a hand-written scanner ignored `--env=production` while
    // `--env production` worked. Both forms bind here, and the report says which token did.
    let attached = explain(&["mycli", "--env=production", "build", "a"]);
    let detached = explain(&["mycli", "--env", "production", "build", "a"]);

    assert!(
        attached.contains("value of env = \"production\", attached"),
        "{attached}"
    );
    assert!(
        detached.contains("value of env = \"production\""),
        "{detached}"
    );
    for report in [&attached, &detached] {
        assert!(report.contains("--env"), "{report}");
        assert!(report.contains("production"), "{report}");
    }
}

#[test]
fn the_separator_is_optional_before_the_explained_line() {
    let mut cmd = usage_cmd();
    cmd.args(["explain", "-f", &example_path("explain.usage.kdl")]);
    cmd.args(["-e", "MYCLI_COLOR=never", "-e", "MYCLI_PROFILE=prod"]);
    // No `--`: `double_dash="automatic"` ends this command's own flag parsing at the
    // program name, so a foreign `--env=prod` is data rather than a flag `usage` rejects.
    cmd.args(["mycli", "-j8", "--env=prod", "build", "a"]);
    let without = stdout_of(&mut cmd);

    assert_eq!(
        without,
        explain(&["mycli", "-j8", "--env=prod", "build", "a"])
    );
}

#[test]
fn a_line_of_its_own_needs_the_separator_to_keep_a_double_dash() {
    let mut cmd = usage_cmd();
    cmd.args(["explain", "-f", &example_path("explain.usage.kdl")]);
    cmd.args(["mycli", "build", "a", "--", "--raw"]);
    let without = stdout_of(&mut cmd);

    // `usage`'s own parse takes the first `--` as its separator — `automatic` ends flag
    // parsing but does not stop a later separator being honoured, which is what
    // a78564c0 settled. So an explained line carrying its own `--` needs the leading one,
    // and the report shows the difference rather than hiding it.
    let with = explain(&["mycli", "build", "a", "--", "--raw"]);
    assert!(without.starts_with("mycli build a --raw\n"), "{without}");
    assert!(with.starts_with("mycli build a -- --raw\n"), "{with}");
    assert!(with.contains("--     separator"), "{with}");
}

#[test]
fn a_second_separator_stays_data() {
    let report = explain(&["mycli", "build", "a", "--", "--raw", "--", "more"]);

    // The first `--` separates; every later one is data, which is what every parser worth
    // comparing against does and what jdx/usage#229 was about.
    assert!(report.contains("[3]  --     separator"), "{report}");
    assert!(
        report.contains("[5]  --     arg extra = \"--\""),
        "{report}"
    );
}

#[test]
fn reports_a_command_line_that_does_not_parse_and_exits_zero() {
    let mut cmd = usage_cmd();
    cmd.args(["explain", "-f", &example_path("explain.usage.kdl")]);
    cmd.args(["--", "mycli", "--env=prod", "build"]);

    // Exit 0: the report succeeded, and this is the case a report is wanted for. Anything
    // else would make the tool useless under `set -e`.
    cmd.assert()
        .success()
        .stdout(contains("value of env = \"prod\", attached"))
        .stdout(contains("errors"))
        .stdout(contains("target"));
}

#[test]
fn reports_a_parse_that_could_not_continue() {
    let mut cmd = usage_cmd();
    cmd.args(["explain", "-f", &example_path("explain.usage.kdl")]);
    cmd.args(["--", "mycli", "-j"]);

    cmd.assert()
        .success()
        .stdout(contains(
            "the parse stopped before the environment and defaults",
        ))
        .stdout(contains("flag -j"))
        .stdout(contains("requires an argument"));
}

#[test]
fn json_carries_the_same_facts() {
    let mut cmd = usage_cmd();
    cmd.args(["explain", "-f", &example_path("explain.usage.kdl")]);
    cmd.args([
        "--format",
        "json",
        "--",
        "mycli",
        "--env=prod",
        "build",
        "a",
    ]);
    let stdout = stdout_of(&mut cmd);

    let json: serde_json::Value = serde_json::from_str(&stdout).unwrap();
    assert_eq!(json["command"], serde_json::json!(["mycli", "build"]));
    assert_eq!(json["tokens"][1]["roles"][1]["kind"], "value");
    assert_eq!(json["tokens"][1]["roles"][1]["attached"], true);
    assert_eq!(json["fallbacks_applied"], true);
}

#[test]
fn reads_a_spec_from_stdin() {
    let mut cmd = usage_cmd();
    cmd.args(["explain", "-f", "-", "--", "mycli", "--jobs", "8"]);
    cmd.write_stdin("name \"mycli\"\nbin \"mycli\"\nflag \"--jobs <n>\"\n");

    cmd.assert().success().stdout(contains("value of jobs"));
}

#[test]
fn reads_a_spec_from_an_argument() {
    let mut cmd = usage_cmd();
    cmd.args([
        "explain",
        "-s",
        "name \"mycli\"\nbin \"mycli\"\nflag \"--jobs <n>\" default=\"1\"\n",
        "--",
        "mycli",
    ]);

    cmd.assert()
        .success()
        .stdout(contains("--jobs  1  default"));
}

#[test]
fn a_bare_invocation_explains_the_fallbacks_alone() {
    let mut cmd = usage_cmd();
    cmd.args(["explain", "-f", &example_path("explain.usage.kdl")]);
    cmd.args(["-e", "MYCLI_COLOR=never"]);

    // No argv at all is a real question: which defaults and environment values fire when
    // nothing is typed.
    cmd.assert()
        .success()
        .stdout(contains("--color  never  env MYCLI_COLOR"))
        .stdout(contains("--jobs   1      default"));
}

#[test]
fn its_own_unknown_flags_are_still_refused() {
    let mut cmd = usage_cmd();
    cmd.args([
        "explain",
        "-f",
        &example_path("explain.usage.kdl"),
        "--nope",
    ]);

    // Deliberately unlike `exec`: a typo in `usage explain`'s own flags is a mistake, not
    // data, because the explained line starts at its program name.
    cmd.assert().failure();
}
