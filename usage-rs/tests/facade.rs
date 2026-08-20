#![cfg(feature = "spec")]
#![deny(unused_variables)]

use std::ffi::OsStr;
use std::path::{Path, PathBuf};

use usage_rs as usage;
use usage_rs::{Args, Cli, Subcommands, ValueEnum};

const INLINE_AFTER_HELP: &str = "Inline command details from a Rust constant.";

#[derive(Cli)]
#[usage(bin = "ex")]
struct Ex {
    #[usage(subcommand)]
    command: Command,
}

#[derive(Subcommands)]
enum Command {
    Show(Show),
    /// Print version information
    Version,
}

#[derive(Cli)]
#[command(bin = "negated-requirements", subcommand_negates_reqs)]
#[allow(dead_code)]
struct NegatedRequirements {
    #[arg(long, required = true)]
    config: Option<String>,
    #[command(subcommand)]
    command: Option<NegatedCommand>,
}

#[derive(Subcommands)]
#[allow(dead_code)]
enum NegatedCommand {
    Run {
        #[arg(long)]
        target: String,
    },
    Show,
}

#[derive(Cli)]
#[command(bin = "argument-conflict", args_conflicts_with_subcommands)]
#[allow(dead_code)]
struct ArgumentConflict {
    #[arg(long)]
    verbose: bool,
    #[command(subcommand)]
    command: Option<ArgumentConflictCommand>,
}

#[derive(Subcommands)]
#[allow(dead_code)]
enum ArgumentConflictCommand {
    Run,
}

#[derive(Cli)]
#[command(bin = "precedence", subcommand_precedence_over_arg)]
struct Precedence {
    #[arg(long, num_args = 1..)]
    values: Vec<String>,
    #[command(subcommand)]
    command: Option<ArgumentConflictCommand>,
}

#[derive(Subcommands, serde::Deserialize)]
enum InlineCommand {
    /// Run a named benchmark.
    #[usage(after_long_help = INLINE_AFTER_HELP)]
    Run {
        #[serde(default)]
        #[arg(long)]
        bench: Option<String>,
        #[usage(long)]
        runs: Option<u32>,
        #[cfg_attr(all(), arg(long))]
        iterations: Option<u32>,
        #[arg]
        label: Option<String>,
        #[cfg(any())]
        #[arg(long)]
        platform_only: Option<String>,
    },
    Empty {},
    PlatformOnly {
        #[cfg(any())]
        #[arg(long)]
        platform_only: Option<String>,
    },
}

#[derive(Cli)]
#[usage(bin = "inline-ex")]
struct InlineEx {
    #[usage(subcommand)]
    command: InlineCommand,
}

#[derive(Cli)]
#[usage(bin = "positional-relations")]
#[usage(group("input", required))]
struct PositionalRelations {
    #[usage(long, conflicts = "value", group = "input")]
    from_file: Option<String>,
    #[usage(conflicts("--from-file", "--stdin"), group = "input")]
    value: Option<String>,
    #[usage(long)]
    stdin: bool,
}

#[derive(Cli)]
#[command(bin = "clap-spellings", rename_all = "kebab-case")]
struct ClapSpellings {
    #[arg(
        id = "output",
        long,
        visible_aliases = ["out", "dest"],
        aliases = ["quietly", "silent-output"]
    )]
    path: Option<String>,
}

#[derive(Cli)]
#[command(bin = "fixed-arity")]
struct FixedArity {
    #[arg(long, num_args = 2, value_names = ["START", "END"])]
    pair: Vec<String>,
    #[arg(long, num_args = 2, value_name = "ITEM")]
    pair_same: Vec<String>,
    #[arg(long, value_names = ["INPUT"])]
    input: Option<String>,
}

#[derive(usage::Args)]
struct FlattenedRelationshipTargets {
    #[usage(long, default = "nested-default")]
    nested: Option<String>,
    #[usage(long)]
    frozen: bool,
    #[usage(long, default_if("--preset", "true"))]
    key: bool,
    #[usage(long)]
    json: bool,
    #[usage(long)]
    preset: bool,
}

#[derive(Cli)]
#[usage(bin = "flattened-relationships")]
struct FlattenedRelationships {
    #[usage(long, overrides = "--nested")]
    replace: bool,
    #[usage(long, conflicts = "--frozen")]
    fix: bool,
    #[usage(long, requires = "--key")]
    signed: bool,
    #[usage(long, requires_if("json", "--key"))]
    mode: Option<String>,
    #[usage(long, required_if = "--json")]
    schema: Option<String>,
    #[usage(long, default_if("--json", "auto"))]
    output: Option<String>,
    #[usage(flatten)]
    shared: FlattenedRelationshipTargets,
}

