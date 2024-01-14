use crate::Spec;
use miette::{Diagnostic, NamedSource, SourceSpan};
use thiserror::Error;

#[derive(Error, Diagnostic, Debug)]
pub enum UsageErr {
    #[error("Invalid flag: {0}")]
    InvalidFlag(String, #[label] SourceSpan, #[source_code] String),

    #[error("Invalid usage config")]
    InvalidInput(
        String,
        #[label = "{0}"] SourceSpan,
        #[source_code] NamedSource,
    ),

    #[error(transparent)]
    IO(#[from] std::io::Error),

    #[error(transparent)]
    Strum(#[from] strum::ParseError),

    #[error(transparent)]
    #[diagnostic(transparent)]
    KdlError(#[from] kdl::KdlError),
}

impl UsageErr {
    pub fn new(msg: String, span: &SourceSpan) -> Self {
        let named_source = Spec::get_parsing_file();
        Self::InvalidInput(msg, *span, named_source)
    }
}

#[macro_export]
macro_rules! bail_parse {
    ($span:expr, $fmt:literal) => {{
        let msg = format!($fmt);
        return std::result::Result::Err(UsageErr::new(msg, $span.span()));
    }};
    ($span:expr, $fmt:literal, $($arg:tt)*) => {{
        let msg = format!($fmt, $($arg)*);
        return std::result::Result::Err(UsageErr::new(msg, $span.span()));
    }};
}
