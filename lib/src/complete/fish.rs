use crate::complete::CompleteOptions;
use heck::ToSnakeCase;

pub fn complete_fish(opts: &CompleteOptions) -> String {
    let bin = &opts.bin;
    let spec_variable = if let Some(cache_key) = &opts.cache_key {
        format!("_usage_spec_{bin}_{}", cache_key.to_snake_case())
    } else {
        format!("_usage_spec_{bin}")
    };
    let mut out = vec![format!(
        r#"
# if "usage" is not installed show an error
if ! command -v usage &> /dev/null
    echo >&2
    echo "Error: usage CLI not found. This is required for completions to work in {bin}." >&2
    echo "See https://usage.jdx.dev for more information." >&2
    return 1
end"#
    )];

    if let Some(usage_cmd) = &opts.usage_cmd {
        if opts.cache_key.is_some() {
            out.push(format!(
                r#"
if ! set -q {spec_variable}
  set -U {spec_variable} ({usage_cmd} | string collect)
end"#
            ));
        } else {
            out.push(format!(
                r#"
set {spec_variable} ({usage_cmd} | string collect)"#
            ));
        }
    }

    if let Some(spec) = &opts.spec {
        let spec_escaped = spec.to_string().replace("'", r"\'");
        out.push(format!(
            r#"
set {spec_variable} '{spec_escaped}'"#
        ));
    }

    out.push(format!(
        r#"complete -xc {bin} -a '(usage complete-word --shell fish -s "${spec_variable}" -- (commandline -cop) (commandline -t))'"#
    ));

    out.join("\n")
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::test::SPEC_KITCHEN_SINK;
    use insta::assert_snapshot;

    #[test]
    fn test_complete_fish() {
        assert_snapshot!(complete_fish(&CompleteOptions {
            shell: "fish".to_string(),
            bin: "mycli".to_string(),
            cache_key: None,
            spec: None,
            usage_cmd: Some("mycli complete --usage".to_string()),
        }));
        assert_snapshot!(complete_fish(&CompleteOptions {
            shell: "fish".to_string(),
            bin: "mycli".to_string(),
            cache_key: Some("1.2.3".to_string()),
            spec: None,
            usage_cmd: Some("mycli complete --usage".to_string()),
        }));
        assert_snapshot!(complete_fish(&CompleteOptions {
            shell: "fish".to_string(),
            bin: "mycli".to_string(),
            cache_key: None,
            spec: Some(SPEC_KITCHEN_SINK.clone()),
            usage_cmd: None,
        }));
    }
}