#[derive(Cli)]
#[usage(bin = "relationship-families")]
#[allow(dead_code)]
struct RelationshipFamilies {
    #[arg(long)]
    mode: Option<String>,
    #[arg(long)]
    scope: Option<String>,
    #[arg(long, required_if_eq("mode", "remote"))]
    token: Option<String>,
    #[arg(
        long,
        required_if_eq_all = [("mode", "remote"), ("scope", "global")]
    )]
    approval: Option<String>,
    #[arg(long, required_unless_present_any = ["stdin", "file"])]
    input: Option<String>,
    #[arg(long, required_unless_present_all = ["stdin", "file"])]
    checksum: Option<String>,
    #[arg(long)]
    stdin: bool,
    #[arg(long)]
    file: Option<String>,
    #[arg(requires_all = ["mode", "scope"])]
    request: Option<String>,
}

#[derive(Cli)]
#[usage(bin = "single-unless-all")]
#[allow(dead_code)]
struct SingleUnlessAll {
    #[arg(long)]
    stdin: bool,
    #[arg(long, required_unless_present_all = ["stdin"])]
    token: Option<String>,
    #[arg(required_unless_present_all = ["stdin"])]
    target: Option<String>,
}

#[derive(usage::Args)]
#[command(next_help_heading = "Network")]
#[allow(dead_code)]
struct HeadedSharedArgs {
    /// Registry URL.
    #[arg(long)]
    registry: Option<String>,
    /// Authentication token.
    #[arg(long, help_heading = "Authentication")]
    token: Option<String>,
}

#[derive(Cli)]
#[usage(bin = "headed-flatten")]
#[allow(dead_code)]
struct HeadedFlatten {
    /// Ordinary root flag.
    #[arg(long)]
    verbose: bool,
    #[usage(flatten)]
    shared: HeadedSharedArgs,
}

#[derive(Args)]
#[allow(dead_code)]
struct FlattenedRepeatPolicy {
    #[arg(long)]
    jobs: Option<u32>,
    #[usage(long, negate = "no-color")]
    color: bool,
}

#[derive(Cli)]
#[usage(bin = "strict-flatten", args_override_self = false)]
struct StrictFlatten {
    #[usage(flatten)]
    shared: FlattenedRepeatPolicy,
}

#[derive(ValueEnum)]
#[usage(ignore_case)]
enum Shell {
    #[value(
        aliases(["bourne-again", "bash-shell"]),
        visible_alias = "b",
        help = "Bourne Again shell"
    )]
    Bash,
    /// Z shell.
    #[value(
        aliases = ["shell-z", "z-shell", "zsh-shell"],
        hide = true
    )]
    Zsh,
    #[cfg(windows)]
    PowerShell,
}

impl std::str::FromStr for Shell {
    type Err = String;

    fn from_str(value: &str) -> Result<Self, Self::Err> {
        #[cfg(windows)]
        if value.eq_ignore_ascii_case("power-shell") {
            return Ok(Self::PowerShell);
        }
        if value.eq_ignore_ascii_case("bash") {
            Ok(Self::Bash)
        } else if value.eq_ignore_ascii_case("zsh") || value.eq_ignore_ascii_case("shell-z") {
            Ok(Self::Zsh)
        } else {
            Err(format!("unsupported shell: {value}"))
        }
    }
}

#[derive(Cli)]
#[usage(bin = "choice-ex")]
struct ChoiceEx {
    #[usage(long, value_enum)]
    shell: Shell,
}

#[derive(Cli)]
#[usage(bin = "strict-ex", unknown_flags = "error")]
struct StrictEx {}

#[derive(Cli)]
#[usage(bin = "unit-root")]
struct UnitRoot;

#[derive(Args)]
struct UnitArgs;

#[derive(Subcommands)]
enum UnitArgsCommand {
    Empty(UnitArgs),
}

#[derive(Cli)]
#[usage(bin = "unit-args")]
struct UnitArgsCli {
    #[usage(subcommand)]
    command: UnitArgsCommand,
}

#[derive(Cli)]
#[usage(bin = "completion-dedup")]
struct CompletionDedup {
    #[usage(long, value_name = "PATH", value_hint = usage::ValueHint::FilePath)]
    input: Option<PathBuf>,
    #[usage(long, value_name = "PATH", value_hint = usage::ValueHint::FilePath)]
    output: Option<PathBuf>,
}

#[derive(Args)]
struct SharedArgs {
    #[usage(long)]
    verbose: bool,
    #[usage(long)]
    target: Option<String>,
}

