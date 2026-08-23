use std::fs;
use std::path::Path;
use std::process::Command;

use usage::sdk::{SdkLanguage, SdkOptions, SdkOutput};
use usage::Spec;

/// Comprehensive spec that exercises all SDK features:
/// version, about, author, global flags, choices, deprecated, aliases,
/// examples, double_dash, var args, negate flags, config props,
/// repeatable boolean flags, numeric choices.
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
        flag "--debug" var=#true help="Repeatable boolean flag"

        arg "input" help="Input file" required=#true
        arg "[extra]" var=#true help="Extra files"

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

        cmd "log" help="Show log level" {
            arg "level" help="Log level" {
                choices "1" "2" "3" "4" "5"
            }
            flag "--json" help="Output as JSON"
        }

        cmd "status" help="Show status" {
            flag "--json" help="Output as JSON"
            output "text" default=#true
            output "json" framing="json" select="--json" help="One status object"
        }

        cmd "check" help="Check the project" {
            flag "--format <FMT>" help="Output format"
            output "human" default=#true help="A human-readable report"
            output "json" framing="json" help="One report object" {
                schema "{\n  \"type\": \"object\"\n}"
            }
            output "jsonl" framing="jsonl" help="One event per line"
            select "--format"
            exit_code 0 "all checks passed"
            exit_code 1 "a check failed"
        }
    "##
    .parse()
    .unwrap()
}

/// A stand-in for a CLI that declares `json` and `jsonl` outputs.
///
/// Writes far more to stderr than a pipe buffer holds, on purpose: a consumer that pipes
/// stderr and reads only stdout deadlocks at exactly that point, and it is the failure a
/// small fixture never reproduces. Exits non-zero while still emitting valid output, which
/// is what a declared `exit_code 1 "a check failed"` means.
#[cfg(not(windows))]
const FAKE_CLI: &str = r#"#!/usr/bin/env python3
import sys

args = sys.argv[1:]
if "--format" in args:
    fmt = args[args.index("--format") + 1]
elif "--json" in args:
    fmt = "json"
else:
    fmt = "human"

for i in range(200):
    print("noise " + "x" * 200, file=sys.stderr)

if fmt == "jsonl":
    for i in range(5):
        print('{"n": %d}' % i, flush=True)
elif fmt == "json":
    print('{"ok": false}')
else:
    print("2 checks failed")

sys.exit(1)
"#;

#[cfg(windows)]
const FAKE_CLI_RS: &str = r##"
use std::env;
use std::io::{self, Write};

