//! A group of mutually exclusive flags declared as an enum.
//!
//! clap#2621's ask, and clap's most-requested derive ergonomic: the flags that exclude one
//! another are variants, so the code that reads them matches on a type instead of on which of
//! several `bool`s is set. Nothing new reaches the spec — the enum lowers to the `group` node
//! and the switches it names — so everything here is checked twice: once against the errors the
//! generated code produces, and once against the KDL it emits and the reference implementation
//! that reads it.

use std::ffi::OsStr;

use usage_argv::{help, Error};
use usage_derive::{ArgGroup, Args, Cli, Subcommands, ValueEnum};

fn argv<const N: usize>(tokens: [&str; N]) -> [&OsStr; N] {
    tokens.map(OsStr::new)
}

/// How to print the result
#[derive(ArgGroup, Debug, PartialEq)]
#[usage(name = "format")]
enum Format {
    /// Print JSON
    Json,
    /// Print YAML
    Yaml,
    /// Print one line per record
    #[usage(short = 'p', long = "plain")]
    PlainText,
}

/// Where to read from
#[derive(ArgGroup, Debug, PartialEq)]
enum Source {
    /// Read from standard input
    Stdin,
    /// Read from the clipboard
    Clipboard,
}

#[derive(Debug, PartialEq, ValueEnum)]
#[usage(ignore_case)]
enum MigrationSource {
    Prettier,
    Biome,
}

/// What the formatter should do.
#[derive(ArgGroup, Debug, PartialEq)]
enum Mode {
    /// Write files in place.
    Write,
    /// Check files without changing them.
    Check,
    /// Migrate a configuration.
    #[usage(value_name = "SOURCE", value_enum)]
    Migrate(MigrationSource),
    /// Read source from standard input as this path.
    StdinFilepath(std::path::PathBuf),
}

/// Lint severity overrides, applied from left to right.
#[derive(ArgGroup, Debug, PartialEq)]
#[usage(name = "lint-filter", multiple)]
enum LintFilter {
    #[usage(short = 'A')]
    Allow(String),
    #[usage(short = 'W')]
    Warn(String),
    #[usage(short = 'D')]
    Deny(String),
}

#[derive(Cli)]
struct OrderedFilters {
    #[usage(arg_group)]
    filters: Vec<LintFilter>,
    #[usage(long, overrides = "--allow")]
    all: bool,
}

#[test]
fn a_multiple_argument_group_preserves_cross_flag_order() {
    let a = argv([
        "-D",
        "all",
        "--allow=no-debugger",
        "-Wstyle",
        "--deny",
        "correctness",
    ]);
    assert_eq!(
        OrderedFilters::parse_from(&a)
            .expect("ordered filters")
            .filters,
        vec![
            LintFilter::Deny("all".into()),
            LintFilter::Allow("no-debugger".into()),
            LintFilter::Warn("style".into()),
            LintFilter::Deny("correctness".into()),
        ]
    );

    let kdl = OrderedFilters::to_kdl();
    assert!(
        kdl.contains("group lint-filter --allow --warn --deny multiple=#true"),
        "{kdl}"
    );
}

#[test]
fn overriding_a_multiple_group_member_removes_its_ordered_occurrences() {
    let a = argv([
        "-A",
        "dead-code",
        "-Wstyle",
        "--allow=unused",
        "--all",
        "-Dwarnings",
    ]);
    let parsed = OrderedFilters::parse_from(&a).expect("overridden filters");
    assert!(parsed.all);
    assert_eq!(
        parsed.filters,
        vec![
            LintFilter::Warn("style".into()),
            LintFilter::Deny("warnings".into()),
        ]
    );
}

#[derive(Cli)]
#[usage(bin = "valued-group")]
struct ValuedGroup {
    #[usage(arg_group)]
    mode: Option<Mode>,
    #[usage(long, required_if_eq("--migrate", "biome"))]
    confirm: bool,
}

#[test]
fn a_group_member_can_carry_a_typed_value() {
    let a = argv(["--migrate", "BIOME", "--confirm"]);
    assert_eq!(
        ValuedGroup::parse_from(&a)
            .expect("value-enum payload")
            .mode,
        Some(Mode::Migrate(MigrationSource::Biome))
    );

    let a = argv(["--stdin-filepath", "src/input.ts"]);
    assert_eq!(
        ValuedGroup::parse_from(&a).expect("path payload").mode,
        Some(Mode::StdinFilepath("src/input.ts".into()))
    );

    let a = argv(["--migrate", "unknown"]);
    assert!(matches!(
        ValuedGroup::parse_from(&a),
        Err(Error::InvalidValue(error)) if error.name == "migrate"
    ));
}