#[derive(Subcommands)]
enum SharedArgsCommand {
    First(SharedArgs),
    Second(SharedArgs),
}

#[derive(Cli)]
#[usage(bin = "shared-args")]
struct SharedArgsCli {
    #[usage(subcommand)]
    command: SharedArgsCommand,
}

#[allow(dead_code)]
#[derive(Args)]
struct SharedNestedLeaf {
    #[usage(long)]
    target: Option<String>,
}

#[allow(dead_code)]
#[derive(Subcommands)]
enum SharedNestedCommand {
    Inner(SharedNestedLeaf),
}

#[allow(dead_code)]
#[derive(Args)]
struct SharedNestedArgs {
    #[usage(subcommand)]
    command: SharedNestedCommand,
}

#[allow(dead_code)]
#[derive(Subcommands)]
enum SharedNestedRootCommand {
    First(SharedNestedArgs),
    Second(SharedNestedArgs),
}

#[allow(dead_code)]
#[derive(Cli)]
#[usage(bin = "shared-nested")]
struct SharedNestedCli {
    #[usage(subcommand)]
    command: SharedNestedRootCommand,
}

const DEFAULT_RUNS: u32 = 7;
const DYNAMIC_ABOUT: &str = "Metadata from a Rust constant.";
const DYNAMIC_AFTER_HELP: &str = "More details from a Rust constant.";

fn computed_version() -> &'static str {
    "1.2.3+runtime"
}

#[cfg(feature = "completions")]
fn runtime_program() -> &'static str {
    "runtime-ex"
}

#[cfg(feature = "completions")]
#[derive(Cli)]
#[usage(
    name = runtime_program(),
    name_spec = "portable-ex",
    bin = runtime_program(),
    bin_spec = "portable-ex",
    completion
)]
struct RuntimeIdentityEx;

#[derive(Cli)]
#[usage(
    bin = "dynamic-ex",
    version = computed_version(),
    version_spec = "1.2.3",
    about = DYNAMIC_ABOUT,
    after_long_help = DYNAMIC_AFTER_HELP
)]
struct DynamicEx {
    #[usage(long, default_value_t = DEFAULT_RUNS, default = "7")]
    runs: u32,
    #[usage(long, default_value_t, default = "0")]
    retries: u16,
}

/// Show one file
#[derive(Args)]
struct Show {
    #[usage(long, value_hint = usage::ValueHint::FilePath)]
    file: PathBuf,
}

#[test]
fn one_dependency_provides_derives_runtime_and_value_hints() {
    let _hint_from_facade = usage::ValueHint::FilePath;
    let argv = [
        OsStr::new("show"),
        OsStr::new("--file"),
        OsStr::new("input.txt"),
    ];
    let cli = Ex::parse_from(&argv).expect("valid command line");
    let Command::Show(show) = cli.command else {
        panic!("show command should be selected");
    };
    assert_eq!(show.file, Path::new("input.txt"));
    assert!(Ex::to_kdl().contains("complete file type=path"));

    let embedded = Ex::app().name("embedded").bin("embedded").spec();
    assert_eq!(embedded.name, "embedded");
    assert_eq!(embedded.bin, Some("embedded"));
}

#[test]
fn unit_subcommands_use_the_facade_derive() {
    let cli = Ex::parse_from(&[OsStr::new("version")]).expect("valid unit subcommand");
    assert!(matches!(cli.command, Command::Version));
}

#[test]
fn unit_cli_and_args_structs_parse_without_shape_rewrites() {
    let root = UnitRoot::parse_from(&[]).expect("unit root should parse");
    let UnitRoot = root;
    assert!(UnitRoot::to_kdl().contains("name unit-root"));

    let cli =
        UnitArgsCli::parse_from(&[OsStr::new("empty")]).expect("unit Args command should parse");
    assert!(matches!(cli.command, UnitArgsCommand::Empty(UnitArgs)));
    assert!(UnitArgsCli::to_kdl().contains("cmd empty"));
}

#[test]
fn identical_builtin_completers_are_emitted_once() {
    let parsed = CompletionDedup::parse_from(&[
        OsStr::new("--input"),
        OsStr::new("in.txt"),
        OsStr::new("--output"),
        OsStr::new("out.txt"),
    ])
    .expect("both path flags should parse");
    assert_eq!(parsed.input.as_deref(), Some(Path::new("in.txt")));
    assert_eq!(parsed.output.as_deref(), Some(Path::new("out.txt")));

    let kdl = CompletionDedup::to_kdl();
    assert_eq!(kdl.matches("complete path type=path").count(), 1, "{kdl}");
}

