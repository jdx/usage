use std::ffi::OsString;

use usage_argv::embedded::Outcome;
use usage_derive::Cli;

#[derive(Debug, Cli)]
#[usage(bin = "ex", version = "1.2.3", completion)]
struct Ex {
    #[usage(long)]
    force: bool,
}

fn argv(words: &[&str]) -> Vec<OsString> {
    words.iter().map(OsString::from).collect()
}

#[test]
fn embedded_dispatch_owns_spec_and_completion_requests() {
    let spec = Ex::embedded_outcome(&argv(&[usage_argv::SPEC_REQUEST]));
    let exit = spec.exit().expect("the spec endpoint responds");
    assert_eq!(exit.code, 0);
    assert!(!exit.stderr);
    assert!(exit.text.contains("name ex"), "{}", exit.text);

    let completion = Ex::embedded_outcome(&argv(&[
        "__complete_word__",
        "--shell",
        "bash",
        "--line",
        "ex --f",
    ]));
    let exit = completion.exit().expect("the completion endpoint responds");
    assert_eq!(exit.code, 0);
    assert!(!exit.stderr);
    assert!(exit.text.contains("--force"), "{}", exit.text);
}

#[test]
fn embedded_dispatch_parses_or_renders_every_other_outcome() {
    let Outcome::Parsed(parsed) = Ex::embedded_outcome(&argv(&["--force"])) else {
        panic!("an ordinary invocation should parse");
    };
    assert!(parsed.force);

    let help = Ex::embedded_outcome(&argv(&["--help"]));
    let exit = help.exit().expect("help is a response");
    assert_eq!(exit.code, 0);
    assert!(!exit.stderr);
    assert!(exit.text.contains("Usage: ex"), "{}", exit.text);

    let failure = Ex::embedded_outcome(&argv(&["--unknown"]));
    let exit = failure.exit().expect("a parse failure is a response");
    assert_eq!(exit.code, 2);
    assert!(exit.stderr);
    assert!(exit.text.contains("--unknown"), "{}", exit.text);
}
