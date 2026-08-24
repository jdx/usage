//! Small, dependency-free diagnostics used by usage.
//!
//! This intentionally implements only the part of miette's API that usage exposed or used.
//! Keeping the familiar names avoids making downstream callers translate errors merely because
//! the renderer is now local.

use std::error::Error as StdError;
use std::fmt::{Debug, Display, Formatter};

#[derive(Clone, Copy, Debug, Default, Eq, PartialEq)]
pub struct SourceOffset(usize);

impl From<usize> for SourceOffset {
    fn from(value: usize) -> Self {
        Self(value)
    }
}

#[derive(Clone, Copy, Debug, Default, Eq, PartialEq)]
pub struct SourceSpan {
    offset: SourceOffset,
    len: usize,
}

impl SourceSpan {
    pub fn new(offset: SourceOffset, len: usize) -> Self {
        Self { offset, len }
    }

    pub fn offset(self) -> usize {
        self.offset.0
    }

    pub fn len(self) -> usize {
        self.len
    }

    pub fn is_empty(self) -> bool {
        self.len == 0
    }
}

impl From<(usize, usize)> for SourceSpan {
    fn from((offset, len): (usize, usize)) -> Self {
        Self::new(offset.into(), len)
    }
}

impl From<std::ops::Range<usize>> for SourceSpan {
    fn from(range: std::ops::Range<usize>) -> Self {
        Self::new(range.start.into(), range.end.saturating_sub(range.start))
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct NamedSource<S> {
    name: String,
    source: S,
}

impl<S> NamedSource<S> {
    pub fn new(name: impl Display, source: S) -> Self {
        Self {
            name: name.to_string(),
            source,
        }
    }

    pub fn name(&self) -> &str {
        &self.name
    }

    pub fn inner(&self) -> &S {
        &self.source
    }
}

#[cfg(feature = "miette")]
impl ::miette::SourceCode for NamedSource<String> {
    fn read_span<'a>(
        &'a self,
        span: &::miette::SourceSpan,
        context_lines_before: usize,
        context_lines_after: usize,
    ) -> std::result::Result<Box<dyn ::miette::SpanContents<'a> + 'a>, ::miette::MietteError> {
        let contents = ::miette::SourceCode::read_span(
            &self.source,
            span,
            context_lines_before,
            context_lines_after,
        )?;
        Ok(Box::new(::miette::MietteSpanContents::new_named(
            self.name.clone(),
            contents.data(),
            *contents.span(),
            contents.line(),
            contents.column(),
            contents.line_count(),
        )))
    }
}

#[derive(Debug)]
pub struct MietteError(String);

impl MietteError {
    pub fn new(message: impl Into<String>) -> Self {
        Self(message.into())
    }
}

impl Display for MietteError {
    fn fmt(&self, f: &mut Formatter<'_>) -> std::fmt::Result {
        f.write_str(&self.0)
    }
}

impl StdError for MietteError {}

pub struct Error(Box<dyn StdError + Send + Sync + 'static>);
pub type Report = Error;
pub type Result<T, E = Error> = std::result::Result<T, E>;

impl Error {
    pub fn new(error: impl StdError + Send + Sync + 'static) -> Self {
        Self(Box::new(error))
    }

    pub fn msg(message: impl Into<String>) -> Self {
        Self::new(MietteError::new(message))
    }

    pub fn downcast_ref<T: StdError + 'static>(&self) -> Option<&T> {
        self.0.downcast_ref()
    }
}

impl Display for Error {
    fn fmt(&self, f: &mut Formatter<'_>) -> std::fmt::Result {
        Display::fmt(&self.0, f)
    }
}

impl Debug for Error {
    fn fmt(&self, f: &mut Formatter<'_>) -> std::fmt::Result {
        if let Some(error) = self.downcast_ref::<crate::error::UsageErr>() {
            return f.write_str(&error.render());
        }
        Display::fmt(self, f)
    }
}

impl StdError for Error {
    fn source(&self) -> Option<&(dyn StdError + 'static)> {
        self.0.source()
    }
}

impl From<crate::error::UsageErr> for Error {
    fn from(value: crate::error::UsageErr) -> Self {
        Self::new(value)
    }
}

pub trait IntoDiagnostic<T> {
    fn into_diagnostic(self) -> Result<T>;
}

impl<T, E> IntoDiagnostic<T> for std::result::Result<T, E>
where
    E: StdError + Send + Sync + 'static,
{
    fn into_diagnostic(self) -> Result<T> {
        self.map_err(Error::new)
    }
}

/// Render one labeled source span in the compact style used for spec diagnostics.
pub(crate) fn render_source(
    title: &str,
    source_name: &str,
    source: &str,
    span: SourceSpan,
    label: &str,
    help: Option<&str>,
) -> String {
    let offset = source.floor_char_boundary(span.offset().min(source.len()));
    let line_start = source[..offset].rfind('\n').map_or(0, |at| at + 1);
    let line_end = source[offset..]
        .find('\n')
        .map_or(source.len(), |at| offset + at);
    let line = &source[line_start..line_end];
    let line_no = source[..line_start].bytes().filter(|b| *b == b'\n').count() + 1;
    let column = source[line_start..offset].chars().count();
    let requested_end = offset.saturating_add(span.len()).min(line_end);
    let span_end = source.ceil_char_boundary(requested_end).min(line_end);
    let marked = source[offset..span_end].chars().count().max(1);
    let width = line_no.to_string().len();
    let location = if source_name.is_empty() {
        String::new()
    } else {
        format!("[{source_name}:{}:{}]", line_no, column + 1)
    };
    let mut out = format!(
        "  × {title}\n {blank:width$} ╭─{location}\n {line_no:>width$} │ {line}\n {blank:width$} · {blank:column$}{mark:─<marked$}┬\n {blank:width$} · {blank:indent$}╰── {label}\n {blank:width$} ╰────",
        blank = "",
        mark = "",
        width = width,
        column = column,
        marked = marked,
        indent = column + marked,
    );
    if let Some(help) = help {
        out.push_str("\n  help: ");
        out.push_str(help);
    }
    out
}

#[macro_export]
macro_rules! __usage_miette {
    ($fmt:literal $(, $arg:expr)* $(,)?) => {
        $crate::miette::Error::msg(format!($fmt $(, $arg)*))
    };
    ($err:expr $(,)?) => {
        $crate::miette::Error::msg($err.to_string())
    };
}

#[macro_export]
macro_rules! __usage_bail {
    ($($arg:tt)*) => {
        return Err($crate::__usage_miette!($($arg)*))
    };
}

pub use crate::__usage_bail as bail;
pub use crate::__usage_miette as miette;

#[cfg(test)]
mod tests {
    use super::{render_source, Error, MietteError, SourceSpan};
    use crate::error::UsageErr;
    use crate::Spec;

