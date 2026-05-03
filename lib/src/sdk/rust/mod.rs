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

    /// Choice type collision: same arg name with different choices in different subcommands.
    #[test]
    fn test_rust_choice_collision() {
        let spec: Spec = r##"
            bin "tool"
            cmd "build" help="Build" {
                arg "env" help="Build environment" {
                    choices "debug" "release"
                }
            }
            cmd "deploy" help="Deploy" {
                arg "env" help="Deploy environment" {
                    choices "staging" "production"
                }
            }
        "##
        .parse()
        .unwrap();
        let output = crate::sdk::generate(&spec, &make_opts());
        let types = get_file(&output, "src/types.rs");
        assert!(types.contains("BuildEnvChoice"));
        assert!(types.contains("DeployEnvChoice"));
        insta::assert_snapshot!(types);
    }

    /// Flags-only subcommand (no positional args).
    #[test]
    fn test_rust_flags_only_subcommand() {
        let spec: Spec = r##"
            bin "app"
            cmd "status" help="Show status" {
                flag "--verbose" help="Show detailed status"
                flag "--json" help="Output as JSON"
            }
        "##
        .parse()
        .unwrap();
        let output = crate::sdk::generate(&spec, &make_opts());
        let client = get_file(&output, "src/client.rs");
        // Must not have double comma in exec signature
        assert!(!client.contains("self, ,"));
        assert!(client.contains(
            "pub fn exec(&self, flags: Option<&StatusFlags>) -> Result<CliResult, CliError>"
        ));
        insta::assert_snapshot!(client);
    }

    /// Config props — covers render_config_struct and config_prop_type.
    #[test]
    fn test_rust_config_props() {
        let spec: Spec = r##"
            bin "myapp"
            config {
                prop "debug" default=#true data_type=boolean help="Enable debug mode"
                prop "port" default=8080 data_type=integer
                prop "rate" default="1.5" data_type=float
                prop "host" data_type=string
                prop "extra" data_type="null"
            }
        "##
        .parse()
        .unwrap();
        let output = crate::sdk::generate(&spec, &make_opts());
        let types = get_file(&output, "src/types.rs");
        assert!(types.contains("pub struct MyappConfig"));
        assert!(types.contains("debug: Option<bool>"));
        assert!(types.contains("port: Option<i64>"));
        assert!(types.contains("rate: Option<f64>"));
        assert!(types.contains("host: String"));
        assert!(types.contains("extra: String"));
        insta::assert_snapshot!(types);
    }

    /// Hidden command — covers cmd.hide early-return paths.
    #[test]
    fn test_rust_hidden_command() {
        let spec: Spec = r##"
            bin "app"
            cmd "visible" help="A visible command" {
                arg "name"
            }
            cmd "secret" hide=#true help="Hidden command" {
                arg "name"
            }
        "##
        .parse()
        .unwrap();
        let output = crate::sdk::generate(&spec, &make_opts());
        let types = get_file(&output, "src/types.rs");
        assert!(types.contains("VisibleArgs"));
        assert!(!types.contains("SecretArgs"));
    }

    /// Flag with aliases, deprecated, short-only, and reserved keyword identifiers.
    #[test]
    fn test_rust_flag_edge_cases() {
        let spec: Spec = r##"
            bin "tool"
            flag "-v" help="Short-only flag"
            flag "--type" help="Reserved keyword as flag name" deprecated="Use --kind instead"
            arg "type" help="Reserved keyword as arg name" required=#true
        "##
        .parse()
        .unwrap();
        let output = crate::sdk::generate(&spec, &make_opts());
        let types = get_file(&output, "src/types.rs");
        // reserved keyword "type" should be sanitized to "_type"
        assert!(types.contains("_type: String,"));
        assert!(types.contains("_type: Option<bool>,"));
        assert!(types.contains("#[deprecated = \"Use --kind instead\"]"));
        insta::assert_snapshot!(types);
        let client = get_file(&output, "src/client.rs");
        // short-only flag build uses "-v"
        assert!(client.contains(r#""-v""#));
        insta::assert_snapshot!(client);
    }

    /// double_dash=automatic, examples, repeatable boolean flag.
    #[test]
    fn test_rust_double_dash_automatic() {
        let spec: Spec = r##"
            bin "runner"
            arg "input" help="Input file"
            arg "extra" double_dash="automatic" var=#true help="Extra files"
            flag "--verbose" var=#true help="Repeatable boolean flag"
            cmd "run" help="Run a task" {
                example "runner run hello" header="Basic run"
                arg "task" help="Task to run" double_dash="automatic"
            }
        "##
        .parse()
        .unwrap();
        let output = crate::sdk::generate(&spec, &make_opts());
        let client = get_file(&output, "src/client.rs");
        assert!(client.contains("double_dash=automatic"));
        assert!(client.contains("Basic run: `runner run hello`"));
        insta::assert_snapshot!(client);
    }

    /// Global flags with flags-only subcommand — covers GlobalFlags type branch.
    #[test]
    fn test_rust_global_flags_flags_only() {
        let spec: Spec = r##"
            bin "app"
            flag "-v --verbose" global=#true help="Verbosity"
            cmd "status" help="Show status" {
                flag "--json" help="JSON output"
            }
            cmd "info" help="Show info" {}
        "##
        .parse()
        .unwrap();
        let output = crate::sdk::generate(&spec, &make_opts());
        let client = get_file(&output, "src/client.rs");
        // "info" subcommand has no own flags, only global flags => GlobalFlags type
        assert!(client.contains("Option<&GlobalFlags>"));
        insta::assert_snapshot!(client);
    }
}
