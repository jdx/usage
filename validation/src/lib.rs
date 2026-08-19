//! Cold-path, portable validation of values declared by a usage spec.
//!
//! Expressions use the language implemented by both `jdx/expr.rs` and the original
//! `expr-lang/expr` Go package. The spec supplies a raw CLI value as `value`, so explicit
//! conversions such as `int(value)` have the same meaning in every runtime.

#![forbid(unsafe_code)]

use expr::{Context, Value};

/// Check that a validation declaration is syntactically valid without evaluating it.
pub fn check(expression: &str) -> Result<(), String> {
    expr::compile(expression)
        .map(|_| ())
        .map_err(|error| error.to_string())
}

/// Evaluate a validation expression for one raw CLI value.
///
/// A valid expression must return a boolean. `false` means the value does not satisfy the
/// declaration; an evaluator error or a non-boolean result means the declaration itself is
/// invalid.
pub fn validate(expression: &str, value: &str) -> Result<bool, String> {
    let mut context = Context::default();
    context.insert("value", value);
    match expr::eval(expression, &context).map_err(|error| error.to_string())? {
        Value::Bool(valid) => Ok(valid),
        result => Err(format!(
            "validation expression must return a boolean, got {result}"
        )),
    }
}

#[cfg(test)]
mod tests {
    use super::{check, validate};

    #[test]
    fn checks_syntax_without_needing_a_sample_value() {
        assert_eq!(check("int(value) > 0"), Ok(()));
        assert!(check("int(value) >").is_err());
    }

    #[test]
    fn validates_raw_values_with_expr_conversions() {
        let expression = "int(value) >= 1 && int(value) <= 65535";
        assert_eq!(validate(expression, "9229"), Ok(true));
        assert_eq!(validate(expression, "0"), Ok(false));
        assert!(validate(expression, "not-a-port").is_err());
    }

    #[test]
    fn requires_a_boolean_result() {
        let error = validate("int(value)", "42").unwrap_err();
        assert!(error.contains("must return a boolean"), "{error}");
    }
}