#[test]
fn case_insensitive_group_values_match_relationships_the_same_way_they_parse() {
    let a = argv(["--migrate", "BIOME"]);
    assert!(ValuedGroup::parse_from(&a).is_err());

    let a = argv(["--migrate", "BIOME", "--confirm"]);
    assert!(ValuedGroup::parse_from(&a).is_ok());
}

#[test]
fn a_value_carrying_group_member_reaches_help_and_the_spec() {
    let help =
        help::render(ValuedGroup::spec(), ValuedGroup::spec().root.cmd, false).expect("a page");
    assert!(help.contains("--migrate <SOURCE>"), "{help}");
    assert!(help.contains("[prettier, biome]"), "{help}");
    assert!(help.contains("--stdin-filepath <STDIN_FILEPATH>"), "{help}");

    let kdl = ValuedGroup::to_kdl();
    assert!(
        kdl.contains("flag --migrate")
            && kdl.contains("arg <SOURCE>")
            && kdl.contains("choices ignore_case=#true")
            && kdl.contains("choice prettier")
            && kdl.contains("choice biome"),
        "{kdl}"
    );
    assert!(
        kdl.contains("group mode --write --check --migrate --stdin-filepath"),
        "{kdl}"
    );
}

#[test]
fn update_reports_an_invalid_group_payload_without_changing_the_value() {
    let mut parsed = ValuedGroup {
        mode: Some(Mode::Write),
        confirm: false,
    };
    let a = argv(["--migrate", "unknown"]);
    assert!(matches!(
        parsed.try_update_from(&a),
        Err(Error::InvalidValue(error)) if error.name == "migrate"
    ));
    assert_eq!(parsed.mode, Some(Mode::Write));
}

/// A CLI whose format is optional and whose source is not.
#[derive(Cli)]
#[usage(bin = "grp")]
struct Grp {
    /// A file to work on
    #[usage(long)]
    file: Option<String>,
    #[usage(arg_group)]
    format: Option<Format>,
    #[usage(arg_group)]
    source: Source,
}

#[test]
fn an_optional_group_may_be_left_alone() {
    let a = argv(["--stdin"]);
    let grp = Grp::parse_from(&a).expect("saying nothing about format is fine");
    assert_eq!(grp.format, None);
    assert_eq!(grp.source, Source::Stdin);
}

#[test]
fn one_member_selects_its_variant() {
    let a = argv(["--stdin", "--yaml"]);
    assert_eq!(
        Grp::parse_from(&a).expect("one member").format,
        Some(Format::Yaml)
    );

    // By its declared spellings too, since a member is a flag like any other.
    let a = argv(["--stdin", "--plain"]);
    assert_eq!(
        Grp::parse_from(&a).expect("one member").format,
        Some(Format::PlainText)
    );
    let a = argv(["--stdin", "-p"]);
    assert_eq!(
        Grp::parse_from(&a).expect("one member").format,
        Some(Format::PlainText)
    );
}

#[test]
fn two_members_cannot_both_be_given() {
    let a = argv(["--stdin", "--json", "--yaml"]);
    assert!(
        matches!(
            Grp::parse_from(&a),
            Err(Error::ConflictingFlags {
                name: "yaml",
                other: "json"
            })
        ),
        "{:?}",
        Grp::parse_from(&a).err()
    );

    // The pair reported is the first two in declaration order, which is what the user has to
    // choose between — and the same pair a hand-written group's pairwise check reports.
    let a = argv(["--stdin", "-p", "--yaml"]);
    assert!(matches!(
        Grp::parse_from(&a),
        Err(Error::ConflictingFlags {
            name: "plain",
            other: "yaml"
        })
    ));
}

#[test]
fn a_bare_field_makes_the_group_required() {
    let a = argv([]);
    assert!(matches!(
        Grp::parse_from(&a),
        Err(Error::MissingGroup {
            group: "source",
            members: ["--stdin", "--clipboard"]
        })
    ));

    let a = argv(["--clipboard"]);
    assert_eq!(
        Grp::parse_from(&a).expect("one member").source,
        Source::Clipboard
    );
}

#[test]
fn a_conflict_answers_before_an_unsatisfied_group_does() {
    // Both are wrong: `source` has no member and `format` has two. The conflict is the more
    // useful answer, and it is the order the rest of the checks already follow.
    let a = argv(["--json", "--yaml"]);
    assert!(matches!(
        Grp::parse_from(&a),
        Err(Error::ConflictingFlags { .. })
    ));
}

