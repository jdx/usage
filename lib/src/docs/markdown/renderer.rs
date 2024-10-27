use crate::docs::markdown::tera::TERA;
use crate::error::UsageErr;
use crate::Spec;
use itertools::Itertools;
use serde::Serialize;
use std::collections::HashMap;
use xx::regex;

#[derive(Debug, Clone)]
pub struct MarkdownRenderer {
    pub(crate) header_level: usize,
    pub(crate) multi: bool,
    pub(crate) spec: Spec,
    url_prefix: Option<String>,
    tera_ctx: tera::Context,
    html_encode: bool,
}

impl MarkdownRenderer {
    pub fn new(spec: &Spec) -> Self {
        Self {
            header_level: 1,
            multi: false,
            spec: spec.clone(),
            tera_ctx: tera::Context::new(),
            url_prefix: None,
            html_encode: true,
        }
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

    pub(crate) fn insert<T: Serialize + ?Sized, S: Into<String>>(&mut self, key: S, val: &T) {
        self.tera_ctx.insert(key, val);
    }

    fn tera_ctx(&self) -> tera::Context {
        let mut ctx = self.tera_ctx.clone();
        ctx.insert("header_level", &self.header_level);
        ctx.insert("multi", &self.multi);
        ctx.insert("spec", &self.spec);
        ctx.insert("url_prefix", &self.url_prefix);
        ctx.insert("html_encode", &self.html_encode);
        ctx
    }

    pub(crate) fn render(&self, template_name: &str) -> Result<String, UsageErr> {
        let mut tera = TERA.clone();

        let html_encode = self.html_encode;
        tera.register_filter(
            "escape_md",
            move |value: &tera::Value, _: &HashMap<String, tera::Value>| {
                let value = value.as_str().unwrap();
                let value = value
                    .lines()
                    .map(|line| {
                        if !html_encode || line.starts_with("    ") {
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
                    .join("\n");
                Ok(value.into())
            },
        );
        let path_re =
            regex!(r"https://(github.com/[^/]+/[^/]+|gitlab.com/[^/]+/[^/]+/-)/blob/[^/]+/");
        tera.register_function("source_code_link", |args: &HashMap<String, tera::Value>| {
            let spec = args.get("spec").unwrap().as_object().unwrap();
            let cmd = args.get("cmd").unwrap().as_object().unwrap();
            let full_cmd = cmd.get("full_cmd").unwrap().as_array();
            let source_code_link_template = spec
                .get("source_code_link_template")
                .and_then(|v| v.as_str());
            if let (Some(full_cmd), Some(source_code_link_template)) =
                (full_cmd, source_code_link_template)
            {
                if full_cmd.is_empty() {
                    return Ok("".into());
                }
                let mut ctx = tera::Context::new();
                let path = full_cmd.iter().map(|v| v.as_str().unwrap()).join("/");
                ctx.insert("spec", spec);
                ctx.insert("cmd", cmd);
                ctx.insert("path", &path);
                let href = TERA.clone().render_str(source_code_link_template, &ctx)?;
                let friendly = path_re.replace_all(&href, "").to_string();
                let link = if path_re.is_match(&href) {
                    format!("[`{friendly}`]({href})")
                } else {
                    format!("[{friendly}]({href})")
                };
                Ok(link.into())
            } else {
                Ok("".into())
            }
        });

        Ok(tera.render(template_name, &self.tera_ctx())?)
    }
}
