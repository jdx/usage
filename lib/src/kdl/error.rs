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
        f.write_str("Failed to parse KDL document")?;
        if let Some(message) = self
            .diagnostics
            .first()
            .and_then(|diagnostic| diagnostic.message.as_deref())
        {
            write!(f, ": {message}")?;
        }
        Ok(())
    }
}

impl Error for KdlError {}

#[cfg(feature = "miette")]
impl ::miette::Diagnostic for KdlError {
    fn related<'a>(
        &'a self,
    ) -> Option<Box<dyn Iterator<Item = &'a dyn ::miette::Diagnostic> + 'a>> {
        Some(Box::new(
            self.diagnostics
                .iter()
                .map(|diagnostic| diagnostic as &dyn ::miette::Diagnostic),
        ))
    }
}

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

#[cfg(feature = "miette")]
impl ::miette::Diagnostic for KdlDiagnostic {
    fn severity(&self) -> Option<::miette::Severity> {
        Some(match self.severity {
            Severity::Error => ::miette::Severity::Error,
            Severity::Warning => ::miette::Severity::Warning,
            Severity::Advice => ::miette::Severity::Advice,
        })
    }

    fn help<'a>(&'a self) -> Option<Box<dyn Display + 'a>> {
        self.help
            .as_ref()
            .map(|help| Box::new(help) as Box<dyn Display>)
    }

    fn source_code(&self) -> Option<&dyn ::miette::SourceCode> {
        Some(&self.input)
    }

    fn labels(&self) -> Option<Box<dyn Iterator<Item = ::miette::LabeledSpan> + '_>> {
        let label = ::miette::LabeledSpan::at(
            self.span.offset()..self.span.offset() + self.span.len(),
            self.label.as_deref().unwrap_or("here"),
        );
        Some(Box::new(std::iter::once(label)))
    }
}
