//! Running the corpus against [`usage_argv`], the compiled parser.
//!
//! usage-argv reads `static` tables that a derive macro is meant to emit. Nothing
//! emits them here, so [`crate::tables`] builds them from a [`Spec`] instead, which
//! also makes the corpus usable as usage-argv's test suite from the first commit.
//!
//! # Scope
//!
//! usage-argv implements binding only. Vectors that turn on something decided
//! after the last token — `required`, `choices`, `env`, defaults, `var_min` and
//! `var_max`, `overrides` — are reported [`Outcome::OutOfScope`] rather than
//! failed, because that behavior belongs to the layer that owns the target
//! struct. The count is asserted, so the out-of-scope set cannot quietly grow.

use std::collections::BTreeMap;
use std::ffi::OsStr;

use usage::{Spec, SpecCommand};
use usage_argv::{Command, Error, Event, Parser};

use crate::tables::{self, convert_unknown_flags, leak};
use crate::{ErrorCode, Expect, Layer, Parsed, Value, Vector};

/// What usage-argv did with a vector.
#[derive(Debug, PartialEq, Eq)]
pub enum Outcome {
    Parsed(Parsed),
    Failed(ErrorCode),
    /// The vector exercises something usage-argv deliberately leaves to a higher
    /// layer. The string says which.
    OutOfScope(&'static str),
    /// The spec would not load. A bug in the vector.
    BadSpec(String),
}

impl Outcome {
    pub fn matches(&self, expect: &Expect) -> bool {
        match (self, expect) {
            (Outcome::Parsed(got), Expect::Ok(want)) => got == want,
            (Outcome::Failed(got), Expect::Error(want)) => got == want,
            _ => false,
        }
    }
}

/// Parse a vector with usage-argv.
pub fn run(vector: &Vector) -> Outcome {
    let spec: Spec = match vector.spec.parse() {
        Ok(spec) => spec,
        Err(e) => return Outcome::BadSpec(e.to_string()),
    };

    if let Some(reason) = out_of_scope(vector) {
        return Outcome::OutOfScope(reason);
    }

    // The spec's own setting belongs to the root command, which is where usage-argv's tables
    // hold it. Everything below inherits it, which the parser now does itself rather than
    // this flattening it on the way in — a second implementation of the same rule, and the
    // one that hid the parser not having it.
    let root = tables::build(&spec.cmd, spec.unknown_flags.map(convert_unknown_flags)).cmd;
    // `default_subcommand` is a property of the spec rather than of a command, so it is
    // resolved once, here, against the root's own subcommands. A name that answers to
    // nothing is left as None: the spec is what it is, and a vector that expects routing
    // will fail loudly rather than this guessing.
    let root: &'static Command<'static> = match spec.default_subcommand.as_deref() {
        Some(name) => {
            // Names before aliases, the rule `usage_argv::find_subcommand` implements for
            // generated code. Spelled out again rather than called because that one panics
            // on a name nothing answers to — a compile error where it runs, but here the
            // vector should fail on its own terms instead.
            let subcommands = || root.subcommands.iter().copied();
            let default = subcommands()
                .find(|sub| sub.name == name)
                .or_else(|| subcommands().find(|sub| sub.aliases.contains(&name)));
            Box::leak(Box::new(Command {
                default_subcommand: default,
                ..*root
            }))
        }
        None => root,
    };
    let argv: Vec<&'static OsStr> = vector
        .argv
        .iter()
        .map(|a| -> &'static OsStr { OsStr::new(leak(a)) })
        .collect();
    let argv: &'static [&'static OsStr] = Box::leak(argv.into_boxed_slice());

    // How a value accumulates depends on the declaration, which the parser
    // deliberately does not know: it reports each occurrence and lets the caller
    // decide. Generated code would assign to a field or push to a Vec; here the
    // spec says which of the two to do.
    let multi = multi_valued(&spec.cmd);

    let mut parser = Parser::new(root, argv);
    let mut cmd = Vec::new();
    let mut flags: BTreeMap<String, Value> = BTreeMap::new();
    let mut args: BTreeMap<String, Value> = BTreeMap::new();

    while let Some(event) = parser.next_event() {
        match event {
            Ok(Event::Command(c)) => cmd.push(c.name.to_string()),
            Ok(Event::Flag {
                flag,
                value,
                negated,
            }) => {
                let name = flag.name.to_string();
                match (multi.get(name.as_str()), value) {
                    // A count flag records one entry per occurrence.
                    (Some(Multi::Count), _) => {
                        match flags.entry(name).or_insert(Value::Bools(vec![])) {
                            Value::Bools(v) => v.push(true),
                            _ => unreachable!("count flags only ever hold bools"),
                        }
                    }
                    (Some(Multi::Var), Some(v)) => {
                        match flags.entry(name).or_insert(Value::Strs(vec![])) {
                            Value::Strs(list) => list.push(string(v)),
                            _ => unreachable!("var flags only ever hold strings"),
                        }
                    }
                    (_, Some(v)) => {
                        flags.insert(name, Value::Str(string(v)));
                    }
                    (_, None) => {
                        flags.insert(name, Value::Bool(!negated));
                    }
                }
            }
            Ok(Event::Arg { arg, value }) => {
                let name = arg.name.to_string();
                if arg.var {
                    match args.entry(name).or_insert(Value::Strs(vec![])) {
                        Value::Strs(list) => list.push(string(value)),
                        _ => unreachable!("var args only ever hold strings"),
                    }
                } else {
                    args.insert(name, Value::Str(string(value)));
                }
            }
            Err(e) => return Outcome::Failed(code(e)),
        }
    }

    Outcome::Parsed(Parsed { cmd, flags, args })
}

fn string(value: &[u8]) -> String {
    usage_argv::as_str(value)
        .expect("corpus values are UTF-8")
        .to_string()
}

fn code(err: Error<'_, '_>) -> ErrorCode {
    match err {
        Error::UnknownFlag { .. } => ErrorCode::UnknownFlag,
        Error::MissingFlagValue { .. } => ErrorCode::MissingFlagValue,
        Error::UnexpectedArg { .. } => ErrorCode::UnexpectedArg,
        Error::ArgRequiresDoubleDash { .. } => ErrorCode::ArgRequiresDoubleDash,
        Error::TooDeep => panic!("no corpus spec is anywhere near MAX_DEPTH"),
        // The parser cannot raise these — they come from the layer above it, which
        // this harness does not exercise: it builds tables from a spec rather than
        // from a derived struct.
        // Everything else comes from the layer above the parser, which this harness
        // does not exercise: it builds tables from a spec rather than from a derived
        // struct. A wildcard rather than a list, because `Error` is `non_exhaustive`
        // — recognizing a new failure should not break this file.
        other => unreachable!("the parser cannot raise {other:?}"),
    }
}

/// Which flags accumulate rather than replace.
enum Multi {
    Count,
    Var,
}

fn multi_valued(cmd: &SpecCommand) -> BTreeMap<String, Multi> {
    let mut out = BTreeMap::new();
    collect_multi(cmd, &mut out);
    out
}

fn collect_multi(cmd: &SpecCommand, out: &mut BTreeMap<String, Multi>) {
    for flag in &cmd.flags {
        if flag.count {
            out.insert(flag.name.clone(), Multi::Count);
        } else if flag.var || flag.arg.as_ref().is_some_and(|a| a.var) {
            out.insert(flag.name.clone(), Multi::Var);
        }
    }
    for sub in cmd.subcommands.values() {
        collect_multi(sub, out);
    }
}

/// Why a vector is not usage-argv's to answer, if it isn't.
///
/// The vector says so. This used to be inferred by looking for a post-binding
/// feature anywhere in the spec, which exempted vectors whose expectation was an
/// ordinary binding: `--no-color` binds false whatever else that flag declares.
fn out_of_scope(vector: &Vector) -> Option<&'static str> {
    match vector.layer {
        Layer::Binding => None,
        Layer::PostBinding => Some(
            "decided after the last token is read, so it belongs to the layer that \
             owns the target struct",
        ),
    }
}
