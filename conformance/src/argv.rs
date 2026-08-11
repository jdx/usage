//! Running the corpus against [`usage_argv`], the compiled parser.
//!
//! usage-argv reads `static` tables that a derive macro is meant to emit. Nothing
//! emits them yet, so this module builds them from a [`Spec`] instead, which also
//! makes the corpus usable as usage-argv's test suite from the first commit.
//!
//! The tables are leaked. They must outlive the parse and be `'static`-shaped,
//! and a test process that builds a handful of small tables and exits is the one
//! place where leaking is the simplest correct answer. Generated code has no such
//! problem: its tables really are `static`.
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
use usage_argv::{
    Arg, Command, DoubleDash, Error, Event, Flag, Parser, UnknownFlags as ArgvUnknownFlags,
};

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

    // Inheritance is resolved here, not in the parser: usage-argv's tables hold the
    // effective value per command, which is what a derive would emit.
    let root = build(
        &spec.cmd,
        convert_unknown_flags(spec.unknown_flags.unwrap_or_default()),
    );
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
    }
}

/// The spec's spelling of the setting, in the parser's terms.
fn convert_unknown_flags(mode: usage::UnknownFlags) -> ArgvUnknownFlags {
    match mode {
        usage::UnknownFlags::Value => ArgvUnknownFlags::Value,
        usage::UnknownFlags::Error => ArgvUnknownFlags::Error,
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

/// Build leaked tables mirroring a spec command.
///
/// `inherited_unknown_flags` is the effective setting from above, which a command
/// that states nothing keeps and passes down.
fn build(
    cmd: &SpecCommand,
    inherited_unknown_flags: ArgvUnknownFlags,
) -> &'static Command<'static> {
    let unknown_flags = cmd
        .unknown_flags
        .map(convert_unknown_flags)
        .unwrap_or(inherited_unknown_flags);
    let flags: Vec<&'static Flag<'static>> = cmd
        .flags
        .iter()
        .map(|f| -> &'static Flag<'static> {
            let longs: Vec<&'static str> = f.long.iter().map(|l| leak(l)).collect();
            let shorts: Vec<u8> = f.short.iter().map(|c| *c as u8).collect();
            Box::leak(Box::new(Flag {
                key: 0,
                name: leak(&f.name),
                longs: Box::leak(longs.into_boxed_slice()),
                shorts: Box::leak(shorts.into_boxed_slice()),
                // usage-lib stores the negation with its dashes; the table wants
                // the bare name.
                negate: f.negate.as_ref().map(|n| leak(n.trim_start_matches('-'))),
                takes_value: f.arg.is_some(),
                // Only a variadic *argument* is greedy. A `var` flag with a
                // single-value argument is repeatable instead: one value per
                // occurrence, which the parser gets by not collecting.
                variadic: f.arg.as_ref().is_some_and(|a| a.var),
                global: f.global,
            }))
        })
        .collect();

    let args: Vec<&'static Arg<'static>> = cmd
        .args
        .iter()
        .map(|a| -> &'static Arg<'static> {
            Box::leak(Box::new(Arg {
                key: 0,
                name: leak(&a.name),
                var: a.var,
                double_dash: match a.double_dash {
                    usage::SpecDoubleDashChoices::Required => DoubleDash::Required,
                    usage::SpecDoubleDashChoices::Preserve => DoubleDash::Preserve,
                    usage::SpecDoubleDashChoices::Automatic => DoubleDash::Automatic,
                    _ => DoubleDash::Optional,
                },
            }))
        })
        .collect();

    let subcommands: Vec<&'static Command<'static>> = cmd
        .subcommands
        .values()
        .map(|sub| build(sub, unknown_flags))
        .collect();

    let aliases: Vec<&'static str> = cmd
        .aliases
        .iter()
        .chain(cmd.hidden_aliases.iter())
        .map(|a| leak(a))
        .collect();

    Box::leak(Box::new(Command {
        name: leak(&cmd.name),
        aliases: Box::leak(aliases.into_boxed_slice()),
        flags: Box::leak(flags.into_boxed_slice()),
        args: Box::leak(args.into_boxed_slice()),
        subcommands: Box::leak(subcommands.into_boxed_slice()),
        unknown_flags,
        key: 0,
    }))
}

fn leak(s: &str) -> &'static str {
    Box::leak(s.to_string().into_boxed_str())
}
