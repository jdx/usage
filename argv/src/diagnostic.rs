//! What a user reads when a command line does not parse.
//!
//! Held to clap's shape on purpose. mise's users read clap's errors today, and the help output on
//! either side of this is already byte-identical to usage-lib's — so the error text is the last
//! thing an adopter's users would notice changing, and the aim is that they do not.
//!
//! The skeleton, measured from clap 4 rather than remembered:
//!
//! ```text
//! error: unexpected argument '--fore' found
//!
//!   tip: a similar argument exists: '--force'
//!
//! Usage: mise use [OPTIONS] [TOOL@VERSION]…
//!
//! For more information, try '--help'.
//! ```
//!
//! Two deliberate departures. The usage line is *ours* — the same one `--help` prints, rendered
//! from the spec — because an error that disagrees with the help about how a command is spelled is
//! worse than one that disagrees with clap. And which errors carry a usage block follows clap:
//! the ones about the shape of the command line do, the ones about a single value do not.

use core::fmt::Write as _;

use crate::spec::{CommandMeta, Spec};
use crate::{Command, Error};

/// Whether to colour, and what with.
///
/// The codes are clap's, so that a terminal shows the same thing: bold red for `error:`, yellow
/// for the offending text, green for a suggestion and for what was expected, bold underline for
/// `Usage:`.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Style {
    coloured: bool,
}

impl Style {
    /// Plain text, for a pipe or a test.
    pub const PLAIN: Style = Style { coloured: false };
    /// Coloured, whatever the terminal is.
    pub const COLOURED: Style = Style { coloured: true };

    /// Colour when stderr is a terminal and the environment has not asked otherwise.
    ///
    /// `NO_COLOR` wins over everything, per the convention: a user who sets it has said once, for
    /// every program, that they do not want this. `CLICOLOR_FORCE` is the other direction, for a
    /// pipe that ends up somewhere that does render colour.
    pub fn auto() -> Style {
        use std::io::IsTerminal as _;
        let forced = std::env::var_os("CLICOLOR_FORCE").is_some_and(|v| v != "0");
        let refused = std::env::var_os("NO_COLOR").is_some_and(|v| !v.is_empty());
        if refused {
            return Style::PLAIN;
        }
        if forced || std::io::stderr().is_terminal() {
            Style::COLOURED
        } else {
            Style::PLAIN
        }
    }

    fn wrap(self, code: &str, text: &str) -> String {
        if self.coloured {
            format!("\u{1b}[{code}m{text}\u{1b}[0m")
        } else {
            text.to_string()
        }
    }

    /// `error:`, and anything else that is the failure itself.
    fn error(self, text: &str) -> String {
        self.wrap("1m\u{1b}[31", text)
    }

    /// What the user typed that did not work.
    fn invalid(self, text: &str) -> String {
        self.wrap("33", text)
    }

    /// What would have worked: a suggestion, a possible value, a missing argument.
    fn valid(self, text: &str) -> String {
        self.wrap("32", text)
    }

    /// A heading, such as `Usage:`.
    fn heading(self, text: &str) -> String {
        self.wrap("1m\u{1b}[4", text)
    }

    /// Something to be typed as it is written.
    fn literal(self, text: &str) -> String {
        self.wrap("1", text)
    }
}

/// A name as a usage line writes it: `<TOOL>`, `[TOOL]…`, `--jobs`.
///
/// The error carries the spec's name for a thing; a user reads the form the help shows. Both come
/// from `help` rather than being decided again here — an error and the page above it describing
/// one argument differently is the confusing kind of inconsistency, and rewriting the rule is how
/// that happens. A flag is spelled with its dashes, which is the whole of what `--jobs` versus
/// `jobs` is about.
fn shown<'a>(meta: Option<&'a CommandMeta<'a>>, name: &str) -> String {
    let Some(meta) = meta else {
        return name.to_string();
    };
    if let Some(arg) = meta.args.iter().find(|a| a.arg.name == name) {
        return crate::help::arg_usage(arg);
    }
    // A flag can be named by an error too — a missing required one, or a value that is not among
    // its choices — and the spec's name for it has no dashes.
    if let Some(flag) = meta
        .flags
        .iter()
        .find(|f| f.flag.name == name || f.value_name == Some(name))
    {
        return crate::help::flag_spelling(flag);
    }
    name.to_string()
}

