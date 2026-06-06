use heck::AsPascalCase;

use crate::sdk::{
    collect_choice_types, collect_type_imports, command_type_name, generated_header, CodeWriter,
};
use crate::spec::arg::SpecDoubleDashChoices;
use crate::spec::cmd::SpecCommand;
use crate::{Spec, SpecArg, SpecFlag};

use super::types::{flag_property_name_rs, sanitize_rs_ident};

pub fn render(spec: &Spec, package_name: &str, source_file: &Option<String>) -> String {
    let mut w = CodeWriter::with_indent("    ");

    w.line(&generated_header("//!", source_file));
    w.line("");
    w.line("use crate::runtime::{CliRunner, CliResult};");

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
        w.line(&format!(
            "use crate::types::{{{}}};",
            all_imports.join(", ")
        ));
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
        package_name,
        &mut w,
    );

    w.finish()
}

fn subcmd_path(cmd: &SpecCommand) -> String {
    cmd.full_cmd
        .iter()
        .map(|s| format!("\"{}\".to_string()", super::types::escape_rs_string(s)))
        .collect::<Vec<_>>()
        .join(", ")
}

fn render_class(
    cmd: &SpecCommand,
    class_name: &str,
    is_root: bool,
    global_flags: &[&SpecFlag],
    bin_name: &str,
    package_name: &str,
    w: &mut CodeWriter,
) {
    let visible_subcmds: Vec<_> = cmd.subcommands.iter().filter(|(_, c)| !c.hide).collect();

    let visible_args: Vec<&SpecArg> = cmd.args.iter().filter(|a| !a.hide).collect();
    let visible_flags: Vec<&SpecFlag> = cmd.flags.iter().filter(|f| !f.hide).collect();
    let has_args = !visible_args.is_empty();
    let has_flags = !visible_flags.is_empty() || !global_flags.is_empty();

    let type_name = command_type_name(cmd, package_name);

    // doc comment on struct
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
    if !class_doc.is_empty() {
        for line in &class_doc {
            w.line(&format!("/// {line}"));
        }
    }

    w.line(&format!("pub struct {class_name} {{"));
    w.indent();
    w.line("runner: CliRunner,");

    // subcommand fields
    for (name, subcmd) in &visible_subcmds {
        let sub_class = AsPascalCase(name).to_string();
        let prop = sanitize_rs_ident(&heck::AsSnakeCase(name).to_string());
        let mut doc_parts = Vec::new();
        if let Some(help) = &subcmd.help {
            doc_parts.push(help.clone());
        }
        if let Some(dep) = &subcmd.deprecated {
            doc_parts.push(format!("DEPRECATED: {dep}"));
        }
        if !doc_parts.is_empty() {
            w.line(&format!("/// {}", doc_parts.join(". ")));
        }
        w.line(&format!("pub {prop}: {sub_class},"));
    }

    w.dedent();
    w.line("}");

    // impl block
    w.line("");
    w.line(&format!("impl {class_name} {{"));
    w.indent();

    // constructor
    if is_root {
        w.line("pub fn new(bin_path: &str) -> Self {");
        w.indent();
        w.line("let runner = CliRunner::new(bin_path);");
        w.line("Self {");
        w.indent();
        for (name, _) in &visible_subcmds {
            let sub_class = AsPascalCase(name).to_string();
            let prop = sanitize_rs_ident(&heck::AsSnakeCase(name).to_string());
            w.line(&format!("{prop}: {sub_class}::new(runner.clone()),"));
        }
        w.line("runner,");
        w.dedent();
        w.line("}");
    } else {
        w.line("pub fn new(runner: CliRunner) -> Self {");
        w.indent();
        w.line("Self {");
        w.indent();
        for (name, _) in &visible_subcmds {
            let sub_class = AsPascalCase(name).to_string();
            let prop = sanitize_rs_ident(&heck::AsSnakeCase(name).to_string());
            w.line(&format!("{prop}: {sub_class}::new(runner.clone()),"));
        }
        w.line("runner,");
        w.dedent();
        w.line("}");
    }
    w.dedent();
    w.line("}");

    // default constructor for root
    if is_root {
        w.line("");
        w.line("pub fn default_bin() -> Self {");
        w.indent();
        w.line(&format!(
            "Self::new(\"{}\")",
            super::types::escape_rs_string(bin_name)
        ));
        w.dedent();
        w.line("}");
    }

    // exec method
    let flags_type = if !global_flags.is_empty() && !visible_flags.is_empty() {
        format!("{type_name}Flags")
    } else if !global_flags.is_empty() && visible_flags.is_empty() {
        "GlobalFlags".to_string()
    } else if !visible_flags.is_empty() {
        format!("{type_name}Flags")
    } else {
        String::new()
    };

    let all_flags: Vec<&SpecFlag> = if !global_flags.is_empty() {
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
        visible_flags.to_vec()
    };
    let has_required_flags = all_flags.iter().any(|f| f.required && f.default.is_empty());

    // doc comment on exec
    let mut exec_doc = Vec::new();
    if !cmd.usage.is_empty() {
        exec_doc.push(cmd.usage.clone());
    }
    for example in &cmd.examples {
        let label = example.header.as_deref().unwrap_or("Example");
        exec_doc.push(format!("{label}: {code}", code = example.code));
    }
    if !exec_doc.is_empty() {
        for line in &exec_doc {
            w.line(&format!("/// {line}"));
        }
    }

    w.line("");
    if has_args && !flags_type.is_empty() {
        if has_required_flags {
            w.line(&format!(
                "pub fn exec(&self, args: {type_name}Args, flags: {flags_type}) -> Result<CliResult, crate::runtime::CliError> {{"
            ));
        } else {
            w.line(&format!(
                "pub fn exec(&self, args: {type_name}Args, flags: Option<{flags_type}>) -> Result<CliResult, crate::runtime::CliError> {{"
            ));
        }
    } else if has_args {
        w.line(&format!(
            "pub fn exec(&self, args: {type_name}Args) -> Result<CliResult, crate::runtime::CliError> {{"
        ));
    } else if !flags_type.is_empty() {
        if has_required_flags {
            w.line(&format!(
                "pub fn exec(&self, flags: {flags_type}) -> Result<CliResult, crate::runtime::CliError> {{"
            ));
        } else {
            w.line(&format!(
                "pub fn exec(&self, flags: Option<{flags_type}>) -> Result<CliResult, crate::runtime::CliError> {{"
            ));
        }
    } else {
        w.line("pub fn exec(&self) -> Result<CliResult, crate::runtime::CliError> {");
    }
    w.indent();

    // build command args
    let path = subcmd_path(cmd);
    let needs_mut = has_args || has_flags;
    if needs_mut {
        w.line(&format!("let mut cmd_args: Vec<String> = vec![{path}];"));
    } else {
        w.line(&format!("let cmd_args: Vec<String> = vec![{path}];"));
    }

    // add positional args with double_dash handling
    if has_args {
        let has_automatic_double_dash = visible_args
            .iter()
            .any(|a| matches!(a.double_dash, SpecDoubleDashChoices::Automatic));

        // Args before `--`: all args without double_dash=required
        for arg in &visible_args {
            if matches!(arg.double_dash, SpecDoubleDashChoices::Required) {
                continue;
            }
            let ident = sanitize_rs_ident(&heck::AsSnakeCase(&arg.name).to_string());
            let arg_required = arg.required && arg.default.is_empty();
            if arg.var {
                if arg_required {
                    w.line(&format!(
                        "cmd_args.extend(args.{ident}.iter().map(|v| v.to_string()));"
                    ));
                } else {
                    w.line(&format!(
                        "if let Some(ref {ident}) = args.{ident} {{ cmd_args.extend({ident}.iter().map(|v| v.to_string())); }}"
                    ));
                }
            } else if arg_required {
                w.line(&format!("cmd_args.push(args.{ident}.to_string());"));
            } else {
                w.line(&format!(
                    "if let Some(ref {ident}) = args.{ident} {{ cmd_args.push({ident}.to_string()); }}"
                ));
            }
        }

        if has_automatic_double_dash {
            w.line("// double_dash=automatic: \"--\" is implied after the first positional arg");
        }
    }

    // add flags (before `--` separator)
    if has_flags {
        w.line("let flag_args = self.build_flag_args(flags);");
        w.line("cmd_args.extend(flag_args);");
    }

    // add `--` and double_dash=required args after flags
    if has_args {
        let has_required_double_dash = visible_args
            .iter()
            .any(|a| matches!(a.double_dash, SpecDoubleDashChoices::Required));

        if has_required_double_dash {
            w.line("cmd_args.push(\"--\".to_string());");
            for arg in &visible_args {
                if !matches!(arg.double_dash, SpecDoubleDashChoices::Required) {
                    continue;
                }
                let ident = sanitize_rs_ident(&heck::AsSnakeCase(&arg.name).to_string());
                let arg_required = arg.required && arg.default.is_empty();
                if arg.var {
                    if arg_required {
                        w.line(&format!(
                            "cmd_args.extend(args.{ident}.iter().map(|v| v.to_string()));"
                        ));
                    } else {
                        w.line(&format!(
                            "if let Some(ref {ident}) = args.{ident} {{ cmd_args.extend({ident}.iter().map(|v| v.to_string())); }}"
                        ));
                    }
                } else if arg_required {
                    w.line(&format!("cmd_args.push(args.{ident}.to_string());"));
                } else {
                    w.line(&format!(
                        "if let Some(ref {ident}) = args.{ident} {{ cmd_args.push({ident}.to_string()); }}"
                    ));
                }
            }
        }
    }

    w.line("self.runner.run(cmd_args)");

    w.dedent();
    w.line("}");

    // build_flag_args method
    if has_flags {
        w.line("");
        if has_required_flags {
            w.line(&format!(
                "fn build_flag_args(&self, flags: {flags_type}) -> Vec<String> {{"
            ));
        } else {
            w.line(&format!(
                "fn build_flag_args(&self, flags: Option<{flags_type}>) -> Vec<String> {{"
            ));
        }
        w.indent();
        w.line("let mut result = Vec::new();");
        if !has_required_flags {
            w.line("let flags = match flags { Some(f) => f, None => return result };");
        }

        for flag in global_flags {
            render_flag_build_rs(flag, w);
        }
        for flag in &visible_flags {
            if !global_flags.iter().any(|gf| gf.name == flag.name) {
                render_flag_build_rs(flag, w);
            }
        }

        w.line("result");
        w.dedent();
        w.line("}");
    }

    // alias methods for subcommand aliases (inside impl block)
    for (name, subcmd) in &visible_subcmds {
        for alias in &subcmd.aliases {
            let alias_method = sanitize_rs_ident(&heck::AsSnakeCase(alias).to_string());
            let target_prop = sanitize_rs_ident(&heck::AsSnakeCase(name).to_string());
            let sub_class = AsPascalCase(name).to_string();
            w.line("");
            w.line(&format!("/// Alias for `{name}`."));
            w.line(&format!(
                "pub fn {alias_method}(&self) -> &{sub_class} {{ &self.{target_prop} }}"
            ));
        }
    }

    w.dedent();
    w.line("}");

    // render subcommand structs
    for (name, subcmd) in &visible_subcmds {
        w.line("");
        let sub_class = AsPascalCase(name).to_string();
        render_class(
            subcmd,
            &sub_class,
            false,
            global_flags,
            bin_name,
            package_name,
            w,
        );
    }
}

