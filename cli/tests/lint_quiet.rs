use assert_cmd::Command;

fn lint(args: &[&str], spec: &str) -> std::process::Output {
    Command::new(assert_cmd::cargo::cargo_bin!("usage"))
        .arg("lint")
        .args(args)
        .arg("-")
        .write_stdin(spec)
        .output()
        .unwrap()
}

const CLEAN: &str = "name \"t\"\nbin \"t\"\nabout \"x\"\nflag \"--a\" help=\"a\"\n";
const INFO: &str = "name \"t\"\nflag \"--a\"\n";
const ERROR: &str = "name \"t\"\nflag \"--a\" help=\"a\"\nflag \"--a\" help=\"b\"\n";

fn stdout(out: &std::process::Output) -> String {
    String::from_utf8(out.stdout.clone()).unwrap()
}

#[test]
fn quiet_prints_nothing_for_a_clean_spec() {
    let loud = lint(&[], CLEAN);
    assert_eq!(stdout(&loud), "No issues found.\n");

    for flag in ["-q", "--quiet"] {
        let quiet = lint(&[flag], CLEAN);
        assert!(quiet.status.success());
        assert_eq!(stdout(&quiet), "");
    }
}

#[test]
fn quiet_keeps_findings_and_drops_the_summary() {
    let loud = stdout(&lint(&[], INFO));
    assert!(loud.contains("Found 0 error(s)"), "{loud}");

    let quiet = lint(&["-q"], INFO);
    assert!(quiet.status.success());
    let text = stdout(&quiet);
    assert!(text.contains("missing-flag-help"), "{text}");
    assert!(!text.contains("Found "), "{text}");
}

#[test]
fn quiet_preserves_exit_codes() {
    let errors = lint(&["-q"], ERROR);
    assert_eq!(errors.status.code(), Some(1));
    assert!(stdout(&errors).contains("duplicate-flag"));

    // A warning only fails under -W.
    let warning = "name \"t\"\narg \"[a]\" help=\"a\"\narg \"<b>\" help=\"b\"\n";
    assert!(lint(&["-q"], warning).status.success());
    let strict = lint(&["-q", "-W"], warning);
    assert_eq!(strict.status.code(), Some(1));
    assert!(stdout(&strict).contains("required-after-optional"));
}

#[test]
fn quiet_leaves_json_alone() {
    let out = lint(&["-q", "--format", "json"], CLEAN);
    assert_eq!(stdout(&out).trim(), "[]");
}
