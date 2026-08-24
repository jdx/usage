//! Small, dependency-free diagnostics used by usage.
//!
//! This intentionally implements only the part of miette's API that usage exposed or used.
//! Keeping the familiar names avoids making downstream callers translate errors merely because
//! the renderer is now local.

use std::error::Error as StdError;
use std::fmt::{Debug, Display, Formatter};
use unicode_width::UnicodeWidthChar;

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
        let mut rendered = if let Some(error) = self.downcast_ref::<crate::error::UsageErr>() {
            error.render()
        } else {
            self.to_string()
        };
        let mut source = self.0.source();
        if source.is_some() {
            rendered.push_str("\n\nCaused by:");
        }
        let mut index = 1;
        while let Some(cause) = source {
            rendered.push_str(&format!("\n  {index}: {cause}"));
            source = cause.source();
            index += 1;
        }
        f.write_str(&rendered)
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
    let lines = line_ranges(source);
    let line_index = lines
        .iter()
        .rposition(|(start, _)| *start <= offset)
        .unwrap_or(0);
    let (line_start, line_end) = lines[line_index];
    let line = expand_tabs(&source[line_start..line_end], 4);
    let line_no = line_index + 1;
    let column = display_width(&source[line_start..offset], 0, 4);
    let requested_end = offset.saturating_add(span.len()).min(line_end);
    let span_end = source.ceil_char_boundary(requested_end).min(line_end);
    let marked = display_width(&source[offset..span_end], column, 4).max(1);
    let width = line_no.to_string().len();
    let location = if source_name.is_empty() {
        format!("[{}:{}]", line_no, column + 1)
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

fn line_ranges(source: &str) -> Vec<(usize, usize)> {
    let mut ranges = Vec::new();
    let mut start = 0;
    let mut chars = source.char_indices().peekable();
    while let Some((at, ch)) = chars.next() {
        let is_newline = matches!(
            ch,
            '\r' | '\n' | '\u{0085}' | '\u{000B}' | '\u{000C}' | '\u{2028}' | '\u{2029}'
        );
        if !is_newline {
            continue;
        }
        ranges.push((start, at));
        if ch == '\r' && chars.peek().is_some_and(|(_, next)| *next == '\n') {
            chars.next();
        }
        start = chars.peek().map_or(source.len(), |(at, _)| *at);
    }
    ranges.push((start, source.len()));
    ranges
}

fn display_width(value: &str, starting_column: usize, tab_width: usize) -> usize {
    let mut column = starting_column;
    for ch in value.chars() {
        if ch == '\t' {
            column += tab_width - (column % tab_width);
        } else {
            column += ch.width().unwrap_or(0);
        }
    }
    column - starting_column
}

fn expand_tabs(value: &str, tab_width: usize) -> String {
    let mut out = String::with_capacity(value.len());
    let mut column = 0;
    for ch in value.chars() {
        if ch == '\t' {
            let spaces = tab_width - (column % tab_width);
            out.extend(std::iter::repeat_n(' ', spaces));
            column += spaces;
        } else {
            out.push(ch);
            column += ch.width().unwrap_or(0);
        }
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
    use std::path::Path;

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
    fn source_renderer_handles_every_kdl_newline() {
        for newline in [
            "\r\n", "\r", "\n", "\u{0085}", "\u{000B}", "\u{000C}", "\u{2028}", "\u{2029}",
        ] {
            let source = format!("first{newline}bad");
            let offset = source.find("bad").unwrap();
            let rendered = render_source(
                "bad value",
                "spec.kdl",
                &source,
                SourceSpan::from(offset..offset + 3),
                "invalid",
                None,
            );

            assert!(
                rendered.contains("[spec.kdl:2:1]"),
                "{newline:?}: {rendered:?}"
            );
            assert!(rendered.contains("2 │ bad"), "{newline:?}: {rendered:?}");
        }
    }

    #[test]
    fn source_renderer_aligns_tabs_and_wide_characters() {
        let source = "\t界bad";
        let offset = source.find("bad").unwrap();
        let rendered = render_source(
            "bad value",
            "",
            source,
            SourceSpan::from(offset..offset + 3),
            "invalid",
            None,
        );

        assert!(rendered.contains("1 │     界bad"), "{rendered}");
        assert!(rendered.contains("  ·       ───┬"), "{rendered}");
        assert!(rendered.contains("[1:7]"), "{rendered}");
    }

    #[test]
    fn kdl_file_errors_include_the_filename_and_location() {
        let directory = tempfile::tempdir().unwrap();
        let path = directory.path().join("broken.usage.kdl");
        std::fs::write(&path, "name \"ok\"\narg \"unterminated\n").unwrap();

        let error = Spec::parse_file(Path::new(&path)).unwrap_err();
        let rendered = format!("{:?}", Error::from(error));
        assert!(
            rendered.contains(&format!("[{}:2:5]", path.display())),
            "{rendered}"
        );
    }

    #[test]
    fn reports_include_error_source_chains() {
        #[derive(Debug, thiserror::Error)]
        #[error("outer failure")]
        struct Outer(#[source] MietteError);

        let rendered = format!("{:?}", Error::new(Outer(MietteError::new("root cause"))));
        assert!(
            rendered.contains("outer failure\n\nCaused by:\n  1: root cause"),
            "{rendered}"
        );
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
