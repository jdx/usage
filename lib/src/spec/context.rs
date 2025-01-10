use crate::error::UsageErr;
use miette::{NamedSource, SourceSpan};
use std::path::{Path, PathBuf};

#[derive(Debug, Default)]
pub struct ParsingContext {
    pub(crate) file: PathBuf,
    pub(crate) spec: String,
}

impl ParsingContext {
    pub(crate) fn new(file: &Path, spec: &str) -> Self {
        Self {
            file: file.to_path_buf(),
            spec: spec.to_string(),
        }
    }

    pub(crate) fn build_err(&self, msg: String, span: SourceSpan) -> UsageErr {
        let source = NamedSource::new(self.file.to_string_lossy(), self.spec.clone());
        UsageErr::InvalidInput(msg, span, source)
    }
}
