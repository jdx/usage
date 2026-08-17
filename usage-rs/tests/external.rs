use std::path::PathBuf;
use std::process::Command;

fn run_fixture(name: &str) {
    let manifest = PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("tests")
        .join("fixtures")
        .join(name)
        .join("Cargo.toml");
    let output = Command::new(env!("CARGO"))
        .args(["run", "--quiet", "--manifest-path"])
        .arg(&manifest)
        .env(
            "CARGO_TARGET_DIR",
            PathBuf::from(env!("CARGO_TARGET_TMPDIR")).join(name),
        )
        .output()
        .expect("cargo should run the external facade fixture");
    assert!(
        output.status.success(),
        "fixture {name} failed:\nstdout:\n{}\nstderr:\n{}",
        String::from_utf8_lossy(&output.stdout),
        String::from_utf8_lossy(&output.stderr)
    );
}

#[test]
fn documented_cargo_alias_is_the_only_dependency() {
    run_fixture("cargo-alias");
}

#[test]
fn direct_dependencies_win_in_a_mixed_configuration() {
    run_fixture("mixed-dependencies");
}
