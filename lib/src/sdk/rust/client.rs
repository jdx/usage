use heck::AsPascalCase;

use crate::sdk::{generated_header, CodeWriter};
use crate::spec::arg::SpecDoubleDashChoices;
use crate::spec::cmd::SpecCommand;
use crate::{Spec, SpecFlag};

use super::types::{flag_property_name_rs, sanitize_rs_ident};

pub fn render(spec: &Spec, package_name: &str, source_file: &Option<String>) -> String {
    let mut w = CodeWriter::with_indent("    ");

    w.line(&generated_header("//!", source_file));
    w.line("");
    w.line("use crate::runtime::{CliRunner, CliResult, CliError};");
    w.line("use crate::types::*;");
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
    _bin_name: &str,
    w: &mut CodeWriter,
) {
    let visible_subcmds: Vec<_> = cmd.subcommands.iter().filter(|(_, c)| !c.hide).collect();

    let visible_args: Vec<&crate::SpecArg> = cmd.args.iter().filter(|a| !a.hide).collect();
    let visible_flags: Vec<&SpecFlag> = cmd.flags.iter().filter(|f| !f.hide).collect();
    let has_args = !visible_args.is_empty();
    let has_flags = !visible_flags.is_empty() || !global_flags.is_empty();

    // struct doc comment
    if let Some(help) = &cmd.help {
        w.line(&format!("/// {help}"));
    }
    if let Some(deprecated) = &cmd.deprecated {
        w.line(&format!("/// DEPRECATED: {deprecated}"));
    }
    if !cmd.aliases.is_empty() {
        w.line(&format!("/// Aliases: {}", cmd.aliases.join(", ")));
    }

    w.line("#[derive(Debug, Clone)]");
    w.line(&format!("pub struct {class_name} {{"));
    w.indent();
    w.line("runner: CliRunner,");
    for (name, _) in &visible_subcmds {
        let sub_class = AsPascalCase(name).to_string();
        w.line(&format!("pub {}: {sub_class},", sanitize_rs_ident(name)));
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
        w.line("Self::with_runner(CliRunner::new(bin_path))");
        w.dedent();
        w.line("}");
        w.line("");
        w.line("pub fn with_runner(runner: CliRunner) -> Self {");
        w.indent();
        w.line("Self {");
        w.indent();
        w.line("runner: runner.clone(),");
        for (name, _) in &visible_subcmds {
            let sub_class = AsPascalCase(name).to_string();
            let prop = sanitize_rs_ident(name);
            w.line(&format!("{prop}: {sub_class}::new(runner.clone()),"));
        }
        w.dedent();
        w.line("}");
        w.dedent();
        w.line("}");
    } else {
        w.line("pub(crate) fn new(runner: CliRunner) -> Self {");
        w.indent();
        w.line("Self {");
        w.indent();
        w.line("runner: runner.clone(),");
        for (name, _) in &visible_subcmds {
            let sub_class = AsPascalCase(name).to_string();
            let prop = sanitize_rs_ident(name);
            w.line(&format!("{prop}: {sub_class}::new(runner.clone()),"));
        }
        w.dedent();
        w.line("}");
        w.dedent();
        w.line("}");
    }

    // exec method
    w.line("");

    let args_type = if has_args {
        format!("{class_name}Args")
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

    // doc comments on exec
    if !cmd.usage.is_empty() {
        w.line(&format!("/// {}", cmd.usage));
    }
    for example in &cmd.examples {
        let label = example.header.as_deref().unwrap_or("Example");
        w.line(&format!("/// {label}: `{}`", example.code));
    }

    let sig = if has_args && !flags_type.is_empty() {
        format!("pub fn exec(&self, args: {args_type}, flags: Option<&{flags_type}>) -> Result<CliResult, CliError> {{")
    } else if has_args {
        format!("pub fn exec(&self, args: {args_type}) -> Result<CliResult, CliError> {{")
    } else if !flags_type.is_empty() {
        format!(
            "pub fn exec(&self, flags: Option<&{flags_type}>) -> Result<CliResult, CliError> {{"
        )
    } else {
        "pub fn exec(&self) -> Result<CliResult, CliError> {".to_string()
    };
    w.line(&sig);
    w.indent();

    // build cmd_args
    let path: String = cmd
        .full_cmd
        .iter()
        .map(|s| format!("\"{s}\".to_string()"))
        .collect::<Vec<_>>()
        .join(", ");
    let mut_decl = if has_args || has_flags {
        "let mut cmd_args"
    } else {
        "let cmd_args"
    };
    w.line(&format!("{mut_decl}: Vec<String> = vec![{path}];"));

    // push args
    if has_args {
        let has_required_double_dash = visible_args
            .iter()
            .any(|a| matches!(a.double_dash, SpecDoubleDashChoices::Required));
        let has_automatic_double_dash = visible_args
            .iter()
            .any(|a| matches!(a.double_dash, SpecDoubleDashChoices::Automatic));

        for arg in &visible_args {
            let ident = sanitize_rs_ident(&heck::AsSnakeCase(&arg.name).to_string());
            if arg.var {
                w.line(&format!(
                    "cmd_args.extend(args.{ident}.iter().map(|v| v.to_string()));"
                ));
            } else {
                let optional = !(arg.required && arg.default.is_empty());
                if optional {
                    w.line(&format!(
                        "if let Some(v) = &args.{ident} {{ cmd_args.push(v.to_string()); }}"
                    ));
                } else {
                    w.line(&format!("cmd_args.push(args.{ident}.to_string());"));
                }
            }
        }

        if has_required_double_dash {
            w.line("cmd_args.push(\"--\".to_string());");
        } else if has_automatic_double_dash {
            w.line("// double_dash=automatic: \"--\" is implied after the first positional arg");
        }
    }

    // push flags
    if has_flags {
        w.line("cmd_args.extend(Self::build_flag_args(flags));");
        w.line("self.runner.run(cmd_args)");
    } else {
        w.line("self.runner.run(cmd_args)");
    }

    w.dedent();
    w.line("}");

    // build_flag_args
    if has_flags {
        w.line("");
        w.line(&format!(
            "fn build_flag_args(flags: Option<&{flags_type}>) -> Vec<String> {{"
        ));
        w.indent();
        w.line("let mut result = Vec::new();");
        w.line("let Some(flags) = flags else { return result };");

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

    w.dedent(); // end impl block
    w.line("}");

    // render subcommand structs
    for (name, subcmd) in &visible_subcmds {
        w.line("");
        let sub_class = AsPascalCase(name).to_string();
        render_class(subcmd, &sub_class, false, global_flags, _bin_name, w);
    }
}

fn render_flag_build_rs(flag: &SpecFlag, w: &mut CodeWriter) {
    let prop_name = flag_property_name_rs(flag);
    let flag_arg_name = if let Some(long) = flag.long.first() {
        format!("--{long}")
    } else if let Some(short) = flag.short.first() {
        format!("-{short}")
    } else {
        format!("--{}", flag.name)
    };

    if flag.arg.is_some() {
        if flag.var {
            // repeatable value flag
            w.line(&format!("if let Some(v) = &flags.{prop_name} {{"));
            w.indent();
            w.line("for item in v {");
            w.indent();
            w.line(&format!("result.push(\"{flag_arg_name}\".to_string());"));
            w.line("result.push(item.to_string());");
            w.dedent();
            w.line("}");
            w.dedent();
            w.line("}");
        } else {
            // single value flag
            w.line(&format!("if let Some(v) = &flags.{prop_name} {{"));
            w.indent();
            w.line(&format!("result.push(\"{flag_arg_name}\".to_string());"));
            w.line("result.push(v.to_string());");
            w.dedent();
            w.line("}");
        }
    } else if flag.count {
        // count flag
        w.line(&format!("if let Some(count) = flags.{prop_name} {{"));
        w.indent();
        w.line(&format!(
            "for _ in 0..count {{ result.push(\"{flag_arg_name}\".to_string()); }}"
        ));
        w.dedent();
        w.line("}");
    } else if flag.var {
        // repeatable boolean flag
        w.line(&format!("if let Some(v) = &flags.{prop_name} {{"));
        w.indent();
        w.line("for item in v {");
        w.indent();
        w.line(&format!(
            "if *item {{ result.push(\"{flag_arg_name}\".to_string()); }}"
        ));
        w.dedent();
        w.line("}");
        w.dedent();
        w.line("}");
    } else {
        // boolean flag
        w.line(&format!("if flags.{prop_name} == Some(true) {{"));
        w.indent();
        w.line(&format!("result.push(\"{flag_arg_name}\".to_string());"));
        w.dedent();
        w.line("}");
        if let Some(negate) = &flag.negate {
            w.line(&format!("else if flags.{prop_name} == Some(false) {{"));
            w.indent();
            w.line(&format!("result.push(\"{negate}\".to_string());"));
            w.dedent();
            w.line("}");
        }
    }
}
