use std::path::PathBuf;
use std::process::{Command, Output};

fn fixture_output(name: &str, args: &[&str]) -> Output {
    let manifest = PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("tests")
        .join("fixtures")
        .join(name)
        .join("Cargo.toml");
    let output = Command::new(env!("CARGO"))
        .args(["run", "--quiet", "--manifest-path"])
        .arg(&manifest)
        .arg("--")
        .args(args)
        .env(
            "CARGO_TARGET_DIR",
            PathBuf::from(env!("CARGO_TARGET_TMPDIR")).join(name),
        )
        .output()
        .expect("cargo should run the external facade fixture");
    output
}

fn run_fixture(name: &str) {
    let output = fixture_output(name, &[]);
    assert!(
        output.status.success(),
        "fixture {name} failed:\nstdout:\n{}\nstderr:\n{}",
        String::from_utf8_lossy(&output.stdout),
        String::from_utf8_lossy(&output.stderr)
    );
}

#[test]
fn runtime_identity_drives_process_output() {
    let ordinary = fixture_output("runtime-identity", &[]);
    assert!(
        ordinary.status.success(),
        "{}",
        String::from_utf8_lossy(&ordinary.stderr)
    );

    let help = fixture_output("runtime-identity", &["--help"]);
    assert!(
        help.status.success(),
        "{}",
        String::from_utf8_lossy(&help.stderr)
    );
    let help = String::from_utf8_lossy(&help.stdout);
    assert!(help.contains("Usage: runtime-ex"), "{help}");

    let version = fixture_output("runtime-identity", &["--version"]);
    assert!(version.status.success());
    assert_eq!(
        String::from_utf8_lossy(&version.stdout),
        "runtime-ex 6.0.1+host\n"
    );

    let failure = fixture_output("runtime-identity", &["--unknown"]);
    assert_eq!(failure.status.code(), Some(2));
    let failure = String::from_utf8_lossy(&failure.stderr);
    assert!(failure.contains("Usage: runtime-ex"), "{failure}");
}

#[test]
fn documented_cargo_alias_is_the_only_dependency() {
    run_fixture("cargo-alias");
}

#[test]
fn direct_dependencies_win_in_a_mixed_configuration() {
    run_fixture("mixed-dependencies");
}

#[test]
fn workspace_inherited_facade_is_resolved() {
    run_fixture("workspace-inheritance");
}

/// The endpoint at process level, which is the half `spec_request` unit tests cannot reach.
///
/// This fixture is the right one to ask: it declares `unknown_flags = "error"` and takes no
/// arguments at all, so any ordinary word is a failure — which is how the control below shows
/// that the request is answered *before* the grammar sees it rather than by passing through it.
#[test]
fn a_spec_request_is_answered_before_the_parse() {
    let control = fixture_output("runtime-identity", &["ordinary-word"]);
    assert_eq!(
        control.status.code(),
        Some(2),
        "a word this CLI does not accept must fail, or the assertion below proves nothing"
    );

    let spec = fixture_output("runtime-identity", &["__usage_spec__"]);
    assert!(
        spec.status.success(),
        "{}",
        String::from_utf8_lossy(&spec.stderr)
    );
    let out = String::from_utf8_lossy(&spec.stdout);
    // The portable identity, not the runtime one: a tool asking a binary for its spec wants
    // the deterministic document, not what this process happens to be called.
    assert!(out.contains("name portable-ex"), "{out}");
    assert!(out.contains("version \"6.0.0\""), "{out}");
    assert!(!out.contains("runtime-ex"), "{out}");
}
