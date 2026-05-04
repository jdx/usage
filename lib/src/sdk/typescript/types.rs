use heck::AsPascalCase;

use crate::spec::cmd::SpecCommand;
use crate::spec::config::SpecConfigProp;
use crate::spec::data_types::SpecDataTypes;
use crate::{Spec, SpecArg, SpecFlag};

use crate::sdk::{
    collect_choice_types, command_type_name, generated_header, ChoiceTypeMap, CodeWriter,
};

pub fn render(spec: &Spec, package_name: &str, source_file: &Option<String>) -> String {
    let mut w = CodeWriter::new();

    w.line(&generated_header("//", source_file));

    // spec metadata constants
    if let Some(version) = &spec.version {
        w.line(&format!("export const VERSION = \"{version}\";"));
    }
    if let Some(about) = &spec.about {
        w.line(&format!("/** {about} */"));
        w.line(&format!("export const ABOUT = \"{about}\";"));
    }
    if let Some(author) = &spec.author {
        w.line(&format!("export const AUTHOR = \"{author}\";"));
    }

    let choice_types = collect_choice_types(&spec.cmd);

    // collect root-level global flags
    let root_global_flags: Vec<&SpecFlag> = spec
        .cmd
        .flags
        .iter()
        .filter(|f| f.global && !f.hide)
        .collect();
    let has_global_flags = !root_global_flags.is_empty();

    if !choice_types.is_empty() {
        w.line("");
        for (name, choices) in choice_types.iter() {
            let union = choices
                .iter()
                .map(|c| format!("\"{c}\""))
                .collect::<Vec<_>>()
                .join(" | ");
            w.line(&format!("export type {name} = {union};"));
        }
    }

    // render GlobalFlags interface if root has global flags
    if has_global_flags {
        w.line("");
        w.line("/** Global flags available on all subcommands. */");
        w.line("export interface GlobalFlags {");
        w.indent();
        for flag in &root_global_flags {
            let prop = flag_property_name(flag);
            let ts_type = flag_ts_simple(flag);
            let optional = if flag.required { "" } else { "?" };
            let mut doc_parts = Vec::new();
            if let Some(help) = &flag.help {
                doc_parts.push(help.clone());
            }
            if let Some(env) = &flag.env {
                doc_parts.push(format!("Environment variable: {env}"));
            }
            if !doc_parts.is_empty() {
                w.line(&format!("/** {} */", doc_parts.join(". ")));
            }
            w.line(&format!("{prop}{optional}: {ts_type};"));
        }
        w.dedent();
        w.line("}");
    }

    render_command_types(
        &spec.cmd,
        package_name,
        &choice_types,
        has_global_flags,
        &mut w,
    );

    if !spec.config.props.is_empty() {
        w.line("");
        let config_name = format!("{}Config", AsPascalCase(package_name));
        w.line(&format!("export interface {config_name} {{"));
        w.indent();
        for (name, prop) in &spec.config.props {
            let ts_type = config_prop_type(prop);
            let optional = if prop.default.is_some() { "?" } else { "" };
            if let Some(help) = &prop.help {
                w.line(&format!("/** {help} */"));
            }
            w.line(&format!("{name}{optional}: {ts_type};"));
        }
        w.dedent();
        w.line("}");
    }

    w.finish()
}

fn render_command_types(
    cmd: &SpecCommand,
    package_name: &str,
    choice_types: &ChoiceTypeMap,
    has_global_flags: bool,
    w: &mut CodeWriter,
) {
    if cmd.hide {
        return;
    }

    let name = command_type_name(cmd, package_name);
    let cmd_name = &cmd.name;

    let visible_args: Vec<&SpecArg> = cmd.args.iter().filter(|a| !a.hide).collect();
    let visible_flags: Vec<&SpecFlag> = cmd.flags.iter().filter(|f| !f.hide).collect();
    let has_any_flags = !visible_flags.is_empty() || has_global_flags;

    if !visible_args.is_empty() {
        w.line("");
        w.line(&format!("export interface {name}Args {{"));
        w.indent();
        for arg in &visible_args {
            render_arg_field(arg, cmd_name, choice_types, w);
        }
        w.dedent();
        w.line("}");
    }

    if has_any_flags {
        w.line("");
        if has_global_flags {
            w.line(&format!(
                "export interface {name}Flags extends GlobalFlags {{"
            ));
        } else {
            w.line(&format!("export interface {name}Flags {{"));
        }
        w.indent();
        for flag in &visible_flags {
            render_flag_field(flag, cmd_name, choice_types, w);
        }
        w.dedent();
        w.line("}");
    }

    for subcmd in cmd.subcommands.values() {
        render_command_types(subcmd, package_name, choice_types, has_global_flags, w);
    }
}

