use crate::docs::markdown::renderer::MarkdownRenderer;
use crate::docs::models::SpecFlag;
use crate::error::UsageErr;

impl MarkdownRenderer {
    pub fn render_flag(&self, flag: &crate::SpecFlag) -> Result<String, UsageErr> {
        let mut flag = SpecFlag::from(flag);
        flag.render_md(self);
        let mut ctx = self.clone();
        ctx.insert("flag", &flag);
        ctx.render("flag_template.md.tera")
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
        let ctx = MarkdownRenderer::new(spec.clone()).with_replace_pre_with_code_fences(true);
        assert_snapshot!(ctx.render_flag(&spec.cmd.flags[0]).unwrap(), @"flag1 description");
    }
}
