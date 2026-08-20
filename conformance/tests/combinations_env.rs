//! Environment-mutating interaction coverage lives in its own test process.

use std::ffi::OsStr;

use usage_derive::Cli;

fn argv<const N: usize>(words: [&str; N]) -> [&OsStr; N] {
    words.map(OsStr::new)
}

#[derive(Cli)]
#[usage(bin = "layered-list")]
struct LayeredList {
    #[usage(
        long,
        env = "USAGE_COMBINATION_VALUES",
        default = "fallback-a,fallback-b",
        delimiter = ','
    )]
    values: Vec<String>,
}

#[test]
fn argv_env_defaults_and_delimiters_keep_their_precedence() {
    unsafe { std::env::remove_var("USAGE_COMBINATION_VALUES") };
    assert_eq!(
        LayeredList::parse_from(&[]).unwrap().values,
        ["fallback-a", "fallback-b"]
    );

    unsafe { std::env::set_var("USAGE_COMBINATION_VALUES", "env-a,env-b") };
    assert_eq!(
        LayeredList::parse_from(&[]).unwrap().values,
        ["env-a", "env-b"]
    );
    assert_eq!(
        LayeredList::parse_from(&argv(["--values", "argv-a,argv-b"]))
            .unwrap()
            .values,
        ["argv-a", "argv-b"]
    );
    unsafe { std::env::remove_var("USAGE_COMBINATION_VALUES") };
}
