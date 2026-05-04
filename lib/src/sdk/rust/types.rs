use heck::AsPascalCase;

use crate::sdk::{
    collect_choice_types, command_type_name, generated_header, ChoiceTypeMap, CodeWriter,
};
use crate::spec::cmd::SpecCommand;
use crate::spec::config::SpecConfigProp;
use crate::spec::data_types::SpecDataTypes;
use crate::{SpecArg, SpecFlag};

pub fn render(spec: &crate::Spec, package_name: &str, source_file: &Option<String>) -> String {
    let mut w = CodeWriter::with_indent("    ");

    w.line(&generated_header("//!", source_file));

    let choice_types = collect_choice_types(&spec.cmd);
    if !choice_types.is_empty() {
        w.line("");
        w.line("use std::fmt;");
    }
    w.line("");

    let root_global_flags: Vec<&SpecFlag> = spec
        .cmd
        .flags
        .iter()
        .filter(|f| f.global && !f.hide)
        .collect();
    let has_global_flags = !root_global_flags.is_empty();

    // spec metadata
    if let Some(version) = &spec.version {
        w.line(&format!("pub const VERSION: &str = \"{version}\";"));
    }
    if let Some(about) = &spec.about {
        w.line(&format!("pub const ABOUT: &str = \"{about}\";"));
    }
    if let Some(author) = &spec.author {
        w.line(&format!("pub const AUTHOR: &str = \"{author}\";"));
    }

    // choice enums
    if !choice_types.is_empty() {
        w.line("");
        for (i, (name, choices)) in choice_types.iter().enumerate() {
            if i > 0 {
                w.line("");
            }
            render_choice_enum(name, choices, &mut w);
        }
    }

    // GlobalFlags
    if has_global_flags {
        w.line("");
        render_flags_struct("GlobalFlags", "", &root_global_flags, &choice_types, &mut w);
    }

    // command types
    render_command_types(
        &spec.cmd,
        package_name,
        &choice_types,
        has_global_flags,
        &root_global_flags,
        &mut w,
    );

    // Config
    if !spec.config.props.is_empty() {
        w.line("");
        let config_name = format!("{}Config", AsPascalCase(package_name));
        render_config_struct(&config_name, &spec.config.props, &mut w);
    }

    w.finish()
}

// ---------------------------------------------------------------------------
// Choice enums
// ---------------------------------------------------------------------------

fn render_choice_enum(name: &str, choices: &[String], w: &mut CodeWriter) {
    w.line("#[derive(Debug, Clone, PartialEq)]");
    w.line(&format!("pub enum {name} {{"));
    w.indent();
    for choice in choices {
        let variant = sanitize_enum_variant(choice);
        w.line(&format!("{variant},"));
    }
    w.dedent();
    w.line("}");

    // Display impl
    w.line("");
    w.line(&format!("impl fmt::Display for {name} {{"));
    w.indent();
    w.line("fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {");
    w.indent();
    w.line("match self {");
    w.indent();
    for choice in choices {
        let variant = sanitize_enum_variant(choice);
        w.line(&format!("Self::{variant} => write!(f, \"{choice}\"),"));
    }
    w.dedent();
    w.line("}");
    w.dedent();
    w.line("}");
    w.dedent();
    w.line("}");

    // as_str method
    w.line("");
    w.line(&format!("impl {name} {{"));
    w.indent();
    w.line("pub fn as_str(&self) -> &'static str {");
    w.indent();
    w.line("match self {");
    w.indent();
    for choice in choices {
        let variant = sanitize_enum_variant(choice);
        w.line(&format!("Self::{variant} => \"{choice}\","));
    }
    w.dedent();
    w.line("}");
    w.dedent();
    w.line("}");
    w.dedent();
    w.line("}");
}

fn sanitize_enum_variant(choice: &str) -> String {
    let pascal = AsPascalCase(choice).to_string();
    if pascal.starts_with(|c: char| c.is_ascii_digit()) {
        format!("V{pascal}")
    } else if is_rs_reserved(&pascal) {
        format!("_{pascal}")
    } else {
        pascal
    }
}

