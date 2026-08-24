use std::ffi::OsString;

use usage_argv::embedded::Outcome;
use usage_argv::ValidationError;
use usage_derive::Cli;

#[derive(Debug, Cli)]
#[usage(bin = "ex", version = "1.2.3", completion)]
struct Ex {
    #[usage(long)]
    force: bool,
}

#[derive(Debug, Cli)]
#[usage(bin = "exi", version = "1.2.3", completion, try_into = ExiCommand)]
struct Exi {
    #[usage(long)]
    force: bool,
    #[usage(long)]
    mode: Option<String>,
}

#[derive(Debug, PartialEq, Eq)]
struct ExiCommand {
    force: bool,
    mode: String,
}

impl TryFrom<Exi> for ExiCommand {
    type Error = ValidationError;

    fn try_from(parsed: Exi) -> Result<Self, Self::Error> {
        let mode = parsed.mode.unwrap_or_else(|| "fast".to_string());
        if mode == "banned" {
            return Err(ValidationError::field("--mode")
                .value(mode)
                .reason("that mode was retired"));
        }
        Ok(Self {
            force: parsed.force,
            mode,
        })
    }
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

#[test]
fn finalizing_dispatch_owns_spec_and_completion_requests() {
    let spec = Exi::embedded_outcome_into(&argv(&[usage_argv::SPEC_REQUEST]));
    let exit = spec.exit().expect("the spec endpoint responds");
    assert_eq!(exit.code, 0);
    assert!(!exit.stderr);
    assert!(exit.text.contains("name exi"), "{}", exit.text);

    let completion = Exi::embedded_outcome_into(&argv(&[
        "__complete_word__",
        "--shell",
        "bash",
        "--line",
        "exi --f",
    ]));
    let exit = completion.exit().expect("the completion endpoint responds");
    assert_eq!(exit.code, 0);
    assert!(!exit.stderr);
    assert!(exit.text.contains("--force"), "{}", exit.text);
}

#[test]
fn finalizing_dispatch_converts_a_parsed_command_line() {
    let Outcome::Parsed(command) = Exi::embedded_outcome_into(&argv(&["--force"])) else {
        panic!("an ordinary invocation should parse and finalize");
    };
    assert_eq!(
        command,
        ExiCommand {
            force: true,
            mode: "fast".to_string(),
        }
    );

    let help = Exi::embedded_outcome_into(&argv(&["--help"]));
    let exit = help.exit().expect("help is a response");
    assert_eq!(exit.code, 0);
    assert!(!exit.stderr);
    assert!(exit.text.contains("Usage: exi"), "{}", exit.text);
}

#[test]
fn finalization_failures_render_like_parse_failures() {
    let rejected = Exi::embedded_outcome_into(&argv(&["--mode", "banned"]));
    let exit = rejected
        .exit()
        .expect("a rejected conversion is a response");
    assert_eq!(exit.code, 2);
    assert!(exit.stderr);
    assert!(exit.text.contains("that mode was retired"), "{}", exit.text);

    let failure = Exi::embedded_outcome_into(&argv(&["--unknown"]));
    let exit = failure.exit().expect("a parse failure is a response");
    assert_eq!(exit.code, 2);
    assert!(exit.stderr);
    assert!(exit.text.contains("--unknown"), "{}", exit.text);
}

#[test]
fn a_host_can_finalize_an_unconverted_outcome_itself() {
    let mapped = Ex::embedded_outcome(&argv(&["--force"])).map(|parsed| parsed.force);
    assert_eq!(mapped.parsed(), Some(true));

    let help = Ex::embedded_outcome(&argv(&["--help"])).map(|parsed| parsed.force);
    assert_eq!(help.exit().map(|exit| exit.code), Some(0));
}
