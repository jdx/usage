use heck::AsPascalCase;
use indexmap::IndexMap;

use crate::spec::cmd::SpecCommand;
use crate::spec::config::SpecConfigProp;
use crate::spec::data_types::SpecDataTypes;
use crate::{Spec, SpecArg, SpecFlag};

use crate::sdk::{collect_choice_types, command_type_name, generated_header, CodeWriter};

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
        for (name, choices) in &choice_types {
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

    render_command_types(&spec.cmd, package_name, &choice_types, has_global_flags, &mut w);

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

    w.to_string()
}

fn render_command_types(
    cmd: &SpecCommand,
    package_name: &str,
    choice_types: &IndexMap<String, Vec<String>>,
    has_global_flags: bool,
    w: &mut CodeWriter,
) {
    if cmd.hide {
        return;
    }

    let name = command_type_name(cmd, package_name);

    let visible_args: Vec<&SpecArg> = cmd.args.iter().filter(|a| !a.hide).collect();
    let visible_flags: Vec<&SpecFlag> = cmd.flags.iter().filter(|f| !f.hide).collect();
    let has_any_flags = !visible_flags.is_empty() || has_global_flags;

    if !visible_args.is_empty() {
        w.line("");
        w.line(&format!("export interface {name}Args {{"));
        w.indent();
        for arg in &visible_args {
            render_arg_field(arg, choice_types, w);
        }
        w.dedent();
        w.line("}");
    }

    if has_any_flags {
        w.line("");
        if has_global_flags {
            w.line(&format!("export interface {name}Flags extends GlobalFlags {{"));
        } else {
            w.line(&format!("export interface {name}Flags {{"));
        }
        w.indent();
        for flag in &visible_flags {
            render_flag_field(flag, choice_types, w);
        }
        w.dedent();
        w.line("}");
    }

    for subcmd in cmd.subcommands.values() {
        render_command_types(subcmd, package_name, choice_types, has_global_flags, w);
    }
}

fn render_arg_field(arg: &SpecArg, choice_types: &IndexMap<String, Vec<String>>, w: &mut CodeWriter) {
    let ts_type = arg_ts_type(arg, choice_types);
    let optional = if arg.required && arg.default.is_none() {
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
    w.line(&format!("{}{optional}: {ts_type};", sanitize_ident(&arg.name)));
}

fn render_flag_field(flag: &SpecFlag, choice_types: &IndexMap<String, Vec<String>>, w: &mut CodeWriter) {
    let ts_type = flag_ts_type(flag, choice_types);
    let optional = if flag.required && flag.default.is_none() {
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
    if let Some(default) = &flag.default {
        doc_parts.push(format!("@default {default}"));
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

fn arg_ts_type(arg: &SpecArg, choice_types: &IndexMap<String, Vec<String>>) -> String {
    let base = if let Some(choices) = &arg.choices {
        let type_name = format!("{}Choice", AsPascalCase(&arg.name));
        if choice_types.contains_key(&type_name) {
            type_name
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

fn flag_ts_type(flag: &SpecFlag, choice_types: &IndexMap<String, Vec<String>>) -> String {
    if flag.count {
        return "number".to_string();
    }

    match &flag.arg {
        Some(arg) => {
            let base = if let Some(choices) = &arg.choices {
                let type_name = format!("{}Choice", AsPascalCase(&flag.name));
                if choice_types.contains_key(&type_name) {
                    type_name
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
        None => "boolean".to_string(),
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
        None => "boolean".to_string(),
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
        "##.parse().unwrap();
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
        "##.parse().unwrap();
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
        "##.parse().unwrap();
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
        "##.parse().unwrap();
        let output = super::super::super::generate(&spec, &make_opts());
        insta::assert_snapshot!(get_file(&output, "client.ts"));
    }

    /// Minimal spec with no args, no flags, no subcommands.
    #[test]
    fn test_minimal_spec() {
        let spec: Spec = r##"
            bin "hello"
        "##.parse().unwrap();
        let output = super::super::super::generate(&spec, &make_opts());
        insta::assert_snapshot!(get_file(&output, "client.ts"));
    }

    /// Test package_name override.
    #[test]
    fn test_package_name_override() {
        let spec: Spec = r##"
            bin "original-cli"
        "##.parse().unwrap();
        let opts = SdkOptions {
            language: SdkLanguage::TypeScript,
            package_name: Some("MyCustomSdk".to_string()),
            source_file: None,
        };
        let output = super::super::super::generate(&spec, &opts);
        insta::assert_snapshot!(get_file(&output, "index.ts"));
    }
}
