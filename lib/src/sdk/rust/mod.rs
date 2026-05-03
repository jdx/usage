use std::path::PathBuf;

use heck::AsPascalCase;
use heck::AsSnakeCase;

use crate::sdk::{SdkFile, SdkOptions, SdkOutput};
use crate::Spec;

mod client;
mod runtime;
mod types;

pub fn generate(spec: &Spec, opts: &SdkOptions) -> SdkOutput {
    let package_name = opts
        .package_name
        .clone()
        .unwrap_or_else(|| spec.bin.clone());
    let crate_name = AsSnakeCase(&package_name).to_string();

    SdkOutput {
        files: vec![
            SdkFile {
                path: PathBuf::from("src/types.rs"),
                content: types::render(spec, &package_name, &opts.source_file),
            },
            SdkFile {
                path: PathBuf::from("src/client.rs"),
                content: client::render(spec, &package_name, &opts.source_file),
            },
            SdkFile {
                path: PathBuf::from("src/runtime.rs"),
                content: runtime::RUNTIME_RS.to_string(),
            },
            SdkFile {
                path: PathBuf::from("src/lib.rs"),
                content: render_lib_rs(&package_name, &opts.source_file),
            },
            SdkFile {
                path: PathBuf::from("Cargo.toml"),
                content: render_cargo_toml(&crate_name),
            },
        ],
    }
}

fn render_lib_rs(package_name: &str, source_file: &Option<String>) -> String {
    let mut w = crate::sdk::CodeWriter::with_indent("    ");
    let header = crate::sdk::generated_header("//!", source_file);
    w.line(&header);
    w.line("");
    w.line("pub mod runtime;");
    w.line("pub mod types;");
    w.line("pub mod client;");
    w.line("");
    let class_name = AsPascalCase(package_name).to_string();
    w.line(&format!("pub use client::{class_name};"));
    w.line("pub use types::*;");
    w.line("pub use runtime::{CliResult, CliError, CliRunner};");
    w.finish()
}

fn render_cargo_toml(crate_name: &str) -> String {
    format!(
        r#"[package]
name = "{crate_name}-sdk"
version = "0.1.0"
edition = "2021"
"#
    )
}

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use crate::sdk::{SdkLanguage, SdkOptions};
    use crate::test::SPEC_KITCHEN_SINK;
    use crate::Spec;

    fn make_opts() -> SdkOptions {
        SdkOptions {
            language: SdkLanguage::Rust,
            package_name: None,
            source_file: Some("test.usage.kdl".to_string()),
        }
    }

    fn get_file<'a>(output: &'a crate::sdk::SdkOutput, name: &str) -> &'a str {
        output
            .files
            .iter()
            .find(|f| f.path.to_str() == Some(name))
            .unwrap_or_else(|| panic!("{name} should exist"))
            .content
            .as_str()
    }

    #[test]
    fn test_rust_types() {
        let output = crate::sdk::generate(&SPEC_KITCHEN_SINK, &make_opts());
        insta::assert_snapshot!(get_file(&output, "src/types.rs"));
    }

    #[test]
    fn test_rust_client() {
        let output = crate::sdk::generate(&SPEC_KITCHEN_SINK, &make_opts());
        insta::assert_snapshot!(get_file(&output, "src/client.rs"));
    }

    #[test]
    fn test_rust_runtime() {
        let output = crate::sdk::generate(&SPEC_KITCHEN_SINK, &make_opts());
        insta::assert_snapshot!(get_file(&output, "src/runtime.rs"));
    }

    #[test]
    fn test_rust_lib() {
        let output = crate::sdk::generate(&SPEC_KITCHEN_SINK, &make_opts());
        insta::assert_snapshot!(get_file(&output, "src/lib.rs"));
    }

    fn full_feature_spec() -> Spec {
        r##"
            bin "mytool"
            name "mytool"
            version "1.2.3"
            about "A powerful CLI tool"
            author "Jane Doe"

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
            }

            cmd "deploy" help="Deploy the project" {
                arg "env" help="Target environment" {
                    choices "staging" "production"
                }
                arg "tags" var=#true help="Deployment tags" var_min=1 var_max=5
                flag "-f --force" help="Force deploy" deprecated="Use --confirm instead"
                flag "--confirm" help="Confirm deployment"
            }
        "##
        .parse()
        .unwrap()
    }

    #[test]
    fn test_rust_full_feature_types() {
        let spec = full_feature_spec();
        let output = crate::sdk::generate(&spec, &make_opts());
        insta::assert_snapshot!(get_file(&output, "src/types.rs"));
    }

    #[test]
    fn test_rust_full_feature_client() {
        let spec = full_feature_spec();
        let output = crate::sdk::generate(&spec, &make_opts());
        insta::assert_snapshot!(get_file(&output, "src/client.rs"));
    }

    #[test]
    fn test_rust_hyphenated_subcommands() {
        let spec: Spec = r##"
            bin "cli"
            cmd "add-remote" help="Add a remote" {
                arg "name"
                arg "url"
            }
            cmd "remove-remote" help="Remove a remote" {
                arg "name"
            }
        "##
        .parse()
        .unwrap();
        let output = crate::sdk::generate(&spec, &make_opts());
        insta::assert_snapshot!(get_file(&output, "src/client.rs"));
    }

    #[test]
    fn test_rust_minimal() {
        let spec: Spec = r##"
            bin "hello"
        "##
        .parse()
        .unwrap();
        let output = crate::sdk::generate(&spec, &make_opts());
        insta::assert_snapshot!(get_file(&output, "src/client.rs"));
    }
}