#[test]
fn one_args_type_can_back_multiple_commands() {
    for command in ["first", "second"] {
        let cli = SharedArgsCli::parse_from(&[OsStr::new(command), OsStr::new("--verbose")])
            .expect("either command should route into the shared Args type");
        match cli.command {
            SharedArgsCommand::First(args) | SharedArgsCommand::Second(args) => {
                assert!(args.verbose);
                assert!(args.target.is_none());
            }
        }
    }

    let kdl = SharedArgsCli::to_kdl();
    assert!(kdl.contains("cmd first"), "{kdl}");
    assert!(kdl.contains("cmd second"), "{kdl}");
    assert_eq!(kdl.matches("flag --verbose").count(), 2, "{kdl}");
}

#[cfg(feature = "completions")]
fn first_targets(_: &usage::complete::CompleteCtx<'_>) -> Vec<usage::complete::Candidate<'static>> {
    vec![usage::complete::Candidate::new("first-target")]
}

#[cfg(feature = "completions")]
fn second_targets(
    _: &usage::complete::CompleteCtx<'_>,
) -> Vec<usage::complete::Candidate<'static>> {
    vec![usage::complete::Candidate::new("second-target")]
}

#[cfg(feature = "completions")]
fn run_ready<F: std::future::Future>(future: F) -> F::Output {
    let mut cx = std::task::Context::from_waker(std::task::Waker::noop());
    let mut future = Box::pin(future);
    match future.as_mut().poll(&mut cx) {
        std::task::Poll::Ready(output) => output,
        std::task::Poll::Pending => panic!("facade completion callback unexpectedly waited"),
    }
}

#[cfg(feature = "completions")]
#[test]
fn shared_args_completion_overlays_stay_on_the_selected_command() {
    static OVERLAYS: [usage::complete::CompletionOverlay<'static>; 2] = [
        usage::complete::CompletionOverlay::sync("first", "target", first_targets),
        usage::complete::CompletionOverlay::sync("second", "target", second_targets),
    ];

    for (command, expected) in [("first", "first-target"), ("second", "second-target")] {
        let argv = [
            std::ffi::OsString::from("__complete_word__"),
            std::ffi::OsString::from("--shell"),
            std::ffi::OsString::from("bash"),
            std::ffi::OsString::from("--line"),
            std::ffi::OsString::from(format!("shared-args {command} --target ")),
        ];
        let rendered = run_ready(
            SharedArgsCli::app()
                .completion_app()
                .completions(&OVERLAYS)
                .completion_request(&argv),
        )
        .expect("hidden completion request should be handled");
        assert_eq!(rendered, format!("{expected}\n"));
    }
}

#[cfg(feature = "completions")]
#[test]
fn shared_nested_completion_overlays_keep_the_parent_route() {
    static OVERLAYS: [usage::complete::CompletionOverlay<'static>; 2] = [
        usage::complete::CompletionOverlay::sync("first inner", "target", first_targets),
        usage::complete::CompletionOverlay::sync("second inner", "target", second_targets),
    ];

    for (command, expected) in [("first", "first-target"), ("second", "second-target")] {
        let argv = [
            std::ffi::OsString::from("__complete_word__"),
            std::ffi::OsString::from("--shell"),
            std::ffi::OsString::from("bash"),
            std::ffi::OsString::from("--line"),
            std::ffi::OsString::from(format!("shared-nested {command} inner --target ")),
        ];
        let rendered = run_ready(
            SharedNestedCli::app()
                .completion_app()
                .completions(&OVERLAYS)
                .completion_request(&argv),
        )
        .expect("hidden completion request should be handled");
        assert_eq!(rendered, format!("{expected}\n"));
    }
}

#[test]
fn struct_style_subcommands_bind_fields_in_place() {
    let cli = InlineEx::parse_from(&[
        OsStr::new("run"),
        OsStr::new("--bench"),
        OsStr::new("startup"),
        OsStr::new("--runs"),
        OsStr::new("5"),
        OsStr::new("--iterations"),
        OsStr::new("9"),
        OsStr::new("nightly"),
    ])
    .expect("inline fields should parse");
    let InlineCommand::Run {
        bench,
        runs,
        iterations,
        label,
    } = cli.command
    else {
        panic!("run command should be selected");
    };
    assert_eq!(bench.as_deref(), Some("startup"));
    assert_eq!(runs, Some(5));
    assert_eq!(iterations, Some(9));
    assert_eq!(label.as_deref(), Some("nightly"));

    let kdl = InlineEx::to_kdl();
    assert!(kdl.contains("cmd run"), "{kdl}");
    assert!(kdl.contains("flag --bench"), "{kdl}");
    assert!(kdl.contains("arg <BENCH>"), "{kdl}");
    assert!(kdl.contains("flag --runs"), "{kdl}");
    assert!(kdl.contains("arg <RUNS>"), "{kdl}");
    assert!(kdl.contains("flag --iterations"), "{kdl}");
    assert!(kdl.contains("arg \"[LABEL]\""), "{kdl}");
    assert!(kdl.contains(INLINE_AFTER_HELP), "{kdl}");
}

