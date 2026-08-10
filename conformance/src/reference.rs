//! Running a vector against usage-lib, the reference implementation.
//!
//! usage-lib interprets a spec at runtime, which is precisely what the corpus
//! describes, so it is the oracle every other implementation is measured
//! against. This module is the adapter: it turns usage-lib's [`ParseOutput`]
//! into the corpus's [`Parsed`] shape, and its errors into an [`ErrorCode`].
//!
//! [`ParseOutput`]: usage::parse::ParseOutput

use std::collections::{BTreeMap, HashMap};

use usage::parse::{ParseValue, Parser};
use usage::Spec;

use crate::{ErrorCode, Expect, Parsed, Value, Vector};

/// What usage-lib did with a vector.
#[derive(Debug, PartialEq, Eq)]
pub enum Observed {
    /// A parse, translated into corpus terms.
    Parsed(Parsed),
    /// A failure classified into one of the grammar's error codes.
    Failed(ErrorCode),
    /// A failure that does not map onto any code the corpus defines. Kept
    /// distinct from `Failed` so an unclassifiable error can never be mistaken
    /// for agreement with an `Error` expectation.
    Unclassified(String),
    /// The spec itself would not load. Always a bug in the vector, not a finding
    /// about the parser.
    BadSpec(String),
}

impl Observed {
    /// Whether this matches what the vector expects.
    pub fn matches(&self, expect: &Expect) -> bool {
        match (self, expect) {
            (Observed::Parsed(got), Expect::Ok(want)) => got == want,
            (Observed::Failed(got), Expect::Error(want)) => got == want,
            _ => false,
        }
    }
}

/// Parse a vector with usage-lib.
pub fn run(vector: &Vector) -> Observed {
    let spec: Spec = match vector.spec.parse() {
        Ok(spec) => spec,
        Err(e) => return Observed::BadSpec(e.to_string()),
    };

    // Always an explicit map, even when empty: a vector that consulted the real
    // environment would pass or fail depending on the machine.
    let env: HashMap<String, String> = vector
        .env
        .iter()
        .map(|(k, v)| (k.clone(), v.clone()))
        .collect();

    // usage-lib expects argv to include the program name, and reports the
    // selected command relative to it.
    let mut input = vec![spec.bin.clone()];
    input.extend(vector.argv.iter().cloned());

    match Parser::new(&spec).with_env(env).parse(&input) {
        Ok(out) => {
            let cmd = out
                .cmds
                .iter()
                .skip(1) // the root command is not part of the path
                .map(|c| c.name.clone())
                .collect();
            let flags = out
                .flags
                .iter()
                .map(|(flag, value)| (flag.name.clone(), convert(value)))
                .collect();
            let args = out
                .args
                .iter()
                .map(|(arg, value)| (arg.name.clone(), convert(value)))
                .collect();
            Observed::Parsed(Parsed { cmd, flags, args })
        }
        Err(e) => classify(&e.to_string()),
    }
}

fn convert(value: &ParseValue) -> Value {
    match value {
        ParseValue::Bool(b) => Value::Bool(*b),
        ParseValue::String(s) => Value::Str(s.clone()),
        ParseValue::MultiBool(b) => Value::Bools(b.clone()),
        ParseValue::MultiString(s) => Value::Strs(s.clone()),
    }
}

/// Map a usage-lib error onto one of the grammar's error codes.
///
/// Matching on the rendered message rather than on `UsageErr` variants: parsing
/// returns a `miette::Error`, so the concrete type is erased by the time it gets
/// here, and `InvalidFlag` would need its `reason` inspected anyway. The strings
/// below are load-bearing and will need revisiting whenever usage-lib rewords a
/// diagnostic — which is why an unrecognized message becomes
/// [`Observed::Unclassified`] and fails loudly instead of quietly matching
/// whatever the vector expected.
fn classify(msg: &str) -> Observed {
    let code = if msg.contains("Missing required flag") {
        ErrorCode::MissingRequiredFlag
    } else if msg.contains("Missing required arg") {
        ErrorCode::MissingRequiredArg
    } else if msg.contains("can only be set after a `--` separator") {
        ErrorCode::ArgRequiresDoubleDash
    } else if msg.contains("requires at least") {
        // Both the arg and flag forms of "too few" say this; they differ only in
        // whether the subject is rendered as <name> or --name, and the corpus
        // does not distinguish subject.
        ErrorCode::VarTooFew
    } else if msg.contains("accepts at most") {
        ErrorCode::VarTooMany
    } else if msg.contains("Invalid flag") {
        // usage-lib funnels several distinct situations through InvalidFlag, so
        // the reason has to be read to tell them apart.
        if msg.contains("requires an argument") || msg.contains("missing value") {
            ErrorCode::MissingFlagValue
        } else if msg.contains("Invalid choice") || msg.contains("expected one of") {
            ErrorCode::InvalidChoice
        } else {
            ErrorCode::UnknownFlag
        }
    } else if msg.contains("Invalid choice") || msg.contains("expected one of") {
        ErrorCode::InvalidChoice
    } else if msg.contains("Unexpected argument") || msg.contains("unexpected") {
        ErrorCode::UnexpectedArg
    } else {
        return Observed::Unclassified(msg.to_string());
    };
    Observed::Failed(code)
}

/// Every duplicate id in the corpus, so uniqueness can be asserted.
pub fn duplicate_ids<'a>(vectors: impl Iterator<Item = &'a Vector>) -> Vec<String> {
    let mut seen: BTreeMap<&str, usize> = BTreeMap::new();
    for v in vectors {
        *seen.entry(v.id.as_str()).or_default() += 1;
    }
    seen.into_iter()
        .filter(|(_, n)| *n > 1)
        .map(|(id, _)| id.to_string())
        .collect()
}
