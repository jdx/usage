pub fn complete_fish(bin: &str, usage_cmd: &str) -> String {
    // let usage = env::USAGE_BIN.display();
    // let bin = &spec.bin;
    // let raw = spec.to_string().replace('\'', r"\'").to_string();
    format!(
        r#"
# if "usage" is not installed show an error
if ! command -v usage &> /dev/null
    echo "Error: usage not found. This is required for completions to work in {bin}. https://usage.jdx.dev" >&2
    return 1
end

set _usage_spec_{bin} ({usage_cmd} | string collect)
complete -xc {bin} -a '(usage complete-word -s "$_usage_spec_{bin}" -- (commandline -cop) (commandline -t))'
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
        # if "usage" is not installed show an error
        if ! command -v usage &> /dev/null
            echo "Error: usage not found. This is required for completions to work in mycli." >&2
            return 1
        end

        set _usage_spec_mycli (mycli complete --usage | string collect)
        complete -xc mycli -a '(usage complete-word -s "$_usage_spec_mycli" -- (commandline -cop) (commandline -t))'
        "###);
    }
}