fn render_arg_field(
    arg: &SpecArg,
    cmd_name: &str,
    choice_types: &ChoiceTypeMap,
    w: &mut CodeWriter,
) {
    let ts_type = arg_ts_type(arg, cmd_name, choice_types);
    let optional = if arg.required && arg.default.is_empty() {
        ""
    } else {
        "?"
    };
    let mut doc_parts = Vec::new();
    if let Some(help) = &arg.help {
        doc_parts.push(help.clone());
    }
    if let Some(env) = &arg.env {
        doc_parts.push(format!("Environment variable: {env}"));
    }
    if arg.var {
        if let Some(min) = arg.var_min {
            doc_parts.push(format!("Min count: {min}"));
        }
        if let Some(max) = arg.var_max {
            doc_parts.push(format!("Max count: {max}"));
        }
    }
    if !doc_parts.is_empty() {
        w.line(&format!("/** {} */", doc_parts.join(". ")));
    }
    w.line(&format!(
        "{}{optional}: {ts_type};",
        sanitize_ident(&arg.name)
    ));
}

fn render_flag_field(
    flag: &SpecFlag,
    cmd_name: &str,
    choice_types: &ChoiceTypeMap,
    w: &mut CodeWriter,
) {
    let ts_type = flag_ts_type(flag, cmd_name, choice_types);
    let optional = if flag.required && flag.default.is_empty() {
        ""
    } else {
        "?"
    };
    let mut doc_parts = Vec::new();
    if let Some(help) = &flag.help {
        doc_parts.push(help.clone());
    }
    if let Some(env) = &flag.env {
        doc_parts.push(format!("Environment variable: {env}"));
    }
    if let Some(deprecated) = &flag.deprecated {
        doc_parts.push(format!("@deprecated {deprecated}"));
    }
    if !flag.default.is_empty() {
        doc_parts.push(format!("@default {}", flag.default.join(", ")));
    }
    // document flag aliases
    let alias_strs: Vec<String> = flag
        .short
        .iter()
        .skip(1)
        .map(|c| format!("-{c}"))
        .chain(flag.long.iter().skip(1).map(|l| format!("--{l}")))
        .collect();
    if !alias_strs.is_empty() {
        doc_parts.push(format!("Aliases: {}", alias_strs.join(", ")));
    }
    if !doc_parts.is_empty() {
        w.line(&format!("/** {} */", doc_parts.join(". ")));
    }
    let prop_name = flag_property_name(flag);
    w.line(&format!("{prop_name}{optional}: {ts_type};"));
}

fn arg_ts_type(arg: &SpecArg, cmd_name: &str, choice_types: &ChoiceTypeMap) -> String {
    let base = if let Some(choices) = &arg.choices {
        if let Some(resolved) = choice_types.lookup(cmd_name, &arg.name) {
            resolved.to_string()
        } else {
            choices
                .choices
                .iter()
                .map(|c| format!("\"{c}\""))
                .collect::<Vec<_>>()
                .join(" | ")
        }
    } else {
        "string".to_string()
    };

    if arg.var {
        format!("{base}[]")
    } else {
        base
    }
}

