// use crate::env;

pub fn complete_zsh(bin: &str, usage_cmd: &str) -> String {
    // let usage = env::USAGE_BIN.display();
    // let cmds = vec![&spec.cmd];
    // let args = render_args(&cmds);
    format!(
        r#"
#compdef {bin}
local curcontext="$curcontext"

# caching config
_usage_{bin}_cache_policy() {{
  if [[ -z "${{lifetime}}" ]]; then
    lifetime=$((60*60*4)) # 4 hours
  fi
  local -a oldp
  oldp=( "$1"(Nms+${{lifetime}}) )
  (( $#oldp ))
}}

_{bin}() {{
  typeset -A opt_args
  local curcontext="$curcontext" spec cache_policy

  if ! command -v usage &> /dev/null; then
      echo >&2
      echo "Error: usage CLI not found. This is required for completions to work in {bin}." >&2
      echo "See https://usage.jdx.dev for more information." >&2
      return 1
  fi

  zstyle -s ":completion:${{curcontext}}:" cache-policy cache_policy
  if [[ -z $cache_policy ]]; then
    zstyle ":completion:${{curcontext}}:" cache-policy _usage_{bin}_cache_policy
  fi

  if ( [[ -z "${{_usage_{bin}_spec:-}}" ]] || _cache_invalid _usage_{bin}_spec ) \
      && ! _retrieve_cache _usage_{bin}_spec;
  then
    spec="$({usage_cmd})"
    _store_cache _usage_{bin}_spec spec
  fi

  _arguments "*: :(($(usage complete-word --shell zsh -s "$spec" -- "${{words[@]}}" )))"
  return 0
}}

if [ "$funcstack[1]" = "_{bin}" ]; then
    _{bin} "$@"
else
    compdef _{bin} {bin}
fi

# vim: noet ci pi sts=0 sw=4 ts=4
"#
    )
}

// fn render_args(cmds: &[&SchemaCmd]) -> String {
//     format!("XX")
// }

#[cfg(test)]
mod tests {
    use super::*;
    use insta::assert_snapshot;

    #[test]
    fn test_complete_zsh() {
        // let spec = r#"
        // "#;
        // let spec = Spec::parse(&Default::default(), spec).unwrap();
        assert_snapshot!(complete_zsh("mycli", "mycli complete --usage").trim(), @r###"
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
    }
}
