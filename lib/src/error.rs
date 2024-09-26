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

    #[error("Invalid usage config")]
    #[diagnostic(transparent)]
    Miette(#[from] miette::MietteError),

    #[error(transparent)]
    IO(#[from] std::io::Error),

    #[error(transparent)]
    Strum(#[from] strum::ParseError),

    #[error(transparent)]
    FromUtf8Error(#[from] std::string::FromUtf8Error),

    #[error(transparent)]
    #[diagnostic(transparent)]
    KdlError(#[from] kdl::KdlError),

    #[error(transparent)]
    #[diagnostic(transparent)]
    XXError(#[from] xx::error::XXError),
}
pub type Result<T> = std::result::Result<T, UsageErr>;

#[macro_export]
macro_rules! bail_parse {
    ($ctx:expr, $span:expr, $fmt:literal) => {{
        let msg = format!($fmt);
        let err = $ctx.build_err(msg, $span);
        return std::result::Result::Err(err);
    }};
    ($ctx:expr, $span:expr, $fmt:literal, $($arg:tt)*) => {{
        let msg = format!($fmt, $($arg)*);
        let err = $ctx.build_err(msg, $span);
        return std::result::Result::Err(err);
    }};
}