fn is_rs_reserved(s: &str) -> bool {
    matches!(
        s,
        "Self"
            | "Super"
            | "Box"
            | "Crate"
            | "Dyn"
            | "Extern"
            | "Fn"
            | "Impl"
            | "Mod"
            | "Mut"
            | "Pub"
            | "Ref"
            | "Static"
            | "Trait"
            | "Type"
            | "Use"
            | "Where"
            | "Yield"
            | "Async"
            | "Await"
            | "Unsafe"
            | "Abstract"
            | "Override"
            | "Macro"
    )
}

// ---------------------------------------------------------------------------
// Command types
// ---------------------------------------------------------------------------

fn render_command_types(
    cmd: &SpecCommand,
    package_name: &str,
    choice_types: &ChoiceTypeMap,
    has_global_flags: bool,
    global_flags: &[&SpecFlag],
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
        render_args_struct(
            &format!("{name}Args"),
            cmd_name,
            &visible_args,
            choice_types,
            w,
        );
    }

    if has_any_flags {
        w.line("");
        let all_flags: Vec<&SpecFlag> = if has_global_flags {
            global_flags
                .iter()
                .copied()
                .chain(
                    visible_flags
                        .iter()
                        .filter(|f| !global_flags.iter().any(|gf| gf.name == f.name))
                        .copied(),
                )
                .collect()
        } else {
            visible_flags
        };
        render_flags_struct(
            &format!("{name}Flags"),
            cmd_name,
            &all_flags,
            choice_types,
            w,
        );
    }

    for subcmd in cmd.subcommands.values() {
        render_command_types(
            subcmd,
            package_name,
            choice_types,
            has_global_flags,
            global_flags,
            w,
        );
    }
}

fn render_args_struct(
    name: &str,
    cmd_name: &str,
    args: &[&SpecArg],
    choice_types: &ChoiceTypeMap,
    w: &mut CodeWriter,
) {
    let all_optional = args.iter().all(|a| !a.required || !a.default.is_empty());
    let derives = if all_optional {
        "#[derive(Debug, Clone, Default)]"
    } else {
        "#[derive(Debug, Clone)]"
    };
    w.line(derives);
    w.line(&format!("pub struct {name} {{"));
    w.indent();
    for arg in args {
        let rs_type = arg_rs_type(arg, cmd_name, choice_types);
        let field_name = sanitize_rs_ident(&heck::AsSnakeCase(&arg.name).to_string());
        let optional = !(arg.required && arg.default.is_empty());

        let field = if optional {
            format!("pub {field_name}: Option<{rs_type}>,")
        } else {
            format!("pub {field_name}: {rs_type},")
        };

        if let Some(help) = &arg.help {
            w.line(&format!("/// {help}"));
        }
        if !arg.default.is_empty() {
            w.line(&format!("/// Default: \"{}\"", arg.default.join(", ")));
        }
        w.line(&field);
    }
    w.dedent();
    w.line("}");
}

fn render_flags_struct(
    name: &str,
    cmd_name: &str,
    flags: &[&SpecFlag],
    choice_types: &ChoiceTypeMap,
    w: &mut CodeWriter,
) {
    let all_optional = flags.iter().all(|f| !(f.required && f.default.is_empty()));
    let derives = if all_optional {
        "#[derive(Debug, Clone, Default)]"
    } else {
        "#[derive(Debug, Clone)]"
    };
    w.line(derives);
    w.line(&format!("pub struct {name} {{"));
    w.indent();
    for flag in flags {
        let rs_type = flag_rs_type(flag, cmd_name, choice_types);
        let prop_name = flag_property_name_rs(flag);
        let optional = !(flag.required && flag.default.is_empty());
        let field = if optional {
            format!("pub {prop_name}: Option<{rs_type}>,")
        } else {
            format!("pub {prop_name}: {rs_type},")
        };
        render_flag_docs(flag, w);
        w.line(&field);
    }
    w.dedent();
    w.line("}");
}

fn render_config_struct(
    name: &str,
    props: &std::collections::BTreeMap<String, SpecConfigProp>,
    w: &mut CodeWriter,
) {
    let all_optional = props.iter().all(|(_, p)| p.default.is_some());
    let derives = if all_optional {
        "#[derive(Debug, Clone, Default)]"
    } else {
        "#[derive(Debug, Clone)]"
    };
    w.line(derives);
    w.line(&format!("pub struct {name} {{"));
    w.indent();
    for (field_name, prop) in props {
        let rs_type = config_prop_type(prop);
        let field = if prop.default.is_some() {
            format!("pub {field_name}: Option<{rs_type}>,")
        } else {
            format!("pub {field_name}: {rs_type},")
        };
        if let Some(help) = &prop.help {
            w.line(&format!("/// {help}"));
        }
        if let Some(default) = &prop.default {
            w.line(&format!("/// Default: {default}"));
        }
        w.line(&field);
    }
    w.dedent();
    w.line("}");
}

