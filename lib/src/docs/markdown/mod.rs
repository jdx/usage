use crate::error::UsageErr;
use tera::{Context, Tera};

const SPEC_TEMPLATE: &str = include_str!("cmd_template.tera");

impl crate::spec::Spec {
    pub fn render_markdown(&self) -> Result<String, UsageErr> {
        let mut ctx = Context::new();
        ctx.insert("header", "#");
        ctx.insert("bin", &self.bin);
        ctx.insert("cmd", &self.cmd);
        let out = Tera::one_off(SPEC_TEMPLATE, &ctx, false)?;
        Ok(out)
    }
}

#[cfg(test)]
mod tests {
    use crate::Spec;
    use insta::assert_snapshot;

    #[test]
    fn test_render_markdown() {
        let spec: Spec = r#"
        bin "mycli"
        arg "arg1" help="arg1 description"
        arg "arg2" help="arg2 description" default="default value" {
            choices "choice1" "choice2" "choice3"
        }
        arg "arg3" help="arg3 description" required=true long_help="arg3 long description"
        arg "argrest" var=true
        
        flag "--flag1" help="flag1 description"
        flag "--flag2" help="flag2 description" long_help="flag2 long description"
        flag "--flag3" help="flag3 description" negate="--no-flag3"
        "#
        .parse()
        .unwrap();
        assert_snapshot!(spec.render_markdown().unwrap());
    }
}
