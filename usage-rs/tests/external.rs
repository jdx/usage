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

/// A reader that has gone away is not a crash. `print!` panics when its write fails, so a
/// generated `parse()` that answered `--help | head -1`, or a completion the shell cancelled,
/// used to panic — and abort with a core dump in a `panic = "abort"` build.
///
/// The read end is dropped before the child starts, so the first write always fails: the
/// ordering is fixed rather than raced.
#[test]
fn a_closed_reader_is_not_a_crash() {
    let built = fixture_output("runtime-identity", &[]);
    assert!(
        built.status.success(),
        "{}",
        String::from_utf8_lossy(&built.stderr)
    );
    let bin = PathBuf::from(env!("CARGO_TARGET_TMPDIR"))
        .join("runtime-identity")
        .join("debug")
        .join(format!(
            "usage-rs-runtime-identity-fixture{}",
            std::env::consts::EXE_SUFFIX
        ));

    let closed_pipe = || {
        let (reader, writer) = std::io::pipe().expect("a pipe");
        drop(reader);
        writer
    };
    // stdout: answers the user asked for keep their success status.
    for args in [&["--help"][..], &["--version"], &["__usage_spec__"]] {
        let output = Command::new(&bin)
            .args(args)
            .stdout(closed_pipe())
            .stderr(std::process::Stdio::piped())
            .output()
            .expect("the fixture should run");
        let stderr = String::from_utf8_lossy(&output.stderr);
        assert_eq!(output.status.code(), Some(0), "{args:?}: {stderr}");
        assert!(!stderr.contains("panicked"), "{args:?}: {stderr}");
    }
    // stderr: a failure keeps its failure status.
    let output = Command::new(&bin)
        .arg("--unknown")
        .stdout(std::process::Stdio::piped())
        .stderr(closed_pipe())
        .output()
        .expect("the fixture should run");
    assert_eq!(output.status.code(), Some(2));
}