fn render_flag_docs(flag: &SpecFlag, w: &mut CodeWriter) {
    let mut doc_parts = Vec::new();
    if let Some(help) = &flag.help {
        doc_parts.push(help.clone());
    }
    if let Some(env) = &flag.env {
        doc_parts.push(format!("Env: {env}"));
    }
    if !flag.default.is_empty() {
        doc_parts.push(format!("Default: {}", flag.default.join(", ")));
    }
    // aliases in doc
    if flag.long.len() > 1 {
        let aliases: Vec<&str> = flag.long.iter().skip(1).map(|s| s.as_str()).collect();
        doc_parts.push(format!("Aliases: {}", aliases.join(", ")));
    }
    if !doc_parts.is_empty() {
        w.line(&format!("/// {}", doc_parts.join(". ")));
    }
    if let Some(deprecated) = &flag.deprecated {
        w.line(&format!("#[deprecated = \"{deprecated}\"]"));
    }
}

// ---------------------------------------------------------------------------
// Type mapping
// ---------------------------------------------------------------------------

fn arg_rs_type(arg: &SpecArg, cmd_name: &str, choice_types: &ChoiceTypeMap) -> String {
    let base = if arg.choices.is_some() {
        if let Some(resolved) = choice_types.lookup(cmd_name, &arg.name) {
            resolved.to_string()
        } else {
            "String".to_string()
        }
    } else {
        "String".to_string()
    };

    if arg.var {
        format!("Vec<{base}>")
    } else {
        base
    }
}

fn flag_rs_type(flag: &SpecFlag, cmd_name: &str, choice_types: &ChoiceTypeMap) -> String {
    if flag.count {
        return "i32".to_string();
    }

    match &flag.arg {
        Some(arg) => {
            let base = if arg.choices.is_some() {
                if let Some(resolved) = choice_types.lookup(cmd_name, &flag.name) {
                    resolved.to_string()
                } else {
                    "String".to_string()
                }
            } else {
                "String".to_string()
            };

            if flag.var {
                format!("Vec<{base}>")
            } else {
                base
            }
        }
        None => {
            if flag.var {
                "Vec<bool>".to_string()
            } else {
                "bool".to_string()
            }
        }
    }
}

fn config_prop_type(prop: &SpecConfigProp) -> String {
    match prop.data_type {
        SpecDataTypes::String => "String".to_string(),
        SpecDataTypes::Integer => "i64".to_string(),
        SpecDataTypes::Float => "f64".to_string(),
        SpecDataTypes::Boolean => "bool".to_string(),
        SpecDataTypes::Null => "String".to_string(),
    }
}

// ---------------------------------------------------------------------------
// Naming
// ---------------------------------------------------------------------------

pub fn flag_property_name_rs(flag: &SpecFlag) -> String {
    if let Some(long) = flag.long.first() {
        return sanitize_rs_ident(&heck::AsSnakeCase(long).to_string());
    }
    if let Some(short) = flag.short.first() {
        return short.to_string();
    }
    sanitize_rs_ident(&heck::AsSnakeCase(&flag.name).to_string())
}

pub fn sanitize_rs_ident(name: &str) -> String {
    let snake = heck::AsSnakeCase(name).to_string();
    match snake.as_str() {
        "type" | "self" | "super" | "mod" | "use" | "fn" | "let" | "mut" | "pub" | "impl"
        | "trait" | "struct" | "enum" | "match" | "if" | "else" | "for" | "while" | "loop"
        | "return" | "break" | "continue" | "where" | "as" | "in" | "ref" | "move" | "async"
        | "await" | "unsafe" | "static" | "const" | "dyn" | "true" | "false" | "crate"
        | "extern" | "default" | "macro" | "yield" | "box" | "override" | "abstract" => {
            format!("_{snake}")
        }
        _ => snake,
    }
}
