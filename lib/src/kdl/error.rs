use std::{error::Error, fmt::Display, sync::Arc};

use crate::miette::SourceSpan;

#[derive(Debug, Clone, Eq, PartialEq)]
pub struct KdlError {
    pub input: Arc<String>,
    pub diagnostics: Vec<KdlDiagnostic>,
}

impl KdlError {
    pub(crate) fn render(&self) -> String {
        self.diagnostics
            .iter()
            .map(|diagnostic| {
                crate::miette::render_source(
                    diagnostic.message.as_deref().unwrap_or("Unexpected error"),
                    "",
                    &diagnostic.input,
                    diagnostic.span,
                    diagnostic.label.as_deref().unwrap_or("here"),
                    diagnostic.help.as_deref(),
                )
            })
            .collect::<Vec<_>>()
            .join("\n\n")
    }
}

impl Display for KdlError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "Failed to parse KDL document")
    }
}

impl Error for KdlError {}

#[derive(Debug, Clone, Copy, Default, Eq, PartialEq)]
pub enum Severity {
    #[default]
    Error,
    Warning,
    Advice,
}

#[derive(Debug, Clone, Eq, PartialEq)]
pub struct KdlDiagnostic {
    pub input: Arc<String>,
    pub span: SourceSpan,
    pub message: Option<String>,
    pub label: Option<String>,
    pub help: Option<String>,
    pub severity: Severity,
}

impl Display for KdlDiagnostic {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str(self.message.as_deref().unwrap_or("Unexpected error"))
    }
}

impl Error for KdlDiagnostic {}
