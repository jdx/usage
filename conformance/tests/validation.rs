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