fn flag_ts_type(flag: &SpecFlag, cmd_name: &str, choice_types: &ChoiceTypeMap) -> String {
    if flag.count {
        return "number".to_string();
    }

    match &flag.arg {
        Some(arg) => {
            let base = if let Some(choices) = &arg.choices {
                if let Some(resolved) = choice_types.lookup(cmd_name, &flag.name) {
                    resolved.to_string()
                } else {
                    choices
                        .choices
                        .iter()
                        .map(|c| format!("\"{c}\""))
                        .collect::<Vec<_>>()
                        .join(" | ")
                }
            } else {
                "string".to_string()
            };

            if flag.var {
                format!("{base}[]")
            } else {
                base
            }
        }
        None => {
            if flag.var {
                "boolean[]".to_string()
            } else {
                "boolean".to_string()
            }
        }
    }
}

fn flag_ts_simple(flag: &SpecFlag) -> String {
    if flag.count {
        return "number".to_string();
    }
    match &flag.arg {
        Some(_) => {
            if flag.var {
                "string[]".to_string()
            } else {
                "string".to_string()
            }
        }
        None => {
            if flag.var {
                "boolean[]".to_string()
            } else {
                "boolean".to_string()
            }
        }
    }
}

fn config_prop_type(prop: &SpecConfigProp) -> String {
    match prop.data_type {
        SpecDataTypes::String => "string".to_string(),
        SpecDataTypes::Integer => "number".to_string(),
        SpecDataTypes::Float => "number".to_string(),
        SpecDataTypes::Boolean => "boolean".to_string(),
        SpecDataTypes::Null => "unknown".to_string(),
    }
}

pub(crate) fn flag_property_name(flag: &SpecFlag) -> String {
    if let Some(long) = flag.long.first() {
        return sanitize_ident(&heck::AsLowerCamelCase(long).to_string());
    }
    if let Some(short) = flag.short.first() {
        return short.to_string();
    }
    sanitize_ident(&flag.name)
}

pub(crate) fn sanitize_ident(name: &str) -> String {
    let camel = heck::AsLowerCamelCase(name).to_string();
    match camel.as_str() {
        "function" | "class" | "const" | "let" | "var" | "type" | "interface" | "new"
        | "delete" | "return" | "export" | "import" | "default" | "in" | "instanceof" => {
            format!("_{camel}")
        }
        _ => camel,
    }
}

#[cfg(test)]
mod tests {
    use crate::sdk::{SdkLanguage, SdkOptions};
    use crate::test::SPEC_KITCHEN_SINK;
    use crate::Spec;

