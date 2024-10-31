// use crate::env;

use crate::Spec;

pub fn complete_zsh(bin: &str, usage_cmd: Option<&str>, spec: Option<&Spec>) -> String {
    // let bin_snake = bin.to_snake_case();
    let mut out = vec![format!(
        r#"
#compdef {bin}
local curcontext="$curcontext""#
    )];

    if let Some(_usage_cmd) = &usage_cmd {
        out.push(format!(
            r#"
# caching config
_usage_{bin}_cache_policy() {{
  if [[ -z "${{lifetime}}" ]]; then
    lifetime=$((60*60*4)) # 4 hours
  fi
  local -a oldp
  oldp=( "$1"(Nms+${{lifetime}}) )
  (( $#oldp ))
}}"#
        ));
    }

    out.push(format!(
        r#"
_{bin}() {{
  typeset -A opt_args
  local curcontext="$curcontext" spec cache_policy

  if ! command -v usage &> /dev/null; then
      echo >&2
      echo "Error: usage CLI not found. This is required for completions to work in {bin}." >&2
      echo "See https://usage.jdx.dev for more information." >&2
      return 1
  fi"#,
    ));

    if let Some(usage_cmd) = &usage_cmd {
        out.push(format!(
            r#"
  zstyle -s ":completion:${{curcontext}}:" cache-policy cache_policy
  if [[ -z $cache_policy ]]; then
    zstyle ":completion:${{curcontext}}:" cache-policy _usage_{bin}_cache_policy
  fi

  if ( [[ -z "${{_usage_{bin}_spec:-}}" ]] || _cache_invalid _usage_{bin}_spec ) \
      && ! _retrieve_cache _usage_{bin}_spec;
  then
    spec="$({usage_cmd})"
    _store_cache _usage_{bin}_spec spec
  fi"#
        ));
    }

    if let Some(spec) = &spec {
        out.push(format!(
            r#"read -r -d '' spec <<'__USAGE_EOF__'
{spec}
__USAGE_EOF__"#,
            spec = spec.to_string().trim()
        ));
    }

    out.push(format!(
        r#"
  _arguments "*: :(($(usage complete-word --shell zsh -s "$spec" -- "${{words[@]}}" )))"
  return 0
}}

if [ "$funcstack[1]" = "_{bin}" ]; then
    _{bin} "$@"
else
    compdef _{bin} {bin}
fi

# vim: noet ci pi sts=0 sw=4 ts=4"#,
    ));

    out.join("\n")
}

// fn render_args(cmds: &[&SchemaCmd]) -> String {
//     format!("XX")
// }

#[cfg(test)]
mod tests {
    use super::*;
    use crate::test::SPEC_KITCHEN_SINK;
    use insta::assert_snapshot;

    #[test]
    fn test_complete_zsh() {
        // let spec = r#"
        // "#;
        // let spec = Spec::parse(&Default::default(), spec).unwrap();
        assert_snapshot!(complete_zsh("mycli", Some("mycli complete --usage"), None).trim(), @r###"
        #compdef mycli
        local curcontext="$curcontext"

        # caching config
        _usage_mycli_cache_policy() {
          if [[ -z "${lifetime}" ]]; then
            lifetime=$((60*60*4)) # 4 hours
          fi
          local -a oldp
          oldp=( "$1"(Nms+${lifetime}) )
          (( $#oldp ))
        }

        _mycli() {
          typeset -A opt_args
          local curcontext="$curcontext" spec cache_policy

          if ! command -v usage &> /dev/null; then
              echo >&2
              echo "Error: usage CLI not found. This is required for completions to work in mycli." >&2
              echo "See https://usage.jdx.dev for more information." >&2
              return 1
          fi

          zstyle -s ":completion:${curcontext}:" cache-policy cache_policy
          if [[ -z $cache_policy ]]; then
            zstyle ":completion:${curcontext}:" cache-policy _usage_mycli_cache_policy
          fi

          if ( [[ -z "${_usage_mycli_spec:-}" ]] || _cache_invalid _usage_mycli_spec ) \
              && ! _retrieve_cache _usage_mycli_spec;
          then
            spec="$(mycli complete --usage)"
            _store_cache _usage_mycli_spec spec
          fi

          _arguments "*: :(($(usage complete-word --shell zsh -s "$spec" -- "${words[@]}" )))"
          return 0
        }

        if [ "$funcstack[1]" = "_mycli" ]; then
            _mycli "$@"
        else
            compdef _mycli mycli
        fi

        # vim: noet ci pi sts=0 sw=4 ts=4
        "###);
        assert_snapshot!(complete_zsh("mycli", None, Some(&SPEC_KITCHEN_SINK)), @r##"

        #compdef mycli
        local curcontext="$curcontext"

        _mycli() {
          typeset -A opt_args
          local curcontext="$curcontext" spec cache_policy

          if ! command -v usage &> /dev/null; then
              echo >&2
              echo "Error: usage CLI not found. This is required for completions to work in mycli." >&2
              echo "See https://usage.jdx.dev for more information." >&2
              return 1
          fi
        read -r -d '' spec <<'__USAGE_EOF__'
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

          _arguments "*: :(($(usage complete-word --shell zsh -s "$spec" -- "${words[@]}" )))"
          return 0
        }

        if [ "$funcstack[1]" = "_mycli" ]; then
            _mycli "$@"
        else
            compdef _mycli mycli
        fi

        # vim: noet ci pi sts=0 sw=4 ts=4
        "##);
    }
}
