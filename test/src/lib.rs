//! Test helpers for a CLI built with usage.
//!
//! A CLI's observable surface is three things: what a command line parses to, what a user is
//! shown when it does not, and what a shell offers while one is being typed. All three are
//! testable from the static tables the derive already emits — no process to spawn, no
//! terminal to fake — and none of them was pleasant to reach for. This crate is the reaching.
//!
//! What it is *not* is a second implementation of any of them. Every page here comes from
//! [`usage_argv::help::page`], the same function `parse()` renders a help request with, and
//! every failure from [`usage_argv::render_failure_plain`], the same renderer it prints with —
//! asked for plain text, so an assertion does not turn on whether stderr was a terminal. A
//! harness that renders its own approximation of a help page is a harness whose passing tests
//! mean nothing, so the rule is that nothing in this crate formats a page.
//!
//! ```
//! # use usage_argv::spec::Spec;
//! # fn ex(spec: &'static Spec<'static>) {
//! use usage_test as test;
//!
//! // What the tree's help looks like, all of it, in one snapshot.
//! let pages = test::help_tree(spec, test::Page::Long);
//!
//! // What a user sees when a command line does not parse.
//! let words = test::argv(["--nope"]);
//! let outcome = test::outcome(spec, &words.words(), some_cli_parse_from);
//! # fn some_cli_parse_from<'v>(
//! #     argv: &[&'v std::ffi::OsStr],
//! # ) -> Result<(), usage_argv::Error<'static, 'v>> {
//! #     Ok(())
//! # }
//! # let _ = (pages, outcome);
//! # }
//! ```
//!
//! Enable it as a dev-dependency feature of the facade:
//!
//! ```toml
//! [dev-dependencies]
//! usage = { package = "usage-rs", version = "6", features = ["test"] }
//! ```

#![forbid(unsafe_code)]

use std::ffi::{OsStr, OsString};

use usage_argv::help::Style;
use usage_argv::spec::{CommandMeta, Spec};
use usage_argv::Command;
use usage_argv::Error;

pub use usage_argv::help::Page;

#[cfg(feature = "completions")]
pub use usage_argv::complete::{Completions, Files, Shell};

/// Words a parse entry point can borrow.
///
/// [`parse_from`](usage_argv) takes `&[&OsStr]`, which a test cannot write as a literal: the
/// `OsString`s have to outlive the slice pointing at them. This owns them, and [`Argv::words`]
/// is the slice.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Argv {
    words: Vec<OsString>,
}

impl Argv {
    /// The words, from anything that can become an `OsString` — including a `&str` literal and
    /// a byte sequence no `str` can hold, which is exactly the case worth testing.
    pub fn new<I, S>(words: I) -> Self
    where
        I: IntoIterator<Item = S>,
        S: Into<OsString>,
    {
        Self {
            words: words.into_iter().map(Into::into).collect(),
        }
    }

    /// The slice a parse entry point takes.
    pub fn words(&self) -> Vec<&OsStr> {
        self.words.iter().map(OsString::as_os_str).collect()
    }
}

/// [`Argv::new`], for a call site that reads better without the type.
pub fn argv<I, S>(words: I) -> Argv
where
    I: IntoIterator<Item = S>,
    S: Into<OsString>,
{
    Argv::new(words)
}

/// What a program would have written, where, and what it would have exited with.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Printed {
    /// The text, exactly as the process would have written it — plain, never coloured, because
    /// a snapshot with escape sequences in it is a snapshot nobody can read.
    pub text: String,
    /// Whether it goes to stderr. Help asked for goes to stdout; help shown because a command
    /// line was wrong does not.
    pub stderr: bool,
    /// The exit status that would have followed.
    pub code: i32,
}

/// What one command line does to a CLI.
///
/// The four things `parse()` can do, as a value instead of a side effect: a struct, a page, a
/// version, or a failure. A test asserting that `ex` with no arguments prints help is asserting
/// on [`Outcome::Help`], which is not something a `Result` can say.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Outcome<T> {
    /// It parsed.
    Parsed(T),
    /// Help was asked for — or shown because nothing was asked for at all, which is the case
    /// with `stderr` set and a non-zero code.
    Help(Printed),
    /// A version was asked for.
    Version(Printed),
    /// The command line did not parse, and this is what the user reads.
    Failed(Printed),
}

impl<T> Outcome<T> {
    /// The parsed struct, panicking with what the CLI would have printed instead.
    pub fn parsed(self) -> T {
        match self {
            Outcome::Parsed(parsed) => parsed,
            other => panic!(
                "the command line did not parse; the CLI would have printed:\n{}",
                other.text().unwrap_or_default()
            ),
        }
    }

