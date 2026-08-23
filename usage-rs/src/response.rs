//! Explicit expansion of `@response-file` arguments.
//!
//! Expansion is separate from the parser so ordinary invocations retain the zero-allocation
//! hot path and applications choose exactly where filesystem access is allowed.

use std::collections::HashSet;
use std::ffi::OsString;
use std::fmt;
use std::path::{Path, PathBuf};

/// Response-file expansion policy.
#[derive(Debug, Clone, Copy)]
pub struct Options {
    /// Maximum number of nested response files. The outermost file counts as one.
    pub max_depth: usize,
}

impl Default for Options {
    fn default() -> Self {
        Self { max_depth: 16 }
    }
}

/// Why a response file could not be expanded.
#[derive(Debug)]
pub enum ErrorKind {
    /// The path could not be resolved or read as UTF-8 text.
    Io(std::io::Error),
    /// The file's shell-style quoting was invalid.
    Syntax(shell_words::ParseError),
    /// Expanding this file would exceed [`Options::max_depth`].
    DepthExceeded { max_depth: usize },
    /// A file included itself, directly or indirectly.
    Cycle,
}

/// A response-file failure with the file that caused it.
#[derive(Debug)]
pub struct Error {
    pub path: PathBuf,
    pub kind: ErrorKind,
}

impl fmt::Display for Error {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match &self.kind {
            ErrorKind::Io(error) => write!(f, "could not read {}: {error}", self.path.display()),
            ErrorKind::Syntax(error) => {
                write!(f, "could not parse {}: {error}", self.path.display())
            }
            ErrorKind::DepthExceeded { max_depth } => write!(
                f,
                "response file {} exceeds the nesting limit of {max_depth}",
                self.path.display()
            ),
            ErrorKind::Cycle => write!(
                f,
                "response file {} includes itself through an include cycle",
                self.path.display()
            ),
        }
    }
}

impl std::error::Error for Error {
    fn source(&self) -> Option<&(dyn std::error::Error + 'static)> {
        match &self.kind {
            ErrorKind::Io(error) => Some(error),
            ErrorKind::Syntax(error) => Some(error),
            ErrorKind::DepthExceeded { .. } | ErrorKind::Cycle => None,
        }
    }
}

/// Expand `@path` arguments using the default policy.
///
/// Each file is UTF-8 shell-style words: whitespace separates arguments, single and double
/// quotes preserve whitespace, and backslash escapes the next character. A nested relative path
/// is resolved beside the file that contains it. `@@value` escapes a literal argument beginning
/// with `@`. Non-UTF-8 arguments and a lone `@` are passed through unchanged.
pub fn expand<I, S>(argv: I) -> Result<Vec<OsString>, Error>
where
    I: IntoIterator<Item = S>,
    S: Into<OsString>,
{
    expand_with(argv, Options::default())
}

/// Expand `@path` arguments with an explicit policy.
pub fn expand_with<I, S>(argv: I, options: Options) -> Result<Vec<OsString>, Error>
where
    I: IntoIterator<Item = S>,
    S: Into<OsString>,
{
    let mut out = Vec::new();
    let mut active = HashSet::new();
    let cwd = std::env::current_dir().map_err(|error| Error {
        path: PathBuf::from("."),
        kind: ErrorKind::Io(error),
    })?;
    for arg in argv {
        expand_one(arg.into(), &cwd, options, &mut active, &mut out)?;
    }
    Ok(out)
}

