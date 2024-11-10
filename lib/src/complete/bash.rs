use crate::complete::CompleteOptions;
use heck::ToSnakeCase;

pub fn complete_bash(opts: &CompleteOptions) -> String {
    let bin = &opts.bin;
    let bin_snake = bin.to_snake_case();
    let spec_variable = if let Some(cache_key) = &opts.cache_key {
        format!("_usage_spec_{bin_snake}_{}", cache_key.to_snake_case())
    } else {
        format!("_usage_spec_{bin_snake}")
    };
    let mut out = vec![format!(
        r#"_{bin}() {{
    if ! command -v usage &> /dev/null; then
        echo >&2
        echo "Error: usage CLI not found. This is required for completions to work in {bin}." >&2
        echo "See https://usage.jdx.dev for more information." >&2
        return 1
    fi"#
    )];

    if let Some(usage_cmd) = &opts.usage_cmd {
        out.push(format!(
            r#"
    if [[ -z ${{{spec_variable}:-}} ]]; then
        {spec_variable}="$({usage_cmd})"
    fi"#
        ));
    }

    if let Some(spec) = &opts.spec {
        out.push(format!(
            r#"
    read -r -d '' {spec_variable} <<'__USAGE_EOF__'
{spec}
__USAGE_EOF__"#,
            spec = spec.to_string().trim()
        ));
    }

    out.push(format!(
        r#"
    COMPREPLY=( $(usage complete-word --shell bash -s "${{{spec_variable}}}" --cword="$COMP_CWORD" -- "${{COMP_WORDS[@]}}" ) )
    if [[ $? -ne 0 ]]; then
        unset COMPREPLY
    fi
    return 0
}}

shopt -u hostcomplete && complete -o nospace -o bashdefault -o nosort -F _{bin_snake} {bin}
# vim: noet ci pi sts=0 sw=4 ts=4 ft=sh
"#
    ));

    out.join("\n")
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::test::SPEC_KITCHEN_SINK;
    use insta::assert_snapshot;

    #[test]
    fn test_complete_bash() {
        assert_snapshot!(complete_bash(&CompleteOptions {
            shell: "bash".to_string(),
            bin: "mycli".to_string(),
            cache_key: None,
            spec: None,
            usage_cmd: Some("mycli complete --usage".to_string()),
        }));
        assert_snapshot!(complete_bash(&CompleteOptions {
            shell: "bash".to_string(),
            bin: "mycli".to_string(),
            cache_key: Some("1.2.3".to_string()),
            spec: None,
            usage_cmd: Some("mycli complete --usage".to_string()),
        }));
        assert_snapshot!(complete_bash(&CompleteOptions {
            shell: "bash".to_string(),
            bin: "mycli".to_string(),
            cache_key: None,
            spec: Some(SPEC_KITCHEN_SINK.clone()),
            usage_cmd: None,
        }));
    }
}