fn render_flag_build_rs(flag: &SpecFlag, w: &mut CodeWriter) {
    let prop_name = flag_property_name_rs(flag);
    let required = flag.required && flag.default.is_empty();

    let flag_arg_name = if let Some(long) = flag.long.first() {
        format!("--{}", super::types::escape_rs_string(long))
    } else if let Some(short) = flag.short.first() {
        format!("-{short}")
    } else {
        format!("--{}", super::types::escape_rs_string(&flag.name))
    };

    if flag.arg.is_some() {
        if flag.var {
            if required {
                w.line(&format!(
                    "for v in &flags.{prop_name} {{ result.push(\"{flag_arg_name}\".to_string()); result.push(v.to_string()); }}"
                ));
            } else {
                w.line(&format!(
                    "if let Some(ref vals) = flags.{prop_name} {{ for v in vals {{ result.push(\"{flag_arg_name}\".to_string()); result.push(v.to_string()); }} }}"
                ));
            }
        } else if required {
            w.line(&format!(
                "result.push(\"{flag_arg_name}\".to_string()); result.push(flags.{prop_name}.to_string());"
            ));
        } else {
            w.line(&format!(
                "if let Some(ref v) = flags.{prop_name} {{ result.push(\"{flag_arg_name}\".to_string()); result.push(v.to_string()); }}"
            ));
        }
    } else if flag.count {
        if required {
            w.line(&format!(
                "for _ in 0..flags.{prop_name} {{ result.push(\"{flag_arg_name}\".to_string()); }}"
            ));
        } else {
            w.line(&format!(
                "if let Some(v) = flags.{prop_name} {{ for _ in 0..v {{ result.push(\"{flag_arg_name}\".to_string()); }} }}"
            ));
        }
    } else if flag.var {
        if required {
            w.line(&format!(
                "for v in &flags.{prop_name} {{ if *v {{ result.push(\"{flag_arg_name}\".to_string()); }} }}"
            ));
        } else {
            w.line(&format!(
                "if let Some(ref vals) = flags.{prop_name} {{ for v in vals {{ if *v {{ result.push(\"{flag_arg_name}\".to_string()); }} }} }}"
            ));
        }
    } else if required {
        if let Some(negate) = &flag.negate {
            w.line(&format!(
                "if flags.{prop_name} {{ result.push(\"{flag_arg_name}\".to_string()); }} else {{ result.push(\"{}\".to_string()); }}",
                super::types::escape_rs_string(negate)
            ));
        } else {
            w.line(&format!(
                "if flags.{prop_name} {{ result.push(\"{flag_arg_name}\".to_string()); }}"
            ));
        }
    } else {
        if let Some(negate) = &flag.negate {
            w.line(&format!(
                "if let Some(v) = flags.{prop_name} {{ if v {{ result.push(\"{flag_arg_name}\".to_string()); }} else {{ result.push(\"{}\".to_string()); }} }}",
                super::types::escape_rs_string(negate)
            ));
        } else {
            w.line(&format!(
                "if let Some(v) = flags.{prop_name} {{ if v {{ result.push(\"{flag_arg_name}\".to_string()); }} }}"
            ));
        }
    }
}