fn expand_one(
    arg: OsString,
    base: &Path,
    options: Options,
    active: &mut HashSet<PathBuf>,
    out: &mut Vec<OsString>,
) -> Result<(), Error> {
    let Some(text) = arg.to_str() else {
        out.push(arg);
        return Ok(());
    };
    if let Some(literal) = text.strip_prefix("@@") {
        out.push(OsString::from(format!("@{literal}")));
        return Ok(());
    }
    let Some(raw_path) = text.strip_prefix('@').filter(|path| !path.is_empty()) else {
        out.push(arg);
        return Ok(());
    };

    let requested = Path::new(raw_path);
    let path = if requested.is_absolute() {
        requested.to_path_buf()
    } else {
        base.join(requested)
    };
    if active.len() >= options.max_depth {
        return Err(Error {
            path,
            kind: ErrorKind::DepthExceeded {
                max_depth: options.max_depth,
            },
        });
    }
    let canonical = std::fs::canonicalize(&path).map_err(|error| Error {
        path: path.clone(),
        kind: ErrorKind::Io(error),
    })?;
    if !active.insert(canonical.clone()) {
        return Err(Error {
            path: canonical,
            kind: ErrorKind::Cycle,
        });
    }

    let result = (|| {
        let contents = std::fs::read_to_string(&canonical).map_err(|error| Error {
            path: canonical.clone(),
            kind: ErrorKind::Io(error),
        })?;
        let words = shell_words::split(&contents).map_err(|error| Error {
            path: canonical.clone(),
            kind: ErrorKind::Syntax(error),
        })?;
        let next_base = canonical.parent().unwrap_or(base);
        for word in words {
            expand_one(OsString::from(word), next_base, options, active, out)?;
        }
        Ok(())
    })();
    active.remove(&canonical);
    result
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::ffi::OsStr;

    fn words(values: &[OsString]) -> Vec<&OsStr> {
        values.iter().map(OsString::as_os_str).collect()
    }

    #[test]
    fn nested_files_are_relative_to_the_file_that_names_them() {
        let dir = tempfile::tempdir().unwrap();
        let nested = dir.path().join("nested");
        std::fs::create_dir(&nested).unwrap();
        std::fs::write(nested.join("more.args"), "'two words' --last").unwrap();
        std::fs::write(
            dir.path().join("main.args"),
            "--first one @nested/more.args",
        )
        .unwrap();

        let expanded = expand([OsString::from(format!(
            "@{}",
            dir.path().join("main.args").display()
        ))])
        .unwrap();
        assert_eq!(
            words(&expanded),
            ["--first", "one", "two words", "--last"].map(OsStr::new)
        );
    }

    #[test]
    fn an_at_sign_can_be_escaped() {
        let expanded = expand(["@@literal", "@", "ordinary"]).unwrap();
        assert_eq!(
            words(&expanded),
            ["@literal", "@", "ordinary"].map(OsStr::new)
        );
    }

    #[test]
    fn include_cycles_are_reported() {
        let dir = tempfile::tempdir().unwrap();
        std::fs::write(dir.path().join("a.args"), "@b.args").unwrap();
        std::fs::write(dir.path().join("b.args"), "@a.args").unwrap();
        let error = expand([OsString::from(format!(
            "@{}",
            dir.path().join("a.args").display()
        ))])
        .unwrap_err();
        assert!(matches!(error.kind, ErrorKind::Cycle));
    }

    #[test]
    fn nesting_has_a_configurable_bound() {
        let dir = tempfile::tempdir().unwrap();
        std::fs::write(dir.path().join("a.args"), "@b.args").unwrap();
        std::fs::write(dir.path().join("b.args"), "value").unwrap();
        let error = expand_with(
            [OsString::from(format!(
                "@{}",
                dir.path().join("a.args").display()
            ))],
            Options { max_depth: 1 },
        )
        .unwrap_err();
        assert!(matches!(
            error.kind,
            ErrorKind::DepthExceeded { max_depth: 1 }
        ));
    }

    #[cfg(feature = "spec")]
    #[test]
    fn expanded_words_feed_the_typed_parser() {
        #[derive(crate::Cli)]
        #[usage(bin = "response-demo")]
        struct Cli {
            #[usage(long)]
            format: Option<String>,
            files: Vec<String>,
        }

        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("args");
        std::fs::write(&path, "--format json 'one file' two").unwrap();
        let expanded = expand([OsString::from(format!("@{}", path.display()))]).unwrap();
        let argv = words(&expanded);
        let parsed = Cli::parse_from(&argv).unwrap();
        assert_eq!(parsed.format.as_deref(), Some("json"));
        assert_eq!(parsed.files, ["one file", "two"]);
    }
}
