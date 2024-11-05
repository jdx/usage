use crate::complete::CompleteOptions;
use heck::ToSnakeCase;

pub fn complete_zsh(opts: &CompleteOptions) -> String {
    let bin = &opts.bin;
    let spec_variable = if let Some(cache_key) = &opts.cache_key {
        format!("_usage_spec_{bin}_{}", cache_key.to_snake_case())
    } else {
        format!("_usage_spec_{bin}")
    };
    // let bin_snake = bin.to_snake_case();
    let mut out = vec![format!(
        r#"
#compdef {bin}
local curcontext="$curcontext""#
    )];

    if let Some(_usage_cmd) = &opts.usage_cmd {
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

    if let Some(usage_cmd) = &opts.usage_cmd {
        out.push(format!(
            r#"
  zstyle -s ":completion:${{curcontext}}:" cache-policy cache_policy
  if [[ -z $cache_policy ]]; then
    zstyle ":completion:${{curcontext}}:" cache-policy _usage_{bin}_cache_policy
  fi

  if ( [[ -z "${{{spec_variable}:-}}" ]] || _cache_invalid {spec_variable} ) \
      && ! _retrieve_cache {spec_variable};
  then
    spec="$({usage_cmd})"
    _store_cache {spec_variable} spec
  fi"#
        ));
    }

    if let Some(spec) = &opts.spec {
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
        assert_snapshot!(complete_zsh(&CompleteOptions {
            shell: "zsh".to_string(),
            bin: "mycli".to_string(),
            cache_key: None,
            spec: None,
            usage_cmd: Some("mycli complete --usage".to_string()),
        }));
        assert_snapshot!(complete_zsh(&CompleteOptions {
            shell: "zsh".to_string(),
            bin: "mycli".to_string(),
            cache_key: Some("1.2.3".to_string()),
            spec: None,
            usage_cmd: Some("mycli complete --usage".to_string()),
        }));
        assert_snapshot!(complete_zsh(&CompleteOptions {
            shell: "zsh".to_string(),
            bin: "mycli".to_string(),
            cache_key: None,
            spec: Some(SPEC_KITCHEN_SINK.clone()),
            usage_cmd: None,
        }));
    }
}