    #[test]
    fn a_kdl_syntax_error_keeps_its_source_label_and_help() {
        let source = "name broken\nflag --output {\n  arg \"unterminated\n}\n";
        let error = source.parse::<Spec>().unwrap_err();
        let rendered = format!("{:?}", Error::from(error));

        assert!(
            rendered.contains("Unexpected newline in single-line quoted string"),
            "{rendered}"
        );
        assert!(rendered.contains("3 │   arg \"unterminated"), "{rendered}");
        assert!(rendered.contains("╰── not quoted string"), "{rendered}");
        assert!(
            rendered.contains("help: You can make a string multi-line"),
            "{rendered}"
        );
    }

    #[test]
    fn source_renderer_aligns_wide_gutters_and_utf8_spans() {
        let source = format!("{}éx", "x\n".repeat(9));
        let offset = source.find('é').unwrap();
        let rendered = render_source(
            "bad value",
            "",
            &source,
            SourceSpan::from(offset..offset + 'é'.len_utf8()),
            "invalid",
            None,
        );

        assert!(rendered.contains(" 10 │ éx\n    · ─┬"), "{rendered}");

        // Robustness for spans supplied by callers rather than the parser: a byte offset in the
        // middle of a code point is normalized instead of panicking.
        let rendered = render_source(
            "bad value",
            "",
            &source,
            SourceSpan::new((offset + 1).into(), 1),
            "invalid",
            None,
        );
        assert!(rendered.contains(" 10 │ éx\n    · ─┬"), "{rendered}");
    }

    #[test]
    fn plain_errors_keep_their_underlying_message() {
        let error = UsageErr::from(MietteError::new("specific detail"));
        assert_eq!(error.to_string(), "Invalid usage config: specific detail");

        let error = "name \"unterminated\n".parse::<Spec>().unwrap_err();
        assert!(error.to_string().contains("Unexpected newline"), "{error}");
    }

    #[cfg(feature = "miette")]
    #[test]
    fn a_kdl_syntax_error_works_with_a_miette_reporter() {
        let source = "name broken\nflag --output {\n  arg \"unterminated\n}\n";
        let error = source.parse::<Spec>().unwrap_err();
        let mut rendered = String::new();

        ::miette::NarratableReportHandler::new()
            .render_report(&mut rendered, &error)
            .unwrap();

        assert!(
            rendered.contains("Unexpected newline in single-line quoted string"),
            "{rendered}"
        );
        assert!(rendered.contains("line 3, columns 7 to 19"), "{rendered}");
        assert!(rendered.contains("not quoted string"), "{rendered}");
        assert!(
            rendered.contains("You can make a string multi-line"),
            "{rendered}"
        );
    }
}
