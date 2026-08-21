use usage_config_build::source_of_spec;

fn generated(settings: &str) -> String {
    source_of_spec(
        &format!("name \"ex\"\nbin \"ex\"\nconfig {{\n{settings}\n}}\n"),
        "ex.usage.kdl",
    )
    .expect("registry")
}

#[test]
fn aliases_reach_the_runtime_registry() {
    let source = generated(
        r#"prop "jobs" type="uint" {
    alias "parallelism" "threads"
}"#,
    );
    assert!(
        source.contains(r#"aliases: &["parallelism", "threads"]"#),
        "{source}"
    );
}

#[test]
fn explicit_optionality_overrides_default_inference() {
    let source = generated(
        r#"prop "required_without_default" type="string" optional=#false
prop "optional_with_default" type="string" default="value" optional=#true"#,
    );
    assert!(
        source.contains("pub required_without_default: String"),
        "{source}"
    );
    assert!(
        source.contains("fold.required(prop::REQUIRED_WITHOUT_DEFAULT)"),
        "{source}"
    );
    assert!(
        source.contains("pub optional_with_default: Option<String>"),
        "{source}"
    );
    assert!(
        source.contains("fold.optional(prop::OPTIONAL_WITH_DEFAULT)"),
        "{source}"
    );
}

#[test]
fn ambiguous_aliases_are_refused_together() {
    let error = source_of_spec(
        r#"name "ex"
bin "ex"
config {
    prop "jobs" { alias "shared" }
    prop "threads" { alias "shared" "jobs" }
}
"#,
        "ex.usage.kdl",
    )
    .expect_err("ambiguous names");
    let text = error.to_string();
    assert!(
        text.contains("`shared` names both `jobs` and `threads`"),
        "{text}"
    );
    assert!(
        text.contains("`jobs` names both `jobs` and `threads`"),
        "{text}"
    );
}
