//! A flag whose value may be left off: `[BUMP]` rather than `<BUMP>`.
//!
//! A spec says this with `arg "[BUMP]" required=#false` inside the flag, and pitchfork's
//! `--bump` is the case that found it — usage-lib rendered `[BUMP]`, usage-argv `<BUMP>`, on
//! three of pitchfork's pages.
//!
//! **Help only, and measured rather than assumed.** usage-lib's parser refuses a bare `--bump`
//! exactly as it refuses a bare `--port`:
//!
//! ```text
//! ["--bump"]        -> ERR Invalid flag `--bump`: requires an argument
//! ["--bump", "5"]   -> OK  bump="5"
//! ```
//!
//! So this changes no binding, which is why it lives in `FlagMeta` and not in `Flag`: a parse
//! never reads it. It cannot be inferred from the type either — `Option<String>` already says
//! the *flag* is optional and says nothing about its value — so it is declared.

use std::ffi::OsStr;

use usage::Spec as LibSpec;
use usage_argv::help;
use usage_derive::Cli;

/// A tool with one of each
#[derive(Cli)]
#[usage(bin = "ex")]
struct Ex {
    /// Automatically find an available port if the expected port is in use
    #[usage(long, value_name = "BUMP", value_optional)]
    bump: Option<String>,
    /// The port to listen on
    #[usage(long, value_name = "PORT")]
    port: Option<String>,
}

fn listing() -> String {
    let page = help::render(Ex::spec(), Ex::command(), false).expect("a page");
    page.split_once("\nFlags:")
        .expect("a flags section")
        .1
        .to_string()
}

#[test]
fn an_optional_value_is_squared_and_a_required_one_angled() {
    let flags = listing();
    assert!(flags.contains("--bump [BUMP]"), "{flags}");
    assert!(flags.contains("--port <PORT>"), "{flags}");
}

#[test]
fn the_reference_renders_it_the_same_way() {
    // The invariant this area runs on: the two renderers agree byte for byte. Through the
    // emitted spec, so the round trip is covered too — square brackets alone come back
    // `required=#true`, since usage-lib reads the attribute as well as the name.
    let spec: LibSpec = Ex::to_kdl().parse().expect("valid spec");
    let theirs = usage::docs::cli::render_help(&spec, &spec.cmd, false);
    let ours = help::render(Ex::spec(), Ex::command(), false).expect("a page");
    assert_eq!(ours, theirs);
}

#[test]
fn the_emitted_spec_says_both_halves() {
    let kdl = Ex::to_kdl();
    assert!(kdl.contains(r#"arg "[BUMP]" required=#false"#), "{kdl}");
    assert!(!kdl.contains("value_optional=#true"), "{kdl}");
    assert!(kdl.contains("arg <PORT>"), "{kdl}");
}

#[test]
fn portable_tables_keep_help_optionality_separate_from_binding() {
    let presentation: LibSpec = Ex::to_kdl().parse().unwrap();
    let presentation = usage_conformance::tables::build_spec(&presentation);
    let bare = [OsStr::new("--bump")];
    let mut parser = usage_argv::Parser::new(presentation.root.cmd, &bare);
    let presentation_error = loop {
        match parser.next_event() {
            Some(Ok(_)) => {}
            Some(Err(_)) => break true,
            None => break false,
        }
    };
    assert!(presentation_error);

    let executable: LibSpec = r#"
name "ex"
flag "--bump [BUMP]" value_optional=#true
"#
    .parse()
    .unwrap();
    let executable = usage_conformance::tables::build_spec(&executable);
    let mut parser = usage_argv::Parser::new(executable.root.cmd, &bare);
    let mut events = Vec::new();
    while let Some(event) = parser.next_event() {
        events.push(event.unwrap());
    }
    assert_eq!(events.len(), 1);
    assert!(matches!(
        events[0],
        usage_argv::Event::Flag { value: None, .. }
    ));
}

#[test]
fn it_still_takes_its_value() {
    // Nothing about binding changed: the value is read where it is given.
    let argv = [OsStr::new("--bump"), OsStr::new("5")];
    let parsed = Ex::parse_from(&argv).expect("parses");
    assert_eq!(parsed.bump.as_deref(), Some("5"));
    assert_eq!(parsed.port.as_deref(), None);
}
