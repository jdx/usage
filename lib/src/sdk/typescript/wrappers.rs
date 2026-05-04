use heck::AsPascalCase;

use crate::sdk::{
    collect_choice_types, collect_type_imports, escape_jsdoc, generated_header, CodeWriter,
};
use crate::spec::arg::SpecDoubleDashChoices;
use crate::spec::cmd::SpecCommand;
use crate::{Spec, SpecArg, SpecFlag};

use super::types::{flag_property_name, sanitize_ident};

pub fn render(spec: &Spec, package_name: &str, source_file: &Option<String>) -> String {
    let mut w = CodeWriter::new();

    w.line(&generated_header("//", source_file));
    w.line("import { CliRunner, CliResult } from \"./runtime\";");

    // collect all type imports needed
    let choice_types = collect_choice_types(&spec.cmd);
    let type_imports = collect_type_imports(&spec.cmd, package_name, &choice_types);
    let has_global_flags = spec.cmd.flags.iter().any(|f| f.global && !f.hide);
    if has_global_flags {
        let mut all_imports = type_imports;
        all_imports.push("GlobalFlags".to_string());
        all_imports.sort();
        all_imports.dedup();
        w.line(&format!(
            "import {{ {} }} from \"./types\";",
            all_imports.join(", ")
        ));
    } else if !type_imports.is_empty() {
        w.line(&format!(
            "import {{ {} }} from \"./types\";",
            type_imports.join(", ")
        ));
    }

    w.line("");

    // collect root-level global flags for propagation to subcommands
    let global_flags: Vec<&SpecFlag> = spec
        .cmd
        .flags
        .iter()
        .filter(|f| f.global && !f.hide)
        .collect();

    let class_name = AsPascalCase(package_name).to_string();

    // render the root class (the main entry point)
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

fn subcmd_path(cmd: &SpecCommand) -> String {
    cmd.full_cmd
        .iter()
        .map(|s| format!("\"{s}\""))
        .collect::<Vec<_>>()
        .join(", ")
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

    // JSDoc on class
    let mut class_doc = Vec::new();
    if let Some(help) = &cmd.help {
        class_doc.push(help.clone());
    } else if let Some(about) = &cmd.help_long {
        class_doc.push(about.clone());
    }
    if let Some(deprecated) = &cmd.deprecated {
        class_doc.push(format!("@deprecated {deprecated}"));
    }
    if !cmd.aliases.is_empty() {
        class_doc.push(format!("Aliases: {}", cmd.aliases.join(", ")));
    }
    if !class_doc.is_empty() {
        let class_doc: Vec<String> = class_doc.iter().map(|s| escape_jsdoc(s)).collect();
        if class_doc.len() == 1 {
            w.line(&format!("/** {} */", class_doc[0]));
        } else {
            w.line("/**");
            for line in &class_doc {
                w.line(&format!(" * {line}"));
            }
            w.line(" */");
        }
    }

    // class declaration
    w.line(&format!("export class {class_name} {{"));
    w.indent();

    // runner field
    w.line("private runner: CliRunner;");

    // subcommand properties
    for (name, subcmd) in &visible_subcmds {
        let sub_class = AsPascalCase(name).to_string();
        let prop = sanitize_ident(name);
        let mut doc_parts = Vec::new();
        if let Some(help) = &subcmd.help {
            doc_parts.push(help.clone());
        }
        if let Some(dep) = &subcmd.deprecated {
            doc_parts.push(format!("@deprecated {dep}"));
        }
        if !doc_parts.is_empty() {
            w.line(&format!("/** {} */", escape_jsdoc(&doc_parts.join(". "))));
        }
        w.line(&format!("readonly {prop}: {sub_class};"));
    }

    // alias getters for subcommand aliases
    for (name, subcmd) in &visible_subcmds {
        for alias in &subcmd.aliases {
            let alias_prop = sanitize_ident(alias);
            let target_prop = sanitize_ident(name);
            let sub_class = AsPascalCase(name).to_string();
            w.line(&format!("/** Alias for `{}` */", escape_jsdoc(name)));
            w.line(&format!(
                "get {alias_prop}(): {sub_class} {{ return this.{target_prop}; }}"
            ));
        }
    }

    // constructor
    w.line("");
    if is_root {
        w.line("constructor(binPath?: string) {");
        w.indent();
        w.line(&format!(
            "this.runner = new CliRunner(binPath ?? \"{bin_name}\");"
        ));
    } else {
        w.line("constructor(runner: CliRunner) {");
        w.indent();
        w.line("this.runner = runner;");
    }
    for (name, _) in &visible_subcmds {
        let sub_class = AsPascalCase(name).to_string();
        let prop = sanitize_ident(name);
        w.line(&format!("this.{prop} = new {sub_class}(this.runner);"));
    }
    w.dedent();
    w.line("}");

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
        if has_args {
            format!(", flags?: {flags_type}")
        } else {
            format!("flags?: {flags_type}")
        }
    } else {
        String::new()
    };

    // JSDoc on exec
    let mut exec_doc = Vec::new();
    if !cmd.usage.is_empty() {
        exec_doc.push(cmd.usage.clone());
    }
    for example in &cmd.examples {
        let label = example.header.as_deref().unwrap_or("Example");
        let lang = if example.lang.is_empty() {
            ""
        } else {
            &example.lang
        };
        exec_doc.push(format!(
            "@example {label}\n```{lang}\n{code}\n```",
            code = example.code
        ));
    }
    if !exec_doc.is_empty() {
        let exec_doc: Vec<String> = exec_doc.iter().map(|s| escape_jsdoc(s)).collect();
        if exec_doc.len() == 1 && !exec_doc[0].contains('\n') {
            w.line(&format!("/** {} */", exec_doc[0]));
        } else {
            w.line("/**");
            for part in &exec_doc {
                for line in part.split('\n') {
                    w.line(&format!(" * {line}"));
                }
            }
            w.line(" */");
        }
    }

    if has_args || has_flags {
        w.line(&format!(
            "async exec({args_param}{flags_param}): Promise<CliResult> {{"
        ));
        w.indent();

        // build command args
        let path = subcmd_path(cmd);
        w.line(&format!("const cmdArgs: string[] = [{path}];"));

        // add positional args with double_dash handling
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
                let ident = sanitize_ident(&arg.name);
                if arg.var {
                    w.line(&format!(
                        "if (args.{ident} !== undefined) {{ cmdArgs.push(...args.{ident}); }}"
                    ));
                } else {
                    w.line(&format!(
                        "if (args.{ident} !== undefined) {{ cmdArgs.push(String(args.{ident})); }}"
                    ));
                }
            }

            if has_required_double_dash {
                w.line("cmdArgs.push(\"--\");");
                // Args after `--`: only double_dash=required args
                for arg in &visible_args {
                    if !matches!(arg.double_dash, SpecDoubleDashChoices::Required) {
                        continue;
                    }
                    let ident = sanitize_ident(&arg.name);
                    if arg.var {
                        w.line(&format!(
                            "if (args.{ident} !== undefined) {{ cmdArgs.push(...args.{ident}); }}"
                        ));
                    } else {
                        w.line(&format!(
                            "if (args.{ident} !== undefined) {{ cmdArgs.push(String(args.{ident})); }}"
                        ));
                    }
                }
            } else if has_automatic_double_dash {
                w.line(
                    "// double_dash=automatic: \"--\" is implied after the first positional arg",
                );
            }
        }

        // add flags
        if has_flags {
            w.line("const flagArgs = this.buildFlagArgs(flags);");
            w.line("return this.runner.run([...cmdArgs, ...flagArgs]);");
        } else {
            w.line("return this.runner.run(cmdArgs);");
        }

        w.dedent();
        w.line("}");

        // buildFlagArgs method
        if has_flags {
            w.line("");
            w.line(&format!(
                "private buildFlagArgs(flags?: {flags_type}): string[] {{"
            ));
            w.indent();
            w.line("const result: string[] = [];");
            w.line("if (!flags) return result;");

            for flag in global_flags {
                render_flag_build(flag, w);
            }
            for flag in &visible_flags {
                if !global_flags.iter().any(|gf| gf.name == flag.name) {
                    render_flag_build(flag, w);
                }
            }

            w.line("return result;");
            w.dedent();
            w.line("}");
        }
    } else {
        // no args/flags: provide a simple exec
        w.line("async exec(): Promise<CliResult> {");
        w.indent();
        let path = subcmd_path(cmd);
        w.line(&format!("return this.runner.run([{path}]);"));
        w.dedent();
        w.line("}");
    }

    w.dedent();
    w.line("}");

    // render subcommand classes
    for (name, subcmd) in &visible_subcmds {
        w.line("");
        let sub_class = AsPascalCase(name).to_string();
        render_class(subcmd, &sub_class, false, global_flags, bin_name, w);
    }
}

