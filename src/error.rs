use miette::{Diagnostic, SourceSpan};
use thiserror::Error;

#[derive(Error, Diagnostic, Debug)]
pub enum UsageErr {
    #[error("Invalid flag: {0}")]
    InvalidFlag(String, #[label] SourceSpan, #[source_code] String),

    #[error("Invalid input: {0}")]
    InvalidInput(String, #[label] SourceSpan, #[source_code] String),

    #[error(transparent)]
    IO(#[from] std::io::Error),

    #[error(transparent)]
    #[diagnostic(transparent)]
    KdlError(#[from] kdl::KdlError),
}
