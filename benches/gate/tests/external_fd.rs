use std::ffi::OsStr;

#[test]
fn exec_terminator_returns_following_flags_to_fd() {
    let parsed = shadow_fd::Cli::parse_from(&[
        OsStr::new("-x"),
        OsStr::new("echo"),
        OsStr::new(";"),
        OsStr::new("--hidden"),
    ])
    .expect("the terminator should end --exec values");

    assert_eq!(parsed.exec, ["echo"]);
    assert!(parsed.hidden);
}

#[test]
fn optional_value_flags_accept_their_bare_forms() {
    for flag in ["--hyperlink", "--strip-cwd-prefix", "--gen-completions"] {
        shadow_fd::Cli::parse_from(&[OsStr::new(flag)])
            .unwrap_or_else(|error| panic!("{flag} should accept an omitted value: {error:?}"));
    }
}

#[test]
fn the_capture_keeps_fd_clap_parser_policies() {
    assert!(shadow_fd::Cli::parse_from(&[OsStr::new("--definitely-not-an-fd-flag")]).is_err());
    assert!(shadow_fd::Cli::parse_from(&[
        OsStr::new("--max-results"),
        OsStr::new("1"),
        OsStr::new("--max-results"),
        OsStr::new("2"),
    ])
    .is_err());
}
