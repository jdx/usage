use crate::docs::markdown::renderer::MarkdownRenderer;
use crate::docs::models::SpecArg;
use crate::error::UsageErr;

impl MarkdownRenderer {
    pub fn render_arg(&self, arg: &crate::SpecArg) -> Result<String, UsageErr> {
        let mut arg = SpecArg::from(arg);
        arg.render_md(self);
        self.render_with("arg_template.md.tera", |ctx| ctx.insert("arg", &arg))
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
        let ctx = MarkdownRenderer::new(spec.clone());
        assert_snapshot!(ctx.render_arg(&spec.cmd.args[0]).unwrap(), @r"
- **`<arg1>`** — arg1 description");
    }
}
