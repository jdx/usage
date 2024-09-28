use crate::docs::markdown::renderer::MarkdownRenderer;
use crate::docs::markdown::tera::TERA;
use crate::error::UsageErr;
use crate::SpecCommand;

impl MarkdownRenderer {
    pub fn render_cmd(&self, cmd: &SpecCommand) -> Result<String, UsageErr> {
        let mut ctx = self.clone();

        ctx.insert("cmd", cmd);

        Ok(TERA.render("cmd_template.md.tera", &ctx.tera_ctx())?)
    }
}

#[cfg(test)]
mod tests {
    use crate::docs::markdown::renderer::MarkdownRenderer;
    use crate::test::SPEC_KITCHEN_SINK;
    use insta::assert_snapshot;

    #[test]
    fn test_render_markdown_cmd() {
        let ctx = MarkdownRenderer::new(&SPEC_KITCHEN_SINK).with_multi(true);
        assert_snapshot!(ctx.render_cmd(&SPEC_KITCHEN_SINK.cmd).unwrap(), @r####"
        # `mycli [args] [flags] [subcommand]`

        ## Arguments

        ### `<arg1>`

        arg1 description

        ### `<arg2>`

        arg2 description

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

        ## Subcommands

        * [`mycli plugin [subcommand]`](/plugin.md)
        "####);
    }
}
