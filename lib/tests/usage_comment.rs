use usage::spec::{is_usage_comment, strip_usage_prefix};

#[test]
fn test_is_usage_comment() {
    let cases = [
        ("#usage", true),
        ("# usage", true),
        ("//usage", true),
        ("// usage", true),
        ("#USAGE", true),
        ("# Usage", true),
        ("//USAGE", true),
        ("// Usage", true),
        ("#usage:", true),
        ("# usage:", true),
        ("//usage:", true),
        ("// usage:", true),
        ("#usage: something", true),
        ("# usage: something", true),
        ("//usage: something", true),
        ("// usage: something", true),
        ("#notusage", false),
        ("usage", false),
        ("", false),
        ("# usag", false),
    ];
    for (input, expected) in cases.iter() {
        assert_eq!(is_usage_comment(input), *expected, "input: {}", input);
    }
}

#[test]
fn test_strip_usage_prefix() {
    let cases = [
        ("#usage this is a test", "this is a test"),
        ("# usage: this is a test", "this is a test"),
        ("//usage:   foo bar", "foo bar"),
        ("// usage:foo bar", "foo bar"),
        ("#USAGE:foo", "foo"),
        ("# Usage: foo", "foo"),
        ("//USAGE: foo", "foo"),
        ("// Usage: foo", "foo"),
        ("#usage:", ""),
        ("# usage:", ""),
        ("//usage:", ""),
        ("// usage:", ""),
        ("#notusage: foo", "#notusage: foo"),
        ("usage: foo", "usage: foo"),
        ("", ""),
    ];
    for (input, expected) in cases.iter() {
        assert_eq!(strip_usage_prefix(input), *expected, "input: {}", input);
    }
}
