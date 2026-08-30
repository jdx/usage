use usage::parse::ParseValue;
use usage::Spec;
use usage_derive::{Args, Cli, Subcommands};

#[derive(Debug, PartialEq, Eq, Args)]
struct TaskClause {
    task: String,
    #[usage(double_dash = "automatic")]
    args: Vec<String>,
}

#[derive(Debug, Cli)]
#[usage(bin = "typed-clause")]
struct TypedClause {
    #[usage(clause, separator = ":::")]
    tasks: Vec<TaskClause>,
}

#[derive(Debug, PartialEq, Eq, Args)]
struct ToolClause {
    #[usage(long)]
    postinstall: Option<String>,
    tool: String,
}

#[derive(Debug, Cli)]
#[usage(bin = "implicit-clause", unknown_flags = "error")]
struct ImplicitClause {
    #[usage(clause)]
    tools: Vec<ToolClause>,
}

#[derive(Debug, Cli)]
#[usage(bin = "related-clause", unknown_flags = "error")]
struct RelatedClause {
    #[usage(long, requires = "tool")]
    force: bool,
    #[usage(long, conflicts = "tool")]
    select: bool,
    #[usage(clause)]
    tools: Vec<ToolClause>,
}

#[derive(Debug, Cli)]
#[usage(bin = "nested-clause")]
struct NestedClause {
    #[usage(subcommand)]
    command: NestedCommands,
}

#[derive(Debug, Subcommands)]
enum NestedCommands {
    Use(NestedUse),
}

#[derive(Debug, Args)]
struct NestedUse {
    #[usage(clause)]
    tools: Vec<ToolClause>,
}

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

