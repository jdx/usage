use crate::docs::markdown::tera::TERA;
use crate::docs::models::Spec;
use crate::error::UsageErr;
use itertools::Itertools;
use serde::Serialize;

fn escape_md(value: &str, html_encode: bool) -> String {
    let mut in_fenced_code_block = false;

    value
        .lines()
        .map(|line| {
            if !html_encode {
                return line.to_string();
            }
            // Indented code is handled before fence state. This is safe because
            // `replace_code_fences` always emits closing fences at column zero.
            if line.starts_with("    ") {
                return line.to_string();
            }
            if in_fenced_code_block {
                if line.trim_end() == "```" {
                    in_fenced_code_block = false;
                }
                return line.to_string();
            }
            // Support the conventional fence shape emitted by `replace_code_fences`
            // without attempting to parse the full Markdown specification.
            if line
                .strip_prefix("```")
                .is_some_and(|suffix| !suffix.starts_with('`'))
            {
                in_fenced_code_block = true;
                return line.to_string();
            }
            // replace '<' with '&lt;' but not inside code blocks
            xx::regex!(r"(`[^`]*`)|(<)")
                .replace_all(line, |caps: &regex::Captures| {
                    if caps.get(1).is_some() {
                        caps.get(1).unwrap().as_str().to_string()
                    } else {
                        "&lt;".to_string()
                    }
                })
                .to_string()
        })
        .join("\n")
}

#[derive(Debug, Clone)]
pub struct MarkdownRenderer {
    pub(crate) spec: Spec,
    pub(crate) header_level: usize,
    pub(crate) multi: bool,
    tera_ctx: tera::Context,
    url_prefix: Option<String>,
    html_encode: bool,
    replace_pre_with_code_fences: bool,
}

impl MarkdownRenderer {
    pub fn new(spec: crate::Spec) -> Self {
        let mut renderer = Self {
            spec: spec.into(),
            header_level: 1,
            multi: false,
            tera_ctx: tera::Context::new(),
            url_prefix: None,
            html_encode: true,
            replace_pre_with_code_fences: false,
        };
        let mut spec = renderer.spec.clone();
        spec.render_md(&renderer);
        renderer.spec = spec;
        renderer
    }

    pub fn with_header_level(mut self, header_level: usize) -> Self {
        self.header_level = header_level;
        self
    }

    pub fn with_multi(mut self, index: bool) -> Self {
        self.multi = index;
        self
    }

    pub fn with_url_prefix<S: Into<String>>(mut self, url_prefix: S) -> Self {
        self.url_prefix = Some(url_prefix.into());
        self
    }

    pub fn with_html_encode(mut self, html_encode: bool) -> Self {
        self.html_encode = html_encode;
        self
    }

    pub fn with_replace_pre_with_code_fences(mut self, replace_pre_with_code_fences: bool) -> Self {
        self.replace_pre_with_code_fences = replace_pre_with_code_fences;
        self
    }

    pub(crate) fn insert<T: Serialize + ?Sized, S: Into<String>>(&mut self, key: S, val: &T) {
        self.tera_ctx.insert(key.into(), val);
    }

    fn tera_ctx(&self) -> tera::Context {
        let mut ctx = self.tera_ctx.clone();
        ctx.insert("spec", &self.spec);
        ctx.insert("header_level", &self.header_level);
        ctx.insert("multi", &self.multi);
        ctx.insert("url_prefix", &self.url_prefix);
        ctx.insert("html_encode", &self.html_encode);
        ctx
    }

    pub(crate) fn render(&self, template_name: &str) -> Result<String, UsageErr> {
        let mut tera = TERA.clone();

        let html_encode = self.html_encode;
        tera.register_filter(
            "escape_md",
            move |value: &tera::Value,
                  _: tera::Kwargs,
                  _: &tera::State|
                  -> tera::TeraResult<String> {
                let value = value.as_str().unwrap();
                let value = escape_md(value, html_encode);
                Ok(value)
            },
        );

        Ok(tera.render(template_name, &self.tera_ctx())?)
    }

    pub(crate) fn replace_code_fences(&self, md: String) -> String {
        if !self.replace_pre_with_code_fences {
            return md;
        }
        // TODO: handle fences inside of <pre> or <code>
        let mut in_code_block = false;
        let mut new_md = String::new();
        for line in md.lines() {
            if let Some(line) = line.strip_prefix("    ") {
                if in_code_block {
                    new_md.push_str(&format!("{line}\n"));
                } else {
                    new_md.push_str(&format!("```\n{line}\n"));
                    in_code_block = true;
                }
            } else {
                if in_code_block {
                    new_md.push_str("```\n");
                    in_code_block = false;
                }
                new_md.push_str(&format!("{line}\n"));
            }
        }
        if in_code_block {
            new_md.push_str("```\n");
        }
        new_md.replace("```\n\n```\n", "\n")
    }
}

#[cfg(test)]
mod tests {
    use super::escape_md;
    use pretty_assertions::assert_eq;

    #[test]
    fn escapes_html_around_fenced_code_blocks() {
        let input = "before <\n```\ninside <\n```  \nafter <";
        let expected = "before &lt;\n```\ninside <\n```  \nafter &lt;";

        assert_eq!(escape_md(input, true), expected);
    }

    #[test]
    fn supports_fence_info_strings() {
        let input = "```bash\necho <value>\n```\nafter <";
        let expected = "```bash\necho <value>\n```\nafter &lt;";

        assert_eq!(escape_md(input, true), expected);
    }

    #[test]
    fn leaves_unclosed_fences_unescaped() {
        let input = "```\necho <value>";

        assert_eq!(escape_md(input, true), input);
    }

    #[test]
    fn ignores_indented_and_longer_fences() {
        let input = "    ```\nindented <\n````\nlonger <";
        let expected = "    ```\nindented &lt;\n````\nlonger &lt;";

        assert_eq!(escape_md(input, true), expected);
    }

    #[test]
    fn leaves_markdown_unchanged_when_html_encoding_is_disabled() {
        let input = "before <\n```\ninside <\n```\nafter <";

        assert_eq!(escape_md(input, false), input);
    }
}
