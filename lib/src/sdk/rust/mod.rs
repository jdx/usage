use std::path::PathBuf;

use heck::AsPascalCase;

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

    SdkOutput {
        files: vec![
            SdkFile {
                path: PathBuf::from("types.rs"),
                content: types::render(spec, &package_name, &opts.source_file),
            },
            SdkFile {
                path: PathBuf::from("client.rs"),
                content: client::render(spec, &package_name, &opts.source_file),
            },
            SdkFile {
                path: PathBuf::from("runtime.rs"),
                content: runtime::RUNTIME_RS.to_string(),
            },
            SdkFile {
                path: PathBuf::from("lib.rs"),
                content: render_lib(&package_name),
            },
        ],
    }
}

fn render_lib(package_name: &str) -> String {
    let class_name = AsPascalCase(package_name).to_string();
    format!(
        "pub mod client;\npub mod runtime;\npub mod types;\n\npub use client::{class_name};\npub use runtime::{{CliError, CliResult, CliRunner}};\npub use types::*;\n"
    )
}

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
        insta::assert_snapshot!(get_file(&output, "types.rs"));
    }

    #[test]
    fn test_rust_client() {
        let output = crate::sdk::generate(&SPEC_KITCHEN_SINK, &make_opts());
        insta::assert_snapshot!(get_file(&output, "client.rs"));
    }

    #[test]
    fn test_rust_runtime() {
        let output = crate::sdk::generate(&SPEC_KITCHEN_SINK, &make_opts());
        insta::assert_snapshot!(get_file(&output, "runtime.rs"));
    }

    #[test]
    fn test_rust_lib() {
        let output = crate::sdk::generate(&SPEC_KITCHEN_SINK, &make_opts());
        insta::assert_snapshot!(get_file(&output, "lib.rs"));
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
        insta::assert_snapshot!(get_file(&output, "types.rs"));
    }

    #[test]
    fn test_rust_full_feature_client() {
        let spec = full_feature_spec();
        let output = crate::sdk::generate(&spec, &make_opts());
        insta::assert_snapshot!(get_file(&output, "client.rs"));
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
        insta::assert_snapshot!(get_file(&output, "client.rs"));
    }

    #[test]
    fn test_rust_minimal() {
        let spec: Spec = r##"
            bin "hello"
        "##
        .parse()
        .unwrap();
        let output = crate::sdk::generate(&spec, &make_opts());
        insta::assert_snapshot!(get_file(&output, "client.rs"));
    }

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
        let client = get_file(&output, "client.rs");
        assert!(client.contains("flags: Option<StatusFlags>"));
        insta::assert_snapshot!(client);
    }

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
        let types = get_file(&output, "types.rs");
        assert!(types.contains("BuildEnvChoice"));
        assert!(types.contains("DeployEnvChoice"));
        insta::assert_snapshot!(types);
    }

    /// Required flag without default — tests non-optional flag type rendering.
    #[test]
    fn test_rust_required_flag_type() {
        let spec: Spec = r##"
            bin "tool"
            flag "--token <t>" required=#true help="Auth token"
        "##
        .parse()
        .unwrap();
        let output = crate::sdk::generate(&spec, &make_opts());
        let types = get_file(&output, "types.rs");
        assert!(types.contains("pub token: String,"));
        assert!(!types.contains("pub token: Option<String>,"));
        let client = get_file(&output, "client.rs");
        assert!(client.contains("flags: ToolFlags"));
        assert!(!client.contains("flags: Option<ToolFlags>"));
        insta::assert_snapshot!(types);
    }

    /// Flag edge cases: short-only, deprecated, count with default, value flag with default,
    /// required flag, repeatable boolean flag.
    #[test]
    fn test_rust_flag_edge_cases() {
        let spec: Spec = r##"
            bin "tool"
            flag "-v" help="Short-only flag"
            flag "--type" help="Reserved keyword" deprecated="Use --kind"
            flag "--level" count=#true default="2" help="Count flag with default"
            flag "--format <fmt>" default="json" help="Value flag with default"
            flag "--confirm" required=#true help="Required flag"
            flag "--verbose" var=#true help="Repeatable boolean flag"
        "##
        .parse()
        .unwrap();
        let output = crate::sdk::generate(&spec, &make_opts());
        let types = get_file(&output, "types.rs");
        insta::assert_snapshot!(types);
        let client = get_file(&output, "client.rs");
        assert!(client.contains(r#""-v""#));
        insta::assert_snapshot!(client);
    }

    /// Args with default values — tests that defaults are preserved.
    #[test]
    fn test_rust_arg_defaults() {
        let spec: Spec = r##"
            bin "runner"
            arg "mode" default="fast" help="Run mode"
            arg "output" help="Output path" required=#true
        "##
        .parse()
        .unwrap();
        let output = crate::sdk::generate(&spec, &make_opts());
        let types = get_file(&output, "types.rs");
        assert!(types.contains("pub mode: Option<String>,"));
        assert!(types.contains("pub output: String,"));
        insta::assert_snapshot!(types);
    }

    /// Config props — covers config struct rendering.
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
        let types = get_file(&output, "types.rs");
        assert!(types.contains("MyappConfig"));
        insta::assert_snapshot!(types);
    }

    /// Boolean flag with default=#false — tests default rendering.
    #[test]
    fn test_rust_boolean_flag_default_false() {
        let spec: Spec = r##"
            bin "app"
            flag "--no-cache" default=#false help="Disable cache"
        "##
        .parse()
        .unwrap();
        let output = crate::sdk::generate(&spec, &make_opts());
        let types = get_file(&output, "types.rs");
        assert!(types.contains("Default: false"));
        insta::assert_snapshot!(types);
    }

    /// Flag with choices — flag arg with choices renders correct type.
    #[test]
    fn test_rust_flag_with_choices() {
        let spec: Spec = r##"
            bin "tool"
            flag "--shell <shell>" help="Shell type" {
                choices "bash" "zsh" "fish"
            }
        "##
        .parse()
        .unwrap();
        let output = crate::sdk::generate(&spec, &make_opts());
        let types = get_file(&output, "types.rs");
        assert!(types.contains("ShellChoice"));
        insta::assert_snapshot!(types);
    }

    /// Flag with env annotation — env variable appears in doc comment.
    #[test]
    fn test_rust_flag_with_env() {
        let spec: Spec = r##"
            bin "app"
            flag "--config <path>" help="Config file" env="APP_CONFIG"
        "##
        .parse()
        .unwrap();
        let output = crate::sdk::generate(&spec, &make_opts());
        let types = get_file(&output, "types.rs");
        assert!(types.contains("Env: APP_CONFIG"));
        insta::assert_snapshot!(types);
    }

    /// Hidden flag excluded from types and client.
    #[test]
    fn test_rust_flag_hide() {
        let spec: Spec = r##"
            bin "app"
            flag "--verbose" help="Verbosity"
            flag "--debug" hide=#true help="Hidden debug flag"
        "##
        .parse()
        .unwrap();
        let output = crate::sdk::generate(&spec, &make_opts());
        let types = get_file(&output, "types.rs");
        assert!(types.contains("verbose"));
        assert!(!types.contains("debug"));
    }

    /// Negate flag rendered in client build method.
    #[test]
    fn test_rust_negate_flag_build() {
        let spec: Spec = r##"
            bin "app"
            flag "--dry-run" help="Dry run" negate="--no-dry-run"
        "##
        .parse()
        .unwrap();
        let output = crate::sdk::generate(&spec, &make_opts());
        let client = get_file(&output, "client.rs");
        assert!(client.contains("--dry-run"));
        assert!(client.contains("--no-dry-run"));
        insta::assert_snapshot!(client);
    }

    /// Count flag rendered in client build method.
    #[test]
    fn test_rust_count_flag_build() {
        let spec: Spec = r##"
            bin "app"
            flag "-v --verbose" count=#true help="Verbosity level"
        "##
        .parse()
        .unwrap();
        let output = crate::sdk::generate(&spec, &make_opts());
        let client = get_file(&output, "client.rs");
        assert!(client.contains(r#""--verbose""#));
        assert!(client.contains("flags.verbose"));
        insta::assert_snapshot!(client);
    }

    /// Repeatable value flag with default — covers var + arg + default in client build.
    #[test]
    fn test_rust_var_value_flag_with_default() {
        let spec: Spec = r##"
            bin "tool"
            flag "--tag <t>" var=#true default="latest" help="Tags"
        "##
        .parse()
        .unwrap();
        let output = crate::sdk::generate(&spec, &make_opts());
        let types = get_file(&output, "types.rs");
        assert!(types.contains("Vec<String>"));
        assert!(types.contains("Default: latest"));
        let client = get_file(&output, "client.rs");
        assert!(client.contains("for v in"));
        insta::assert_snapshot!(types);
    }

    /// Flag with multiple long aliases — `-f --format --fmt <fmt>`.
    #[test]
    fn test_rust_multiple_aliases() {
        let spec: Spec = r##"
            bin "tool"
            flag "-f --format --fmt <fmt>" help="Output format"
        "##
        .parse()
        .unwrap();
        let output = crate::sdk::generate(&spec, &make_opts());
        let types = get_file(&output, "types.rs");
        assert!(types.contains("Aliases: fmt"));
        let client = get_file(&output, "client.rs");
        assert!(client.contains("--format"));
        insta::assert_snapshot!(client);
    }

    /// Global repeatable flags — covers flag var branches in GlobalFlags.
    #[test]
    fn test_rust_global_repeatable_flags() {
        let spec: Spec = r##"
            bin "app"
            flag "-v --verbose" global=#true var=#true help="Repeatable verbose"
            flag "--tag <t>" global=#true var=#true help="Repeatable tag"
            cmd "run" help="Run" {
                arg "target"
            }
        "##
        .parse()
        .unwrap();
        let output = crate::sdk::generate(&spec, &make_opts());
        let types = get_file(&output, "types.rs");
        assert!(types.contains("Vec<bool>"));
        assert!(types.contains("Vec<String>"));
        insta::assert_snapshot!(types);
    }

    /// Optional variadic arg — covers the optional + var branch in client rendering.
    #[test]
    fn test_rust_optional_variadic_arg() {
        let spec: Spec = r##"
            bin "tool"
            arg "[files]" var=#true help="Input files"
        "##
        .parse()
        .unwrap();
        let output = crate::sdk::generate(&spec, &make_opts());
        let types = get_file(&output, "types.rs");
        assert!(types.contains("Vec<String>"));
        let client = get_file(&output, "client.rs");
        assert!(client.contains("if let Some(ref files) = args.files"));
        insta::assert_snapshot!(client);
    }

    /// Hidden command — covers cmd.hide early-return.
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
        let types = get_file(&output, "types.rs");
        assert!(types.contains("VisibleArgs"));
        assert!(!types.contains("SecretArgs"));
    }

    /// Optional arg without default.
    #[test]
    fn test_rust_optional_arg_empty_flags() {
        let spec: Spec = r##"
            bin "app"
            arg "[name]" help="Optional arg without default"
            cmd "check" help="Check something" {
                arg "target" required=#true help="Required arg"
                arg "mode" default="quick" help="Optional arg with default"
            }
        "##
        .parse()
        .unwrap();
        let output = crate::sdk::generate(&spec, &make_opts());
        let types = get_file(&output, "types.rs");
        assert!(types.contains("pub name: Option<String>,"));
        insta::assert_snapshot!(types);
    }

    /// Deeply nested subcommands — 3+ levels.
    #[test]
    fn test_rust_deep_nesting() {
        let spec: Spec = r##"
            bin "app"
            cmd "db" help="Database operations" {
                cmd "migration" help="Migration management" {
                    cmd "create" help="Create a new migration" {
                        arg "name"
                        flag "--template <t>" help="Migration template"
                    }
                    cmd "run" help="Run pending migrations" {
                        flag "--step <n>" help="Number of migrations to run"
                    }
                }
            }
        "##
        .parse()
        .unwrap();
        let output = crate::sdk::generate(&spec, &make_opts());
        let client = get_file(&output, "client.rs");
        assert!(client.contains("pub struct Db"));
        assert!(client.contains("pub struct Migration"));
        assert!(client.contains("pub struct Create"));
        insta::assert_snapshot!(client);
    }

    /// Example without lang attribute — tests exec doc path.
    #[test]
    fn test_rust_example_without_lang() {
        let spec: Spec = r##"
            bin "app"
            cmd "greet" help="Greet someone" {
                example "app greet hello"
                arg "name" help="Name to greet"
            }
        "##
        .parse()
        .unwrap();
        let output = crate::sdk::generate(&spec, &make_opts());
        let client = get_file(&output, "client.rs");
        assert!(client.contains("app greet hello"));
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
        let client = get_file(&output, "client.rs");
        assert!(client.contains("GlobalFlags"));
        insta::assert_snapshot!(client);
    }

    /// double_dash=automatic — covers arg ordering and separator insertion.
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
        let client = get_file(&output, "client.rs");
        assert!(client.contains("double_dash=automatic"));
        insta::assert_snapshot!(client);
    }

    /// Config with all props having defaults — tests Default derive on config struct.
    #[test]
    fn test_rust_config_all_optional() {
        let spec: Spec = r##"
            bin "app"
            config {
                prop "debug" default=#true data_type=boolean
                prop "port" default=8080 data_type=integer
            }
        "##
        .parse()
        .unwrap();
        let output = crate::sdk::generate(&spec, &make_opts());
        let types = get_file(&output, "types.rs");
        assert!(types.contains("AppConfig"));
        insta::assert_snapshot!(types);
    }

    /// Boolean config prop with default=#false.
    #[test]
    fn test_rust_config_boolean_default_false() {
        let spec: Spec = r##"
            bin "app"
            config {
                prop "verbose" default=#false data_type=boolean help="Verbose output"
                prop "dry_run" default=#true data_type=boolean help="Dry run mode"
            }
        "##
        .parse()
        .unwrap();
        let output = crate::sdk::generate(&spec, &make_opts());
        let types = get_file(&output, "types.rs");
        assert!(types.contains("verbose"));
        assert!(types.contains("dry_run"));
        insta::assert_snapshot!(types);
    }

    /// String config prop with default.
    #[test]
    fn test_rust_config_string_with_default() {
        let spec: Spec = r##"
            bin "app"
            config {
                prop "host" default="localhost" data_type=string help="Server host"
                prop "name" default="myapp" data_type=string
            }
        "##
        .parse()
        .unwrap();
        let output = crate::sdk::generate(&spec, &make_opts());
        let types = get_file(&output, "types.rs");
        assert!(types.contains("host"));
        assert!(types.contains("name"));
        insta::assert_snapshot!(types);
    }

    /// Package name override.
    #[test]
    fn test_rust_package_name_override() {
        let spec: Spec = r##"
            bin "original-cli"
        "##
        .parse()
        .unwrap();
        let opts = SdkOptions {
            language: SdkLanguage::Rust,
            package_name: Some("my_custom_sdk".to_string()),
            source_file: None,
        };
        let output = crate::sdk::generate(&spec, &opts);
        let lib = get_file(&output, "lib.rs");
        assert!(lib.contains("MyCustomSdk"));
        insta::assert_snapshot!(lib);
    }
}
