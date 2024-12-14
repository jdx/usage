use crate::docs::markdown::renderer::MarkdownRenderer;
use crate::docs::models::SpecCommand;
use crate::error::UsageErr;

impl MarkdownRenderer {
    pub fn render_cmd(&self, cmd: &crate::SpecCommand) -> Result<String, UsageErr> {
        let mut cmd = SpecCommand::from(cmd);
        cmd.render_md(self);
        let mut ctx = self.clone();
        ctx.insert("cmd", &cmd);
        ctx.render("cmd_template.md.tera")
    }
}

#[cfg(test)]
mod tests {
    use crate::docs::markdown::renderer::MarkdownRenderer;
    use crate::test::SPEC_KITCHEN_SINK;
    use insta::assert_snapshot;

    #[test]
    fn test_render_markdown_cmd() {
        let ctx = MarkdownRenderer::new(SPEC_KITCHEN_SINK.clone())
            .with_multi(true)
            .with_replace_pre_with_code_fences(true);
        assert_snapshot!(ctx.render_cmd(&SPEC_KITCHEN_SINK.cmd).unwrap(), @r"
        # `mycli`

        - **Usage**: `mycli [FLAGS] <ARGS>… <SUBCOMMAND>`

        ## Arguments

        ### `<arg1>`

        arg1 description

        ### `[arg2]`

        arg2 description

        **Choices:**

        - `choice1`
        - `choice2`
        - `choice3`

        **Default:** `default value`

        ### `<arg3>`

        arg3 long description

        ### `<argrest>...`

        ### `[with-default]`

        **Default:** `default value`

        ## Flags

        ### `--flag1`

        flag1 description

        ### `--flag2`

        flag2 long description

        includes a code block:

        ```
        $ echo hello world
        hello world

        more code
        ```

        some docs

        ```
        $ echo hello world
        hello world
        ```

        ### `--flag3`

        flag3 description

        ### `--with-default`

        **Default:** `default value`

        ### `--shell <shell>`

        **Choices:**

        - `bash`
        - `zsh`
        - `fish`

        ## Subcommands

        - [`mycli plugin <SUBCOMMAND>`](/plugin.md)
        ");
    }
}