fn render_flag_build(flag: &SpecFlag, w: &mut CodeWriter) {
    let prop_name = flag_property_name(flag);

    // use the first long name for the flag argument, or short if no long
    let flag_arg_name = if let Some(long) = flag.long.first() {
        format!("--{long}")
    } else if let Some(short) = flag.short.first() {
        format!("-{short}")
    } else {
        format!("--{}", flag.name)
    };

    if flag.arg.is_some() {
        if flag.var {
            // repeatable value flag: --flag val1 --flag val2
            w.line(&format!(
                "if (flags.{prop_name} !== undefined) {{ for (const v of flags.{prop_name}) {{ result.push(\"{flag_arg_name}\", String(v)); }} }}"
            ));
        } else {
            // single value flag
            w.line(&format!(
                "if (flags.{prop_name} !== undefined) {{ result.push(\"{flag_arg_name}\", String(flags.{prop_name})); }}"
            ));
        }
    } else if flag.count {
        w.line(&format!(
            "if (flags.{prop_name} !== undefined && flags.{prop_name} > 0) {{ for (let i = 0; i < flags.{prop_name}; i++) {{ result.push(\"{flag_arg_name}\"); }} }}"
        ));
    } else if flag.var {
        // repeatable boolean flag
        w.line(&format!(
            "if (flags.{prop_name} !== undefined) {{ for (const v of flags.{prop_name}) {{ if (v) result.push(\"{flag_arg_name}\"); }} }}"
        ));
    } else {
        // boolean flag
        w.line(&format!(
            "if (flags.{prop_name}) {{ result.push(\"{flag_arg_name}\"); }}"
        ));

        // handle negate
        if let Some(negate) = &flag.negate {
            w.line(&format!(
                "else if (flags.{prop_name} === false) {{ result.push(\"{negate}\"); }}"
            ));
        }
    }
}
