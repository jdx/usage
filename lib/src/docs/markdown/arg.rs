use crate::docs::markdown::renderer::MarkdownRenderer;
use crate::docs::markdown::tera::TERA;
use crate::error::UsageErr;
use crate::SpecArg;

impl MarkdownRenderer {
    pub fn render_arg(&self, arg: &SpecArg) -> Result<String, UsageErr> {
        let tera = TERA.clone();
        let mut ctx = self.clone();
        ctx.insert("arg", arg);

        Ok(tera.render("arg_template.md.tera", &ctx.tera_ctx())?)
    }
}

#[cfg(test)]
mod tests {
    use crate::docs::markdown::renderer::MarkdownRenderer;
    use crate::spec;
    use insta::assert_snapshot;

    #[test]
    fn test_render_markdown_arg() {
        let spec = spec! { r#"arg "arg1" help="arg1 description""# }.unwrap();
        let ctx = MarkdownRenderer::new(&spec);
        assert_snapshot!(ctx.render_arg(&spec.cmd.args[0]).unwrap(), @r#"


        arg1 description
        "#);
    }
}
