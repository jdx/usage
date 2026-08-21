use serde::Deserialize;

#[derive(Deserialize)]
struct Vector {
    expression: String,
    value: String,
    valid: bool,
}

#[test]
fn rust_matches_the_portable_validation_vectors() {
    let vectors: Vec<Vector> =
        serde_json::from_str(include_str!("../validation.json")).expect("valid vectors");
    assert!(!vectors.is_empty(), "validation fixture must not be empty");
    for vector in vectors {
        let actual = usage_validation::validate(&vector.expression, &vector.value)
            .unwrap_or_else(|err| panic!("{} with {:?}: {err}", vector.expression, vector.value));
        assert_eq!(
            actual, vector.valid,
            "{} with {:?}",
            vector.expression, vector.value
        );
    }
}

/// `matches` is in the vectors above, so this only records the other half of the choice:
/// which builtins the Rust runtime does *not* carry, and that it says so.
///
/// The failure is at evaluation, not at `check`, which has only ever parsed — `nosuchfn(value)`
/// behaves identically and always has.
#[test]
fn a_builtin_this_build_leaves_out_names_the_feature_to_add() {
    let expression = "date(value) > now()";
    usage_validation::check(expression).expect("it parses; the language has these builtins");
    let err = usage_validation::validate(expression, "2020-01-01")
        .expect_err("this build does not carry them");
    assert!(err.contains("`temporal` feature"), "{err}");
}
