use usage::docs::markdown::{MarkdownRenderer, MarkdownTheme};
use usage_argv::help;
use usage_derive::{Args, Cli, Subcommands};

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

#[derive(Args)]
#[allow(dead_code)]
struct FlattenedAdmonitions {
    #[usage(note = "Flattened argument note.")]
    input: Option<String>,

    #[usage(long, warning = "Flattened flag warning.")]
    config: bool,
}

#[derive(Subcommands)]
#[allow(dead_code)]
enum FlattenedCommands {
    Run(FlattenedAdmonitions),
}

#[derive(Cli)]
#[usage(bin = "flat", flatten_help)]
#[allow(dead_code)]
struct FlattenedCli {
    #[usage(subcommand)]
    command: Option<FlattenedCommands>,
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

    let compact = MarkdownRenderer::new(spec.clone())
        .render_cmd(&spec.cmd)
        .expect("markdown page");
    assert!(
        compact.contains("  > **Note:** Only changes import resolution."),
        "{compact}"
    );
    assert!(
        compact.contains("  > **Warning:** Type-aware linting discovers its own configuration."),
        "{compact}"
    );

    let detailed = MarkdownRenderer::new(spec.clone())
        .with_theme(MarkdownTheme::Detailed)
        .render_cmd(&spec.cmd)
        .expect("detailed markdown page");
    assert!(
        detailed.contains("> **Note:** Only changes import resolution."),
        "{detailed}"
    );
    assert!(
        detailed.contains("> **Warning:** Type-aware linting discovers its own configuration."),
        "{detailed}"
    );
}

#[test]
fn semantic_blocks_survive_flattened_and_nested_value_layouts() {
    let flattened = help::render(FlattenedCli::spec(), FlattenedCli::spec().root.cmd, true)
        .expect("flattened long help");
    assert!(
        flattened.contains("Note: Flattened argument note."),
        "{flattened}"
    );
    assert!(
        flattened.contains("Warning: Flattened flag warning."),
        "{flattened}"
    );

    let mut spec: usage::Spec = Ex::to_kdl().parse().expect("generated spec");
    let config = spec
        .cmd
        .flags
        .iter_mut()
        .find(|flag| flag.name == "config")
        .expect("config flag");
    config
        .arg
        .as_mut()
        .expect("config value")
        .admonitions
        .push(usage::SpecAdmonition::note(
            "Nested value first.\n\nNested value last.",
        ));

    let portable_terminal = usage::docs::cli::render_help(&spec, &spec.cmd, true);
    assert!(
        portable_terminal.contains("Note: Nested value first.\n\n    Nested value last."),
        "{portable_terminal:?}"
    );
    assert!(
        portable_terminal
            .lines()
            .all(|line| line.trim_end() == line),
        "{portable_terminal:?}"
    );

    let compact = MarkdownRenderer::new(spec.clone())
        .render_cmd(&spec.cmd)
        .expect("markdown page");
    assert!(
        compact.contains("  > **Note:** Nested value first.\n  > \n  > Nested value last."),
        "{compact:?}"
    );

    let detailed = MarkdownRenderer::new(spec.clone())
        .with_theme(MarkdownTheme::Detailed)
        .render_cmd(&spec.cmd)
        .expect("detailed markdown page");
    assert!(
        detailed.contains("> **Note:** Nested value first.\n> \n> Nested value last."),
        "{detailed:?}"
    );
}
