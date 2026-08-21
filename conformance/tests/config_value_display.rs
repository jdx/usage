//! One value, written the same way by the spec parser and by the resolver.
//!
//! A `config` block's default is coerced by the *spec's* parser and its choices are read by the
//! *runtime's*, so one character of difference refused a `default=1.0` beside a `choice 1.0`
//! written identically. `usage-lib` cannot depend on `usage-config` — a CLI carries a resolver,
//! not a spec parser — so the rule exists in both. This crate depends on both, which makes it
//! the one place that can see them at once, so a change to either is a failure rather than a
//! surprise found later in a fleet CLI.

#[test]
fn the_spec_and_the_runtime_write_a_value_the_same_way() {
    for value in [
        0.0_f64,
        1.0,
        -1.0,
        0.5,
        1.25,
        1e3,
        1e300,
        f64::MIN,
        f64::MAX,
    ] {
        assert_eq!(
            usage::spec::config::SpecConfigValue::Float(value).display(),
            usage_config::Value::Float(value).display(),
            "the two crates write {value} differently"
        );
    }
}
