use heck::ToShoutySnakeCase;

pub fn complete_bash(bin: &str, usage_cmd: &str) -> String {
    // let usage = env::USAGE_BIN.display();
    let bin_up = bin.to_shouty_snake_case();
    // let bin = &spec.bin;
    // let raw = shell_escape::unix::escape(spec.to_string().into());
    format!(
        r#"
_{bin}() {{
    if ! command -v usage &> /dev/null; then
        echo "Error: usage not found. This is required for completions to work in {bin}." >&2
        return 1
    fi

    if [[ -z ${{_USAGE_SPEC_{bin_up}:-}} ]]; then
        _USAGE_SPEC_{bin_up}="$({usage_cmd})"
    fi
    
    COMPREPLY=( $(usage complete-word -s "${{_USAGE_SPEC_{bin_up}}}" --cword="$COMP_CWORD" -- "${{COMP_WORDS[@]}}" ) )
    if [[ $? -ne 0 ]]; then
        unset COMPREPLY
    fi
    return 0
}}

shopt -u hostcomplete && complete -o nospace -o bashdefault -o nosort -F _{bin} {bin}
# vim: noet ci pi sts=0 sw=4 ts=4 ft=sh
"#
    )
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_complete_bash() {
        assert_snapshot!(complete_bash("mycli", "mycli complete --usage").trim(), @r###"
        _mycli() {
            if ! command -v usage &> /dev/null; then
                echo "Error: usage not found. This is required for completions to work in mycli." >&2
                return 1
            fi

            if [[ -z ${_USAGE_SPEC_MYCLI:-} ]]; then
                _USAGE_SPEC_MYCLI="$(mycli complete --usage)"
            fi
            
            COMPREPLY=( $(usage complete-word -s "${_USAGE_SPEC_MYCLI}" --cword="$COMP_CWORD" -- "${COMP_WORDS[@]}" ) )
            if [[ $? -ne 0 ]]; then
                unset COMPREPLY
            fi
            return 0
        }

        shopt -u hostcomplete && complete -o nospace -o bashdefault -o nosort -F _mycli mycli
        # vim: noet ci pi sts=0 sw=4 ts=4 ft=sh
        "###);
    }
}
