use crate::complete::CompleteOptions;
use heck::ToSnakeCase;

pub fn complete_bash(opts: &CompleteOptions) -> String {
    let usage_bin = &opts.usage_bin;
    let bin = &opts.bin;
    let bin_snake = bin.to_snake_case();
    let spec_variable = if let Some(cache_key) = &opts.cache_key {
        format!("_usage_spec_{bin_snake}_{}", cache_key.to_snake_case())
    } else {
        format!("_usage_spec_{bin_snake}")
    };
    let mut out = vec![];
    if opts.include_bash_completion_lib {
        out.push(include_str!("../../bash-completion/bash_completion").to_string());
        out.push("\n".to_string());
    };
    out.push(format!(
        r#"_{bin_snake}() {{
    if ! type -P {usage_bin} &> /dev/null; then
        echo >&2
        echo "Error: {usage_bin} CLI not found. This is required for completions to work in {bin}." >&2
        echo "See https://usage.jdx.dev for more information." >&2
        return 1
    fi"#));

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
	local cur prev words cword was_split comp_args
    _comp_initialize -n : -- "$@" || return
    # shellcheck disable=SC2207
	_comp_compgen -- -W "$(command {usage_bin} complete-word --shell bash -s "${{{spec_variable}}}" --cword="$cword" -- "${{words[@]}}")"
	_comp_ltrim_colon_completions "$cur"
    # shellcheck disable=SC2181
    if [[ $? -ne 0 ]]; then
        unset COMPREPLY
    fi
    return 0
}}

if [[ "${{BASH_VERSINFO[0]}}" -eq 4 && "${{BASH_VERSINFO[1]}}" -ge 4 || "${{BASH_VERSINFO[0]}}" -gt 4 ]]; then
    shopt -u hostcomplete && complete -o nospace -o bashdefault -o nosort -F _{bin_snake} {bin}
else
    shopt -u hostcomplete && complete -o nospace -o bashdefault -F _{bin_snake} {bin}
fi
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
            usage_bin: "usage".to_string(),
            shell: "bash".to_string(),
            bin: "mycli".to_string(),
            cache_key: None,
            spec: None,
            usage_cmd: Some("mycli complete --usage".to_string()),
            include_bash_completion_lib: false,
        }));
        assert_snapshot!(complete_bash(&CompleteOptions {
            usage_bin: "usage".to_string(),
            shell: "bash".to_string(),
            bin: "mycli".to_string(),
            cache_key: Some("1.2.3".to_string()),
            spec: None,
            usage_cmd: Some("mycli complete --usage".to_string()),
            include_bash_completion_lib: false,
        }));
        assert_snapshot!(complete_bash(&CompleteOptions {
            usage_bin: "usage".to_string(),
            shell: "bash".to_string(),
            bin: "mycli".to_string(),
            cache_key: None,
            spec: Some(SPEC_KITCHEN_SINK.clone()),
            usage_cmd: None,
            include_bash_completion_lib: false,
        }));
    }
}