/// The word that was bound to a named argument, recovered from argv.
///
/// The parse itself does not carry it: an error that owned the offending text would allocate on
/// the one path this crate promises not to, so [`Error::InvalidChoice`] names the argument and
/// stops. Recovering it here is what that promise assumes — the diagnostics are a layer that may
/// do the work, and by the time one is being written the parse has already failed.
fn value_bound_to(
    root: &Command<'_>,
    argv: &[&std::ffi::OsStr],
    name: &str,
    refused: &[&str],
) -> Option<String> {
    let mut parser = crate::Parser::new(root, argv);
    let mut last = None;
    while let Some(event) = parser.next_event() {
        let value = match event {
            Ok(crate::Event::Arg { arg, value }) if arg.name == name => value,
            Ok(crate::Event::Flag {
                flag,
                value: Some(value),
                ..
            }) if flag.name == name => value,
            Ok(_) => continue,
            Err(_) => break,
        };
        let value = String::from_utf8_lossy(value).into_owned();
        // The *offending* one, not the last. A repeatable flag or a variadic argument may be
        // given several values, and the check refuses the first that is not allowed — reporting
        // whichever came last would name a value that is perfectly good and leave the wrong one
        // unmentioned.
        if !refused.is_empty() && !refused.contains(&value.as_str()) {
            return Some(value);
        }
        last = Some(value);
    }
    last
}

/// The command the words reached, which is the one an error is about.
///
/// Walked rather than carried on the error: only some variants know their command, and a caller
/// that has just been handed an error has the argv it came from. The walk stops where the parse
/// stopped, which is the command whose usage line belongs in the message.
fn command_reached<'t>(root: &'t Command<'t>, argv: &[&std::ffi::OsStr]) -> &'t Command<'t> {
    let mut parser = crate::Parser::new(root, argv);
    while let Some(event) = parser.next_event() {
        if event.is_err() {
            break;
        }
    }
    parser.command()
}

/// The path to a command, as a user would type it, and its metadata.
fn found<'a>(spec: &'a Spec<'a>, cmd: &Command<'_>) -> Option<(Vec<&'a str>, &'a CommandMeta<'a>)> {
    crate::help::find(spec, cmd)
}

