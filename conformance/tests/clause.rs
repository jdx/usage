use usage::parse::ParseValue;
use usage::Spec;

fn spec() -> Spec {
    r#"
min_usage_version "6.6"
name "clause"
bin "clause"
clause "tasks" separator=":::" {
  arg "<task>"
  arg "[args]..." var=#true double_dash="automatic"
}
"#
    .parse()
    .expect("valid clause spec")
}

fn strings<'a>(
    mut instance: impl Iterator<Item = (&'a std::sync::Arc<usage::SpecArg>, &'a ParseValue)>,
    name: &str,
) -> Vec<String> {
    instance
        .find(|(arg, _)| arg.name == name)
        .map(|(_, value)| match value {
            ParseValue::String(value) => vec![value.clone()],
            ParseValue::MultiString(values) => values.clone(),
            other => panic!("unexpected value: {other:?}"),
        })
        .unwrap_or_default()
}

#[test]
fn clause_instances_preserve_values_and_restart_flags() {
    let parsed = usage::Parser::new(&spec())
        .parse(&["clause", "lint", "--fix", ":::", "test", "--all"].map(str::to_string))
        .expect("valid invocation");
    let instances = &parsed.clauses["tasks"];
    assert_eq!(instances.len(), 2);
    assert_eq!(strings(instances[0].iter(), "task"), ["lint"]);
    assert_eq!(strings(instances[0].iter(), "args"), ["--fix"]);
    assert_eq!(strings(instances[1].iter(), "task"), ["test"]);
    assert_eq!(strings(instances[1].iter(), "args"), ["--all"]);
    assert!(parsed.args.is_empty());
}

#[test]
fn explicit_double_dash_protects_a_literal_separator() {
    let parsed = usage::Parser::new(&spec())
        .parse(&["clause", "lint", "--", ":::", "tail"].map(str::to_string))
        .expect("valid invocation");
    let instances = &parsed.clauses["tasks"];
    assert_eq!(instances.len(), 1);
    assert_eq!(strings(instances[0].iter(), "args"), [":::", "tail"]);
}

#[test]
fn clause_round_trips_through_canonical_kdl() {
    let spec = spec();
    let emitted = spec.to_string();
    let reparsed: Spec = emitted.parse().expect("emitted clause spec reparses");
    let clause = reparsed.cmd.clause.expect("clause retained");
    assert_eq!(clause.name, "tasks");
    assert_eq!(clause.separator, ":::");
    assert_eq!(clause.args.len(), 2);
}

#[test]
fn clause_arguments_participate_in_relationship_checks() {
    for (spec, argv, expected) in [
        (
            r#"name "clause"
bin "clause"
clause "items" separator=":::" {
  arg "[output]" requires="input"
  arg "[input]"
}
"#,
            vec!["clause", "artifact"],
            "input",
        ),
        (
            r#"name "clause"
bin "clause"
flag "--json" conflicts="task"
clause "items" separator=":::" { arg "[task]" }
"#,
            vec!["clause", "--json", "lint"],
            "conflicts with task",
        ),
        (
            r#"name "clause"
bin "clause"
clause "items" separator=":::" {
  arg "[trigger]"
  arg "[dependent]" required_if="trigger"
}
"#,
            vec!["clause", "yes"],
            "dependent",
        ),
        (
            r#"name "clause"
bin "clause"
clause "items" separator=":::" {
  arg "[output]" requires="input"
  arg "[input]"
}
"#,
            vec!["clause", "first", ":::", "second", "input"],
            "instance 1",
        ),
        (
            r#"name "clause"
bin "clause"
clause "items" separator=":::" {
  arg "[trigger]"
  arg "[dependent]" {
    required_if_eq "trigger" "yes"
  }
}
"#,
            vec!["clause", "yes", ":::", "no", "present"],
            "instance 1",
        ),
    ] {
        let spec: Spec = spec.parse().expect("valid relationship spec");
        let error = usage::Parser::new(&spec)
            .parse(&argv.into_iter().map(str::to_string).collect::<Vec<_>>())
            .unwrap_err();
        assert!(format!("{error:?}").contains(expected), "{error:?}");
    }
}
