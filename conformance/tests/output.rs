//! What a command writes, declared beside the command.
//!
//! Not something clap can express, so a CLI that wants to say it writes JSON declares a
//! `--json` flag and puts the rest in prose. Across the jdx.dev fleet that is 123 flag
//! declarations in three incompatible spellings, none of which says what the JSON holds.
//!
//! Two things this records, because they are the whole point of the shape:
//!
//! - the token a user types and the wire format a consumer reads are separate fields, so
//!   `ndjson` and `jsonl` can be the same contract under different names;
//! - selection is resolved on the way through, so the flag that picks an output arrives at
//!   usage-lib already knowing which values it accepts.

use usage::Spec as LibSpec;
use usage_derive::{Args, Cli, Subcommands};

/// Check the project
#[derive(Args)]
#[usage(
    output("human", default, help = "A human-readable report"),
    output(
        "json",
        media_type = "application/json",
        framing = "json",
        help = "One report object",
        schema = "{\n  \"type\": \"object\"\n}"
    ),
    output(
        "jsonl",
        media_type = "application/x-ndjson",
        framing = "jsonl",
        help = "One event per line"
    ),
    output("checkstyle", media_type = "application/xml"),
    output("legacy", hide),
    exit_code(0, "all checks passed"),
    exit_code(1, "a check failed")
)]
struct Check {
    /// Output format
    #[usage(long, select)]
    format: Option<String>,
    /// Only report, never fix
    #[usage(long)]
    dry_run: bool,
}

/// List what is installed
///
/// The other spelling: a boolean flag picks one output rather than a value naming it.
#[derive(Args)]
#[usage(
    output("text", default),
    output("json", framing = "json", select = "--json")
)]
struct List {
    #[usage(long)]
    json: bool,
}

/// Say nothing about what it writes
#[derive(Args)]
struct Quiet {
    #[usage(long)]
    plain: bool,
}

#[derive(Subcommands)]
enum Commands {
    Check(Check),
    List(List),
    Quiet(Quiet),
}

#[derive(Cli)]
#[usage(name = "ex", bin = "ex", exit_code(130, "interrupted"))]
#[allow(dead_code)]
struct Ex {
    #[usage(subcommand)]
    command: Commands,
}

#[derive(Cli)]
#[usage(name = "root-order", output("json"))]
#[allow(dead_code)]
struct RootOrder {
    #[usage(long)]
    verbose: bool,
}

fn parsed() -> LibSpec {
    let kdl = Ex::to_kdl();
    kdl.parse()
        .unwrap_or_else(|e| panic!("usage-lib could not parse the emitted spec: {e}\n\n{kdl}"))
}

#[test]
fn a_name_and_a_framing_are_different_things() {
    let spec = parsed();
    let check = spec.cmd.subcommands.get("check").unwrap();
    let declared: Vec<(&str, &str)> = check
        .outputs
        .iter()
        .map(|o| (o.name.as_str(), o.framing.as_str()))
        .collect();
    assert_eq!(
        declared,
        vec![
            ("human", "text"),
            ("json", "json"),
            ("jsonl", "jsonl"),
            ("checkstyle", "text"),
            ("legacy", "text"),
        ]
    );
    assert!(check.outputs[0].default);
    assert!(check.outputs[2].framing.is_streaming());
    assert!(check.outputs[4].hide);
    assert_eq!(
        check.outputs[3].media_type.as_deref(),
        Some("application/xml")
    );
}

#[test]
fn a_media_type_without_a_selector_starts_a_markdown_list() {
    let spec: LibSpec = r#"
name "ex"
output "xml" media_type="application/xml"
"#
    .parse()
    .expect("standalone output spec");
    let markdown = usage::docs::markdown::MarkdownRenderer::new(spec.clone())
        .render_cmd(&spec.cmd)
        .expect("markdown page");
    assert!(
        markdown.contains("`xml`\n\n- **Media type**: `application/xml`"),
        "{markdown}"
    );
}

#[test]
fn a_schema_reaches_the_spec() {
    let spec = parsed();
    let check = spec.cmd.subcommands.get("check").unwrap();
    assert_eq!(
        check.outputs[1].schema.as_deref(),
        Some("{\n  \"type\": \"object\"\n}")
    );
}

#[test]
fn a_field_level_select_names_its_own_flag() {
    let spec = parsed();
    let check = spec.cmd.subcommands.get("check").unwrap();
    assert_eq!(check.select.as_deref(), Some("--format"));

    // Resolved on the way in, so usage-lib's copy of the flag knows the values without
    // anything downstream having to join outputs to selectors itself.
    let format = check
        .flags
        .iter()
        .find(|f| f.name == "format")
        .expect("--format should be in the spec");
    assert_eq!(
        format
            .arg
            .as_ref()
            .and_then(|a| a.choices.as_ref())
            .map(|c| c.values()),
        Some(vec![
            "human".into(),
            "json".into(),
            "jsonl".into(),
            "checkstyle".into(),
        ])
    );
}

#[test]
fn a_boolean_selector_survives() {
    let spec = parsed();
    let list = spec.cmd.subcommands.get("list").unwrap();
    let json = list.outputs.iter().find(|o| o.name == "json").unwrap();
    assert_eq!(json.select.as_deref(), Some("--json"));
    assert_eq!(
        json.select_argv(list).map(|s| s.argv()),
        Some(vec!["--json".to_string()])
    );
}

#[test]
fn exit_codes_fold_from_the_root() {
    let spec = parsed();
    let check = spec.cmd.subcommands.get("check").unwrap();
    let codes: Vec<(i64, String)> = usage::effective_exit_codes(&spec, std::slice::from_ref(check))
        .into_iter()
        .map(|e| (e.code, e.help))
        .collect();
    assert_eq!(
        codes,
        vec![
            (130, "interrupted".to_string()),
            (0, "all checks passed".to_string()),
            (1, "a check failed".to_string()),
        ]
    );
}

#[test]
fn a_command_that_says_nothing_carries_nothing() {
    let spec = parsed();
    let quiet = spec.cmd.subcommands.get("quiet").unwrap();
    assert!(quiet.outputs.is_empty());
    assert!(quiet.select.is_none());
    // The root's codes still reach it: those are CLI-wide.
    assert!(quiet.exit_codes.is_empty());
}

#[test]
fn the_derives_kdl_is_what_usage_lib_would_write() {
    // The claim the whole design rests on: one authoring surface, one document. If the two
    // writers disagree about a byte, an adopter's checked-in spec churns depending on which
    // tool last touched it.
    let direct = Ex::to_kdl();
    let parsed: LibSpec = direct.parse().unwrap();
    assert_eq!(direct, parsed.to_string());
}

#[test]
fn root_outputs_follow_root_flags_in_both_writers() {
    let direct = RootOrder::to_kdl();
    let parsed: LibSpec = direct.parse().unwrap();
    assert_eq!(direct, parsed.to_string());
    assert!(direct.find("flag --verbose") < direct.find("output json"));
}
