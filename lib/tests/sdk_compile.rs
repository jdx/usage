use std::fs;
use std::path::Path;
use std::process::Command;

use usage::sdk::{SdkLanguage, SdkOptions, SdkOutput};
use usage::Spec;

/// Comprehensive spec that exercises all SDK features:
/// version, about, author, global flags, choices, deprecated, aliases,
/// examples, double_dash, var args, negate flags, config props.
fn full_spec() -> Spec {
    r##"
        bin "mytool"
        name "mytool"
        version "1.2.3"
        about "A powerful CLI tool"
        author "Jane Doe"

        config {
            prop "debug" default=#true data_type=boolean help="Enable debug mode"
            prop "port" default=8080 data_type=integer help="Port number"
            prop "rate" default="1.5" data_type=float help="Rate limit"
            prop "host" data_type=string help="Host address"
        }

        flag "-v --verbose" help="Verbosity level" count=#true global=#true
        flag "-C --config <path>" help="Config file path" global=#true env="MYTOOL_CONFIG"
        flag "--dry-run" help="Show what would be done" negate="--no-dry-run"

        arg "input" help="Input file" required=#true
        arg "extra" var=#true help="Extra files"

        cmd "build" help="Build the project" deprecated="Use compile instead" {
            alias "b"
            arg "target" help="Build target" {
                choices "debug" "release"
            }
            arg "output" help="Output directory" double_dash="required"
            flag "-j --jobs <n>" help="Parallel jobs" var=#true
            flag "--release" help="Build in release mode"
            example "mytool build --release target" header="Build in release mode" lang="bash"
        }

        cmd "deploy" help="Deploy the project" {
            arg "env" help="Target environment" {
                choices "staging" "production"
            }
            arg "tags" var=#true help="Deployment tags" var_min=1 var_max=5
            flag "-f --force" help="Force deploy" deprecated="Use --confirm instead"
            flag "--confirm" help="Confirm deployment"
        }

        cmd "status" help="Show status" {
            flag "--json" help="Output as JSON"
        }
    "##
    .parse()
    .unwrap()
}

fn write_sdk_to_dir(output: &SdkOutput, dir: &Path) {
    for file in &output.files {
        let path = dir.join(&file.path);
        if let Some(parent) = path.parent() {
            fs::create_dir_all(parent).unwrap();
        }
        fs::write(&path, &file.content).unwrap();
    }
}

fn tool_exists(name: &str) -> bool {
    Command::new(name).arg("--version").output().is_ok()
}

// ---------------------------------------------------------------------------
// Rust
// ---------------------------------------------------------------------------

#[test]
fn test_rust_sdk_compiles() {
    if !tool_exists("cargo") {
        eprintln!("Skipping Rust SDK compile test - cargo not found");
        return;
    }

    let spec = full_spec();
    let output = usage::sdk::generate(
        &spec,
        &SdkOptions {
            language: SdkLanguage::Rust,
            package_name: None,
            source_file: None,
        },
    );

    let dir = tempfile::tempdir().unwrap();
    write_sdk_to_dir(&output, dir.path());

    let result = Command::new("cargo")
        .args(["check", "--manifest-path"])
        .arg(dir.path().join("Cargo.toml"))
        .output()
        .expect("Failed to run cargo check");

    if !result.status.success() {
        let stderr = String::from_utf8_lossy(&result.stderr);
        panic!("Generated Rust SDK does not compile:\n{stderr}");
    }
}

// ---------------------------------------------------------------------------
// Python
// ---------------------------------------------------------------------------

#[test]
fn test_python_sdk_imports() {
    if !tool_exists("python3") {
        eprintln!("Skipping Python SDK import test - python3 not found");
        return;
    }

    let spec = full_spec();
    let output = usage::sdk::generate(
        &spec,
        &SdkOptions {
            language: SdkLanguage::Python,
            package_name: None,
            source_file: None,
        },
    );

    let dir = tempfile::tempdir().unwrap();
    // Python files are flat in the package directory
    let pkg_dir = dir.path().join("mytool_sdk");
    fs::create_dir_all(&pkg_dir).unwrap();
    write_sdk_to_dir(&output, &pkg_dir);

    // Validate syntax + imports for each module
    for module in &["types", "client", "runtime"] {
        let result = Command::new("python3")
            .args([
                "-c",
                &format!(
                    "import sys; sys.path.insert(0, '{}'); from mytool_sdk.{module} import *",
                    dir.path().display()
                ),
            ])
            .output()
            .expect("Failed to run python3");

        if !result.status.success() {
            let stderr = String::from_utf8_lossy(&result.stderr);
            panic!("Generated Python SDK {module}.py has errors:\n{stderr}");
        }
    }
}

// ---------------------------------------------------------------------------
// TypeScript
// ---------------------------------------------------------------------------

#[test]
fn test_typescript_sdk_typechecks() {
    if !tool_exists("tsc") {
        eprintln!("Skipping TypeScript SDK typecheck test - tsc not found");
        return;
    }

    let spec = full_spec();
    let output = usage::sdk::generate(
        &spec,
        &SdkOptions {
            language: SdkLanguage::TypeScript,
            package_name: None,
            source_file: None,
        },
    );

    let dir = tempfile::tempdir().unwrap();
    write_sdk_to_dir(&output, dir.path());

    // Install @types/node for Node.js type declarations
    if !tool_exists("npm") {
        eprintln!("Skipping TypeScript SDK typecheck test - npm not found");
        return;
    }

    let npm_result = Command::new("npm")
        .args(["init", "-y", "--scope=sdk-test"])
        .current_dir(dir.path())
        .output()
        .expect("Failed to run npm init");
    if !npm_result.status.success() {
        let stderr = String::from_utf8_lossy(&npm_result.stderr);
        panic!("npm init failed:\n{stderr}");
    }

    let npm_result = Command::new("npm")
        .args(["install", "@types/node"])
        .current_dir(dir.path())
        .output()
        .expect("Failed to install @types/node");
    if !npm_result.status.success() {
        let stderr = String::from_utf8_lossy(&npm_result.stderr);
        panic!("npm install @types/node failed:\n{stderr}");
    }

    // Write tsconfig.json referencing @types/node
    let tsconfig = r#"{
        "compilerOptions": {
            "target": "ES2020",
            "module": "ES2020",
            "moduleResolution": "bundler",
            "strict": true,
            "noEmit": true,
            "skipLibCheck": true,
            "esModuleInterop": true,
            "types": ["node"]
        },
        "include": ["./*.ts"]
    }"#;
    fs::write(dir.path().join("tsconfig.json"), tsconfig).unwrap();

    let result = Command::new("tsc")
        .args(["--project"])
        .arg(dir.path().join("tsconfig.json"))
        .output()
        .expect("Failed to run tsc");

    if !result.status.success() {
        let stderr = String::from_utf8_lossy(&result.stderr);
        panic!("Generated TypeScript SDK does not typecheck:\n{stderr}");
    }
}
