use crate::{Spec, SpecCommand};
use once_cell::sync::Lazy;
use tera::Tera;

pub fn render_help(spec: &Spec, cmd: &SpecCommand, long: bool) -> String {
    let mut ctx = tera::Context::new();
    ctx.insert("spec", spec);
    ctx.insert("cmd", cmd);
    ctx.insert("long", &long);
    let template = if long {
        "spec_template_long.tera"
    } else {
        "spec_template_short.tera"
    };
    TERA.render(template, &ctx).unwrap().trim().to_string() + "\n"
}

static TERA: Lazy<Tera> = Lazy::new(|| {
    let mut tera = Tera::default();

    #[rustfmt::skip]
    tera.add_raw_templates([
        ("spec_template_short.tera", include_str!("templates/spec_template_short.tera")),
        ("spec_template_long.tera", include_str!("templates/spec_template_long.tera")),
    ]).unwrap();

    // tera.register_filter(
    //     "repeat",
    //     move |value: &tera::Value, args: &HashMap<String, tera::Value>| {
    //         let value = value.as_str().unwrap();
    //         let count = args.get("count").unwrap().as_u64().unwrap();
    //         Ok(value.repeat(count as usize).into())
    //     },
    // );

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
          --color
            Enable color output
            [env: MYCLI_COLOR]
          --verbose
            Verbose output
            [env: MYCLI_VERBOSE]
          --debug
            Debug mode
        ");
    }

    #[test]
    fn test_render_help_with_arg_env() {
        let spec = crate::spec! { r#"
bin "testcli"
arg "<input>" env="MY_INPUT" help="Input file"
arg "<output>" env="MY_OUTPUT" help="Output file"
arg "<extra>" help="Extra arg without env"
        "# }
        .unwrap();

        assert_snapshot!(render_help(&spec, &spec.cmd, false), @r"
        Usage: testcli <ARGS>…

        Arguments:
          <input>  Input file [env: MY_INPUT]
          <output>  Output file [env: MY_OUTPUT]
          <extra>  Extra arg without env
        ");

        assert_snapshot!(render_help(&spec, &spec.cmd, true), @r"
        Usage: testcli <ARGS>…

        Arguments:
          <input>
            Input file
            [env: MY_INPUT]
          <output>
            Output file
            [env: MY_OUTPUT]
          <extra>
            Extra arg without env
        ");
    }
}
