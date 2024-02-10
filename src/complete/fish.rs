use crate::env;

pub fn complete_fish(bin: &str, usage_cmd: &str) -> String {
    let usage = env::USAGE_BIN.display();
    // let bin = &spec.bin;
    // let raw = spec.to_string().replace('\'', r"\'").to_string();
    format!(
        r#"
set _usage_spec_{bin} ({usage_cmd})
complete -xc {bin} -a '({usage} complete-word -s "$_usage_spec_{bin}" --ctoken=(commandline -t) -- (commandline -op))'
"#
    )
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_complete_fish() {
        // let spec = r#"
        // "#;
        // let spec = Spec::parse(&Default::default(), spec).unwrap();
        assert_snapshot!(complete_fish("mycli", "mycli complete --usage").trim(), @r###"
        set _usage_spec_mycli (mycli complete --usage)
        complete -xc mycli -a '(/Users/jdx/src/usage/target/debug/deps/usage-6b6342071eb3064a complete-word -s "$_usage_spec_mycli" --ctoken=(commandline -t) -- (commandline -op))'
        "###);
    }
}