#[test]
fn empty_struct_style_subcommands_do_not_emit_unused_bindings() {
    let cli = InlineEx::parse_from(&[OsStr::new("empty")]).expect("empty command should parse");
    assert!(matches!(cli.command, InlineCommand::Empty {}));

    let cli = InlineEx::parse_from(&[OsStr::new("platform-only")])
        .expect("a command whose fields are cfg'd out should parse");
    assert!(matches!(cli.command, InlineCommand::PlatformOnly {}));
}

#[test]
fn defaults_render_clap_shaped_parse_errors() {
    let argv = [OsStr::new("--wat")];
    let Err(err) = Ex::parse_from(&argv) else {
        panic!("unknown flag should fail");
    };
    // `render_failure` colours via `Style::auto()` when stderr is a TTY or
    // `CLICOLOR_FORCE` is set, which would put ANSI codes inside the quotes and
    // break a literal substring check. Plain style is what a pipe (and this
    // assertion) wants.
    let message =
        usage::diagnostic::render(Ex::spec(), &argv, &err, usage::diagnostic::Style::PLAIN);
    assert!(
        message.contains("unexpected argument '--wat'"),
        "defaults should enable diagnostics; got:\n{message}"
    );
}

#[test]
fn emitted_specs_preserve_value_enum_aliases_and_case_policy() {
    let cli = ChoiceEx::parse_from(&[OsStr::new("--shell"), OsStr::new("SHELL-Z")])
        .expect("aliases and ASCII case folding should parse");
    assert!(matches!(cli.shell, Shell::Zsh));

    let kdl = ChoiceEx::to_kdl();
    assert!(kdl.contains("choices ignore_case=#true"), "{kdl}");
    assert!(
        kdl.contains("choice bash help=\"Bourne Again shell\""),
        "{kdl}"
    );
    assert!(kdl.contains("alias b\n"), "{kdl}");
    assert!(!kdl.contains("alias b hide=#true"), "{kdl}");
    assert!(
        kdl.contains("choice zsh help=\"Z shell.\" hide=#true"),
        "{kdl}"
    );
    assert!(kdl.contains("alias shell-z hide=#true"), "{kdl}");
    assert!(kdl.contains("alias z-shell hide=#true"), "{kdl}");
    assert!(kdl.contains("alias zsh-shell hide=#true"), "{kdl}");
    assert!(kdl.contains("alias bourne-again hide=#true"), "{kdl}");
    #[cfg(not(windows))]
    assert_eq!(<Shell as usage::spec::ValueEnum>::CHOICES, &["bash", "b"]);
    #[cfg(windows)]
    assert_eq!(
        <Shell as usage::spec::ValueEnum>::CHOICES,
        &["bash", "b", "power-shell"]
    );
    #[cfg(not(windows))]
    assert!(!kdl.contains("power-shell"), "{kdl}");
}

#[test]
fn positional_relationships_parse_and_emit_losslessly() {
    let from_file =
        PositionalRelations::parse_from(&[OsStr::new("--from-file"), OsStr::new("vars.env")])
            .expect("the flag satisfies the group");
    assert_eq!(from_file.from_file.as_deref(), Some("vars.env"));
    assert!(!from_file.stdin);

    let positional = PositionalRelations::parse_from(&[OsStr::new("literal")])
        .expect("the positional satisfies the group");
    assert_eq!(positional.value.as_deref(), Some("literal"));

    let err = PositionalRelations::parse_from(&[
        OsStr::new("--from-file"),
        OsStr::new("vars.env"),
        OsStr::new("literal"),
    ]);
    assert!(err.is_err(), "the flag conflicts with the positional");

    let kdl = PositionalRelations::to_kdl();
    assert!(kdl.contains("conflicts --from-file --stdin"), "{kdl}");
    assert_eq!(
        kdl.matches("conflicts=VALUE").count(),
        1,
        "a single flag conflict should be emitted once: {kdl}"
    );
    assert!(
        !kdl.contains("arg \"[VALUE]\" conflicts="),
        "several positional conflicts belong only in the child node: {kdl}"
    );
    assert!(kdl.contains("group input --from-file VALUE"), "{kdl}");
}

