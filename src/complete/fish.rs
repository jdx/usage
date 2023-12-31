use crate::{env, Spec};

pub fn complete_fish(spec: &Spec) -> String {
    let usage = &*env::USAGE_CMD;
    let bin = &spec.bin;
    let raw = spec.to_string().replace('\'', r"\'").to_string();
    format!(
        r#"
set _usage_spec_{bin} '{raw}'
complete -xc {bin} -a '({usage} complete-word -s "$_usage_spec_{bin}" --ctoken=(commandline -t) -- (commandline -op))'
"#
    )
}

#[cfg(test)]
mod tests {
    use crate::parse::spec::Spec;

    use super::*;

    #[test]
    fn test_complete_fish() {
        let spec: Spec = r#"
        "#
        .parse()
        .unwrap();
        assert_snapshot!(complete_fish(&spec).trim(), @r###"
        set _usage_spec_ ''
        complete -xc  -a '(usage complete-word -s "$_usage_spec_" --ctoken=(commandline -t) -- (commandline -op))'
        "###);
    }
}