/// Render `error` the way a user should read it.
///
/// `argv` is what was being parsed, which is how the message finds the command to show a usage
/// line for. A [`Error::Help`] renders as nothing: it is not a failure, and a caller that has not
/// handled it before reaching here has a bug this cannot paper over.
pub fn render(
    spec: &Spec<'_>,
    argv: &[&std::ffi::OsStr],
    error: &Error<'_, '_>,
    style: Style,
) -> String {
    let cmd = command_reached(spec.root.cmd, argv);
    let path = found(spec, cmd)
        .map(|(path, _)| path.join(" "))
        .unwrap_or_else(|| spec.bin.unwrap_or(spec.name).to_string());
    let usage = found(spec, cmd)
        .map(|(path, meta)| crate::help::usage_line(&path, meta))
        .unwrap_or_else(|| path.clone());

    let mut out = String::new();
    let mut with_usage = false;

    match error {
        // The shape of the command line: clap shows a usage block for these.
        Error::UnknownFlag { token } => {
            with_usage = true;
            let _ = writeln!(
                out,
                "{} unexpected argument '{}' found",
                style.error("error:"),
                style.invalid(&String::from_utf8_lossy(token))
            );
        }
        Error::UnexpectedArg { token } => {
            with_usage = true;
            let word = String::from_utf8_lossy(token);
            // A word where a subcommand was expected reads better as one — which is the same
            // distinction clap draws between an unexpected argument and an unrecognized
            // subcommand.
            if cmd.subcommands.is_empty() {
                let _ = writeln!(
                    out,
                    "{} unexpected argument '{}' found",
                    style.error("error:"),
                    style.invalid(&word)
                );
            } else {
                let _ = writeln!(
                    out,
                    "{} unrecognized subcommand '{}'",
                    style.error("error:"),
                    style.invalid(&word)
                );
            }
        }
        Error::MissingRequired { name } => {
            with_usage = true;
            let _ = writeln!(
                out,
                "{} the following required arguments were not provided:",
                style.error("error:")
            );
            let _ = writeln!(
                out,
                "  {}",
                style.valid(&shown(found(spec, cmd).map(|(_, meta)| meta), name))
            );
        }
        Error::MissingSubcommand => {
            with_usage = true;
            let _ = writeln!(
                out,
                "{} '{}' requires a subcommand but one was not provided",
                style.error("error:"),
                style.invalid(&path)
            );
        }

        // About one value: clap shows no usage block, on the grounds that the shape was right.
        Error::MissingFlagValue { flag } => {
            let name = flag
                .longs
                .first()
                .map(|l| format!("--{l}"))
                .or_else(|| flag.shorts.first().map(|s| format!("-{}", *s as char)))
                .unwrap_or_else(|| flag.name.to_string());
            let value = found(spec, cmd)
                .and_then(|(_, meta)| {
                    meta.flags
                        .iter()
                        .find(|m| core::ptr::eq(m.flag, *flag))
                        .and_then(|m| m.value_name)
                })
                .map(|v| format!(" <{v}>"))
                .unwrap_or_default();
            let _ = writeln!(
                out,
                "{} a value is required for '{}' but none was supplied",
                style.error("error:"),
                style.invalid(&format!("{name}{value}"))
            );
        }
        Error::InvalidChoice { name, choices } => {
            let shown_name = shown(found(spec, cmd).map(|(_, meta)| meta), name);
            match value_bound_to(spec.root.cmd, argv, name, choices) {
                Some(value) => {
                    let _ = writeln!(
                        out,
                        "{} invalid value '{}' for '{}'",
                        style.error("error:"),
                        style.invalid(&value),
                        style.literal(&shown_name)
                    );
                }
                // Nothing in argv bound to it, which means the value came from somewhere else —
                // an environment variable, or a default the spec declared.
                None => {
                    let _ = writeln!(
                        out,
                        "{} invalid value for '{}'",
                        style.error("error:"),
                        style.literal(&shown_name)
                    );
                }
            }
            let listed: Vec<String> = choices.iter().map(|c| style.valid(c)).collect();
            let _ = writeln!(out, "  [possible values: {}]", listed.join(", "));
        }
        Error::InvalidValue(invalid) => {
            let _ = writeln!(
                out,
                "{} invalid value '{}' for '{}': {}",
                style.error("error:"),
                style.invalid(&invalid.value),
                style.literal(&shown(found(spec, cmd).map(|(_, m)| m), invalid.name)),
                invalid.reason
            );
        }
        Error::ConflictingFlags { name, other } => {
            let _ = writeln!(
                out,
                "{} the argument '{}' cannot be used with '{}'",
                style.error("error:"),
                style.invalid(name),
                style.invalid(other)
            );
            with_usage = true;
        }
        Error::VarTooFew { name, min, got } => {
            let _ = writeln!(
                out,
                "{} {min} values required for '{}' but {got} were provided",
                style.error("error:"),
                style.literal(&shown(found(spec, cmd).map(|(_, m)| m), name))
            );
        }
        Error::VarTooMany { name, max, got } => {
            let _ = writeln!(
                out,
                "{} {max} values allowed for '{}' but {got} were provided",
                style.error("error:"),
                style.literal(&shown(found(spec, cmd).map(|(_, m)| m), name))
            );
        }
        Error::ArgRequiresDoubleDash { arg } => {
            with_usage = true;
            let _ = writeln!(
                out,
                "{} '{}' can only be given after '{}'",
                style.error("error:"),
                style.literal(arg.name),
                style.literal("--")
            );
        }
        Error::TooDeep => {
            let _ = writeln!(
                out,
                "{} this command line nests deeper than the parser goes",
                style.error("error:")
            );
        }
        // Not a failure. A caller reaching here with one has skipped handling it, and inventing a
        // message would hide that rather than help.
        Error::Help { .. } => return String::new(),
    }

    if with_usage {
        let _ = writeln!(
            out,
            "\n{} {}",
            style.heading("Usage:"),
            style.literal(&usage)
        );
    }
    let _ = writeln!(
        out,
        "\nFor more information, try '{}'.",
        style.literal("--help")
    );
    out
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::spec::{ArgMeta, FlagMeta};
    use crate::{Arg, Flag};

    static FORCE: Flag = Flag {
        key: 1,
        name: "force",
        longs: &["force"],
        shorts: b"f",
        ..Flag::BOOL
    };
    static JOBS: Flag = Flag {
        key: 2,
        name: "jobs",
        longs: &["jobs"],
        ..Flag::VALUE
    };
    static TOOL: Arg = Arg {
        key: 3,
        name: "TOOL",
        ..Arg::REQUIRED
    };
    /// Variadic and choice-bearing, so "which value was refused" has a wrong answer available.
    static SHELLS: Arg = Arg {
        key: 7,
        name: "SHELLS",
        ..Arg::VAR
    };
    static USE: Command = Command {
        name: "use",
        flags: &[&FORCE, &JOBS],
        args: &[&TOOL, &SHELLS],
        ..Command::EMPTY
    };
    static ROOT: Command = Command {
        name: "ex",
        subcommands: &[&USE],
        ..Command::EMPTY
    };
    static USE_META: CommandMeta = CommandMeta {
        cmd: &USE,
        about: Some("Use a tool"),
        flags: &[
            FlagMeta {
                flag: &FORCE,
                help: Some("Force it"),
                ..FlagMeta::EMPTY
            },
            FlagMeta {
                flag: &JOBS,
                help: Some("How many"),
                value_name: Some("JOBS"),
                ..FlagMeta::EMPTY
            },
        ],
        args: &[
            ArgMeta {
                arg: &TOOL,
                help: Some("Which tool"),
                required: true,
                ..ArgMeta::EMPTY
            },
            ArgMeta {
                arg: &SHELLS,
                help: Some("Which shells"),
                choices: &["bash", "zsh"],
                required: false,
                ..ArgMeta::EMPTY
            },
        ],
        ..CommandMeta::EMPTY
    };
    static ROOT_META: CommandMeta = CommandMeta {
        cmd: &ROOT,
        subcommands: &[&USE_META],
        ..CommandMeta::EMPTY
    };
    static SPEC: Spec = Spec {
        name: "ex",
        bin: Some("ex"),
        root: &ROOT_META,
        ..Spec::EMPTY
    };

    fn rendered(words: &[&str], error: Error<'static, 'static>) -> String {
        let owned: Vec<std::ffi::OsString> = words.iter().map(std::ffi::OsString::from).collect();
        let argv: Vec<&std::ffi::OsStr> = owned.iter().map(|o| o.as_os_str()).collect();
        render(&SPEC, &argv, &error, Style::PLAIN)
    }

    #[test]
    fn an_unknown_flag_reads_as_clap_writes_it() {
        assert_eq!(
            rendered(&["use"], Error::UnknownFlag { token: b"--fore" }),
            "error: unexpected argument '--fore' found\n\
             \n\
             Usage: ex use [-f --force] [--jobs <JOBS>] <TOOL> [SHELLS]…\n\
             \n\
             For more information, try '--help'.\n"
        );
    }

    #[test]
    fn the_usage_line_is_the_one_the_help_prints() {
        // Not clap's, which spells a usage line its own way. An error that disagrees with the
        // help about how a command is written is worse than one that disagrees with clap.
        let message = rendered(&["use"], Error::UnknownFlag { token: b"--fore" });
        let line = message
            .lines()
            .find_map(|l| l.strip_prefix("Usage: "))
            .expect("a usage line");
        assert_eq!(line, crate::help::usage_line(&["ex", "use"], &USE_META));
    }

    #[test]
    fn a_missing_value_names_what_it_wanted() {
        // No usage block: the shape of the command line was right, one value was missing — which
        // is the distinction clap draws too.
        assert_eq!(
            rendered(&["use"], Error::MissingFlagValue { flag: &JOBS }),
            "error: a value is required for '--jobs <JOBS>' but none was supplied\n\
             \n\
             For more information, try '--help'.\n"
        );
    }

    #[test]
    fn a_word_where_a_subcommand_was_expected_says_so() {
        // The root has subcommands, so an unexpected word there is an unrecognized subcommand;
        // inside `use`, which has none, the same error is an unexpected argument.
        let at_root = rendered(&[], Error::UnexpectedArg { token: b"nonesuch" });
        assert!(
            at_root.starts_with("error: unrecognized subcommand 'nonesuch'"),
            "{at_root}"
        );
        let in_use = rendered(&["use"], Error::UnexpectedArg { token: b"extra" });
        assert!(
            in_use.starts_with("error: unexpected argument 'extra' found"),
            "{in_use}"
        );
    }

    #[test]
    fn a_required_argument_is_listed_the_way_clap_lists_it() {
        assert_eq!(
            rendered(&["use"], Error::MissingRequired { name: "<TOOL>" }),
            "error: the following required arguments were not provided:\n  \
             <TOOL>\n\
             \n\
             Usage: ex use [-f --force] [--jobs <JOBS>] <TOOL> [SHELLS]…\n\
             \n\
             For more information, try '--help'.\n"
        );
    }

    #[test]
    fn colour_is_the_same_codes_clap_uses() {
        // Measured from clap 4 rather than remembered: bold red for `error:`, yellow for what was
        // typed, bold underline for `Usage:`, bold for what to type.
        let owned = [std::ffi::OsString::from("use")];
        let argv: Vec<&std::ffi::OsStr> = owned.iter().map(|o| o.as_os_str()).collect();
        let message = render(
            &SPEC,
            &argv,
            &Error::UnknownFlag { token: b"--fore" },
            Style::COLOURED,
        );
        assert!(
            message.starts_with("\u{1b}[1m\u{1b}[31merror:\u{1b}[0m"),
            "{message:?}"
        );
        assert!(message.contains("\u{1b}[33m--fore\u{1b}[0m"), "{message:?}");
        assert!(
            message.contains("\u{1b}[1m\u{1b}[4mUsage:\u{1b}[0m"),
            "{message:?}"
        );
        // And nothing at all when plain, which is what a pipe or a test gets.
        assert!(!rendered(&["use"], Error::UnknownFlag { token: b"--fore" }).contains('\u{1b}'));
    }

    #[test]
    fn a_help_request_renders_nothing() {
        // It is not a failure, and a caller that reaches here with one has skipped handling it —
        // which a message would hide rather than help.
        assert_eq!(
            rendered(
                &["use"],
                Error::Help {
                    cmd: &USE,
                    long: true
                }
            ),
            ""
        );
    }
    #[test]
    fn a_name_is_spelled_the_way_the_help_spells_it() {
        // One rule, taken from `help` rather than decided again here: an error and the page above
        // it describing the same argument differently is the confusing kind of inconsistency.
        let message = rendered(&["use"], Error::MissingRequired { name: "TOOL" });
        assert!(message.contains("  <TOOL>"), "{message}");

        // A variadic keeps its ellipsis, exactly as the usage line writes it.
        let message = rendered(
            &["use"],
            Error::VarTooFew {
                name: "SHELLS",
                min: 2,
                got: 1,
            },
        );
        assert!(message.contains("'[SHELLS]…'"), "{message}");

        // And a *flag* is spelled with its dashes: the spec calls it `jobs`, a user reads
        // `--jobs`.
        let message = rendered(&["use"], Error::MissingRequired { name: "jobs" });
        assert!(message.contains("  --jobs"), "{message}");
    }

    #[test]
    fn the_value_named_is_the_one_that_was_refused() {
        // A variadic given several values: the check refuses the first that is not allowed, so
        // naming whichever came last would name a value that is perfectly good and leave the
        // wrong one unmentioned.
        let owned = [
            std::ffi::OsString::from("use"),
            std::ffi::OsString::from("node"),
            std::ffi::OsString::from("fsh"),
            std::ffi::OsString::from("zsh"),
        ];
        let argv: Vec<&std::ffi::OsStr> = owned.iter().map(|o| o.as_os_str()).collect();
        let message = render(
            &SPEC,
            &argv,
            &Error::InvalidChoice {
                name: "SHELLS",
                choices: &["bash", "zsh"],
            },
            Style::PLAIN,
        );
        assert!(message.contains("invalid value 'fsh'"), "{message}");
        assert!(
            !message.contains("'zsh'\n"),
            "named a value that was fine: {message}"
        );
    }
}
