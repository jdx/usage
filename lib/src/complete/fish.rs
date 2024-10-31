use crate::Spec;

pub fn complete_fish(bin: &str, usage_cmd: Option<&str>, spec: Option<&Spec>) -> String {
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

    if let Some(usage_cmd) = &usage_cmd {
        out.push(format!(
            r#"
set _usage_spec_{bin} ({usage_cmd} | string collect)"#
        ));
    }

    if let Some(spec) = &spec {
        let spec_escaped = spec.to_string().replace("'", r"\'");
        out.push(format!(
            r#"
set -x _usage_spec_{bin} '{spec_escaped}'"#
        ));
    }

    out.push(format!(
        r#"complete -xc {bin} -a '(usage complete-word --shell fish -s "$_usage_spec_{bin}" -- (commandline -cop) (commandline -t))'"#
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
        // let spec = r#"
        // "#;
        // let spec = Spec::parse(&Default::default(), spec).unwrap();
        assert_snapshot!(complete_fish("mycli", Some("mycli complete --usage"), None).trim(), @r###"
        # if "usage" is not installed show an error
        if ! command -v usage &> /dev/null
            echo >&2
            echo "Error: usage CLI not found. This is required for completions to work in mycli." >&2
            echo "See https://usage.jdx.dev for more information." >&2
            return 1
        end

        set _usage_spec_mycli (mycli complete --usage | string collect)
        complete -xc mycli -a '(usage complete-word --shell fish -s "$_usage_spec_mycli" -- (commandline -cop) (commandline -t))'
        "###);

        assert_snapshot!(complete_fish("mycli", None, Some(&SPEC_KITCHEN_SINK)).trim(), @r##"
        # if "usage" is not installed show an error
        if ! command -v usage &> /dev/null
            echo >&2
            echo "Error: usage CLI not found. This is required for completions to work in mycli." >&2
            echo "See https://usage.jdx.dev for more information." >&2
            return 1
        end

        set -x _usage_spec_mycli 'name "mycli"
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
        '
        complete -xc mycli -a '(usage complete-word --shell fish -s "$_usage_spec_mycli" -- (commandline -cop) (commandline -t))'
        "##);
    }
}
