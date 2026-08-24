use std::ffi::OsStr;
use std::sync::atomic::{AtomicU16, Ordering};

use usage::Spec as LibSpec;
use usage_derive::Cli;

static NEXT_PORT: AtomicU16 = AtomicU16::new(4100);

fn default_port() -> u16 {
    NEXT_PORT.fetch_add(1, Ordering::Relaxed)
}

fn default_format() -> String {
    String::new()
}

#[derive(Debug, Cli)]
#[usage(bin = "serve")]
struct Serve {
    /// Port to listen on
    #[usage(long, default_fn = default_port, default_note = "selected at runtime")]
    port: u16,
}

#[derive(Debug, Cli)]
struct Report {
    #[usage(long, choices("files", "timings"), default_fn = default_format)]
    debug: String,
}

#[test]
fn a_function_supplies_a_fresh_typed_default() {
    let first = Serve::parse_from(&[]).expect("the computed default makes the flag optional");
    let second = Serve::parse_from(&[]).expect("the function is evaluated for each parse");
    assert_eq!(first.port, 4100);
    assert_eq!(second.port, 4101);

    let argv = [OsStr::new("--port"), OsStr::new("9000")];
    let explicit = Serve::parse_from(&argv).expect("argv still wins");
    assert_eq!(explicit.port, 9000);
}

#[test]
fn a_typed_computed_default_is_not_revalidated_as_cli_text() {
    let defaulted = Report::parse_from(&[]).expect("the typed default is already valid");
    assert_eq!(defaulted.debug, "");

    let explicit = [OsStr::new("--debug"), OsStr::new("files")];
    assert_eq!(Report::parse_from(&explicit).unwrap().debug, "files");

    let invalid = [OsStr::new("--debug"), OsStr::new("other")];
    assert!(matches!(
        Report::parse_from(&invalid),
        Err(usage_argv::Error::InvalidChoice { name: "debug", .. })
    ));
}

#[test]
fn portable_metadata_describes_but_does_not_invent_the_runtime_value() {
    let typed = Serve::spec().root.flags[0];
    assert!(!typed.required);
    assert!(typed.default.is_empty());
    assert_eq!(
        typed.help,
        Some("Port to listen on (default: selected at runtime)")
    );
    assert!(typed.long_help.is_none());
    let long = usage_argv::help::render(Serve::spec(), Serve::command(), true)
        .expect("the root help page");
    assert!(
        long.contains("Port to listen on (default: selected at runtime)"),
        "{long}"
    );

    let kdl = Serve::to_kdl();
    assert!(!kdl.contains("default="), "{kdl}");
    let portable: LibSpec = kdl.parse().expect("the emitted spec remains portable");
    assert!(portable.cmd.flags[0].default.is_empty());
    assert_eq!(
        portable.cmd.flags[0].help.as_deref(),
        Some("Port to listen on (default: selected at runtime)")
    );
}
