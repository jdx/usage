use miette::Diagnostic;
use thiserror::Error;

#[derive(Error, Diagnostic, Debug)]
#[allow(dead_code)]
pub enum UsageCLIError {
    // #[error("Invalid markdown template")]
    // MarkdownParseError {
    //     message: String,
    //     #[label = "{message}"]
    //     label: SourceSpan,
    //     #[source_code]
    //     src: NamedSource,
    // },
}
