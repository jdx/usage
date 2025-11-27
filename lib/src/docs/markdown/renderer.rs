use crate::docs::markdown::tera::TERA;
use crate::docs::models::Spec;
use crate::error::UsageErr;
use itertools::Itertools;
use serde::Serialize;
use std::collections::HashMap;
use xx::regex;

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
        self.tera_ctx.insert(key, val);
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
            move |value: &tera::Value, _: &HashMap<String, tera::Value>| {
                let value = value.as_str().unwrap();
                let mut in_code_block = false;
                let value = value
                    .lines()
                    .map(|line| {
                        if line.trim_start().starts_with("```") {
                            in_code_block = !in_code_block;
                            return line.to_string();
                        }
                        if !html_encode || line.starts_with("    ") || in_code_block {
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