    /// What would have been printed, for anything that is not a parse.
    pub fn printed(&self) -> Option<&Printed> {
        match self {
            Outcome::Parsed(_) => None,
            Outcome::Help(printed) | Outcome::Version(printed) | Outcome::Failed(printed) => {
                Some(printed)
            }
        }
    }

    /// The text of [`Outcome::printed`], for an assertion that only cares what it says.
    pub fn text(&self) -> Option<&str> {
        self.printed().map(|printed| printed.text.as_str())
    }
}

/// What a CLI would do with one command line.
///
/// Pass the CLI's own `parse_from` — `outcome(Ex::spec(), &words.words(), Ex::parse_from)` —
/// which is the function item, not a call. Nothing is printed and nothing exits; the page or
/// message the process would have produced comes back in the [`Outcome`].
///
/// A CLI whose identity is computed at run time (`#[usage(version = …)]` and its neighbours)
/// reports its declared spec values here, since the harness has the spec and not the program.
/// Executable views are the same: this is the plain entry point, and a view is selected by
/// argv0 in `parse_from_argv`.
///
/// A failure's text is whatever the build being tested would print: the clap-shaped message
/// where `diagnostics` is on, as the facade's defaults have it, and the compact error where a
/// parser-only build turned the renderer off.
pub fn outcome<'v, T>(
    spec: &Spec<'_>,
    argv: &[&'v OsStr],
    parse_from: impl FnOnce(&[&'v OsStr]) -> Result<T, Error<'static, 'v>>,
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
            Outcome::Version(Printed {
                text: format!("{bin} {}\n", version.unwrap_or_default()),
                stderr: false,
                code: 0,
            })
        }
        Err(Error::Help { cmd, long }) => Outcome::Help(page(
            spec,
            argv,
            cmd,
            if long { Page::Long } else { Page::Short },
            false,
            0,
        )),
        // Help nobody asked for, because the command line was empty: stderr, and clap's status.
        Err(Error::MissingArgsHelp { cmd }) => {
            Outcome::Help(page(spec, argv, cmd, Page::Short, true, 2))
        }
        Err(Error::HelpAll { cmd }) => Outcome::Help(page(spec, argv, cmd, Page::All, false, 0)),
        Err(error) => Outcome::Failed(Printed {
            // The plain renderer, not the process one: `render_failure` asks the environment
            // whether to colour, and a test whose expected string depends on whether stderr is
            // a terminal is a test that passes in one place and fails in the other.
            text: usage_argv::render_failure_plain(spec, argv, &error),
            stderr: true,
            code: 2,
        }),
    }
}

