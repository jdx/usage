use crate::{env, Spec};

pub fn complete_bash(spec: &Spec) -> String {
    let usage = &*env::USAGE_CMD;
    let bin = &spec.bin;
    let raw = shell_escape::unix::escape(spec.to_string().into());
    format!(
        r#"
_{bin}() {{
    local raw
    spec={raw}

    COMPREPLY=($({usage} complete-word -s "$spec" --cword="$COMP_CWORD" -- "${{COMP_WORDS[@]}}"))
    #COMPREPLY=($(compgen -W "${{COMPREPLY[*]}}" -- "${{COMP_WORDS[$COMP_CWORD]}}"))
    return 0
}}

complete -F _{bin} {bin}
# vim: noet ci pi sts=0 sw=4 ts=4 ft=sh
"#
    )
}

// fn render_args(cmds: &[&SchemaCmd]) -> String {
//     format!("XX")
// }

#[cfg(test)]
mod tests {
    use crate::parse::spec::Spec;

    use super::*;

    #[test]
    fn test_complete_bash() {
        let spec: Spec = r#"
        "#
        .parse()
        .unwrap();
        assert_snapshot!(complete_bash(&spec).trim(), @r###"
        _() {
            local raw
            spec=''

            COMPREPLY=($(usage complete-word -s "$spec" --cword="$COMP_CWORD" -- "${COMP_WORDS[@]}"))
            #COMPREPLY=($(compgen -W "${COMPREPLY[*]}" -- "${COMP_WORDS[$COMP_CWORD]}"))
            return 0
        }

        complete -F _ 
        # vim: noet ci pi sts=0 sw=4 ts=4 ft=sh
        "###);
    }
}
