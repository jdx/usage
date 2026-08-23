use usage::docs::markdown::MarkdownRenderer;
use usage_argv::help;
use usage_derive::Cli;

#[derive(Cli)]
#[usage(bin = "ex")]
#[allow(dead_code)]
struct Ex {
    /// Override the discovered configuration.
    #[usage(
        long,
        note = "Only changes import resolution.",
        warning = "Type-aware linting discovers its own configuration."
    )]
    config: Option<String>,

    /// File to inspect.
    #[usage(note = "Directories are traversed recursively.")]
    file: Option<String>,
}

#[test]
fn semantic_blocks_adapt_to_terminal_help_and_markdown() {
    let terminal = help::render(Ex::spec(), Ex::spec().root.cmd, true).expect("long help");
    assert!(
        terminal.contains("Note: Only changes import resolution."),
        "{terminal}"
    );
    assert!(
        terminal.contains("Warning: Type-aware linting discovers its own configuration."),
        "{terminal}"
    );
    assert!(
        terminal.contains("Note: Directories are traversed recursively."),
        "{terminal}"
    );
    assert!(!terminal.contains("::: warning"), "{terminal}");

    let kdl = Ex::to_kdl();
    assert!(
        kdl.contains("note \"Only changes import resolution.\""),
        "{kdl}"
    );
    assert!(
        kdl.contains("warning \"Type-aware linting discovers its own configuration.\""),
        "{kdl}"
    );

    let spec: usage::Spec = kdl.parse().expect("generated spec");
    let portable_terminal = usage::docs::cli::render_help(&spec, &spec.cmd, true);
    assert!(
        portable_terminal.contains("Note: Only changes import resolution."),
        "{portable_terminal}"
    );
    assert!(
        portable_terminal.contains("Warning: Type-aware linting discovers its own configuration."),
        "{portable_terminal}"
    );
    assert!(
        portable_terminal.contains("Note: Directories are traversed recursively."),
        "{portable_terminal}"
    );

    let markdown = MarkdownRenderer::new(spec.clone())
        .render_cmd(&spec.cmd)
        .expect("markdown page");
    assert!(
        markdown.contains("> **Note:** Only changes import resolution."),
        "{markdown}"
    );
    assert!(
        markdown.contains("> **Warning:** Type-aware linting discovers its own configuration."),
        "{markdown}"
    );
}
