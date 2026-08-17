use miette::{Diagnostic, NamedSource, SourceSpan};
use thiserror::Error;

/// Everything that can go wrong reading a spec or a command line against one.
///
/// `#[non_exhaustive]`, so a caller matching on it needs a `_` arm. That is the point:
/// this enum grows every time the spec learns to say something new — `MissingGroup`
/// arrived with groups, `ArgRequiresDoubleDash` with `double_dash` — and without this
/// each one is a major release for everyone downstream.
#[derive(Error, Diagnostic, Debug)]
#[non_exhaustive]
pub enum UsageErr {
    #[error("Invalid flag `{token}`: {reason}")]
    InvalidFlag {
        token: String,
        reason: String,
        #[label("{reason}")]
        span: SourceSpan,
        #[source_code]
        input: String,
    },

    #[error("Missing required flag: --{0} <{0}>")]
    MissingFlag(String),

    /// A required group had none of its members given.
    ///
    /// Its own variant rather than a [`UsageErr::MissingFlag`] holding a sentence,
    /// because there is no one flag to name: the group is the thing that was not
    /// satisfied, and a caller that renders errors itself needs the members as members.
    #[error("Missing one of the required flags in group {group}: {members}")]
    MissingGroup { group: String, members: String },

    #[error("Invalid usage config")]
    InvalidInput(
        String,
        #[label = "{0}"] SourceSpan,
        #[source_code] NamedSource<String>,
    ),

    #[error("Missing required arg: <{0}>")]
    MissingArg(String),

    /// A command that declares `subcommand_required` was given none.
    ///
    /// The spec could say this and the parser did not read it, so `mise generate` — which
    /// declares it — parsed as though it were a complete invocation. usage-argv and clap both
    /// refuse it.
    #[error("`{0}` needs a subcommand: one of {1}")]
    MissingSubcommand(String, String),

    #[error("Argument <{0}> can only be set after a `--` separator")]
    ArgRequiresDoubleDash(String),

    #[error("{0}")]
    Help(String),

    #[error("Invalid usage config")]
    #[diagnostic(transparent)]
    Miette(#[from] miette::MietteError),

    #[error(transparent)]
    IO(#[from] std::io::Error),

    #[error(transparent)]
    Strum(#[from] strum::ParseError),

    #[error(transparent)]
    FromUtf8Error(#[from] std::string::FromUtf8Error),

    #[cfg(feature = "tera")]
    #[error(transparent)]
    TeraError(#[from] tera::Error),

    #[error(transparent)]
    #[diagnostic(transparent)]
    KdlError(#[from] kdl::KdlError),

    #[error(transparent)]
    #[diagnostic(transparent)]
    XXError(#[from] xx::error::XXError),

    #[error("Variadic argument <{name}> requires at least {min} value(s), got {got}")]
    VarArgTooFew {
        name: String,
        min: usize,
        got: usize,
    },

    #[error("Variadic argument <{name}> accepts at most {max} value(s), got {got}")]
    VarArgTooMany {
        name: String,
        max: usize,
        got: usize,
    },

    #[error("Variadic flag --{name} requires at least {min} value(s), got {got}")]
    VarFlagTooFew {
        name: String,
        min: usize,
        got: usize,
    },

    #[error("Variadic flag --{name} accepts at most {max} value(s), got {got}")]
    VarFlagTooMany {
        name: String,
        max: usize,
        got: usize,
    },

    #[error("Invalid file path: {0}")]
    InvalidPath(String),

    #[error("Unsupported shell: {0}")]
    UnsupportedShell(String),
}
pub type Result<T> = std::result::Result<T, UsageErr>;

#[macro_export]
macro_rules! bail_parse {
    ($ctx:expr, $span:expr, $fmt:literal) => {{
        let span: miette::SourceSpan = ($span.offset(), $span.len()).into();
        let msg = format!($fmt);
        let err = $ctx.build_err(msg, span);
        return std::result::Result::Err(err);
    }};
    ($ctx:expr, $span:expr, $fmt:literal, $($arg:tt)*) => {{
        let span: miette::SourceSpan = ($span.offset(), $span.len()).into();
        let msg = format!($fmt, $($arg)*);
        let err = $ctx.build_err(msg, span);
        return std::result::Result::Err(err);
    }};
}