fn implicit_spec() -> Spec {
    r#"
min_usage_version "6.6"
name "implicit-clause"
bin "implicit-clause"
unknown_flags "error"
clause "tools" {
  flag "--postinstall <COMMAND>"
  arg "<tool>"
}
"#
    .parse()
    .expect("valid implicit clause spec")
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
fn explicit_separator_does_not_hide_a_missing_scoped_flag_value() {
    let spec: Spec = r#"
name "clause"
bin "clause"
clause "tasks" separator=":::" {
  flag "--postinstall <COMMAND>"
  arg "<task>"
}
"#
    .parse()
    .expect("valid clause spec");
    let error = usage::Parser::new(&spec)
        .parse(&["clause", "--postinstall", ":::"].map(str::to_string))
        .unwrap_err();
    assert!(
        error.to_string().contains("requires an argument"),
        "{error}"
    );
}

#[test]
fn double_dash_does_not_hide_a_missing_implicit_scoped_flag_value() {
    let spec: Spec = r#"
name "implicit-clause"
bin "implicit-clause"
clause "tools" {
  flag "--postinstall <COMMAND>"
  arg "<tool>"
}
"#
    .parse()
    .expect("valid implicit clause spec");
    let error = usage::Parser::new(&spec)
        .parse(&["implicit-clause", "--postinstall", "--", "a"].map(str::to_string))
        .unwrap_err();
    assert!(
        error.to_string().contains("requires an argument"),
        "{error}"
    );
}

#[test]
fn clause_variadic_can_preserve_double_dash() {
    let spec: Spec = r#"
name "clause"
bin "clause"
clause "tasks" separator=":::" {
  arg "<task>"
  arg "[args]..." double_dash="preserve"
}
"#
    .parse()
    .expect("valid clause spec");
    let parsed = usage::Parser::new(&spec)
        .parse(&["clause", "lint", "--", "--fix"].map(str::to_string))
        .expect("preserved double dash is clause data");
    assert_eq!(
        strings(parsed.clauses["tasks"][0].iter(), "args"),
        ["--", "--fix"]
    );
}

#[test]
fn clause_arguments_can_accept_negative_numbers() {
    let spec: Spec = r#"
name "clause"
bin "clause"
unknown_flags "error"
clause "numbers" separator=":::" {
  arg "<number>" allow_negative_numbers=#true
}
"#
    .parse()
    .expect("valid clause spec");
    let parsed = usage::Parser::new(&spec)
        .parse(&["clause", "-1", ":::", "-2"].map(str::to_string))
        .expect("negative numbers are clause values, not flags");
    let instances = &parsed.clauses["numbers"];
    assert_eq!(strings(instances[0].iter(), "number"), ["-1"]);
    assert_eq!(strings(instances[1].iter(), "number"), ["-2"]);
}

#[test]
fn clause_round_trips_through_canonical_kdl() {
    let spec = spec();
    let emitted = spec.to_string();
    let reparsed: Spec = emitted.parse().expect("emitted clause spec reparses");
    let clause = reparsed.cmd.clause.expect("clause retained");
    assert_eq!(clause.name, "tasks");
    assert_eq!(clause.separator.as_deref(), Some(":::"));
    assert_eq!(clause.args.len(), 2);
}

#[test]
fn implicit_clause_round_trips_and_scopes_reference_flags() {
    let spec = implicit_spec();
    let emitted = spec.to_string();
    assert!(emitted.contains("clause tools {"), "{emitted}");
    assert!(emitted.contains("flag --postinstall"), "{emitted}");
    let reparsed: Spec = emitted.parse().expect("emitted implicit clause reparses");
    assert_eq!(reparsed.cmd.clause.as_ref().unwrap().separator, None);

    let parsed = usage::Parser::new(&reparsed)
        .parse(
            &[
                "implicit-clause",
                "--postinstall",
                "A",
                "a",
                "--postinstall",
                "B",
                "b",
            ]
            .map(str::to_string),
        )
        .expect("valid invocation");
    assert_eq!(parsed.clauses["tools"].len(), 2);
    assert_eq!(strings(parsed.clauses["tools"][0].iter(), "tool"), ["a"]);
    assert_eq!(strings(parsed.clauses["tools"][1].iter(), "tool"), ["b"]);
    let flags = &parsed.clause_flags["tools"];
    assert_eq!(flags.len(), 2);
    assert!(matches!(
        flags[0]
            .iter()
            .find(|(flag, _)| flag.name == "postinstall")
            .map(|(_, value)| value),
        Some(ParseValue::String(value)) if value == "A"
    ));
    assert!(matches!(
        flags[1]
            .iter()
            .find(|(flag, _)| flag.name == "postinstall")
            .map(|(_, value)| value),
        Some(ParseValue::String(value)) if value == "B"
    ));

    for (case, invalid) in [
        vec![
            "implicit-clause",
            "--postinstall",
            "A",
            "--postinstall",
            "B",
            "a",
        ],
        vec!["implicit-clause", "--postinstall", "A"],
    ]
    .into_iter()
    .enumerate()
    {
        assert!(
            usage::Parser::new(&reparsed)
                .parse(&invalid.into_iter().map(str::to_string).collect::<Vec<_>>())
                .is_err(),
            "accepted invalid implicit clause {case}"
        );
    }
}

#[test]
fn scoped_flag_requirements_are_checked_per_instance() {
    let spec: Spec = r#"
name "implicit-clause"
bin "implicit-clause"
flag "--format <FORMAT>" default="json"
clause "tools" {
  flag "--postinstall <COMMAND>" required=#true
  arg "<tool>"
}
"#
    .parse()
    .expect("valid required scoped flag spec");

    usage::Parser::new(&spec)
        .parse(&["implicit-clause".to_string()])
        .expect("a required scoped flag does not manufacture a clause instance");
    usage::Parser::new(&spec)
        .parse(&[
            "implicit-clause".to_string(),
            "--postinstall".to_string(),
            "setup".to_string(),
            "a".to_string(),
        ])
        .expect("the scoped flag satisfies its own instance");

    for argv in [
        vec!["implicit-clause", "a"],
        vec!["implicit-clause", "--postinstall", "setup", "a", "b"],
    ] {
        let error = usage::Parser::new(&spec)
            .parse(&argv.into_iter().map(str::to_string).collect::<Vec<_>>())
            .unwrap_err();
        assert!(format!("{error:?}").contains("postinstall"), "{error:?}");
    }
}

#[test]
fn scoped_flag_conflicts_do_not_cross_clause_instances() {
    let spec: Spec = r#"
name "implicit-clause"
bin "implicit-clause"
clause "tools" {
  flag "--build" conflicts="--skip-build"
  flag "--skip-build"
  arg "<tool>"
}
"#
    .parse()
    .expect("valid scoped conflict spec");

    usage::Parser::new(&spec)
        .parse(&["implicit-clause", "--build", "a", "--skip-build", "b"].map(str::to_string))
        .expect("flags in different instances do not conflict");
    let error = usage::Parser::new(&spec)
        .parse(&["implicit-clause", "--build", "--skip-build", "a"].map(str::to_string))
        .unwrap_err();
    assert!(format!("{error:?}").contains("conflicts"), "{error:?}");
}

#[test]
fn scoped_flag_fallbacks_fill_each_existing_instance() {
    let spec: Spec = r#"
name "implicit-clause"
bin "implicit-clause"
clause "tools" {
  flag "--postinstall <COMMAND>" env="SETUP" required=#true
  flag "--mode <MODE>" default="auto" conflicts="--special"
  flag "--special"
  flag "--label <LABEL>" {
    default_if "--special" "special"
  }
  flag "--from-format <FORMAT>" {
    default_if "--format=json" "json"
  }
  arg "<tool>"
}
"#
    .parse()
    .expect("valid scoped fallback spec");
    let parsed = usage::Parser::new(&spec)
        .with_env([("SETUP".to_string(), "shared".to_string())].into())
        .parse(&["implicit-clause", "a", "--special", "b"].map(str::to_string))
        .expect("scoped fallbacks satisfy each instance");
    let instances = &parsed.clause_flags["tools"];
    assert_eq!(instances.len(), 2);
    for instance in instances {
        assert!(instance.iter().any(|(flag, value)| {
            flag.name == "postinstall"
                && matches!(value, ParseValue::String(value) if value == "shared")
        }));
        assert!(instance.iter().any(|(flag, value)| {
            flag.name == "mode" && matches!(value, ParseValue::String(value) if value == "auto")
        }));
    }
    assert!(!instances[0].keys().any(|flag| flag.name == "label"));
    assert!(instances[1].iter().any(|(flag, value)| {
        flag.name == "label" && matches!(value, ParseValue::String(value) if value == "special")
    }));
    assert!(instances
        .iter()
        .all(|instance| !instance.keys().any(|flag| flag.name == "from-format")));

    let empty = usage::Parser::new(&spec)
        .with_env([("SETUP".to_string(), "shared".to_string())].into())
        .parse(&["implicit-clause".to_string()])
        .expect("fallbacks do not manufacture a clause instance");
    assert!(empty.clauses.get("tools").is_none_or(Vec::is_empty));
    assert!(empty.clause_flags.get("tools").is_none_or(Vec::is_empty));
}

#[test]
fn runtime_tables_keep_scoped_flag_identity_when_emitted() {
    let built = usage_conformance::tables::build_spec(&implicit_spec());
    let emitted = built.to_kdl();
    assert_eq!(
        emitted.matches("flag --postinstall").count(),
        1,
        "{emitted}"
    );
    emitted
        .parse::<Spec>()
        .expect("runtime tables emit a reparsable scoped flag");
}

#[test]
fn implicit_clause_rejects_ambiguous_layouts_and_flag_spellings() {
    for invalid in [
        r#"name bad
bin bad
clause tools { arg "[tool]" }
"#,
        r#"name bad
bin bad
clause tools { arg "<tool>" var=#true }
"#,
        r#"name bad
bin bad
clause tools { arg "<tool>"; arg "<version>" }
"#,
        r#"name bad
bin bad
flag --postinstall
clause tools { flag --postinstall; arg "<tool>" }
"#,
        r#"name bad
bin bad
flagset shared { flag --postinstall }
use shared
clause tools { flag --postinstall; arg "<tool>" }
"#,
    ] {
        assert!(invalid.parse::<Spec>().is_err(), "accepted:\n{invalid}");
    }
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

#[test]
fn derive_collects_each_clause_instance() {
    let parsed = TypedClause::parse_from(&[
        std::ffi::OsStr::new("lint"),
        std::ffi::OsStr::new("--fix"),
        std::ffi::OsStr::new(":::"),
        std::ffi::OsStr::new("test"),
        std::ffi::OsStr::new("--all"),
    ])
    .expect("typed clause parses");
    assert_eq!(
        parsed.tasks,
        [
            TaskClause {
                task: "lint".into(),
                args: vec!["--fix".into()]
            },
            TaskClause {
                task: "test".into(),
                args: vec!["--all".into()]
            },
        ]
    );
    let emitted = TypedClause::to_kdl();
    assert!(emitted.contains("clause tasks separator=:::"), "{emitted}");
}

#[test]
fn explicit_clause_rejects_a_trailing_separator() {
    let error =
        TypedClause::parse_from(&[std::ffi::OsStr::new("lint"), std::ffi::OsStr::new(":::")])
            .unwrap_err();
    assert!(
        matches!(error, usage_argv::Error::MissingRequired { name: "TASK" }),
        "{error:?}"
    );
}

#[test]
fn derived_clause_arguments_appear_in_compiled_help() {
    let spec = TypedClause::spec();
    let usage = usage_argv::help::usage_line(&["typed-clause"], spec.root);
    assert!(usage.contains("<TASK>"), "{usage}");
    assert!(usage.contains(":::"), "{usage}");

    let help = usage_argv::help::short_help(spec, &["typed-clause"], &[spec.root]);
    assert!(help.contains("Arguments:"), "{help}");
    assert!(help.contains("<TASK>"), "{help}");
    assert!(help.contains("[ARGS]…"), "{help}");
}

#[test]
fn implicit_clause_flags_apply_to_the_following_positional() {
    let emitted = ImplicitClause::to_kdl();
    assert!(emitted.contains("clause tools {"), "{emitted}");
    assert_eq!(
        emitted.matches("flag --postinstall").count(),
        1,
        "{emitted}"
    );
    let spec = ImplicitClause::spec();
    let help = usage_argv::help::short_help(spec, &["implicit-clause"], &[spec.root]);
    assert!(help.contains("--postinstall"), "{help}");
    assert!(help.contains("[TOOL]…"), "{help}");
    for line in ["implicit-clause --", "implicit-clause a --"] {
        let split =
            usage_argv::complete::split(line, line.len(), usage_argv::complete::Shell::Bash);
        assert!(
            usage_argv::complete::candidates(spec, &split)
                .iter()
                .any(|candidate| candidate.value == "--postinstall"),
            "scoped flag missing after {line:?}"
        );
    }
    let parsed = ImplicitClause::parse_from(&[
        std::ffi::OsStr::new("--postinstall"),
        std::ffi::OsStr::new("setup-a"),
        std::ffi::OsStr::new("a"),
        std::ffi::OsStr::new("--postinstall"),
        std::ffi::OsStr::new("setup-b"),
        std::ffi::OsStr::new("b"),
        std::ffi::OsStr::new("c"),
    ])
    .expect("implicit clauses parse");
    assert_eq!(
        parsed.tools,
        [
            ToolClause {
                postinstall: Some("setup-a".into()),
                tool: "a".into(),
            },
            ToolClause {
                postinstall: Some("setup-b".into()),
                tool: "b".into(),
            },
            ToolClause {
                postinstall: None,
                tool: "c".into(),
            },
        ]
    );
}

#[test]
fn command_flags_can_name_clause_arguments() {
    let spec = RelatedClause::spec();
    let force = spec
        .root
        .flags
        .iter()
        .find(|flag| flag.flag.name == "force")
        .expect("force metadata");
    assert_eq!(force.requires, ["tool"]);

    RelatedClause::parse_from(&[
        std::ffi::OsStr::new("--force"),
        std::ffi::OsStr::new("node"),
    ])
    .expect("the clause terminal satisfies the command flag requirement");

    let missing = RelatedClause::parse_from(&[std::ffi::OsStr::new("--force")]).unwrap_err();
    assert!(
        matches!(missing, usage_argv::Error::MissingRequired { name: "TOOL" }),
        "{missing:?}"
    );

    let conflict = RelatedClause::parse_from(&[
        std::ffi::OsStr::new("--select"),
        std::ffi::OsStr::new("node"),
    ])
    .unwrap_err();
    assert!(
        matches!(
            conflict,
            usage_argv::Error::ConflictingFlags { other: "TOOL", .. }
        ),
        "{conflict:?}"
    );

    let kdl = RelatedClause::to_kdl();
    assert!(kdl.contains("requires=TOOL"), "{kdl}");
    assert!(kdl.contains("conflicts=TOOL"), "{kdl}");
    let portable: Spec = kdl.parse().expect("derived relationship spec reparses");
    let explained = usage::Parser::new(&portable)
        .explain(&["related-clause", "--force", "node"].map(str::to_string))
        .expect("the reference parser binds the invocation");
    assert!(
        explained.errors.is_empty(),
        "the reference parser resolves command relationships into the clause: {explained:#?}"
    );
    let conflict = usage::Parser::new(&portable)
        .parse(&["related-clause", "--select", "node"].map(str::to_string))
        .unwrap_err();
    assert!(
        conflict.to_string().contains("conflicts with TOOL"),
        "{conflict}"
    );
}

#[test]
fn an_implicit_clause_may_have_no_instances() {
    let parsed = ImplicitClause::parse_from(&[]).expect("an outer Vec clause is optional");
    assert!(parsed.tools.is_empty());

    let parsed = usage::Parser::new(&implicit_spec())
        .parse(&["implicit-clause".to_string()])
        .expect("the reference parser agrees that an outer clause is optional");
    assert!(parsed.clauses.get("tools").is_none_or(Vec::is_empty));
}

#[test]
fn non_root_derive_emits_and_parses_implicit_clauses() {
    let spec = NestedClause::spec();
    let use_meta = spec.root.cmd.subcommands[0];
    assert!(use_meta.clause.is_some());
    assert_eq!(use_meta.clause.unwrap().args[0].name, "TOOL");

    let parsed = NestedClause::parse_from(&[
        std::ffi::OsStr::new("use"),
        std::ffi::OsStr::new("--postinstall"),
        std::ffi::OsStr::new("setup"),
        std::ffi::OsStr::new("a"),
    ])
    .expect("nested clause parses");
    let NestedCommands::Use(command) = parsed.command;
    assert_eq!(command.tools[0].postinstall.as_deref(), Some("setup"));
    assert_eq!(command.tools[0].tool, "a");
}

#[test]
fn implicit_clause_rejects_duplicate_or_unterminated_scoped_flags() {
    let duplicate = ImplicitClause::parse_from(&[
        std::ffi::OsStr::new("--postinstall"),
        std::ffi::OsStr::new("one"),
        std::ffi::OsStr::new("--postinstall"),
        std::ffi::OsStr::new("two"),
        std::ffi::OsStr::new("tool"),
    ])
    .unwrap_err();
    assert!(matches!(duplicate, usage_argv::Error::DuplicateFlag { .. }));

    let unterminated = ImplicitClause::parse_from(&[
        std::ffi::OsStr::new("--postinstall"),
        std::ffi::OsStr::new("setup"),
    ])
    .unwrap_err();
    assert!(
        matches!(
            unterminated,
            usage_argv::Error::MissingRequired { name: "TOOL" }
        ),
        "{unterminated:?}"
    );
}
