use usage_derive::Cli;

#[derive(Cli)]
#[usage(
    bin = "ex",
    default_subcommand = "install",
    default_subcommand_help,
    unknown_flags = "error"
)]
struct Ex {
    #[usage(long, global = true)]
    #[allow(dead_code)]
    color: bool,
    #[usage(subcommand)]
    #[allow(dead_code)]
    command: Option<Commands>,
}

#[derive(usage_derive::Subcommands)]
enum Commands {
    Install,
    #[allow(dead_code)]
    Query,
}

#[test]
fn the_opt_in_survives_derive_kdl_and_json_roundtrips() {
    let kdl = Ex::to_kdl();
    assert!(kdl.contains("default_subcommand_help #true"), "{kdl}");
    let spec: usage::Spec = kdl.parse().unwrap();
    assert!(spec.default_subcommand_help);
    let json = serde_json::to_string(&spec).unwrap();
    let json: serde_json::Value = serde_json::from_str(&json).unwrap();
    assert_eq!(json["default_subcommand_help"], true);
    let again: usage::Spec = spec.to_string().parse().unwrap();
    assert!(again.default_subcommand_help);
}

#[test]
fn parent_help_names_the_default_and_appends_its_page() {
    let page = usage_argv::help::long_help(Ex::spec(), &["ex"], &[Ex::spec().root]);
    assert!(page.contains("install"), "{page}");
    assert!(page.contains("(default)"), "{page}");
    assert!(page.contains("Default command: install"), "{page}");
    assert!(page.contains("Usage: ex install"), "{page}");
}

#[test]
fn the_appended_default_page_does_not_repeat_the_root_global_flags() {
    // The root's own `Flags:` section already lists `--color`; the appended `install` page
    // must not show it again under its own `Global flags:` section — on both the short and
    // the long page, since each has its own render path down to `with_default_command_help`.
    for page in [
        usage_argv::help::short_help(Ex::spec(), &["ex"], &[Ex::spec().root]),
        usage_argv::help::long_help(Ex::spec(), &["ex"], &[Ex::spec().root]),
    ] {
        let (root, appended) = page
            .split_once("Default command: ")
            .expect("the default command's page is appended");
        assert!(root.contains("--color"), "{page}");
        assert!(
            !appended.contains("--color") && !appended.contains("Global flags:"),
            "the appended page should not repeat the root's --color under its own Global flags section:\n{page}"
        );
    }
}

#[test]
fn the_appended_default_page_is_styled_like_the_parent() {
    let page = usage_argv::help::render_styled(
        Ex::spec(),
        Ex::spec().root.cmd,
        true,
        usage_argv::help::Style::COLOURED,
    )
    .expect("the root page");
    // The command name in "Commands:" and in "Default command:" should carry the same
    // (green + bold) styling as any other subcommand name on the page.
    assert!(
        page.contains("Default command: \u{1b}[1;32minstall\u{1b}[0m"),
        "{page:?}"
    );
    let (_, appended) = page
        .split_once("Default command: ")
        .expect("the default command's page is appended");
    assert!(
        appended.contains("\u{1b}[1;33mUsage:\u{1b}[0m"),
        "the appended default page should carry the same ANSI styling as the parent:\n{page:?}"
    );

    // usage-lib's reference renderer styles the same line the same way.
    let lib_spec: usage::Spec = Ex::to_kdl().parse().expect("the derive emits a valid spec");
    let lib_page = usage::docs::cli::render_help_styled(
        &lib_spec,
        &lib_spec.cmd,
        true,
        usage::docs::cli::Style::COLOURED,
    );
    assert!(
        lib_page.contains("Default command: \u{1b}[1;32minstall\u{1b}[0m"),
        "{lib_page:?}"
    );
}