#[test]
fn the_group_reaches_the_emitted_spec_and_usage_lib_agrees() {
    let kdl = Grp::to_kdl();
    // The switches are this command's flags, written inline where the field was declared.
    for flag in [
        r#"flag --json help="Print JSON""#,
        r#"flag --yaml help="Print YAML""#,
        r#"flag "-p --plain" help="Print one line per record""#,
        r#"flag --stdin help="Read from standard input""#,
        r#"flag --clipboard help="Read from the clipboard""#,
    ] {
        assert!(kdl.contains(flag), "{flag} missing from:\n{kdl}");
    }
    assert!(kdl.contains("group format --json --yaml --plain"), "{kdl}");
    assert!(
        kdl.contains("group source --stdin --clipboard required=#true"),
        "{kdl}"
    );

    // The reference implementation reads what the derive wrote and enforces the same rule,
    // which is the point of the spec being the definition rather than a summary.
    let spec: usage::Spec = kdl.parse().expect("the emitted spec should parse");
    let format = spec.cmd.groups.iter().find(|g| g.name == "format").unwrap();
    assert!(!format.required);
    assert!(!format.multiple);
    assert_eq!(format.members.len(), 3);
    let source = spec.cmd.groups.iter().find(|g| g.name == "source").unwrap();
    assert!(source.required);
    assert_eq!(source.members.len(), 2);
}

#[test]
fn help_lists_the_members_with_their_own_descriptions() {
    let page = help::render(Grp::spec(), Grp::spec().root.cmd, false).expect("a page");
    for line in [
        "      --json",
        "      --yaml",
        "  -p, --plain",
        "      --stdin",
        "      --clipboard",
    ] {
        assert!(
            page.lines().any(|l| l.starts_with(line)),
            "no line starts `{line}`:\n{page}"
        );
    }
    assert!(page.contains("Print one line per record"), "{page}");
}

/// The same enum on a subcommand's own `Args`, beside a flattened group.
#[derive(Args)]
struct Shared {
    /// Say more
    #[usage(long, short = 'v')]
    verbose: bool,
}

/// Convert something
#[derive(Args)]
struct Convert {
    #[usage(flatten)]
    shared: Shared,
    #[usage(arg_group)]
    format: Option<Format>,
    /// What to convert
    target: String,
}

#[derive(Subcommands)]
enum Command {
    Convert(Convert),
}

#[derive(Cli)]
#[usage(bin = "nested")]
struct Nested {
    #[usage(subcommand)]
    command: Option<Command>,
}

#[test]
fn a_group_works_on_a_subcommand_beside_a_flattened_one() {
    let a = argv(["convert", "--json", "-v", "x"]);
    let Some(Command::Convert(convert)) = Nested::parse_from(&a).expect("parses").command else {
        panic!("expected convert");
    };
    assert_eq!(convert.format, Some(Format::Json));
    assert!(convert.shared.verbose);
    assert_eq!(convert.target, "x");

    let a = argv(["convert", "--json", "--yaml", "x"]);
    assert!(matches!(
        Nested::parse_from(&a),
        Err(Error::ConflictingFlags { .. })
    ));

    // The subcommand's flags are joined in the order the fields were written: the flattened
    // struct's, then the group's, then this command's own positional.
    let kdl = Nested::to_kdl();
    assert!(kdl.contains("group format --json --yaml --plain"), "{kdl}");
}

/// A sibling flag that names a group member — the relationship lookup Bugbot caught as missing.
#[derive(Cli)]
#[usage(bin = "rel")]
struct Rel {
    #[usage(arg_group)]
    format: Option<Format>,
    /// Only legal beside JSON
    #[usage(long, requires = "--json")]
    pretty: bool,
    /// Last one wins against JSON
    #[usage(long, overrides = "--json")]
    raw: bool,
    /// Cannot sit beside YAML
    #[usage(long, conflicts = "--yaml")]
    strict: bool,
}

#[test]
fn a_sibling_relationship_can_name_a_group_member() {
    // requires: --pretty alone is MissingRequired for --json.
    let a = argv(["--pretty"]);
    assert!(matches!(
        Rel::parse_from(&a),
        Err(Error::MissingRequired { name: "json", .. })
    ));
    let a = argv(["--json", "--pretty"]);
    let rel = Rel::parse_from(&a).expect("json satisfies pretty");
    assert_eq!(rel.format, Some(Format::Json));
    assert!(rel.pretty);

    // conflicts: --strict with --yaml.
    let a = argv(["--yaml", "--strict"]);
    assert!(matches!(
        Rel::parse_from(&a),
        Err(Error::ConflictingFlags { .. })
    ));
    let a = argv(["--json", "--strict"]);
    assert!(Rel::parse_from(&a).expect("json does not conflict").strict);

    // overrides: --raw displaces a prior --json.
    let a = argv(["--json", "--raw"]);
    let rel = Rel::parse_from(&a).expect("raw displaces json");
    assert_eq!(rel.format, None);
    assert!(rel.raw);
}
