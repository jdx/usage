use std::path::PathBuf;

use heck::AsPascalCase;

use crate::sdk::{
    collect_choice_types, collect_type_imports, command_type_name, escape_py_docstring,
    generated_header, ChoiceTypeMap, CodeWriter, SdkFile, SdkOptions, SdkOutput,
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
    format!("from .client import {class_name}\nfrom .runtime import CliResult, CliRunner\nfrom .types import *\n")
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
        for (name, choices) in choice_types.iter() {
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
        render_flags_dataclass("GlobalFlags", "", &root_global_flags, &choice_types, &mut w);
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
                    SpecDataTypes::Boolean => match d.as_str() {
                        "#true" | "true" => " = True".to_string(),
                        "#false" | "false" => " = False".to_string(),
                        other => format!(" = {other}"),
                    },
                    SpecDataTypes::Integer | SpecDataTypes::Float => {
                        let numeric = d.trim_matches('"');
                        format!(" = {numeric}")
                    }
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
        render_args_dataclass(
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
        render_flags_dataclass(
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

fn render_args_dataclass(
    name: &str,
    cmd_name: &str,
    args: &[&SpecArg],
    choice_types: &ChoiceTypeMap,
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
            let py_type = arg_py_type(arg, cmd_name, choice_types);
            let is_required_no_default = arg.required && arg.default.is_empty();
            let field = if !is_required_no_default && !arg.default.is_empty() {
                // has default value
                let default_val = &arg.default[0];
                format!(
                    "{}: Optional[{}] = \"{default_val}\"",
                    sanitize_py_ident(&arg.name),
                    py_type
                )
            } else if !is_required_no_default {
                // optional without explicit default
                format!(
                    "{}: Optional[{}] = None",
                    sanitize_py_ident(&arg.name),
                    py_type
                )
            } else {
                // required, no default
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
    cmd_name: &str,
    flags: &[&SpecFlag],
    choice_types: &ChoiceTypeMap,
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
            let py_type = flag_py_type(flag, cmd_name, choice_types);
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
                } else if flag.var {
                    // var value flag with default — cannot use list literal as default
                    // (Python dataclass requires immutable defaults), use None with comment
                    format!("{prop_name}: Optional[{py_type}] = None  # default: {default_val}")
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

fn arg_py_type(arg: &SpecArg, cmd_name: &str, choice_types: &ChoiceTypeMap) -> String {
    let base = if let Some(choices) = &arg.choices {
        if let Some(resolved) = choice_types.lookup(cmd_name, &arg.name) {
            resolved.to_string()
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

fn flag_py_type(flag: &SpecFlag, cmd_name: &str, choice_types: &ChoiceTypeMap) -> String {
    if flag.count {
        return "int".to_string();
    }

    match &flag.arg {
        Some(arg) => {
            let base = if let Some(choices) = &arg.choices {
                if let Some(resolved) = choice_types.lookup(cmd_name, &flag.name) {
                    resolved.to_string()
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
        None => {
            if flag.var {
                "list[bool]".to_string()
            } else {
                "bool".to_string()
            }
        }
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
        | "str" | "float" | "bytes" | "object" | "super" | "property" | "static"         | "true"
        | "false" | "none" => format!("_{snake}"),
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
    let choice_types = collect_choice_types(&spec.cmd);
    let type_imports = collect_type_imports(&spec.cmd, package_name, &choice_types);
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
        w.line(&format!("\"\"\"{}\"\"\"", escape_py_docstring(&class_doc.join(". "))));
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

    // exec method — build signature with four explicit arms to avoid double comma
    let flags_type = if !global_flags.is_empty() && !visible_flags.is_empty() {
        format!("{class_name}Flags")
    } else if !global_flags.is_empty() && visible_flags.is_empty() {
        "GlobalFlags".to_string()
    } else if !visible_flags.is_empty() {
        format!("{class_name}Flags")
    } else {
        String::new()
    };
    let sig = if has_args && !flags_type.is_empty() {
        format!("def exec(self, args: {class_name}Args, flags: Optional[{flags_type}] = None) -> CliResult:")
    } else if has_args {
        format!("def exec(self, args: {class_name}Args) -> CliResult:")
    } else if !flags_type.is_empty() {
        format!("def exec(self, flags: Optional[{flags_type}] = None) -> CliResult:")
    } else {
        "def exec(self) -> CliResult:".to_string()
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
        w.line(&format!("\"\"\"{}\"\"\"", escape_py_docstring(&exec_doc.join("\\n"))));
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

        // Args before `--`: all args without double_dash=required
        for arg in &visible_args {
            if matches!(arg.double_dash, SpecDoubleDashChoices::Required) {
                continue;
            }
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
            // Args after `--`: only double_dash=required args
            for arg in &visible_args {
                if !matches!(arg.double_dash, SpecDoubleDashChoices::Required) {
                    continue;
                }
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

    /// Flags-only subcommand (no positional args) — tests exec signature without double comma.
    #[test]
    fn test_python_flags_only_subcommand() {
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
        let client = get_file(&output, "client.py");
        assert!(!client.contains("def exec(self, ,"));
        assert!(
            client.contains("def exec(self, flags: Optional[StatusFlags] = None) -> CliResult:")
        );
        insta::assert_snapshot!(client);
    }

    /// Choice type collision: same arg name with different choices in different subcommands.
    #[test]
    fn test_python_choice_collision() {
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
        let types = get_file(&output, "types.py");
        // Must have separate choice types due to collision
        assert!(types.contains("BuildEnvChoice"));
        assert!(types.contains("DeployEnvChoice"));
        assert!(types.contains(r#""debug""#));
        assert!(types.contains(r#""staging""#));
        insta::assert_snapshot!(types);
    }

    /// Args with default values — tests that defaults are preserved, not dropped to None.
    #[test]
    fn test_python_arg_defaults() {
        let spec: Spec = r##"
            bin "runner"
            arg "mode" default="fast" help="Run mode"
            arg "output" help="Output path" required=#true
        "##
        .parse()
        .unwrap();
        let output = crate::sdk::generate(&spec, &make_opts());
        let types = get_file(&output, "types.py");
        assert!(types.contains(r#"mode: Optional[str] = "fast""#));
        assert!(types.contains("output: str"));
        // required arg must come before optional
        let output_pos = types.find("output: str").unwrap();
        let mode_pos = types.find(r#"mode: Optional[str] = "fast""#).unwrap();
        assert!(
            output_pos < mode_pos,
            "required arg must precede optional arg"
        );
    }

    /// Config props — covers config dataclass and config_prop_type.
    #[test]
    fn test_python_config_props() {
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
        let types = get_file(&output, "types.py");
        assert!(types.contains("class MyappConfig"));
        insta::assert_snapshot!(types);
    }

    /// Hidden command, hidden arg/flag — covers early-return and empty dataclass paths.
    #[test]
    fn test_python_hidden_command() {
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
        let types = get_file(&output, "types.py");
        assert!(types.contains("VisibleArgs"));
        assert!(!types.contains("SecretArgs"));
    }

    /// Flag edge cases: short-only, aliases, deprecated, count+default, required flag,
    /// repeatable boolean flag, non-bool value flag with default.
    #[test]
    fn test_python_flag_edge_cases() {
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
        let types = get_file(&output, "types.py");
        insta::assert_snapshot!(types);
        let client = get_file(&output, "client.py");
        // short-only flag build
        assert!(client.contains(r#""-v""#));
        // repeatable boolean flag build
        assert!(client.contains("for v in flags.verbose:"));
        insta::assert_snapshot!(client);
    }

    /// double_dash=automatic, examples in exec doc, global flags with flags-only subcommand.
    #[test]
    fn test_python_exec_edge_cases() {
        let spec: Spec = r##"
            bin "runner"
            flag "-v --verbose" global=#true help="Verbosity"
            arg "input" help="Input file"
            arg "extra" double_dash="automatic" var=#true help="Extra files"
            cmd "run" help="Run a task" {
                example "runner run hello" header="Basic run"
                arg "task" help="Task to run" double_dash="automatic"
            }
            cmd "info" help="Show info" {}
        "##
        .parse()
        .unwrap();
        let output = crate::sdk::generate(&spec, &make_opts());
        let client = get_file(&output, "client.py");
        assert!(client.contains("double_dash=automatic"));
        assert!(client.contains("Basic run: runner run hello"));
        // "info" has no own flags, only global flags => GlobalFlags type
        assert!(client.contains("flags: Optional[GlobalFlags] = None"));
        insta::assert_snapshot!(client);
    }

    /// Optional arg without default and empty flags dataclass.
    #[test]
    fn test_python_optional_arg_empty_flags() {
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
        let types = get_file(&output, "types.py");
        // optional arg without default should have = None
        assert!(types.contains("name: Optional[str] = None"));
        insta::assert_snapshot!(types);
    }

    /// Deeply nested subcommands — 3+ levels.
    #[test]
    fn test_python_deep_nesting() {
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
        let client = get_file(&output, "client.py");
        // deeply nested class must exist
        assert!(client.contains("class Db:"));
        assert!(client.contains("class Migration:"));
        assert!(client.contains("class Create:"));
        insta::assert_snapshot!(client);
    }

    /// Test package_name override.
    #[test]
    fn test_python_package_name_override() {
        let spec: Spec = r##"
            bin "original-cli"
        "##
        .parse()
        .unwrap();
        let opts = SdkOptions {
            language: SdkLanguage::Python,
            package_name: Some("my_custom_sdk".to_string()),
            source_file: None,
        };
        let output = crate::sdk::generate(&spec, &opts);
        let init = get_file(&output, "__init__.py");
        assert!(init.contains("MyCustomSdk"));
        insta::assert_snapshot!(init);
    }

    /// Global flags with flags-only subcommand — covers GlobalFlags type branch.
    #[test]
    fn test_python_global_flags_flags_only() {
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
        let client = get_file(&output, "client.py");
        // "info" subcommand has no own flags, only global flags => GlobalFlags type
        assert!(client.contains("Optional[GlobalFlags]"));
        insta::assert_snapshot!(client);
    }

    /// Flag with choices — flag arg with choices renders correct type.
    #[test]
    fn test_python_flag_with_choices() {
        let spec: Spec = r##"
            bin "tool"
            flag "--shell <shell>" help="Shell type" {
                choices "bash" "zsh" "fish"
            }
        "##
        .parse()
        .unwrap();
        let output = crate::sdk::generate(&spec, &make_opts());
        let types = get_file(&output, "types.py");
        assert!(types.contains("Literal[\"bash\", \"zsh\", \"fish\"]"));
        insta::assert_snapshot!(types);
    }

    /// Flag with env annotation — env variable appears in comment.
    #[test]
    fn test_python_flag_with_env() {
        let spec: Spec = r##"
            bin "app"
            flag "--config <path>" help="Config file" env="APP_CONFIG"
        "##
        .parse()
        .unwrap();
        let output = crate::sdk::generate(&spec, &make_opts());
        let types = get_file(&output, "types.py");
        assert!(types.contains("Env: APP_CONFIG"));
        insta::assert_snapshot!(types);
    }

    /// Hidden flag excluded from types and client.
    #[test]
    fn test_python_flag_hide() {
        let spec: Spec = r##"
            bin "app"
            flag "--verbose" help="Verbosity"
            flag "--debug" hide=#true help="Hidden debug flag"
        "##
        .parse()
        .unwrap();
        let output = crate::sdk::generate(&spec, &make_opts());
        let types = get_file(&output, "types.py");
        assert!(types.contains("verbose"));
        assert!(!types.contains("debug"));
    }

    /// Negate flag rendered in client build method.
    #[test]
    fn test_python_negate_flag_build() {
        let spec: Spec = r##"
            bin "app"
            flag "--dry-run" help="Dry run" negate="--no-dry-run"
        "##
        .parse()
        .unwrap();
        let output = crate::sdk::generate(&spec, &make_opts());
        let client = get_file(&output, "client.py");
        assert!(client.contains("--dry-run"));
        assert!(client.contains("--no-dry-run"));
        insta::assert_snapshot!(client);
    }

    /// Count flag rendered in client build method.
    #[test]
    fn test_python_count_flag_build() {
        let spec: Spec = r##"
            bin "app"
            flag "-v --verbose" count=#true help="Verbosity level"
        "##
        .parse()
        .unwrap();
        let output = crate::sdk::generate(&spec, &make_opts());
        let client = get_file(&output, "client.py");
        assert!(client.contains(r#""--verbose""#));
        assert!(client.contains("flags.verbose"));
        insta::assert_snapshot!(client);
    }

    /// Repeatable value flag with default — covers var + arg + default in client build.
    #[test]
    fn test_python_var_value_flag_with_default() {
        let spec: Spec = r##"
            bin "tool"
            flag "--tag <t>" var=#true default="latest" help="Tags"
        "##
        .parse()
        .unwrap();
        let output = crate::sdk::generate(&spec, &make_opts());
        let types = get_file(&output, "types.py");
        assert!(types.contains(r#"list[str]"#));
        assert!(types.contains(r#"default: latest"#));
        let client = get_file(&output, "client.py");
        assert!(client.contains("for v in flags.tag:"));
        insta::assert_snapshot!(types);
    }

    /// Flag with multiple long aliases — `-f --format --fmt <fmt>`.
    #[test]
    fn test_python_multiple_aliases() {
        let spec: Spec = r##"
            bin "tool"
            flag "-f --format --fmt <fmt>" help="Output format"
        "##
        .parse()
        .unwrap();
        let output = crate::sdk::generate(&spec, &make_opts());
        let types = get_file(&output, "types.py");
        assert!(types.contains("Aliases: fmt"));
        let client = get_file(&output, "client.py");
        // should use first long for the flag argument name
        assert!(client.contains("--format"));
        insta::assert_snapshot!(client);
    }

    /// Boolean flag with default=#false — covers the "= False" branch.
    #[test]
    fn test_python_boolean_flag_default_false() {
        let spec: Spec = r##"
            bin "app"
            flag "--no-cache" default=#false help="Disable cache"
        "##
        .parse()
        .unwrap();
        let output = crate::sdk::generate(&spec, &make_opts());
        let types = get_file(&output, "types.py");
        assert!(types.contains("no_cache: bool = False"));
        insta::assert_snapshot!(types);
    }

    /// Boolean config prop with default=#false — covers the "= False" branch.
    #[test]
    fn test_python_config_boolean_default_false() {
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
        let types = get_file(&output, "types.py");
        assert!(types.contains("verbose: bool = False"));
        assert!(types.contains("dry_run: bool = True"));
        insta::assert_snapshot!(types);
    }

    /// String config prop with default — covers String/Null match arm with default.
    #[test]
    fn test_python_config_string_with_default() {
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
        let types = get_file(&output, "types.py");
        assert!(types.contains(r#"host: str = "localhost""#));
        assert!(types.contains(r#"name: str = "myapp""#));
        insta::assert_snapshot!(types);
    }

    /// Config with all props having defaults — tests Default derive on config dataclass.
    #[test]
    fn test_python_config_all_optional() {
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
        let types = get_file(&output, "types.py");
        assert!(types.contains("class AppConfig:"));
        insta::assert_snapshot!(types);
    }

    /// Optional variadic arg — covers the optional + var branch in client rendering.
    #[test]
    fn test_python_optional_variadic_arg() {
        let spec: Spec = r##"
            bin "tool"
            arg "[files]" var=#true help="Input files"
        "##
        .parse()
        .unwrap();
        let output = crate::sdk::generate(&spec, &make_opts());
        let types = get_file(&output, "types.py");
        assert!(types.contains("list[str]"));
        let client = get_file(&output, "client.py");
        // optional variadic arg should use extend with None guard
        assert!(client.contains("if args.files is not None: cmd_args.extend(args.files)"));
        insta::assert_snapshot!(client);
    }

    /// Example without lang attribute — tests single-line exec doc path.
    #[test]
    fn test_python_example_without_lang() {
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
        let client = get_file(&output, "client.py");
        assert!(client.contains("app greet hello"));
        insta::assert_snapshot!(client);
    }

    /// Required flag without default — tests non-optional flag type rendering.
    #[test]
    fn test_python_required_flag_type() {
        let spec: Spec = r##"
            bin "tool"
            flag "--token <t>" required=#true help="Auth token"
        "##
        .parse()
        .unwrap();
        let output = crate::sdk::generate(&spec, &make_opts());
        let types = get_file(&output, "types.py");
        // required flag without default should NOT be Optional
        assert!(types.contains("token: str"));
        assert!(!types.contains("token: Optional[str]"));
        insta::assert_snapshot!(types);
    }

    /// Global repeatable flags — covers flag_ts_simple var branches.
    #[test]
    fn test_python_global_repeatable_flags() {
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
        let types = get_file(&output, "types.py");
        // GlobalFlags should have list[bool] and list[str] types
        assert!(types.contains("list[bool]"));
        assert!(types.contains("list[str]"));
        insta::assert_snapshot!(types);
    }

    /// Client edge cases: double_dash=automatic, examples, global flags, repeatable boolean flag.
    #[test]
    fn test_python_client_edge_cases() {
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
        let output = crate::sdk::generate(&spec, &make_opts());
        let client = get_file(&output, "client.py");
        assert!(client.contains("double_dash=automatic"));
        assert!(client.contains("Basic run: runner run hello"));
        // GlobalFlags type for info subcommand
        assert!(client.contains("Optional[GlobalFlags]"));
        // repeatable boolean flag
        assert!(client.contains("for v in flags.debug:"));
        insta::assert_snapshot!(client);
    }

    /// Config and flag edge cases — config with various data types, env, deprecated, aliases.
    #[test]
    fn test_python_config_and_flag_edge_cases() {
        let spec: Spec = r##"
            bin "myapp"
            config {
                prop "debug" default=#true data_type=boolean help="Enable debug mode"
                prop "port" default=8080 data_type=integer
                prop "rate" default="1.5" data_type=float
                prop "host" data_type=string
            }
            arg "input" help="Input file" env="MYAPP_INPUT"
            flag "--type" help="Reserved keyword" deprecated="Use --kind"
            flag "-f --format --fmt <fmt>" help="Flag with short and long alias"
            flag "-v" help="Short-only flag"
        "##
        .parse()
        .unwrap();
        let output = crate::sdk::generate(&spec, &make_opts());
        let types = get_file(&output, "types.py");
        assert!(types.contains("MyappConfig"));
        insta::assert_snapshot!(types);
    }

    /// double_dash=automatic — covers arg ordering and separator insertion.
    #[test]
    fn test_python_double_dash_automatic() {
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
        let client = get_file(&output, "client.py");
        assert!(client.contains("double_dash=automatic"));
        insta::assert_snapshot!(client);
    }
}
