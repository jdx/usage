use std::path::PathBuf;

use heck::AsPascalCase;
use indexmap::IndexMap;

use crate::sdk::{
    collect_choice_types, collect_type_imports, command_type_name, generated_header, CodeWriter,
    SdkFile, SdkOptions, SdkOutput,
};
use crate::spec::arg::SpecDoubleDashChoices;
use crate::spec::cmd::SpecCommand;
use crate::spec::config::SpecConfigProp;
use crate::spec::data_types::SpecDataTypes;
use crate::{Spec, SpecArg, SpecFlag};

mod runtime;

pub fn generate(spec: &Spec, opts: &SdkOptions) -> SdkOutput {
    let package_name = opts
        .package_name
        .clone()
        .unwrap_or_else(|| spec.bin.clone());

    SdkOutput {
        files: vec![
            SdkFile {
                path: PathBuf::from("types.py"),
                content: render_types(spec, &package_name, &opts.source_file),
            },
            SdkFile {
                path: PathBuf::from("client.py"),
                content: render_client(spec, &package_name, &opts.source_file),
            },
            SdkFile {
                path: PathBuf::from("runtime.py"),
                content: runtime::RUNTIME_PY.to_string(),
            },
            SdkFile {
                path: PathBuf::from("__init__.py"),
                content: render_init(&package_name),
            },
        ],
    }
}

fn render_init(package_name: &str) -> String {
    let class_name = AsPascalCase(package_name).to_string();
    format!("from .client import {class_name}\nfrom .types import *\n")
}

// ---------------------------------------------------------------------------
// types.py
// ---------------------------------------------------------------------------

fn render_types(spec: &Spec, package_name: &str, source_file: &Option<String>) -> String {
    let mut w = CodeWriter::with_indent("    ");

    w.line(&generated_header("#", source_file));
    w.line("from __future__ import annotations");
    w.line("from dataclasses import dataclass");
    w.line("from typing import Literal, Optional, Union");
    w.line("");

    // spec metadata
    if let Some(version) = &spec.version {
        w.line(&format!("VERSION = \"{version}\""));
    }
    if let Some(about) = &spec.about {
        w.line(&format!("ABOUT = \"{about}\""));
    }
    if let Some(author) = &spec.author {
        w.line(&format!("AUTHOR = \"{author}\""));
    }

    let choice_types = collect_choice_types(&spec.cmd);
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
                .join(", ");
            w.line(&format!("{name} = Literal[{union}]"));
        }
    }

    if has_global_flags {
        w.line("");
        render_flags_dataclass("GlobalFlags", &root_global_flags, &choice_types, &mut w);
    }

    render_command_types(
        &spec.cmd,
        package_name,
        &choice_types,
        has_global_flags,
        &root_global_flags,
        &mut w,
    );

    if !spec.config.props.is_empty() {
        w.line("");
        let config_name = format!("{}Config", AsPascalCase(package_name));
        w.line("");
        w.line("@dataclass");
        w.line(&format!("class {config_name}:"));
        w.indent();
        // Python dataclass requires non-default fields before default fields
        let (required, optional): (Vec<_>, Vec<_>) = spec
            .config
            .props
            .iter()
            .partition(|(_, p)| p.default.is_none());

        for (name, prop) in required.iter().chain(optional.iter()) {
            let py_type = config_prop_type(prop);
            let default = if let Some(d) = &prop.default {
                match prop.data_type {
                    SpecDataTypes::Boolean => format!(" = {d}"),
                    SpecDataTypes::Integer | SpecDataTypes::Float => format!(" = {d}"),
                    _ => format!(" = \"{d}\""),
                }
            } else {
                String::new()
            };
            if let Some(help) = &prop.help {
                w.line(&format!("# {help}"));
            }
            w.line(&format!("{name}: {py_type}{default}"));
        }
        w.dedent();
    }

    w.finish()
}

