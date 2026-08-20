//! Repeating a scalar is a correction by default, with strictness available per command.

use std::ffi::OsStr;

use usage_derive::Cli;

#[derive(Cli, Debug)]
#[usage(bin = "permissive")]
struct Permissive {
    #[usage(long)]
    jobs: Option<u32>,
    #[usage(long)]
    verbose: bool,
}

#[derive(Cli, Debug)]
#[usage(bin = "strict", args_override_self = false)]
struct Strict {
    #[usage(long)]
    jobs: Option<u32>,
    #[usage(long)]
    verbose: bool,
}

fn argv<'a>(words: &'a [&'a str]) -> Vec<&'a OsStr> {
    words.iter().map(OsStr::new).collect()
}

#[test]
fn a_later_scalar_wins_by_default() {
    let parsed = Permissive::parse_from(&argv(&["--jobs", "1", "--jobs", "2"]))
        .expect("repeating a scalar is permitted by default");
    assert_eq!(parsed.jobs, Some(2));

    let parsed = Permissive::parse_from(&argv(&["--verbose", "--verbose"]))
        .expect("the same policy applies to switches");
    assert!(parsed.verbose);
}

#[test]
fn a_command_can_reject_duplicate_scalars() {
    let parsed = Strict::parse_from(&argv(&["--jobs", "1", "--verbose"]))
        .expect("one occurrence remains valid");
    assert_eq!(parsed.jobs, Some(1));
    assert!(parsed.verbose);

    for words in [
        &["--jobs", "1", "--jobs", "2"][..],
        &["--verbose", "--verbose"][..],
    ] {
        let err = Strict::parse_from(&argv(words)).expect_err("strict commands reject repeats");
        assert!(format!("{err:?}").contains("DuplicateFlag"), "{err:?}");
    }
}

#[test]
fn only_the_non_default_policy_is_emitted() {
    let permissive = Permissive::to_kdl();
    assert!(!permissive.contains("args_override_self"), "{permissive}");

    let strict = Strict::to_kdl();
    assert!(strict.contains("args_override_self #false"), "{strict}");
}
