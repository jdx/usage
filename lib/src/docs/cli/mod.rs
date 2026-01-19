use crate::{Spec, SpecCommand};
use std::sync::LazyLock;
use tera::Tera;

pub fn render_help(spec: &Spec, cmd: &SpecCommand, long: bool) -> String {
    // Convert to docs models to get layout calculations
    let docs_spec = crate::docs::models::Spec::from(spec.clone());
    let docs_cmd = crate::docs::models::SpecCommand::from(cmd);

    let mut ctx = tera::Context::new();
    ctx.insert("spec", &docs_spec);
    ctx.insert("cmd", &docs_cmd);
    ctx.insert("long", &long);
    let template = if long {
        "spec_template_long.tera"
    } else {
        "spec_template_short.tera"
    };
    TERA.render(template, &ctx).unwrap().trim().to_string() + "\n"
}

static TERA: LazyLock<Tera> = LazyLock::new(|| {
    let mut tera = Tera::default();

    #[rustfmt::skip]
    tera.add_raw_templates([
        ("spec_template_short.tera", include_str!("templates/spec_template_short.tera")),
        ("spec_template_long.tera", include_str!("templates/spec_template_long.tera")),
    ]).unwrap();

    // Register ljust filter for left-justifying text with padding
    tera.register_filter(
        "ljust",
        |value: &tera::Value, args: &std::collections::HashMap<String, tera::Value>| {
            let value = value.as_str().unwrap_or("");
            let width = args.get("width").and_then(|v| v.as_u64()).unwrap_or(0) as usize;
            let result = format!("{:<width$}", value, width = width);
            Ok(result.into())
        },
    );

    tera
});

#[cfg(test)]
mod tests {
    use super::*;
    use insta::assert_snapshot;

    #[test]
    fn test_render_help_with_env() {
        let spec = crate::spec! { r#"
bin "testcli"
flag "--color" env="MYCLI_COLOR" help="Enable color output"
flag "--verbose" env="MYCLI_VERBOSE" help="Verbose output"
flag "--debug" help="Debug mode"
        "# }
        .unwrap();

        assert_snapshot!(render_help(&spec, &spec.cmd, false), @r"
        Usage: testcli [FLAGS]

        Flags:
          --color  Enable color output [env: MYCLI_COLOR]
          --verbose  Verbose output [env: MYCLI_VERBOSE]
          --debug  Debug mode
        ");

        assert_snapshot!(render_help(&spec, &spec.cmd, true), @r"
        Usage: testcli [FLAGS]

        Flags:
          --color    Enable color output
            [env: MYCLI_COLOR]
          --verbose  Verbose output
            [env: MYCLI_VERBOSE]
          --debug    Debug mode
        ");
    }

    #[test]
    fn test_render_help_with_arg_env() {
        let spec = crate::spec! { r#"
bin "testcli"
arg "<input>" env="MY_INPUT" help="Input file"
arg "<output>" env="MY_OUTPUT" help="Output file"
arg "<extra>" help="Extra arg without env"
arg "[default]" help="Arg with default value" default="default value"
        "# }
        .unwrap();

        assert_snapshot!(render_help(&spec, &spec.cmd, false), @r"
        Usage: testcli <ARGS>…

        Arguments:
          <input>  Input file [env: MY_INPUT]
          <output>  Output file [env: MY_OUTPUT]
          <extra>  Extra arg without env
          [default]  Arg with default value (default: default value)
        ");

        assert_snapshot!(render_help(&spec, &spec.cmd, true), @r"
        Usage: testcli <ARGS>…

        Arguments:
          <input>    Input file
            [env: MY_INPUT]
          <output>   Output file
            [env: MY_OUTPUT]
          <extra>    Extra arg without env
          [default]  Arg with default value
            (default: default value)
        ");
    }
}
