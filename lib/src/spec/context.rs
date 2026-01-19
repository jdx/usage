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

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_new() {
        let ctx = ParsingContext::new(Path::new("/path/to/file.kdl"), "name \"test\"");
        assert_eq!(ctx.file, PathBuf::from("/path/to/file.kdl"));
        assert_eq!(ctx.spec, "name \"test\"");
    }

    #[test]
    fn test_default() {
        let ctx = ParsingContext::default();
        assert_eq!(ctx.file, PathBuf::new());
        assert_eq!(ctx.spec, "");
    }

    #[test]
    fn test_build_err() {
        let ctx = ParsingContext::new(Path::new("test.kdl"), "invalid content");
        let span: SourceSpan = (0, 7).into();
        let err = ctx.build_err("test error".to_string(), span);
        match err {
            UsageErr::InvalidInput(msg, err_span, _) => {
                assert_eq!(msg, "test error");
                assert_eq!(err_span.offset(), 0);
                assert_eq!(err_span.len(), 7);
            }
            _ => panic!("Expected InvalidInput error"),
        }
    }
}
