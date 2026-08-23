//! Process control for a CLI embedded in another runtime.
//!
//! [`crate::Cli::parse`](https://docs.rs/usage-rs) owns a process: it prints help, versions and
//! failures, then exits where appropriate. An N-API module, WASM host or editor integration cannot
//! let a library end its process. [`outcome`] turns the same decisions into a value the host can
//! print or return instead.

use std::ffi::OsStr;

use crate::help::{self, Page, Style};
use crate::spec::Spec;
use crate::{Command, Error};

/// What an embedded host should print before returning the supplied status.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Exit {
    /// The text exactly as the process-facing renderer produced it.
    pub text: String,
    /// Whether the text belongs on stderr rather than stdout.
    pub stderr: bool,
    /// The status the standalone process would have exited with.
    pub code: i32,
}

/// The two things parsing inside another runtime can do.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Outcome<T> {
    /// The command line parsed and the host can run it.
    Parsed(T),
    /// The host should print a response and return its status without running the command.
    Exit(Exit),
}

impl<T> Outcome<T> {
    /// The parsed value, if the command line was an invocation rather than a response.
    pub fn parsed(self) -> Option<T> {
        match self {
            Self::Parsed(parsed) => Some(parsed),
            Self::Exit(_) => None,
        }
    }

    /// The response an embedded host should return, if parsing did not produce a value.
    pub fn exit(&self) -> Option<&Exit> {
        match self {
            Self::Parsed(_) => None,
            Self::Exit(exit) => Some(exit),
        }
    }
}

/// Parse one command line without printing or ending the host process.
///
/// Pass the CLI's own `parse_from` function item:
///
/// ```ignore
/// match usage::embedded::outcome(Cli::spec(), Cli::command(), &argv, Cli::parse_from) {
///     usage::embedded::Outcome::Parsed(cli) => run(cli),
///     usage::embedded::Outcome::Exit(exit) => host.respond(exit),
/// }
/// ```
///
/// Help and version go to stdout with status 0. Help produced by `arg_required_else_help` and
/// parse failures go to stderr with status 2. Rendering follows the terminal attached to the
/// destination stream and honors `NO_COLOR` and `CLICOLOR_FORCE`.
///
/// The version response uses the portable identity in `spec`. A CLI whose version or binary name
/// is computed at runtime should handle [`Error::Version`] itself so it can use that runtime value.
pub fn outcome<'v, T>(
    spec: &Spec<'_>,
    root: &Command<'_>,
    argv: &[&'v OsStr],
    parse_from: impl FnOnce(&[&'v OsStr]) -> Result<T, Error<'static, 'v>>,
) -> Outcome<T> {
    outcome_with_styles(
        spec,
        root,
        argv,
        parse_from,
        Style::auto(),
        Style::auto_stderr(),
    )
}

fn outcome_with_styles<'v, T>(
    spec: &Spec<'_>,
    root: &Command<'_>,
    argv: &[&'v OsStr],
    parse_from: impl FnOnce(&[&'v OsStr]) -> Result<T, Error<'static, 'v>>,
    stdout_style: Style,
    stderr_style: Style,
) -> Outcome<T> {
    match parse_from(argv) {
        Ok(parsed) => Outcome::Parsed(parsed),
        Err(Error::Version { long }) => {
            let bin = spec.bin.unwrap_or(spec.name);
            let version = if long {
                spec.long_version.or(spec.version)
            } else {
                spec.version
            };
            Outcome::Exit(Exit {
                text: format!("{bin} {}\n", version.unwrap_or_default()),
                stderr: false,
                code: 0,
            })
        }
        Err(Error::Help { cmd, long }) => Outcome::Exit(Exit {
            text: help::page(
                spec,
                root,
                argv,
                cmd,
                if long { Page::Long } else { Page::Short },
                stdout_style,
            )
            .unwrap_or_default(),
            stderr: false,
            code: 0,
        }),
        Err(Error::HelpAll { cmd }) => Outcome::Exit(Exit {
            text: help::page(spec, root, argv, cmd, Page::All, stdout_style).unwrap_or_default(),
            stderr: false,
            code: 0,
        }),
        Err(Error::MissingArgsHelp { cmd }) => Outcome::Exit(Exit {
            text: help::page(spec, root, argv, cmd, Page::Short, stderr_style).unwrap_or_default(),
            stderr: true,
            code: 2,
        }),
        Err(error) => Outcome::Exit(Exit {
            text: crate::render_failure(spec, argv, &error),
            stderr: true,
            code: 2,
        }),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::spec::{CommandMeta, Spec};
    use crate::{Arg, Command, Flag};

    static FILE: Arg<'static> = Arg::REQUIRED;
    static FORCE: Flag<'static> = Flag {
        key: 1,
        name: "force",
        longs: &["force"],
        ..Flag::BOOL
    };
    static ROOT: Command<'static> = Command {
        name: "ex",
        flags: &[&FORCE],
        args: &[&FILE],
        ..Command::EMPTY
    };
    static ROOT_META: CommandMeta<'static> = CommandMeta {
        cmd: &ROOT,
        ..CommandMeta::EMPTY
    };
    static SPEC: Spec<'static> = Spec {
        name: "ex",
        bin: Some("ex"),
        version: Some("1.2.3"),
        root: &ROOT_META,
        ..Spec::EMPTY
    };

    fn failed<'v, T>(
        error: Error<'static, 'v>,
    ) -> impl FnOnce(&[&'v OsStr]) -> Result<T, Error<'static, 'v>> {
        |_| Err(error)
    }

    #[test]
    fn a_parsed_value_is_returned_to_the_host() {
        let got = outcome_with_styles(&SPEC, &ROOT, &[], |_| Ok(7), Style::PLAIN, Style::PLAIN);
        assert_eq!(got.parsed(), Some(7));
    }

    #[test]
    fn help_and_version_are_stdout_successes() {
        let help = outcome_with_styles(
            &SPEC,
            &ROOT,
            &[],
            failed::<()>(Error::Help {
                cmd: &ROOT,
                long: false,
            }),
            Style::PLAIN,
            Style::PLAIN,
        );
        let exit = help.exit().expect("help is a response");
        assert!(!exit.stderr);
        assert_eq!(exit.code, 0);
        assert!(exit.text.contains("Usage: ex"), "{}", exit.text);

        let version = outcome_with_styles(
            &SPEC,
            &ROOT,
            &[],
            failed::<()>(Error::Version { long: false }),
            Style::PLAIN,
            Style::PLAIN,
        );
        assert_eq!(
            version.exit().map(|exit| exit.text.as_str()),
            Some("ex 1.2.3\n")
        );
    }

    #[test]
    fn automatic_help_and_failures_are_stderr_status_two() {
        let help = outcome_with_styles(
            &SPEC,
            &ROOT,
            &[],
            failed::<()>(Error::MissingArgsHelp { cmd: &ROOT }),
            Style::PLAIN,
            Style::PLAIN,
        );
        let exit = help.exit().expect("automatic help is a response");
        assert!(exit.stderr);
        assert_eq!(exit.code, 2);

        let argv = [OsStr::new("--wat")];
        let failure = outcome_with_styles(
            &SPEC,
            &ROOT,
            &argv,
            failed::<()>(Error::UnknownFlag {
                token: argv[0].as_encoded_bytes(),
            }),
            Style::PLAIN,
            Style::PLAIN,
        );
        let exit = failure.exit().expect("a failure is a response");
        assert!(exit.stderr);
        assert_eq!(exit.code, 2);
        assert!(exit.text.contains("--wat"), "{}", exit.text);
    }
}
