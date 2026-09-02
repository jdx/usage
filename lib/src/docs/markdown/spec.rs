use crate::docs::markdown::renderer::MarkdownRenderer;
use crate::error::UsageErr;

impl MarkdownRenderer {
    pub fn render_spec(&self) -> Result<String, UsageErr> {
        let all_commands = self.spec().cmd.all_subcommands();
        let config = &self.spec().config;
        self.render_with("spec_template.md.tera", |ctx| {
            ctx.insert("all_commands", &all_commands);
            ctx.insert("config", config);
        })
    }

    pub fn render_index(&self) -> Result<String, UsageErr> {
        let all_commands = self.spec().cmd.all_subcommands();
        // So the index can link the settings page it sits beside. Without this the page was
        // written and nothing pointed at it, which for a reader who starts at the index is
        // the same as not writing it.
        let config = &self.spec().config;
        // The name the page will actually be written under, so the link cannot point somewhere
        // else than the file — including when a `settings` command has taken `settings.md`.
        self.render_with("index_template.md.tera", |ctx| {
            // An index links separate pages even when the renderer's ordinary mode is single-file.
            ctx.insert("multi", &false);
            ctx.insert("all_commands", &all_commands);
            ctx.insert("config", config);
            ctx.insert("config_page", &self.config_page());
        })
    }
}

#[cfg(test)]
mod tests {
    use crate::docs::markdown::renderer::MarkdownRenderer;
    use crate::test::SPEC_KITCHEN_SINK;
    use crate::Spec;
    use insta::assert_snapshot;

    /// `--replace-pre-with-code-fences` reaches a command's long help on the single-file page.
    ///
    /// `MarkdownRenderer::new` used to render the whole docs model eagerly, before the builder
    /// methods that set the options ran, and rendering marks each item done so the later pass
    /// no-opped. The multi-page path hid this by re-deriving each page from the raw command it
    /// is handed; the single-file page renders the stored model, so for it the flag did
    /// nothing at all — the whole option was a no-op on `usage generate markdown` without
    /// `--multi`.
    #[test]
    fn a_command_s_long_help_gets_the_rendering_options_it_was_asked_for() {
        let src = "name \"v\"\nbin \"v\"\ncmd \"go\" help=\"Go\" long_help=\"Goes.\\n\\n    v go --now\\n\\nOk.\"\n";
        let spec: Spec = src.parse().unwrap();

        let with_fences = MarkdownRenderer::new(spec.clone())
            .with_indented_blocks_to_code_fences(true)
            .render_spec()
            .unwrap();
        assert!(
            with_fences.contains("```\nv go --now\n```"),
            "the option did not reach the command's help:\n{with_fences}"
        );

        // And without it the block stays indented, so the assertion above is about the option
        // rather than about something else in the pipeline.
        let without = MarkdownRenderer::new(spec).render_spec().unwrap();
        assert!(!without.contains("```"), "{without}");
        assert!(without.contains("    v go --now"), "{without}");
    }

    /// The former builder name still reaches the same option.
    ///
    /// It is kept, not renamed away, so an existing docs build keeps compiling; that only
    /// helps if it keeps working, which nothing else here would notice.
    #[test]
    fn the_former_builder_name_still_works() {
        let src = "name \"v\"\nbin \"v\"\ncmd \"go\" help=\"Go\" long_help=\"Goes.\\n\\n    v go --now\\n\\nOk.\"\n";
        let spec: Spec = src.parse().unwrap();
        let page = MarkdownRenderer::new(spec)
            .with_replace_pre_with_code_fences(true)
            .render_spec()
            .unwrap();
        assert!(page.contains("```\nv go --now\n```"), "{page}");
    }

    /// An option set after a render still applies, because the builders drop the rendered model.
    #[test]
    fn an_option_set_after_a_render_still_takes_effect() {
        let src = "name \"v\"\nbin \"v\"\ncmd \"go\" help=\"Go\" long_help=\"Goes.\\n\\n    v go --now\\n\\nOk.\"\n";
        let spec: Spec = src.parse().unwrap();

        let ctx = MarkdownRenderer::new(spec);
        assert!(!ctx.render_spec().unwrap().contains("```"));
        let ctx = ctx.with_indented_blocks_to_code_fences(true);
        assert!(
            ctx.render_spec().unwrap().contains("```\nv go --now\n```"),
            "a render before the builder froze the model"
        );
    }

    #[test]
    fn test_render_markdown_spec() {
        let ctx = MarkdownRenderer::new(SPEC_KITCHEN_SINK.clone());
        assert_snapshot!(ctx.render_spec().unwrap(), @"
        # `mycli`

        - **Usage:** `mycli [FLAGS] <ARGS>… <SUBCOMMAND>`

        ## Arguments
        - **`<arg1>`** — arg1 description
        - **`[arg2]`** — arg2 description

          **Choices:** `choice1`, `choice2`, `choice3`

          **Default:** `default value`
        - **`<arg3>`** — arg3 long description
        - **`<argrest>…`**
        - **`[with-default]`**

          **Default:** `default value`

        ## Flags
        - **`--flag1`** — flag1 description
        - **`--flag2`** — flag2 long description

          includes a code block:

              $ echo hello world
              hello world

              more code

          Examples:

              # run with no arguments to use the interactive selector
              $ mise use

              # set the current version of node to 20.x in mise.toml of current directory
              # will write the fuzzy version (e.g.: 20)

          some docs

              $ echo hello world
              hello world
        - **`--flag3`** — flag3 description
        - **`--with-default`**

          **Default:** `default value`
        - **`--shell <shell>`**

          **Choices:** `bash`, `zsh`, `fish`

        ## `mycli plugin`

        - **Usage:** `mycli plugin <SUBCOMMAND>`
        - **Source code:** [`src/cli/plugin.rs`](https://github.com/jdx/mise/blob/main/src/cli/plugin.rs)

        ## `mycli plugin install`

        - **Usage:** `mycli plugin install [FLAGS] <plugin> <version>`
        - **Source code:** [`src/cli/plugin/install.rs`](https://github.com/jdx/mise/blob/main/src/cli/plugin/install.rs)

        install a plugin

        ### Arguments
        - **`<plugin>`**
        - **`<version>`**

        ### Flags
        - **`-g --global`**
        - **`-d --dir <dir>`**
        - **`-f --force`**
        ");
    }

    #[test]
    fn hidden_commands_are_omitted_from_single_file_markdown() {
        let spec: Spec = r#"
            name "mycli"
            bin "mycli"
            cmd "visible" help="A documented command"
            cmd "setup" hide=#true help="An internal command"
        "#
        .parse()
        .unwrap();

        let output = MarkdownRenderer::new(spec).render_spec().unwrap();

        assert!(output.contains("## `mycli visible`"), "{output}");
        assert!(!output.contains("mycli setup"), "{output}");
        assert!(!output.contains("An internal command"), "{output}");
    }

    #[test]
    fn package_metadata_reaches_markdown() {
        let spec: Spec = r#"
            name "metadata"
            bin "metadata"
            author "Example Maintainers"
            license "MIT OR Apache-2.0"
            repository "https://example.com/tool"
        "#
        .parse()
        .unwrap();
        let output = MarkdownRenderer::new(spec).render_spec().unwrap();

        assert!(
            output.contains("**Author:** Example Maintainers"),
            "{output}"
        );
        assert!(
            output.contains("**License:** MIT OR Apache-2.0"),
            "{output}"
        );
        assert!(
            output.contains("**Repository:** https://example.com/tool"),
            "{output}"
        );
    }
}
