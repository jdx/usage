use crate::docs::markdown::renderer::MarkdownRenderer;
use crate::error::UsageErr;

impl MarkdownRenderer {
    pub fn render_spec(&self) -> Result<String, UsageErr> {
        let all_commands = self.spec.cmd.all_subcommands();
        // From the raw block for the same reason `render_config` does: the copy on `self.spec`
        // was rendered by `new` before the builder options existed, and rendering happens once.
        let mut config = crate::docs::models::SpecConfig::from(&self.raw_config);
        config.render_md(self);
        self.render_with("spec_template.md.tera", |ctx| {
            ctx.insert("all_commands", &all_commands);
            ctx.insert("config", &config);
        })
    }

    pub fn render_index(&self) -> Result<String, UsageErr> {
        let all_commands = self.spec.cmd.all_subcommands();
        // So the index can link the settings page it sits beside. Without this the page was
        // written and nothing pointed at it, which for a reader who starts at the index is
        // the same as not writing it.
        let config = crate::docs::models::SpecConfig::from(&self.raw_config);
        // The name the page will actually be written under, so the link cannot point somewhere
        // else than the file — including when a `settings` command has taken `settings.md`.
        self.render_with("index_template.md.tera", |ctx| {
            // An index links separate pages even when the renderer's ordinary mode is single-file.
            ctx.insert("multi", &false);
            ctx.insert("all_commands", &all_commands);
            ctx.insert("config", &config);
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

    #[test]
    fn test_render_markdown_spec() {
        let ctx = MarkdownRenderer::new(SPEC_KITCHEN_SINK.clone());
        assert_snapshot!(ctx.render_spec().unwrap(), @"
        # `mycli`

        - **Usage**: `mycli [FLAGS] <ARGS>… <SUBCOMMAND>`

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

        - **Usage**: `mycli plugin <SUBCOMMAND>`
        - **Source code**: [`src/cli/plugin.rs`](https://github.com/jdx/mise/blob/main/src/cli/plugin.rs)

        ## `mycli plugin install`

        - **Usage**: `mycli plugin install [FLAGS] <plugin> <version>`
        - **Source code**: [`src/cli/plugin/install.rs`](https://github.com/jdx/mise/blob/main/src/cli/plugin/install.rs)

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
            output.contains("**author**: Example Maintainers"),
            "{output}"
        );
        assert!(
            output.contains("**license**: MIT OR Apache-2.0"),
            "{output}"
        );
        assert!(
            output.contains("**repository**: https://example.com/tool"),
            "{output}"
        );
    }
}
