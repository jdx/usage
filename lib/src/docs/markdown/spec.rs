use crate::docs::markdown::renderer::MarkdownRenderer;
use crate::error::UsageErr;

impl MarkdownRenderer {
    pub fn render_spec(&self) -> Result<String, UsageErr> {
        let mut ctx = self.clone();
        ctx.insert("all_commands", &self.spec.cmd.all_subcommands());
        ctx.render("spec_template.md.tera")
    }

    pub fn render_index(&self) -> Result<String, UsageErr> {
        let mut ctx = self.clone();
        ctx.multi = false;
        ctx.insert("all_commands", &self.spec.cmd.all_subcommands());
        ctx.render("index_template.md.tera")
    }
}

#[cfg(test)]
mod tests {
    use crate::docs::markdown::renderer::MarkdownRenderer;
    use crate::test::SPEC_KITCHEN_SINK;
    use insta::assert_snapshot;

    #[test]
    fn test_render_markdown_spec() {
        let ctx = MarkdownRenderer::new(&SPEC_KITCHEN_SINK);
        assert_snapshot!(ctx.render_spec().unwrap(), @r#####"
        # `mycli`

        **Usage**: `mycli [FLAGS] <ARGS>… <SUBCOMMAND>`

        ## Arguments

        ### `<arg1>`

        arg1 description

        ### `<arg2>`

        arg2 description

        #### Choices

        - `choice1`
        - `choice2`
        - `choice3`

        #### Default

        `default value`

        ### `<arg3>`

        arg3 long description

        ### `<argrest>...`

        ## Flags

        ### `--flag1`

        flag1 description

        ### `--flag2`

        flag2 long description

        ### `--flag3`

        flag3 description

        ### `--shell <shell>`

        #### Choices

        - `bash`
        - `zsh`
        - `fish`

        ## `mycli plugin`

        **Usage**: `mycli plugin <SUBCOMMAND>`

        ## `mycli plugin install`

        **Usage**: `mycli plugin install [FLAGS] <plugin> <version>`

        ### Arguments

        #### `<plugin>`

        #### `<version>`

        ### Flags

        #### `-g --global`

        #### `-d --dir <dir>`

        #### `-f --force`
        "#####);
    }
}