fn main() {
    let args: Vec<String> = env::args().skip(1).collect();
    let format = args
        .iter()
        .position(|arg| arg == "--format")
        .and_then(|i| args.get(i + 1))
        .map(String::as_str)
        .or_else(|| args.iter().any(|arg| arg == "--json").then_some("json"))
        .unwrap_or("human");

    for _ in 0..200 {
        eprintln!("noise {}", "x".repeat(200));
    }
    match format {
        "jsonl" => {
            for i in 0..5 {
                println!(r#"{{"n": {i}}}"#);
                io::stdout().flush().unwrap();
            }
        }
        "json" => println!(r#"{{"ok": false}}"#),
        _ => println!("2 checks failed"),
    }
    std::process::exit(1);
}
"##;

fn write_fake_cli(dir: &Path) -> std::path::PathBuf {
    #[cfg(windows)]
    {
        let source = dir.join("fakecli.rs");
        let path = dir.join("fakecli.exe");
        fs::write(&source, FAKE_CLI_RS).unwrap();
        let compiled = Command::new("rustc")
            .arg(&source)
            .arg("-o")
            .arg(&path)
            .output()
            .expect("Failed to compile the Windows fake CLI");
        assert!(
            compiled.status.success(),
            "Failed to compile the Windows fake CLI:\n{}",
            String::from_utf8_lossy(&compiled.stderr)
        );
        path
    }
    #[cfg(not(windows))]
    {
        use std::os::unix::fs::PermissionsExt;

        let path = dir.join("fakecli");
        fs::write(&path, FAKE_CLI).unwrap();
        fs::set_permissions(&path, fs::Permissions::from_mode(0o755)).unwrap();
        path
    }
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

fn npx_tsc_available() -> bool {
    Command::new("npx")
        .args(["--yes", "typescript", "tsc", "--version"])
        .output()
        .map(|o| o.status.success())
        .unwrap_or(false)
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
        // The directory arrives as argv rather than inside the source. Interpolated into the
        // string literal it used to sit in, a Windows path is read by Python's lexer first:
        // `C:\Users\RUNNER~1\...` fails to even parse, because `\U` starts a unicode escape.
        // Passing it as an argument keeps the path away from any escaping rules at all.
        let result = Command::new("python3")
            .args([
                "-c",
                &format!(
                    "import sys; sys.path.insert(0, sys.argv[1]); from mytool_sdk.{module} import *"
                ),
            ])
            .arg(dir.path())
            .output()
            .expect("Failed to run python3");

        if !result.status.success() {
            let stderr = String::from_utf8_lossy(&result.stderr);
            panic!("Generated Python SDK {module}.py has errors:\n{stderr}");
        }
    }
}

/// Runs the generated client against a real process, which is a different claim from the
/// one above.
///
/// `test_python_sdk_imports` proves the module parses. It cannot see a stderr deadlock, a
/// child left running after an early break, or a wrong exit code, because none of those
/// happen until something is actually executed.
#[test]
fn test_python_sdk_streams_a_real_process() {
    if !tool_exists("python3") {
        eprintln!("Skipping Python SDK stream test - python3 not found");
        return;
    }

    let output = usage::sdk::generate(
        &full_spec(),
        &SdkOptions {
            language: SdkLanguage::Python,
            package_name: None,
            source_file: None,
        },
    );

    let dir = tempfile::tempdir().unwrap();
    let pkg_dir = dir.path().join("mytool_sdk");
    fs::create_dir_all(&pkg_dir).unwrap();
    write_sdk_to_dir(&output, &pkg_dir);
    let cli = write_fake_cli(dir.path());

    let driver = r#"
import sys
sys.path.insert(0, sys.argv[1])
from mytool_sdk.client import Mytool

cli = Mytool(sys.argv[2])

# A declared non-zero code is an outcome, so a parsed result still carries it.
j = cli.check.exec_json()
assert j.data == {"ok": False}, j.data
assert j.exit_code == 1, j.exit_code
assert not j.ok

# The whole stream, with far more on stderr than a pipe buffer holds. Without the
# drain thread this hangs here rather than failing.
stream = cli.check.exec_jsonl()
events = list(stream)
assert events == [{"n": i} for i in range(5)], events
assert stream.exit_code == 1, stream.exit_code
assert len(stream.stderr) > 40000, len(stream.stderr)

# Stopping early must reap the child rather than leave it running.
early = cli.check.exec_jsonl()
it = iter(early)
assert next(it) == {"n": 0}
early.close()
assert early.exit_code is not None

# The boolean spelling picks its output the same way.
s = cli.status.exec_json()
assert s.exit_code == 1

print("ok")
"#;

    let result = Command::new("python3")
        .args(["-c", driver])
        .arg(dir.path())
        .arg(&cli)
        .output()
        .expect("Failed to run python3");
    if !result.status.success() {
        panic!(
            "generated Python client failed against a real process:\n{}\n{}",
            String::from_utf8_lossy(&result.stdout),
            String::from_utf8_lossy(&result.stderr)
        );
    }
}

// ---------------------------------------------------------------------------
// TypeScript
// ---------------------------------------------------------------------------

#[test]
fn test_typescript_sdk_typechecks() {
    if !npx_tsc_available() {
        eprintln!("Skipping TypeScript SDK typecheck test - tsc not available via npx");
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

    let result = Command::new("npx")
        .args(["--yes", "--package", "typescript", "tsc", "--project"])
        .arg(dir.path().join("tsconfig.json"))
        .output()
        .expect("Failed to run tsc");

    if !result.status.success() {
        let stdout = String::from_utf8_lossy(&result.stdout);
        let stderr = String::from_utf8_lossy(&result.stderr);
        panic!("Generated TypeScript SDK does not typecheck:\nstdout: {stdout}\nstderr: {stderr}");
    }

    // Typechecking says the shapes line up; it says nothing about whether the stream works.
    // The same three hazards as the Python side — a stderr pipe nobody drains, a child left
    // running after an early break, and a lost exit code — only appear when it runs.
    if !tool_exists("node") {
        eprintln!("Skipping TypeScript SDK stream check - node not found");
        return;
    }
    let cli = write_fake_cli(dir.path());
    let driver = format!(
        r#"
import {{ Mytool }} from "./client";
import {{ strict as assert }} from "node:assert";

async function main() {{
  const cli = new Mytool({cli:?});

  // A declared non-zero code is an outcome, so a parsed result still carries it.
  const j = await cli.check.execJson();
  assert.deepEqual(j.data, {{ ok: false }});
  assert.equal(j.exitCode, 1);
  assert.equal(j.ok, false);

  // The whole stream, with far more on stderr than a pipe buffer holds.
  const stream = cli.check.execJsonl();
  const seen: unknown[] = [];
  for await (const e of stream) seen.push(e);
  assert.deepEqual(seen, [0, 1, 2, 3, 4].map((n) => ({{ n }})));
  assert.equal(await stream.wait(), 1);
  assert.ok(stream.stderr.length > 40000, `stderr was ${{stream.stderr.length}}`);

  // Breaking out must kill the child rather than leave it running. `close()` can only
  // signal from JS, so the code arrives with `wait()` — and it arriving at all is the
  // proof the process is gone.
  const early = cli.check.execJsonl();
  for await (const e of early) {{
    assert.deepEqual(e, {{ n: 0 }});
    break;
  }}
  await early.wait();
  assert.notEqual(early.exitCode, null);
}}

main().catch((e) => {{ console.error(e); process.exit(1); }});
"#
    );
    fs::write(dir.path().join("drive.ts"), driver).unwrap();

    // Compiled to CommonJS in place, then run: no bundler, no loader flags, nothing that
    // varies with the Node version on the machine.
    let run_config = r#"{
        "compilerOptions": {
            "target": "ES2020",
            "module": "commonjs",
            "strict": true,
            "skipLibCheck": true,
            "esModuleInterop": true,
            "types": ["node"],
            "outDir": "./out"
        },
        "include": ["./*.ts"]
    }"#;
    fs::write(dir.path().join("tsconfig.run.json"), run_config).unwrap();
    let built = Command::new("npx")
        .args(["--yes", "--package", "typescript", "tsc", "--project"])
        .arg(dir.path().join("tsconfig.run.json"))
        .output()
        .expect("Failed to run tsc");
    if !built.status.success() {
        panic!(
            "generated TypeScript did not compile for the stream check:\n{}",
            String::from_utf8_lossy(&built.stdout)
        );
    }
    let ran = Command::new("node")
        .arg(dir.path().join("out/drive.js"))
        .output()
        .expect("Failed to run node");
    if !ran.status.success() {
        panic!(
            "generated TypeScript client failed against a real process:\n{}\n{}",
            String::from_utf8_lossy(&ran.stdout),
            String::from_utf8_lossy(&ran.stderr)
        );
    }
}
