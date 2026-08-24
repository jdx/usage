//! What a command's exit status means.
//!
//! ```kdl
//! exit_code 0 "all checks passed"
//! exit_code 1 "a check failed"
//! exit_code 2 "configuration error"
//! ```
//!
//! Declared at the root for the CLI-wide convention and on a command for what it adds or
//! refines. A man page conventionally carries an `EXIT STATUS` section and this project's
//! renderer had no way to fill one; an agent reading the spec through `usage mcp` had no
//! way to tell "one check failed" from "the tool broke".
//!
//! Folding is per code and nearest-wins, so `exit_code 1 "a check failed"` on a command
//! *refines* a CLI-wide `exit_code 1 "error"` rather than replacing the whole table. The
//! alternative — a command that redeclares anything owns the full set — would make every
//! command restate `0` and `130`.

use crate::kdl::{KdlEntry, KdlNode, KdlValue};
use serde::Serialize;

use crate::error::Result;
use crate::spec::cmd::SpecCommand;
use crate::spec::context::ParsingContext;
use crate::spec::helpers::{string_entry, NodeHelper};
use crate::spec::Spec;

/// One documented exit status.
#[derive(Debug, Default, Clone, Serialize)]
#[non_exhaustive]
pub struct SpecExitCode {
    pub code: i64,
    pub help: String,
}

impl SpecExitCode {
    pub fn new(code: i64, help: impl Into<String>) -> Self {
        Self {
            code,
            help: help.into(),
        }
    }

    pub(crate) fn parse(ctx: &ParsingContext, node: &NodeHelper) -> Result<Self> {
        node.ensure_arg_len(2..=2)?;
        let entry = node.arg(0)?;
        let Some(code) = entry.value.as_integer() else {
            bail_parse!(ctx, entry.entry.span(), "an exit code must be a number");
        };
        // A shell reports `status & 0xff`, so 256 and 0 are the same thing to anyone
        // reading it, and a spec that claims otherwise documents something that cannot be
        // observed. Windows is the counterexample — it really does return values wider
        // than a byte — so this is a range check rather than a `u8`.
        if !(0..=255).contains(&code) {
            bail_parse!(
                ctx,
                entry.entry.span(),
                "exit code {code} is outside 0-255; a POSIX shell reports the low byte, so \
                 anything higher is indistinguishable from {}",
                code & 0xff
            );
        }
        let help = node.arg(1)?.ensure_string()?;
        if help.is_empty() {
            bail_parse!(
                ctx,
                node.span(),
                "exit code {code} needs a description; an undocumented code is what the \
                 declaration exists to replace"
            );
        }
        Ok(SpecExitCode {
            code: code as i64,
            help,
        })
    }
}

impl From<&SpecExitCode> for KdlNode {
    fn from(exit_code: &SpecExitCode) -> KdlNode {
        let mut node = KdlNode::new("exit_code");
        node.push(KdlEntry::new(KdlValue::Integer(exit_code.code as i128)));
        node.push(string_entry(None, &exit_code.help));
        node
    }
}

/// The exit codes in effect for a command, CLI-wide declarations folded in.
///
/// Nearest wins per code, and the root's order is preserved so a table reads the same way
/// on every page. A command's own codes append in declaration order.
///
/// Folded on read rather than at parse time, following `unknown_flags`: folding early
/// would write the root's codes into every command block on re-emission.
pub fn effective_exit_codes(spec: &Spec, path: &[SpecCommand]) -> Vec<SpecExitCode> {
    effective_exit_codes_ref(spec, path.iter())
}

/// Reference-based form used by tree walkers that already hold the command chain.
pub fn effective_exit_codes_ref<'a>(
    spec: &Spec,
    path: impl IntoIterator<Item = &'a SpecCommand>,
) -> Vec<SpecExitCode> {
    let mut out: Vec<SpecExitCode> = spec.exit_codes.clone();
    for cmd in path {
        for code in &cmd.exit_codes {
            match out.iter_mut().find(|e| e.code == code.code) {
                Some(existing) => *existing = code.clone(),
                None => out.push(code.clone()),
            }
        }
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::error::UsageErr;

    fn parse(src: &str) -> Spec {
        src.parse().expect("the fixture should parse")
    }

    fn error(src: &str) -> String {
        match src.parse::<Spec>().expect_err("should be rejected") {
            UsageErr::InvalidInput(msg, ..) => msg,
            other => other.to_string(),
        }
    }

    #[test]
    fn a_command_refines_the_cli_wide_table_rather_than_replacing_it() {
        let spec = parse(
            r#"
name "ex"
exit_code 0 "ok"
exit_code 1 "error"
exit_code 130 "interrupted"
cmd "check" {
    exit_code 1 "a check failed"
    exit_code 2 "configuration error"
}
"#,
        );
        let check = spec.cmd.subcommands["check"].clone();
        let codes: Vec<(i64, String)> = effective_exit_codes(&spec, &[check])
            .into_iter()
            .map(|e| (e.code, e.help))
            .collect();
        // 1 is refined in place, 0 and 130 come down untouched, 2 appends. The
        // alternative — a command that says anything owns the whole table — would make
        // every command restate 0 and 130.
        assert_eq!(
            codes,
            vec![
                (0, "ok".to_string()),
                (1, "a check failed".to_string()),
                (130, "interrupted".to_string()),
                (2, "configuration error".to_string()),
            ]
        );
    }

    #[test]
    fn a_code_outside_a_byte_says_what_a_shell_would_report() {
        let err = error("name \"ex\"\nexit_code 256 \"nope\"\n");
        assert!(err.contains("outside 0-255"), "{err}");
        assert!(err.contains("indistinguishable from 0"), "{err}");
    }

    #[test]
    fn a_code_needs_a_description() {
        assert!(error("name \"ex\"\nexit_code 1 \"\"\n").contains("needs a description"));
    }

    #[test]
    fn codes_survive_a_round_trip() {
        let src = "name \"ex\"\nexit_code 0 ok\ncmd \"go\" {\n    exit_code 3 \"broke\"\n}\n";
        let spec = parse(src);
        let again = parse(&spec.to_string());
        assert_eq!(spec.to_string(), again.to_string());
        assert_eq!(again.exit_codes[0].code, 0);
        assert_eq!(again.cmd.subcommands["go"].exit_codes[0].help, "broke");
    }
}