fn render_command_types(
    cmd: &SpecCommand,
    package_name: &str,
    choice_types: &IndexMap<String, Vec<String>>,
    has_global_flags: bool,
    global_flags: &[&SpecFlag],
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
        render_args_dataclass(&format!("{name}Args"), &visible_args, choice_types, w);
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
        render_flags_dataclass(&format!("{name}Flags"), &all_flags, choice_types, w);
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

fn render_args_dataclass(
    name: &str,
    args: &[&SpecArg],
    choice_types: &IndexMap<String, Vec<String>>,
    w: &mut CodeWriter,
) {
    w.line("");
    w.line("@dataclass");
    w.line(&format!("class {name}:"));
    w.indent();
    if args.is_empty() {
        w.line("pass");
    } else {
        // Python dataclass requires non-default fields before default fields
        let (required, optional): (Vec<_>, Vec<_>) = args
            .iter()
            .copied()
            .partition(|a| a.required && a.default.is_empty());

        for arg in required.iter().chain(optional.iter()) {
            let py_type = arg_py_type(arg, choice_types);
            let optional = !(arg.required && arg.default.is_empty());
            let field = if optional {
                format!(
                    "{}: Optional[{}] = None",
                    sanitize_py_ident(&arg.name),
                    py_type
                )
            } else if !arg.default.is_empty() {
                let default_val = &arg.default[0];
                format!(
                    "{}: {} = \"{default_val}\"",
                    sanitize_py_ident(&arg.name),
                    py_type
                )
            } else {
                format!("{}: {}", sanitize_py_ident(&arg.name), py_type)
            };
            if let Some(help) = &arg.help {
                w.line(&format!("# {help}"));
            }
            w.line(&field);
        }
    }
    w.dedent();
}

fn render_flags_dataclass(
    name: &str,
    flags: &[&SpecFlag],
    choice_types: &IndexMap<String, Vec<String>>,
    w: &mut CodeWriter,
) {
    w.line("");
    w.line("@dataclass");
    w.line(&format!("class {name}:"));
    w.indent();
    if flags.is_empty() {
        w.line("pass");
    } else {
        // Python dataclass requires non-default fields before default fields
        let (required, optional): (Vec<_>, Vec<_>) = flags
            .iter()
            .copied()
            .partition(|f| f.required && f.default.is_empty());

        for flag in required.iter().chain(optional.iter()) {
            let py_type = flag_py_type(flag, choice_types);
            let prop_name = flag_property_name_py(flag);
            let optional = !(flag.required && flag.default.is_empty());
            let field = if !flag.default.is_empty() {
                // has explicit default — use first value
                let default_val = &flag.default[0];
                if flag.count {
                    format!("{prop_name}: {py_type} = {default_val}")
                } else if flag.arg.is_none() {
                    // boolean with default
                    match default_val.as_str() {
                        "true" | "#true" => format!("{prop_name}: {py_type} = True"),
                        "false" | "#false" => format!("{prop_name}: {py_type} = False"),
                        _ => {
                            // unrecognized boolean default — cannot emit as valid Python bool;
                            // fall back to Optional[bool] = None with a comment
                            format!("{prop_name}: Optional[bool] = None  # default: {default_val}")
                        }
                    }
                } else {
                    format!("{prop_name}: Optional[{py_type}] = \"{default_val}\"")
                }
            } else if optional {
                format!("{prop_name}: Optional[{py_type}] = None")
            } else {
                format!("{prop_name}: {py_type}")
            };
            let mut doc_parts = Vec::new();
            if let Some(help) = &flag.help {
                doc_parts.push(help.clone());
            }
            if let Some(env) = &flag.env {
                doc_parts.push(format!("Env: {env}"));
            }
            if let Some(deprecated) = &flag.deprecated {
                doc_parts.push(format!("Deprecated: {deprecated}"));
            }
            if flag.long.len() > 1 {
                let aliases: Vec<&str> = flag.long.iter().skip(1).map(|s| s.as_str()).collect();
                doc_parts.push(format!("Aliases: {}", aliases.join(", ")));
            }
            if !flag.short.is_empty() {
                let shorts: Vec<String> = flag.short.iter().map(|c| format!("-{c}")).collect();
                doc_parts.push(format!("Short: {}", shorts.join(", ")));
            }
            if !doc_parts.is_empty() {
                w.line(&format!("# {}", doc_parts.join(". ")));
            }
            w.line(&field);
        }
    }
    w.dedent();
}

fn arg_py_type(arg: &SpecArg, choice_types: &IndexMap<String, Vec<String>>) -> String {
    let base = if let Some(choices) = &arg.choices {
        let type_name = format!("{}Choice", AsPascalCase(&arg.name));
        if choice_types.contains_key(&type_name) {
            type_name
        } else {
            let union = choices
                .choices
                .iter()
                .map(|c| format!("\"{c}\""))
                .collect::<Vec<_>>()
                .join(", ");
            format!("Literal[{union}]")
        }
    } else {
        "str".to_string()
    };

    if arg.var {
        format!("list[{base}]")
    } else {
        base
    }
}

fn flag_py_type(flag: &SpecFlag, choice_types: &IndexMap<String, Vec<String>>) -> String {
    if flag.count {
        return "int".to_string();
    }

    match &flag.arg {
        Some(arg) => {
            let base = if let Some(choices) = &arg.choices {
                let type_name = format!("{}Choice", AsPascalCase(&flag.name));
                if choice_types.contains_key(&type_name) {
                    type_name
                } else {
                    let union = choices
                        .choices
                        .iter()
                        .map(|c| format!("\"{c}\""))
                        .collect::<Vec<_>>()
                        .join(", ");
                    format!("Literal[{union}]")
                }
            } else {
                "str".to_string()
            };

            if flag.var {
                format!("list[{base}]")
            } else {
                base
            }
        }
        None => "bool".to_string(),
    }
}

fn config_prop_type(prop: &SpecConfigProp) -> String {
    match prop.data_type {
        SpecDataTypes::String => "str".to_string(),
        SpecDataTypes::Integer => "int".to_string(),
        SpecDataTypes::Float => "float".to_string(),
        SpecDataTypes::Boolean => "bool".to_string(),
        SpecDataTypes::Null => "object".to_string(),
    }
}

fn flag_property_name_py(flag: &SpecFlag) -> String {
    // Python uses snake_case for attributes
    if let Some(long) = flag.long.first() {
        return sanitize_py_ident(&heck::AsSnakeCase(long).to_string());
    }
    if let Some(short) = flag.short.first() {
        return short.to_string();
    }
    sanitize_py_ident(&flag.name)
}

fn sanitize_py_ident(name: &str) -> String {
    let snake = heck::AsSnakeCase(name).to_string();
    match snake.as_str() {
        "class" | "def" | "return" | "import" | "from" | "global" | "lambda" | "pass" | "raise"
        | "with" | "yield" | "del" | "try" | "except" | "finally" | "while" | "for" | "if"
        | "elif" | "else" | "and" | "or" | "not" | "in" | "is" | "as" | "break" | "continue"
        | "assert" | "type" | "input" | "id" | "list" | "dict" | "set" | "print" | "range"
        | "format" | "help" | "vars" | "dir" | "exec" | "exit" | "quit" | "bool" | "int"
        | "str" | "float" | "bytes" | "object" | "super" | "property" | "static" | "True"
        | "False" | "None" => format!("_{snake}"),
        _ => snake,
    }
}

// ---------------------------------------------------------------------------
// client.py
// ---------------------------------------------------------------------------

fn render_client(spec: &Spec, package_name: &str, source_file: &Option<String>) -> String {
    let mut w = CodeWriter::with_indent("    ");

    w.line(&generated_header("#", source_file));
    w.line("from __future__ import annotations");
    w.line("from typing import Optional");
    w.line("from .runtime import CliResult, CliRunner");

    // collect imports from types
    let type_imports = collect_type_imports(&spec.cmd, package_name);
    let has_global_flags = spec.cmd.flags.iter().any(|f| f.global && !f.hide);
    let mut all_imports = type_imports;
    if has_global_flags {
        all_imports.push("GlobalFlags".to_string());
    }
    all_imports.sort();
    all_imports.dedup();
    if !all_imports.is_empty() {
        w.line(&format!("from .types import {}", all_imports.join(", ")));
    }

    w.line("");

    let global_flags: Vec<&SpecFlag> = spec
        .cmd
        .flags
        .iter()
        .filter(|f| f.global && !f.hide)
        .collect();

    let class_name = AsPascalCase(package_name).to_string();
    render_class(
        &spec.cmd,
        &class_name,
        true,
        &global_flags,
        &spec.bin,
        &mut w,
    );

    w.finish()
}

fn render_class(
    cmd: &SpecCommand,
    class_name: &str,
    is_root: bool,
    global_flags: &[&SpecFlag],
    bin_name: &str,
    w: &mut CodeWriter,
) {
    let visible_subcmds: Vec<_> = cmd.subcommands.iter().filter(|(_, c)| !c.hide).collect();

    let visible_args: Vec<&SpecArg> = cmd.args.iter().filter(|a| !a.hide).collect();
    let visible_flags: Vec<&SpecFlag> = cmd.flags.iter().filter(|f| !f.hide).collect();
    let has_args = !visible_args.is_empty();
    let has_flags = !visible_flags.is_empty() || !global_flags.is_empty();

    // docstring on class
    let mut class_doc = Vec::new();
    if let Some(help) = &cmd.help {
        class_doc.push(help.clone());
    } else if let Some(about) = &cmd.help_long {
        class_doc.push(about.clone());
    }
    if let Some(deprecated) = &cmd.deprecated {
        class_doc.push(format!("DEPRECATED: {deprecated}"));
    }
    if !cmd.aliases.is_empty() {
        class_doc.push(format!("Aliases: {}", cmd.aliases.join(", ")));
    }

    w.line(&format!("class {class_name}:"));
    w.indent();

    if !class_doc.is_empty() {
        w.line(&format!("\"\"\"{}\"\"\"", class_doc.join(". ")));
    }

    // constructor
    if is_root {
        w.line(&format!(
            "def __init__(self, bin_path: str = \"{bin_name}\") -> None:"
        ));
    } else {
        w.line("def __init__(self, runner: CliRunner) -> None:");
    }
    w.indent();
    if is_root {
        w.line("self._runner = CliRunner(bin_path)");
    } else {
        w.line("self._runner = runner");
    }
    for (name, _) in &visible_subcmds {
        let sub_class = AsPascalCase(name).to_string();
        let prop = sanitize_py_ident(name);
        w.line(&format!("self.{prop} = {sub_class}(self._runner)"));
    }
    w.dedent();

    // exec method
    let args_param = if has_args {
        format!("args: {class_name}Args")
    } else {
        String::new()
    };
    let flags_type = if !global_flags.is_empty() && !visible_flags.is_empty() {
        format!("{class_name}Flags")
    } else if !global_flags.is_empty() && visible_flags.is_empty() {
        "GlobalFlags".to_string()
    } else if !visible_flags.is_empty() {
        format!("{class_name}Flags")
    } else {
        String::new()
    };
    let flags_param = if !flags_type.is_empty() {
        format!(", flags: Optional[{flags_type}] = None")
    } else {
        String::new()
    };
    let sig = if args_param.is_empty() && flags_param.is_empty() {
        "def exec(self) -> CliResult:".to_string()
    } else {
        format!("def exec(self, {args_param}{flags_param}) -> CliResult:")
    };

    // docstring on exec
    let mut exec_doc = Vec::new();
    if !cmd.usage.is_empty() {
        exec_doc.push(cmd.usage.clone());
    }
    for example in &cmd.examples {
        let label = example.header.as_deref().unwrap_or("Example");
        exec_doc.push(format!("{label}: {code}", code = example.code));
    }

    w.line("");
    if !exec_doc.is_empty() {
        w.line(&sig);
        w.indent();
        w.line(&format!("\"\"\"{}\"\"\"", exec_doc.join("\\n")));
    } else {
        w.line(&sig);
        w.indent();
    }

    let path: String = cmd
        .full_cmd
        .iter()
        .map(|s| format!("\"{s}\""))
        .collect::<Vec<_>>()
        .join(", ");
    w.line(&format!("cmd_args: list[str] = [{path}]"));

    if has_args {
        let has_required_double_dash = visible_args
            .iter()
            .any(|a| matches!(a.double_dash, SpecDoubleDashChoices::Required));
        let has_automatic_double_dash = visible_args
            .iter()
            .any(|a| matches!(a.double_dash, SpecDoubleDashChoices::Automatic));

        for arg in &visible_args {
            let ident = sanitize_py_ident(&arg.name);
            if arg.var {
                w.line(&format!(
                    "if args.{ident} is not None: cmd_args.extend(args.{ident})"
                ));
            } else {
                w.line(&format!(
                    "if args.{ident} is not None: cmd_args.append(str(args.{ident}))"
                ));
            }
        }

        if has_required_double_dash {
            w.line("cmd_args.append(\"--\")");
        } else if has_automatic_double_dash {
            w.line("# double_dash=automatic: \"--\" is implied after the first positional arg");
        }
    }

    if has_flags {
        w.line("flag_args = self._build_flag_args(flags)");
        w.line("return self._runner.run(cmd_args + flag_args)");
    } else {
        w.line("return self._runner.run(cmd_args)");
    }

    w.dedent();

    // _build_flag_args
    if has_flags {
        w.line("");
        w.line(&format!(
            "def _build_flag_args(self, flags: Optional[{flags_type}]) -> list[str]:"
        ));
        w.indent();
        w.line("result: list[str] = []");
        w.line("if flags is None: return result");

        for flag in global_flags {
            render_flag_build_py(flag, w);
        }
        for flag in &visible_flags {
            // skip global flags already rendered above
            if !global_flags.iter().any(|gf| gf.name == flag.name) {
                render_flag_build_py(flag, w);
            }
        }

        w.line("return result");
        w.dedent();
    }

    // alias properties for subcommand aliases
    for (name, subcmd) in &visible_subcmds {
        for alias in &subcmd.aliases {
            let alias_prop = sanitize_py_ident(alias);
            let target_prop = sanitize_py_ident(name);
            let sub_class = AsPascalCase(name).to_string();
            w.line("");
            w.line("@property");
            w.line(&format!("def {alias_prop}(self) -> {sub_class}:"));
            w.indent();
            w.line(&format!("\"\"\"Alias for {name}.\"\"\""));
            w.line(&format!("return self.{target_prop}"));
            w.dedent();
        }
    }

    w.dedent(); // end class

    // render subcommand classes
    for (name, subcmd) in &visible_subcmds {
        w.line("");
        let sub_class = AsPascalCase(name).to_string();
        render_class(subcmd, &sub_class, false, global_flags, bin_name, w);
    }
}

fn render_flag_build_py(flag: &SpecFlag, w: &mut CodeWriter) {
    let prop_name = flag_property_name_py(flag);
    let flag_arg_name = if let Some(long) = flag.long.first() {
        format!("--{long}")
    } else if let Some(short) = flag.short.first() {
        format!("-{short}")
    } else {
        format!("--{}", flag.name)
    };

    if flag.arg.is_some() {
        if flag.var {
            w.line(&format!("if flags.{prop_name} is not None:"));
            w.indent();
            w.line(&format!(
                "for v in flags.{prop_name}: result.extend([\"{flag_arg_name}\", str(v)])"
            ));
            w.dedent();
        } else {
            w.line(&format!(
                "if flags.{prop_name} is not None: result.extend([\"{flag_arg_name}\", str(flags.{prop_name})])"
            ));
        }
    } else if flag.count {
        w.line(&format!(
            "if flags.{prop_name} is not None and flags.{prop_name} > 0: result.extend([\"{flag_arg_name}\"] * flags.{prop_name})"
        ));
    } else if flag.var {
        w.line(&format!("if flags.{prop_name} is not None:"));
        w.indent();
        w.line(&format!("for v in flags.{prop_name}:"));
        w.indent();
        w.line(&format!("if v: result.append(\"{flag_arg_name}\")"));
        w.dedent();
        w.dedent();
    } else {
        w.line(&format!(
            "if flags.{prop_name}: result.append(\"{flag_arg_name}\")"
        ));
        if let Some(negate) = &flag.negate {
            w.line(&format!(
                "elif flags.{prop_name} is False: result.append(\"{negate}\")"
            ));
        }
    }
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
            language: SdkLanguage::Python,
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
    fn test_python_types() {
        let output = crate::sdk::generate(&SPEC_KITCHEN_SINK, &make_opts());
        insta::assert_snapshot!(get_file(&output, "types.py"));
    }

    #[test]
    fn test_python_client() {
        let output = crate::sdk::generate(&SPEC_KITCHEN_SINK, &make_opts());
        insta::assert_snapshot!(get_file(&output, "client.py"));
    }

    #[test]
    fn test_python_runtime() {
        let output = crate::sdk::generate(&SPEC_KITCHEN_SINK, &make_opts());
        insta::assert_snapshot!(get_file(&output, "runtime.py"));
    }

    #[test]
    fn test_python_init() {
        let output = crate::sdk::generate(&SPEC_KITCHEN_SINK, &make_opts());
        insta::assert_snapshot!(get_file(&output, "__init__.py"));
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
    fn test_python_full_feature_types() {
        let spec = full_feature_spec();
        let output = crate::sdk::generate(&spec, &make_opts());
        insta::assert_snapshot!(get_file(&output, "types.py"));
    }

    #[test]
    fn test_python_full_feature_client() {
        let spec = full_feature_spec();
        let output = crate::sdk::generate(&spec, &make_opts());
        insta::assert_snapshot!(get_file(&output, "client.py"));
    }

    #[test]
    fn test_python_hyphenated_subcommands() {
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
        insta::assert_snapshot!(get_file(&output, "client.py"));
    }

    #[test]
    fn test_python_minimal() {
        let spec: Spec = r##"
            bin "hello"
        "##
        .parse()
        .unwrap();
        let output = crate::sdk::generate(&spec, &make_opts());
        insta::assert_snapshot!(get_file(&output, "client.py"));
    }
}
