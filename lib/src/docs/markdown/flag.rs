use crate::docs::markdown::renderer::MarkdownRenderer;
use crate::docs::markdown::tera::TERA;
use crate::error::UsageErr;
use crate::SpecFlag;

impl MarkdownRenderer {
    pub fn render_flag(&self, flag: &SpecFlag) -> Result<String, UsageErr> {
        let tera = TERA.clone();
        let mut ctx = self.clone();
        ctx.insert("flag", &flag);

        Ok(tera.render("flag_template.md.tera", &ctx.tera_ctx())?)
    }
}

#[cfg(test)]
mod tests {
    use crate::docs::markdown::renderer::MarkdownRenderer;
    use crate::spec;
    use insta::assert_snapshot;

    #[test]
    fn test_render_markdown_flag() {
        let spec = spec! { r#"flag "--flag1" help="flag1 description""# }.unwrap();
        let ctx = MarkdownRenderer::new(&spec);
        assert_snapshot!(ctx.render_flag(&spec.cmd.flags[0]).unwrap(), @r#"


        flag1 description
        "#);
    }
}