#[test]
fn clap_field_ids_and_aliases_need_no_rewrite() {
    for spelling in [
        "--output",
        "--out",
        "--dest",
        "--quietly",
        "--silent-output",
    ] {
        let parsed = ClapSpellings::parse_from(&[OsStr::new(spelling), OsStr::new("file")])
            .expect("every visible alias should parse");
        assert_eq!(parsed.path.as_deref(), Some("file"));
    }

    let kdl = ClapSpellings::to_kdl();
    assert!(kdl.contains("--output --out --dest"), "{kdl}");
    assert!(!kdl.contains("--dest --quietly"), "{kdl}");
    assert!(
        kdl.contains("alias --quietly --silent-output hide=#true"),
        "{kdl}"
    );
}

#[test]
fn clap_value_arity_stays_on_each_flag_occurrence() {
    let parsed = FixedArity::parse_from(&[
        OsStr::new("--pair"),
        OsStr::new("a"),
        OsStr::new("b"),
        OsStr::new("--input"),
        OsStr::new("file"),
    ])
    .expect("the fixed-arity occurrence should consume exactly two values");
    assert_eq!(parsed.pair, ["a", "b"]);
    assert!(parsed.pair_same.is_empty());
    assert_eq!(parsed.input.as_deref(), Some("file"));

    let kdl = FixedArity::to_kdl();
    assert!(kdl.contains("flag --pair"), "{kdl}");
    assert!(kdl.contains("arg \"<START> <END>\""), "{kdl}");
    assert!(!kdl.contains("flag --pair var_min=2"), "{kdl}");
    assert!(kdl.contains("arg <INPUT>"), "{kdl}");
    let help = usage::help::render(FixedArity::spec(), FixedArity::command(), false)
        .expect("the root has help to render");
    assert!(help.contains("--input <INPUT>"), "{help}");
    assert!(help.contains("--pair-same <ITEM> <ITEM>"), "{help}");
    assert!(
        kdl.contains("flag --pair-same") && kdl.contains("arg \"<ITEM> <ITEM>\""),
        "{kdl}"
    );

    assert!(
        FixedArity::parse_from(&[OsStr::new("--pair"), OsStr::new("only-one")]).is_err(),
        "a partial fixed-arity occurrence must fail its value minimum"
    );
}

#[test]
fn relationships_resolve_targets_inside_flattened_args() {
    let parent_wins = FlattenedRelationships::parse_from(&[
        OsStr::new("--nested"),
        OsStr::new("given"),
        OsStr::new("--replace"),
    ])
    .expect("a parent flag should displace a flattened flag");
    assert!(parent_wins.replace);
    assert_eq!(parent_wins.shared.nested.as_deref(), Some("nested-default"));

    let flattened_wins = FlattenedRelationships::parse_from(&[
        OsStr::new("--replace"),
        OsStr::new("--nested"),
        OsStr::new("given"),
    ])
    .expect("a later flattened flag should displace its parent peer");
    assert!(!flattened_wins.replace);
    assert_eq!(flattened_wins.shared.nested.as_deref(), Some("given"));

    assert!(
        FlattenedRelationships::parse_from(&[OsStr::new("--fix"), OsStr::new("--frozen"),])
            .is_err()
    );
    assert!(FlattenedRelationships::parse_from(&[OsStr::new("--signed")]).is_err());
    let defaulted =
        FlattenedRelationships::parse_from(&[OsStr::new("--signed"), OsStr::new("--preset")])
            .expect("a flattened conditional default should satisfy the parent requirement");
    assert!(defaulted.signed);
    assert!(defaulted.shared.key);
    assert!(
        FlattenedRelationships::parse_from(&[OsStr::new("--mode"), OsStr::new("json"),]).is_err()
    );
    assert!(FlattenedRelationships::parse_from(&[OsStr::new("--json")]).is_err());

    let parsed = FlattenedRelationships::parse_from(&[
        OsStr::new("--json"),
        OsStr::new("--schema"),
        OsStr::new("schema.json"),
    ])
    .expect("the flattened condition should satisfy the schema relationship");
    assert!(!parsed.fix);
    assert!(!parsed.replace);
    assert!(!parsed.signed);
    assert!(parsed.mode.is_none());
    assert_eq!(parsed.schema.as_deref(), Some("schema.json"));
    assert_eq!(parsed.output.as_deref(), Some("auto"));
    assert!(parsed.shared.json);
    assert!(!parsed.shared.frozen);
    assert_eq!(parsed.shared.nested.as_deref(), Some("nested-default"));
    assert!(!parsed.shared.key);
    assert!(!parsed.shared.preset);

    let kdl = FlattenedRelationships::to_kdl();
    assert!(kdl.contains("conflicts=--frozen"), "{kdl}");
    assert!(kdl.contains("overrides=--nested"), "{kdl}");
    assert!(kdl.contains("requires=--key"), "{kdl}");
    assert!(kdl.contains("required_if=--json"), "{kdl}");
}

