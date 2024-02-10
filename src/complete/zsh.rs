pub fn complete_zsh(bin: &str, usage_cmd: &str) -> String {
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

  _arguments '*: :( $(usage complete-word -s "$spec" -- "${{words[@]}}" ) )'
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

    #[test]
    fn test_complete_zsh() {
        // let spec = r#"
        // "#;
        // let spec = Spec::parse(&Default::default(), spec).unwrap();
        assert_snapshot!(complete_zsh("mycli", "mycli complete --usage").trim(), @r###"
        #compdef mycli

        _mycli() {
          typeset -A opt_args
          local curcontext="$curcontext"

          if [[ -z "${_usage_spec_mycli:-}" ]]; then
            #echo "Fetching usage spec..." >&2
            _usage_spec_mycli="$(mycli complete --usage)"
          fi

          _arguments '*: :( $(usage complete-word -s "${_usage_spec_mycli}" -- "${words[@]}" ) )'
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
