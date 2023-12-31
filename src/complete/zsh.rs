use crate::Spec;

pub fn complete_zsh(spec: &Spec) -> String {
    // let cmds = vec![&spec.cmd];
    // let args = render_args(&cmds);
    let bin = &spec.bin;
    let raw = spec.to_string();
    format!(
        r#"
#compdef {bin}
_{bin}() {{
  typeset -A opt_args
  local context state line curcontext=$curcontext
  local spec='{raw}'

  _arguments -s -S \
   '-h[Show help information]' \
   '-V[Show version information]' \
   '*:: :->command' && return
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
    use crate::parse::spec::Spec;

    #[test]
    fn test_complete_zsh() {
        let spec: Spec = r#"
        "#
        .parse()
        .unwrap();
        assert_snapshot!(complete_zsh(&spec).trim(), @r###"
        #compdef 
        _() {
          typeset -A opt_args
          local context state line curcontext=$curcontext
          local spec=''

          _arguments -s -S \
           '-h[Show help information]' \
           '-V[Show version information]' \
           '*:: :->command' && return
        }

        if [ "$funcstack[1]" = "_" ]; then
            _ "$@"
        else
            compdef _ 
        fi

        # vim: noet ci pi sts=0 sw=4 ts=4
        "###);
    }
}