#[test]
fn complete_relationship_families_follow_clap_truth_tables() {
    assert!(RelationshipFamilies::parse_from(&[
        OsStr::new("--mode"),
        OsStr::new("remote"),
        OsStr::new("--stdin"),
    ])
    .is_err());
    assert!(RelationshipFamilies::parse_from(&[
        OsStr::new("--mode"),
        OsStr::new("remote"),
        OsStr::new("--token"),
        OsStr::new("secret"),
        OsStr::new("--scope"),
        OsStr::new("global"),
        OsStr::new("--stdin"),
    ])
    .is_err());
    RelationshipFamilies::parse_from(&[
        OsStr::new("--mode"),
        OsStr::new("remote"),
        OsStr::new("--token"),
        OsStr::new("secret"),
        OsStr::new("--scope"),
        OsStr::new("global"),
        OsStr::new("--approval"),
        OsStr::new("yes"),
        OsStr::new("--stdin"),
        OsStr::new("--file"),
        OsStr::new("input.txt"),
    ])
    .expect("all conditional requirements are satisfied");

    assert!(RelationshipFamilies::parse_from(
        &[OsStr::new("--stdin"), OsStr::new("request.json"),]
    )
    .is_err());
    RelationshipFamilies::parse_from(&[
        OsStr::new("--mode"),
        OsStr::new("local"),
        OsStr::new("--scope"),
        OsStr::new("project"),
        OsStr::new("--stdin"),
        OsStr::new("--checksum"),
        OsStr::new("sum"),
        OsStr::new("request.json"),
    ])
    .expect("requires_all accepts every satisfied target");

    let kdl = RelationshipFamilies::to_kdl();
    assert!(kdl.contains("required_if_eq --mode remote"), "{kdl}");
    assert!(
        kdl.contains("required_if_eq_all --mode remote --scope global"),
        "{kdl}"
    );
    assert!(kdl.contains("required_unless --stdin --file"), "{kdl}");
    assert!(kdl.contains("required_unless_all --stdin --file"), "{kdl}");
    assert!(kdl.contains("requires --mode --scope"), "{kdl}");
}

#[test]
fn single_unless_all_survives_the_short_property_form() {
    let kdl = SingleUnlessAll::to_kdl();
    assert!(
        kdl.matches("required_unless_all=--stdin").count() >= 2,
        "{kdl}"
    );
    let reparsed: usage_parser::Spec = kdl.parse().expect("the emitted properties parse back");
    let token = reparsed
        .cmd
        .flags
        .iter()
        .find(|flag| flag.name == "token")
        .unwrap();
    assert_eq!(token.required_unless_all, ["--stdin"]);
    assert_eq!(reparsed.cmd.args[0].required_unless_all, ["--stdin"]);
}

#[test]
fn flattened_args_keep_their_help_heading_topology() {
    let spec = HeadedFlatten::spec();
    let registry = spec
        .root
        .flags
        .iter()
        .find(|field| field.flag.name == "registry")
        .unwrap();
    let token = spec
        .root
        .flags
        .iter()
        .find(|field| field.flag.name == "token")
        .unwrap();
    assert_eq!(registry.help_heading, Some("Network"));
    assert_eq!(token.help_heading, Some("Authentication"));

    for long in [false, true] {
        let help = usage::help::render(spec, spec.root.cmd, long).unwrap();
        let ordinary = help.find("Flags:").unwrap();
        let network = help.find("Network:").unwrap();
        let authentication = help.find("Authentication:").unwrap();
        assert!(ordinary < network && network < authentication, "{help}");
    }
}

#[test]
fn a_parent_repeat_policy_applies_through_flattening() {
    assert!(
        StrictFlatten::parse_from(&[
            OsStr::new("--jobs"),
            OsStr::new("1"),
            OsStr::new("--jobs"),
            OsStr::new("2"),
        ])
        .is_err(),
        "the strict parent must reject a repeated flattened scalar"
    );

    let parsed = StrictFlatten::parse_from(&[OsStr::new("--color"), OsStr::new("--no-color")])
        .expect("opposite negate forms override each other");
    assert!(!parsed.shared.color);

    for words in [
        &[OsStr::new("--color"), OsStr::new("--color")][..],
        &[
            OsStr::new("--color"),
            OsStr::new("--color"),
            OsStr::new("--no-color"),
        ][..],
    ] {
        assert!(
            StrictFlatten::parse_from(words).is_err(),
            "a repeated spelling remains a duplicate"
        );
    }
}

