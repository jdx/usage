use miette::Diagnostic;
use miette7 as miette;
use thiserror::Error;

#[derive(Error, Diagnostic, Debug)]
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