/// The parsed struct, or the text a user would have read instead.
///
/// [`outcome`] without the four-way distinction, for the common test: parse this, and if it
/// does not parse, show me what the user gets. A help or version request is text here too,
/// since neither is a parsed struct.
pub fn parse<'v, T>(
    spec: &Spec<'_>,
    argv: &[&'v OsStr],
    parse_from: impl FnOnce(&[&'v OsStr]) -> Result<T, Error<'static, 'v>>,
) -> Result<T, String> {
    match outcome(spec, argv, parse_from) {
        Outcome::Parsed(parsed) => Ok(parsed),
        other => Err(other.text().unwrap_or_default().to_string()),
    }
}

/// One command's help page, found by the words a user would type.
///
/// `path` names the command without the binary: `&[]` is the root, `&["config", "ls"]` is a
/// nested one. Aliases work, because a test should be able to ask the way a user would.
///
/// Panics when the path names no command, listing what the parent does have — a test naming a
/// command that was renamed should say so, not quietly assert about the wrong page.
pub fn help(spec: &Spec<'_>, path: &[&str], want: Page) -> String {
    let route = route(spec, path);
    render(spec, &route, want).expect("a route built from the spec's own tables should render")
}

/// Every command's help page, depth-first, in one string.
///
/// The drift test: snapshot this, and any change to any page of any command — a flag's help,
/// a new subcommand, a heading that moved — shows up as a diff in one place. Hidden commands
/// are included and marked, since a hidden command still has a page that can regress.
pub fn help_tree(spec: &Spec<'_>, want: Page) -> String {
    let mut out = String::new();
    let bin = spec.bin.unwrap_or(spec.name);
    let mut stack = vec![(vec![bin], vec![spec.root])];
    while let Some((path, chain)) = stack.pop() {
        let meta = *chain
            .last()
            .expect("a chain always holds the command it describes");
        let route: Vec<&Command<'_>> = chain.iter().map(|meta| meta.cmd).collect();
        if !out.is_empty() {
            out.push('\n');
        }
        out.push_str("=== ");
        out.push_str(&path.join(" "));
        if meta.hide {
            out.push_str(" (hidden)");
        }
        out.push_str(" ===\n");
        if let Some(page) = render(spec, &route, want) {
            out.push_str(&page);
        }

        // Reversed, because the stack pops backwards and a reader expects declaration order.
        for sub in meta.subcommands.iter().rev() {
            let mut path = path.clone();
            path.push(sub.cmd.name);
            let mut chain = chain.clone();
            chain.push(sub);
            stack.push((path, chain));
        }
    }
    out
}

/// What a shell would be offered for a half-typed line, cursor at the end.
///
/// The line includes the program name, exactly as a shell passes it: `"ex config --for"`.
#[cfg(feature = "completions")]
pub fn candidates(spec: &Spec<'_>, line: &str) -> Vec<String> {
    described(spec, line)
        .into_iter()
        .map(|(value, _)| value)
        .collect()
}

/// [`candidates`], with the description a shell shows beside each one.
#[cfg(feature = "completions")]
pub fn described(spec: &Spec<'_>, line: &str) -> Vec<(String, Option<String>)> {
    let split = usage_argv::complete::split(line, line.len(), Shell::Bash);
    usage_argv::complete::candidates(spec, &split)
        .into_iter()
        .map(|candidate| {
            (
                candidate.value,
                candidate.description.map(|text| text.into_owned()),
            )
        })
        .collect()
}

/// The whole completion answer, including whether the position admits paths.
///
/// [`candidates`] is the common question; this is for a test about the other half — that a
/// `<PATH>` argument offers files and a `--jobs` value does not.
#[cfg(feature = "completions")]
pub fn completion<'a>(spec: &'a Spec<'a>, line: &str) -> Completions<'a> {
    completion_at(spec, line, line.len(), Shell::Bash)
}

/// [`completion`] with the cursor and the shell spelled out.
///
/// `cursor` is a byte offset into `line`, which is how a shell reports it — and what makes a
/// test about completing in the *middle* of a command line possible at all.
#[cfg(feature = "completions")]
pub fn completion_at<'a>(
    spec: &'a Spec<'a>,
    line: &str,
    cursor: usize,
    shell: Shell,
) -> Completions<'a> {
    let split = usage_argv::complete::split(line, cursor, shell);
    usage_argv::complete::complete(spec, &split)
}

/// The page a help request becomes, as a [`Printed`].
fn page(
    spec: &Spec<'_>,
    argv: &[&OsStr],
    cmd: &Command<'_>,
    want: Page,
    stderr: bool,
    code: i32,
) -> Printed {
    // The command a request arrived at is not a route: the same `Subcommands` type mounted
    // twice is one address. `help::page` rebuilds the route from the words, which is what the
    // program does with the same argv.
    let text = usage_argv::help::page(spec, spec.root.cmd, argv, cmd, want, Style::PLAIN)
        .unwrap_or_default();
    Printed { text, stderr, code }
}

fn render(spec: &Spec<'_>, route: &[&Command<'_>], want: Page) -> Option<String> {
    match want {
        Page::Short => usage_argv::help::render_at_styled(spec, route, false, Style::PLAIN),
        Page::Long => usage_argv::help::render_at_styled(spec, route, true, Style::PLAIN),
        Page::All => usage_argv::help::render_all_at_styled(spec, route, Style::PLAIN),
    }
}

/// The route a path names, by name and then by alias, one level at a time.
fn route<'a>(spec: &Spec<'a>, path: &[&str]) -> Vec<&'a Command<'a>> {
    let mut here: &'a CommandMeta<'a> = spec.root;
    let mut route = vec![here.cmd];
    for name in path {
        let next = here
            .subcommands
            .iter()
            .find(|sub| sub.cmd.name == *name)
            .or_else(|| {
                here.subcommands
                    .iter()
                    .find(|sub| sub.cmd.aliases.contains(name))
            });
        let Some(next) = next else {
            panic!(
                "{:?} names no subcommand of {:?}; it has {:?}",
                name,
                here.cmd.name,
                here.subcommands
                    .iter()
                    .map(|sub| sub.cmd.name)
                    .collect::<Vec<_>>()
            )
        };
        here = next;
        route.push(here.cmd);
    }
    route
}
