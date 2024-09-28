use crate::Spec;
use serde::Serialize;

#[derive(Debug, Clone)]
pub struct MarkdownRenderer {
    pub(crate) header_level: usize,
    pub(crate) multi: bool,
    pub(crate) spec: Spec,
    url_prefix: Option<String>,
    tera_ctx: tera::Context,
}

impl MarkdownRenderer {
    pub fn new(spec: &Spec) -> Self {
        Self {
            header_level: 1,
            multi: false,
            spec: spec.clone(),
            tera_ctx: tera::Context::new(),
            url_prefix: None,
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

    pub(crate) fn insert<T: Serialize + ?Sized, S: Into<String>>(&mut self, key: S, val: &T) {
        self.tera_ctx.insert(key, val);
    }

    pub(crate) fn tera_ctx(&self) -> tera::Context {
        let mut ctx = self.tera_ctx.clone();
        ctx.insert("header_level", &self.header_level);
        ctx.insert("multi", &self.multi);
        ctx.insert("spec", &self.spec);
        ctx.insert("url_prefix", &self.url_prefix);
        ctx
    }
}