    fn make_opts() -> SdkOptions {
        SdkOptions {
            language: SdkLanguage::TypeScript,
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
    fn test_typescript_types() {
        let output = super::super::super::generate(&SPEC_KITCHEN_SINK, &make_opts());
        insta::assert_snapshot!(get_file(&output, "types.ts"));
    }

    #[test]
    fn test_typescript_client() {
        let output = super::super::super::generate(&SPEC_KITCHEN_SINK, &make_opts());
        insta::assert_snapshot!(get_file(&output, "client.ts"));
    }

    #[test]
    fn test_typescript_runtime() {
        let output = super::super::super::generate(&SPEC_KITCHEN_SINK, &make_opts());
        insta::assert_snapshot!(get_file(&output, "runtime.ts"));
    }

    #[test]
    fn test_typescript_index() {
        let output = super::super::super::generate(&SPEC_KITCHEN_SINK, &make_opts());
        insta::assert_snapshot!(get_file(&output, "index.ts"));
    }

    /// Spec with version, about, author, global flags, double_dash,
    /// deprecated, aliases, examples, repeatable value flags.
    fn full_feature_spec() -> Spec {
        let spec: Spec = r##"
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

            cmd "build" help="Build the project" deprecated="Use 'compile' instead" {
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
        "##
        .parse()
        .unwrap();
        spec
    }

    #[test]
    fn test_full_feature_types() {
        let spec = full_feature_spec();
        let output = super::super::super::generate(&spec, &make_opts());
        insta::assert_snapshot!(get_file(&output, "types.ts"));
    }

    #[test]
    fn test_full_feature_client() {
        let spec = full_feature_spec();
        let output = super::super::super::generate(&spec, &make_opts());
        insta::assert_snapshot!(get_file(&output, "client.ts"));
    }

    /// Spec with config props.
    #[test]
    fn test_config_props() {
        let spec: Spec = r##"
            bin "myapp"
            config {
                prop "debug" default=#true data_type=boolean help="Enable debug mode"
                prop "port" default=8080 data_type=integer env="MYAPP_PORT"
                prop "host" data_type=string
            }
        "##
        .parse()
        .unwrap();
        let output = super::super::super::generate(&spec, &make_opts());
        // just verify it doesn't crash and has the config interface
        let types = get_file(&output, "types.ts");
        assert!(types.contains("MyappConfig"));
        assert!(types.contains("debug?: boolean"));
        assert!(types.contains("port?: number"));
        assert!(types.contains("host: string"));
    }

    /// Spec with hyphenated subcommand names.
    #[test]
    fn test_hyphenated_subcommands() {
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
        let output = super::super::super::generate(&spec, &make_opts());
        insta::assert_snapshot!(get_file(&output, "client.ts"));
    }

    /// Spec with deeply nested subcommands.
    #[test]
    fn test_deep_nesting() {
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
        let output = super::super::super::generate(&spec, &make_opts());
        insta::assert_snapshot!(get_file(&output, "client.ts"));
    }

    /// Minimal spec with no args, no flags, no subcommands.
    #[test]
    fn test_minimal_spec() {
        let spec: Spec = r##"
            bin "hello"
        "##
        .parse()
        .unwrap();
        let output = super::super::super::generate(&spec, &make_opts());
        insta::assert_snapshot!(get_file(&output, "client.ts"));
    }

    /// Test package_name override.
    #[test]
    fn test_package_name_override() {
        let spec: Spec = r##"
            bin "original-cli"
        "##
        .parse()
        .unwrap();
        let opts = SdkOptions {
            language: SdkLanguage::TypeScript,
            package_name: Some("MyCustomSdk".to_string()),
            source_file: None,
        };
        let output = super::super::super::generate(&spec, &opts);
        insta::assert_snapshot!(get_file(&output, "index.ts"));
    }

    /// Choice type collision: same arg name with different choices in different subcommands.
    #[test]
    fn test_choice_collision() {
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
        let output = super::super::super::generate(&spec, &make_opts());
        let types = get_file(&output, "types.ts");
        assert!(types.contains("BuildEnvChoice"));
        assert!(types.contains("DeployEnvChoice"));
        assert!(types.contains(r#""debug""#));
        assert!(types.contains(r#""staging""#));
        insta::assert_snapshot!(types);
    }

    /// Flags-only subcommand (no positional args).
    #[test]
    fn test_flags_only_subcommand() {
        let spec: Spec = r##"
            bin "app"
            cmd "status" help="Show status" {
                flag "--verbose" help="Show detailed status"
                flag "--json" help="Output as JSON"
            }
        "##
        .parse()
        .unwrap();
        let output = super::super::super::generate(&spec, &make_opts());
        let client = get_file(&output, "client.ts");
        assert!(client.contains("exec(flags?: StatusFlags): CliResult"));
        insta::assert_snapshot!(client);
    }

    /// Config with all data_type variants, arg with env, flag with deprecated + aliases,
    /// reserved keyword identifiers.
    #[test]
    fn test_typescript_config_and_flag_edge_cases() {
        let spec: Spec = r##"
            bin "myapp"
            config {
                prop "debug" default=#true data_type=boolean help="Enable debug mode"
                prop "port" default=8080 data_type=integer
                prop "rate" default="1.5" data_type=float
                prop "host" data_type=string
                prop "extra" data_type="null"
            }
            arg "input" help="Input file" env="MYAPP_INPUT"
            flag "--type" help="Reserved keyword" deprecated="Use --kind"
            flag "-f --format --fmt <fmt>" help="Flag with short and long alias"
            flag "-v" help="Short-only flag"
        "##
        .parse()
        .unwrap();
        let output = super::super::super::generate(&spec, &make_opts());
        let types = get_file(&output, "types.ts");
        assert!(types.contains("MyappConfig"));
        insta::assert_snapshot!(types);
    }

    /// Hidden command — covers cmd.hide early-return in render_command_types.
    #[test]
    fn test_typescript_hidden_command() {
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
        let output = super::super::super::generate(&spec, &make_opts());
        let types = get_file(&output, "types.ts");
        assert!(types.contains("VisibleArgs"));
        assert!(!types.contains("SecretArgs"));
    }

    /// double_dash=automatic, examples with lang, global flags with flags-only subcommand,
    /// repeatable boolean flag, short-only flag build.
    #[test]
    fn test_typescript_client_edge_cases() {
        let spec: Spec = r##"
            bin "runner"
            flag "-v --verbose" global=#true help="Verbosity"
            flag "--debug" var=#true help="Repeatable boolean flag"
            arg "input" help="Input file"
            arg "extra" double_dash="automatic" var=#true help="Extra files"
            cmd "run" help="Run a task" {
                example "runner run hello" header="Basic run" lang="bash"
                arg "task" help="Task to run" double_dash="automatic"
            }
            cmd "info" help="Show info" {}
        "##
        .parse()
        .unwrap();
        let output = super::super::super::generate(&spec, &make_opts());
        let client = get_file(&output, "client.ts");
        assert!(client.contains("double_dash=automatic"));
        assert!(client.contains("@example Basic run"));
        assert!(client.contains("```bash"));
        // GlobalFlags type for info subcommand
        assert!(client.contains("flags?: GlobalFlags"));
        // repeatable boolean flag
        assert!(client.contains("for (const v of flags.debug)"));
        insta::assert_snapshot!(client);
    }

    /// Args with default values — tests that defaults are preserved, not dropped to undefined.
    #[test]
    fn test_typescript_arg_defaults() {
        let spec: Spec = r##"
            bin "runner"
            arg "mode" default="fast" help="Run mode"
            arg "output" help="Output path" required=#true
        "##
        .parse()
        .unwrap();
        let output = super::super::super::generate(&spec, &make_opts());
        let types = get_file(&output, "types.ts");
        assert!(types.contains("mode?: string"));
        assert!(types.contains("output: string"));
        insta::assert_snapshot!(types);
    }

    /// Optional arg without default and empty flags interface.
    #[test]
    fn test_typescript_optional_arg_empty_flags() {
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
        let output = super::super::super::generate(&spec, &make_opts());
        let types = get_file(&output, "types.ts");
        assert!(types.contains("name?: string"));
        insta::assert_snapshot!(types);
    }

    /// Flag with choices — flag arg with choices renders correct type.
    #[test]
    fn test_typescript_flag_with_choices() {
        let spec: Spec = r##"
            bin "tool"
            flag "--shell <shell>" help="Shell type" {
                choices "bash" "zsh" "fish"
            }
        "##
        .parse()
        .unwrap();
        let output = super::super::super::generate(&spec, &make_opts());
        let types = get_file(&output, "types.ts");
        assert!(types.contains(r#""bash" | "zsh" | "fish""#));
        insta::assert_snapshot!(types);
    }

    /// Flag with env annotation — env variable appears in JSDoc.
    #[test]
    fn test_typescript_flag_with_env() {
        let spec: Spec = r##"
            bin "app"
            flag "--config <path>" help="Config file" env="APP_CONFIG"
        "##
        .parse()
        .unwrap();
        let output = super::super::super::generate(&spec, &make_opts());
        let types = get_file(&output, "types.ts");
        assert!(types.contains("APP_CONFIG"));
        insta::assert_snapshot!(types);
    }

    /// Hidden flag excluded from types and client.
    #[test]
    fn test_typescript_flag_hide() {
        let spec: Spec = r##"
            bin "app"
            flag "--verbose" help="Verbosity"
            flag "--debug" hide=#true help="Hidden debug flag"
        "##
        .parse()
        .unwrap();
        let output = super::super::super::generate(&spec, &make_opts());
        let types = get_file(&output, "types.ts");
        assert!(types.contains("verbose"));
        assert!(!types.contains("debug"));
    }

    /// Negate flag rendered in client build method.
    #[test]
    fn test_typescript_negate_flag_build() {
        let spec: Spec = r##"
            bin "app"
            flag "--dry-run" help="Dry run" negate="--no-dry-run"
        "##
        .parse()
        .unwrap();
        let output = super::super::super::generate(&spec, &make_opts());
        let client = get_file(&output, "client.ts");
        assert!(client.contains("--dry-run"));
        assert!(client.contains("--no-dry-run"));
        insta::assert_snapshot!(client);
    }

    /// Count flag rendered in client build method.
    #[test]
    fn test_typescript_count_flag_build() {
        let spec: Spec = r##"
            bin "app"
            flag "-v --verbose" count=#true help="Verbosity level"
        "##
        .parse()
        .unwrap();
        let output = super::super::super::generate(&spec, &make_opts());
        let client = get_file(&output, "client.ts");
        assert!(client.contains("-v"));
        insta::assert_snapshot!(client);
    }

    /// Repeatable value flag with default — covers var + arg + default in client build.
    #[test]
    fn test_typescript_var_value_flag_with_default() {
        let spec: Spec = r##"
            bin "tool"
            flag "--tag <t>" var=#true default="latest" help="Tags"
        "##
        .parse()
        .unwrap();
        let output = super::super::super::generate(&spec, &make_opts());
        let types = get_file(&output, "types.ts");
        assert!(types.contains("string[]"));
        let client = get_file(&output, "client.ts");
        assert!(client.contains("for (const v of flags.tag)"));
        insta::assert_snapshot!(types);
    }

    /// Flag with multiple long aliases — `-f --format --fmt <fmt>`.
    #[test]
    fn test_typescript_multiple_aliases() {
        let spec: Spec = r##"
            bin "tool"
            flag "-f --format --fmt <fmt>" help="Output format"
        "##
        .parse()
        .unwrap();
        let output = super::super::super::generate(&spec, &make_opts());
        let types = get_file(&output, "types.ts");
        assert!(types.contains("fmt"));
        let client = get_file(&output, "client.ts");
        // should use first long for the flag argument name
        assert!(client.contains("--format"));
        insta::assert_snapshot!(client);
    }

    /// Required flag without default — tests non-optional flag type in Flags interface.
    #[test]
    fn test_typescript_required_flag_type() {
        let spec: Spec = r##"
            bin "tool"
            flag "--token <t>" required=#true help="Auth token"
        "##
        .parse()
        .unwrap();
        let output = super::super::super::generate(&spec, &make_opts());
        let types = get_file(&output, "types.ts");
        // required flag without default should NOT have "?"
        assert!(types.contains("token: string;"));
        assert!(!types.contains("token?:"));
        insta::assert_snapshot!(types);
    }

    /// Global repeatable flags — covers flag_ts_simple var branches in GlobalFlags.
    #[test]
    fn test_typescript_global_repeatable_flags() {
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
        let output = super::super::super::generate(&spec, &make_opts());
        let types = get_file(&output, "types.ts");
        // GlobalFlags should have boolean[] and string[] types
        assert!(types.contains("boolean[]"));
        assert!(types.contains("string[]"));
        insta::assert_snapshot!(types);
    }

    /// Boolean flag with default=#false — covers False default in types.
    #[test]
    fn test_typescript_boolean_flag_default_false() {
        let spec: Spec = r##"
            bin "app"
            flag "--no-cache" default=#false help="Disable cache"
        "##
        .parse()
        .unwrap();
        let output = super::super::super::generate(&spec, &make_opts());
        let types = get_file(&output, "types.ts");
        assert!(types.contains("@default false"));
        insta::assert_snapshot!(types);
    }

    /// Config with all props having defaults — tests Default-friendly rendering.
    #[test]
    fn test_typescript_config_all_optional() {
        let spec: Spec = r##"
            bin "app"
            config {
                prop "debug" default=#true data_type=boolean
                prop "port" default=8080 data_type=integer
            }
        "##
        .parse()
        .unwrap();
        let output = super::super::super::generate(&spec, &make_opts());
        let types = get_file(&output, "types.ts");
        assert!(types.contains("AppConfig"));
        insta::assert_snapshot!(types);
    }

    /// Optional variadic arg — covers the optional + var branch.
    #[test]
    fn test_typescript_optional_variadic_arg() {
        let spec: Spec = r##"
            bin "tool"
            arg "[files]" var=#true help="Input files"
        "##
        .parse()
        .unwrap();
        let output = super::super::super::generate(&spec, &make_opts());
        let types = get_file(&output, "types.ts");
        assert!(types.contains("string[]"));
        let client = get_file(&output, "client.ts");
        // optional variadic arg should use spread with undefined guard
        assert!(client.contains("args.files !== undefined"));
        insta::assert_snapshot!(client);
    }

    /// Boolean config prop with default=#false — covers the false default branch.
    #[test]
    fn test_typescript_config_boolean_default_false() {
        let spec: Spec = r##"
            bin "app"
            config {
                prop "verbose" default=#false data_type=boolean help="Verbose output"
                prop "dry_run" default=#true data_type=boolean help="Dry run mode"
            }
        "##
        .parse()
        .unwrap();
        let output = super::super::super::generate(&spec, &make_opts());
        let types = get_file(&output, "types.ts");
        assert!(types.contains("verbose?: boolean"));
        assert!(types.contains("dry_run?: boolean"));
        insta::assert_snapshot!(types);
    }

    /// String config prop with default — covers string default branch.
    #[test]
    fn test_typescript_config_string_with_default() {
        let spec: Spec = r##"
            bin "app"
            config {
                prop "host" default="localhost" data_type=string help="Server host"
                prop "name" default="myapp" data_type=string
            }
        "##
        .parse()
        .unwrap();
        let output = super::super::super::generate(&spec, &make_opts());
        let types = get_file(&output, "types.ts");
        assert!(types.contains("host?: string"));
        assert!(types.contains("name?: string"));
        insta::assert_snapshot!(types);
    }

    /// Example without lang — covers example rendering without language tag.
    #[test]
    fn test_typescript_example_without_lang() {
        let spec: Spec = r##"
            bin "app"
            cmd "greet" help="Greet someone" {
                example "app greet hello"
                arg "name" help="Name to greet"
            }
        "##
        .parse()
        .unwrap();
        let output = super::super::super::generate(&spec, &make_opts());
        let client = get_file(&output, "client.ts");
        assert!(client.contains("app greet hello"));
        insta::assert_snapshot!(client);
    }

    /// Flag edge cases — short-only, deprecated, count with default, value flag with default,
    /// required flag, repeatable boolean flag.
    #[test]
    fn test_typescript_flag_edge_cases() {
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
        let output = super::super::super::generate(&spec, &make_opts());
        let types = get_file(&output, "types.ts");
        insta::assert_snapshot!(types);
        let client = get_file(&output, "client.ts");
        // short-only flag build
        assert!(client.contains(r#""-v""#));
        // repeatable boolean flag build
        assert!(client.contains("for (const v of flags.verbose)"));
        insta::assert_snapshot!(client);
    }

    /// Global flags with flags-only subcommand — covers GlobalFlags type branch.
    #[test]
    fn test_typescript_global_flags_flags_only() {
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
        let output = super::super::super::generate(&spec, &make_opts());
        let client = get_file(&output, "client.ts");
        // "info" subcommand has no own flags, only global flags => GlobalFlags type
        assert!(client.contains("GlobalFlags"));
        insta::assert_snapshot!(client);
    }

    /// double_dash=automatic — covers arg ordering and separator insertion.
    #[test]
    fn test_typescript_double_dash_automatic() {
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
        let output = super::super::super::generate(&spec, &make_opts());
        let client = get_file(&output, "client.ts");
        assert!(client.contains("double_dash=automatic"));
        insta::assert_snapshot!(client);
    }
}
