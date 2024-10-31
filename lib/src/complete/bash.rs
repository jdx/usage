use crate::Spec;
use heck::ToShoutySnakeCase;

pub fn complete_bash(bin: &str, usage_cmd: Option<&str>, spec: Option<&Spec>) -> String {
    let bin_up = bin.to_shouty_snake_case();
    let mut out = vec![format!(
        r#"_{bin}() {{
    if ! command -v usage &> /dev/null; then
        echo >&2
        echo "Error: usage CLI not found. This is required for completions to work in {bin}." >&2
        echo "See https://usage.jdx.dev for more information." >&2
        return 1
    fi"#
    )];

    if let Some(usage_cmd) = &usage_cmd {
        out.push(format!(
            r#"
    if [[ -z ${{_USAGE_SPEC_{bin_up}:-}} ]]; then
        _USAGE_SPEC_{bin_up}="$({usage_cmd})"
    fi"#
        ));
    }

    if let Some(spec) = &spec {
        out.push(format!(
            r#"
    read -r -d '' _USAGE_SPEC_{bin_up} <<'__USAGE_EOF__'
{spec}
__USAGE_EOF__"#,
            spec = spec.to_string().trim()
        ));
    }

    out.push(format!(
        r#"
    COMPREPLY=( $(usage complete-word --shell bash -s "${{_USAGE_SPEC_{bin_up}}}" --cword="$COMP_CWORD" -- "${{COMP_WORDS[@]}}" ) )
    if [[ $? -ne 0 ]]; then
        unset COMPREPLY
    fi
    return 0
}}

shopt -u hostcomplete && complete -o nospace -o bashdefault -o nosort -F _{bin} {bin}
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
        assert_snapshot!(complete_bash("mycli", Some("mycli complete --usage"), None).trim(), @r###"
        _mycli() {
            if ! command -v usage &> /dev/null; then
                echo >&2
                echo "Error: usage CLI not found. This is required for completions to work in mycli." >&2
                echo "See https://usage.jdx.dev for more information." >&2
                return 1
            fi

            if [[ -z ${_USAGE_SPEC_MYCLI:-} ]]; then
                _USAGE_SPEC_MYCLI="$(mycli complete --usage)"
            fi

            COMPREPLY=( $(usage complete-word --shell bash -s "${_USAGE_SPEC_MYCLI}" --cword="$COMP_CWORD" -- "${COMP_WORDS[@]}" ) )
            if [[ $? -ne 0 ]]; then
                unset COMPREPLY
            fi
            return 0
        }

        shopt -u hostcomplete && complete -o nospace -o bashdefault -o nosort -F _mycli mycli
        # vim: noet ci pi sts=0 sw=4 ts=4 ft=sh
        "###);

        assert_snapshot!(complete_bash("mycli", None, Some(&SPEC_KITCHEN_SINK)).trim(), @r##"
        _mycli() {
            if ! command -v usage &> /dev/null; then
                echo >&2
                echo "Error: usage CLI not found. This is required for completions to work in mycli." >&2
                echo "See https://usage.jdx.dev for more information." >&2
                return 1
            fi

            read -r -d '' _USAGE_SPEC_MYCLI <<'__USAGE_EOF__'
        name "mycli"
        bin "mycli"
        source_code_link_template "https://github.com/jdx/mise/blob/main/src/cli/{{path}}.rs"
        flag "--flag1" help="flag1 description"
        flag "--flag2" help="flag2 description" {
            long_help "flag2 long description"
        }
        flag "--flag3" help="flag3 description" negate="--no-flag3"
        flag "--shell" {
            arg "<shell>" {
                choices "bash" "zsh" "fish"
            }
        }
        arg "<arg1>" help="arg1 description"
        arg "<arg2>" help="arg2 description" default="default value" {
            choices "choice1" "choice2" "choice3"
        }
        arg "<arg3>" help="arg3 description" help_long="arg3 long description"
        arg "<argrest>..." var=true
        cmd "plugin" {
            cmd "install" {
                flag "-g --global"
                flag "-d --dir" {
                    arg "<dir>"
                }
                flag "-f --force" negate="--no-force"
                arg "<plugin>"
                arg "<version>"
            }
        }
        __USAGE_EOF__

            COMPREPLY=( $(usage complete-word --shell bash -s "${_USAGE_SPEC_MYCLI}" --cword="$COMP_CWORD" -- "${COMP_WORDS[@]}" ) )
            if [[ $? -ne 0 ]]; then
                unset COMPREPLY
            fi
            return 0
        }

        shopt -u hostcomplete && complete -o nospace -o bashdefault -o nosort -F _mycli mycli
        # vim: noet ci pi sts=0 sw=4 ts=4 ft=sh
        "##);
    }
}
