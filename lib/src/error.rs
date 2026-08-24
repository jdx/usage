use crate::kdl;
use crate::miette::{NamedSource, SourceSpan};
use thiserror::Error;

/// Everything that can go wrong reading a spec or a command line against one.
///
/// `#[non_exhaustive]`, so a caller matching on it needs a `_` arm. That is the point:
/// this enum grows every time the spec learns to say something new — `MissingGroup`
/// arrived with groups, `ArgRequiresDoubleDash` with `double_dash` — and without this
/// each one is a major release for everyone downstream.
///
/// With the `miette` feature enabled, this implements `miette::Diagnostic` so applications can
/// pass it directly to their existing miette reporter. The feature is disabled by default.
#[derive(Error, Debug)]
#[non_exhaustive]
pub enum UsageErr {
    #[error("Invalid flag `{token}`: {reason}")]
    InvalidFlag {
        token: String,
        reason: String,
        span: SourceSpan,
        input: String,
    },

    #[error("Missing required flag: --{0} <{0}>")]
    MissingFlag(String),

    #[error("Flag --{0} cannot be used multiple times")]
    DuplicateFlag(String),

    /// A required group had none of its members given.
    ///
    /// Its own variant rather than a [`UsageErr::MissingFlag`] holding a sentence,
    /// because there is no one flag to name: the group is the thing that was not
    /// satisfied, and a caller that renders errors itself needs the members as members.
    #[error("Missing one of the required flags in group {group}: {members}")]
    MissingGroup { group: String, members: String },

    #[error("Invalid usage config")]
    InvalidInput(String, SourceSpan, NamedSource<String>),

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

    #[error("{0}")]
    Version(String),

    #[error("Invalid usage config: {0}")]
    Miette(#[from] crate::miette::MietteError),

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
    KdlError(#[from] kdl::KdlError),

    /// A file the spec model was asked to read could not be read.
    ///
    /// Carries the path as well as the io error: "No such file or directory" on its own
    /// names nothing, and this is reported for spec files given on a command line.
    #[error("{0}\nFile: {1}")]
    FileError(std::io::Error, std::path::PathBuf),

    /// A `run=` script could not be run, exited non-zero, or produced output usage
    /// could not read. The message names the shell and the script.
    #[error("{0}")]
    ShellError(String),

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

    #[error("Invalid spec view: {0}")]
    InvalidView(String),

    /// A command's `output`/`select` declarations do not agree with each other, or with
    /// the flags around them. Spanless like [`UsageErr::InvalidView`], because selection
    /// is resolved once the whole document is read — a `select` may name a flag declared
    /// on an ancestor, so the node spans are long gone by the time it can be checked.
    #[error("Invalid output declaration: {0}")]
    InvalidOutput(String),

    #[error("Invalid value for {name}: {value}: {reason}")]
    InvalidValue {
        name: String,
        value: String,
        reason: String,
    },

    #[error("Unsupported shell: {0}")]
    UnsupportedShell(String),

    #[error("No injected output was provided for mount command: {0}")]
    MissingMountOutput(String),
}
pub type Result<T> = std::result::Result<T, UsageErr>;

impl UsageErr {
    pub(crate) fn render(&self) -> String {
        match self {
            Self::InvalidInput(message, span, source) => crate::miette::render_source(
                "Invalid usage config",
                source.name(),
                source.inner(),
                *span,
                message,
                None,
            ),
            Self::InvalidFlag {
                reason,
                span,
                input,
                ..
            } => crate::miette::render_source(&self.to_string(), "", input, *span, reason, None),
            Self::KdlError(error) => error.render(),
            _ => self.to_string(),
        }
    }
}

#[cfg(feature = "miette")]
impl ::miette::Diagnostic for UsageErr {
    fn source_code(&self) -> Option<&dyn ::miette::SourceCode> {
        match self {
            Self::InvalidInput(_, _, source) => Some(source),
            Self::InvalidFlag { input, .. } => Some(input),
            _ => None,
        }
    }

    fn labels(&self) -> Option<Box<dyn Iterator<Item = ::miette::LabeledSpan> + '_>> {
        let (span, label) = match self {
            Self::InvalidInput(message, span, _) => (*span, message.as_str()),
            Self::InvalidFlag { reason, span, .. } => (*span, reason.as_str()),
            _ => return None,
        };
        let label = ::miette::LabeledSpan::at(span.offset()..span.offset() + span.len(), label);
        Some(Box::new(std::iter::once(label)))
    }

    fn related<'a>(
        &'a self,
    ) -> Option<Box<dyn Iterator<Item = &'a dyn ::miette::Diagnostic> + 'a>> {
        match self {
            Self::KdlError(error) => Some(Box::new(
                error
                    .diagnostics
                    .iter()
                    .map(|diagnostic| diagnostic as &dyn ::miette::Diagnostic),
            )),
            _ => None,
        }
    }
}

#[macro_export]
macro_rules! bail_parse {
    ($ctx:expr, $span:expr, $fmt:literal) => {{
        let span: $crate::miette::SourceSpan = ($span.offset(), $span.len()).into();
        let msg = format!($fmt);
        let err = $ctx.build_err(msg, span);
        return std::result::Result::Err(err);
    }};
    ($ctx:expr, $span:expr, $fmt:literal, $($arg:tt)*) => {{
        let span: $crate::miette::SourceSpan = ($span.offset(), $span.len()).into();
        let msg = format!($fmt, $($arg)*);
        let err = $ctx.build_err(msg, span);
        return std::result::Result::Err(err);
    }};
}