#[test]
fn typed_subcommands_negate_only_parent_requirements() {
    assert!(NegatedRequirements::parse_from(&[]).is_err());
    NegatedRequirements::parse_from(&[OsStr::new("show")])
        .expect("the selected child satisfies the parent requirement");
    assert!(NegatedRequirements::parse_from(&[OsStr::new("run")]).is_err());
    NegatedRequirements::parse_from(&[
        OsStr::new("run"),
        OsStr::new("--target"),
        OsStr::new("release"),
    ])
    .expect("the selected child still enforces its own requirement");

    let kdl = NegatedRequirements::to_kdl();
    assert!(kdl.contains("subcommand_negates_reqs #true"), "{kdl}");
}

#[test]
fn typed_parent_arguments_exclude_a_later_subcommand() {
    ArgumentConflict::parse_from(&[OsStr::new("run")])
        .expect("a subcommand without parent arguments remains valid");
    assert!(
        ArgumentConflict::parse_from(&[OsStr::new("--verbose"), OsStr::new("run")]).is_err(),
        "a parent flag must exclude a later subcommand"
    );

    let kdl = ArgumentConflict::to_kdl();
    assert!(
        kdl.contains("args_conflicts_with_subcommands #true"),
        "{kdl}"
    );
}

#[test]
fn typed_subcommands_can_interrupt_variadic_values() {
    let parsed = Precedence::parse_from(&[
        OsStr::new("--values"),
        OsStr::new("a"),
        OsStr::new("b"),
        OsStr::new("run"),
    ])
    .expect("the known child should end the variadic flag");
    assert_eq!(parsed.values, ["a", "b"]);
    assert!(matches!(parsed.command, Some(ArgumentConflictCommand::Run)));
    assert!(Precedence::to_kdl().contains("subcommand_precedence_over_arg #true"));
}

#[test]
fn emitted_parser_settings_are_portable_spec_metadata() {
    let Err(_) = StrictEx::parse_from(&[OsStr::new("--wat")]) else {
        panic!("strict parsing should reject an unknown flag");
    };

    let kdl = StrictEx::to_kdl();
    assert!(kdl.contains("unknown_flags error"), "{kdl}");

    let spec: usage_parser::Spec = kdl.parse().expect("usage-lib should read derive output");
    assert_eq!(
        spec.unknown_flags,
        Some(usage_parser::UnknownFlags::Error),
        "the portable spec should retain the runtime setting"
    );
}

#[test]
fn runtime_metadata_expressions_have_explicit_portable_values() {
    let _parse_entry = DynamicEx::parse as fn() -> DynamicEx;
    let cli = DynamicEx::parse_from(&[]).expect("the typed default should be evaluated");
    assert_eq!(cli.runs, DEFAULT_RUNS);
    assert_eq!(cli.retries, 0);

    let kdl = DynamicEx::to_kdl();
    assert!(kdl.contains("version \"1.2.3\""), "{kdl}");
    assert!(kdl.contains("default=\"7\""), "{kdl}");
    assert!(kdl.contains("default=\"0\""), "{kdl}");
    assert!(kdl.contains(DYNAMIC_ABOUT), "{kdl}");
    assert!(kdl.contains(DYNAMIC_AFTER_HELP), "{kdl}");
    let spec: usage_parser::Spec = kdl.parse().expect("the static values should be portable");
    assert_eq!(spec.version.as_deref(), Some("1.2.3"));
}

#[cfg(feature = "completions")]
#[test]
fn runtime_program_identity_is_separate_from_the_portable_spec() {
    let kdl = RuntimeIdentityEx::to_kdl();
    assert!(kdl.contains("name portable-ex"), "{kdl}");
    assert!(kdl.contains("bin portable-ex"), "{kdl}");

    let runtime = RuntimeIdentityEx::runtime_app().spec();
    assert_eq!(runtime.name, "runtime-ex");
    assert_eq!(runtime.bin, Some("runtime-ex"));

    let script = RuntimeIdentityEx::completion_script(usage::complete::Shell::Bash);
    assert!(script.contains("runtime-ex"), "{script}");
    assert!(!script.contains("'portable-ex'"), "{script}");
}
