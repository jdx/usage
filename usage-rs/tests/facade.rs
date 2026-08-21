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

#[derive(Cli)]
#[usage(
    bin = "view-host",
    version = "1.2.3",
    before_help = "HOST-ONLY SURROUNDING HELP",
    unknown_flags = "error",
    view(
        "view-run",
        root = "run",
        global = "--verbose",
        global = "--flat-required",
        global = "--flat-default",
        global = "--help-all"
    )
)]
#[cfg_attr(feature = "completions", usage(completion))]
struct ViewHost {
    #[arg(long, global)]
    root_token: String,
    #[arg(long, global, required = true)]
    verbose: bool,
    #[arg(long, global)]
    color: bool,
    #[arg(long = "help-all", global, action = usage::ArgAction::HelpAll)]
    help_all: bool,
    #[command(flatten)]
    view_globals: ViewGlobals,
    #[arg(long)]
    root_number: u32,
    #[command(subcommand)]
    command: ViewCommand,
}

#[derive(Args)]
struct ViewGlobals {
    #[arg(long, global, required = true)]
    flat_required: bool,
    #[arg(long, global, default_value = "carried")]
    flat_default: String,
    #[arg(long, global)]
    flat_hidden: String,
}

#[derive(Subcommands)]
enum ViewCommand {
    /// Run one task.
    Run {
        #[arg(short = 'v', long)]
        verbose: bool,
        #[arg(long)]
        dry_run: bool,
        task: String,
    },
}

#[test]
fn executable_views_emit_and_dispatch_from_argv0() {
    let kdl = ViewHost::to_kdl();
    assert!(kdl.contains("view view-run root=run"), "{kdl}");
    let portable: usage_parser::Spec = kdl.parse().expect("usage-lib should read the view");
    let view = portable
        .for_view("view-run")
        .expect("the declared command should materialize");
    assert_eq!(view.bin, "view-run");
    assert_eq!(view.cmd.name, "view-run");
    assert!(view.cmd.flags.iter().any(|flag| flag.name == "verbose"));

    let parsed = ViewHost::parse_from_argv(&[
        OsStr::new("/usr/local/bin/view-run"),
        OsStr::new("--verbose"),
        OsStr::new("--flat-required"),
        OsStr::new("--dry-run"),
        OsStr::new("build"),
    ])
    .expect("argv0 should promote the declared command");
    assert_eq!(parsed.root_token, "");
    assert!(parsed.verbose);
    assert!(parsed.view_globals.flat_required);
    assert_eq!(parsed.view_globals.flat_default, "carried");
    assert_eq!(parsed.view_globals.flat_hidden, "");
    assert_eq!(parsed.root_number, 0);
    assert!(!parsed.color);
    let ViewCommand::Run {
        verbose,
        dry_run,
        task,
    } = parsed.command;
    assert!(verbose);
    assert!(dry_run);
    assert_eq!(task, "build");

    assert!(matches!(
        ViewHost::parse_from_argv(&[
            OsStr::new("view-run"),
            OsStr::new("--flat-required"),
            OsStr::new("build")
        ]),
        Err(usage::Error::MissingRequired { name: "verbose" })
    ));

    assert!(matches!(
        ViewHost::parse_from_argv(&[
            OsStr::new("view-run"),
            OsStr::new("--verbose"),
            OsStr::new("build")
        ]),
        Err(usage::Error::MissingRequired {
            name: "flat-required"
        })
    ));

    let root_error = match ViewHost::parse_from(&[
        OsStr::new("--flat-required"),
        OsStr::new("run"),
        OsStr::new("build"),
    ]) {
        Err(error) => error,
        Ok(_) => panic!("the host root should enforce its required fields"),
    };
    assert!(
        matches!(
            root_error,
            usage::Error::MissingRequired {
                name: "flat-hidden"
            }
        ),
        "{root_error:?}"
    );

    let help_argv = [OsStr::new("view-run"), OsStr::new("--help")];
    let (cmd, long) =
        match ViewHost::parse_from_argv(&[OsStr::new("view-run"), OsStr::new("--help")]) {
            Err(usage::Error::Help { cmd, long }) => (cmd, long),
            _ => panic!("the view should return the promoted command's help request"),
        };
    let route = usage::help::route_to_view(
        ViewHost::command(),
        &help_argv,
        cmd,
        &ViewHost::spec().views[0],
    )
    .expect("the injected route should be recoverable");
    let page = usage::help::render_view_at_styled(
        ViewHost::spec(),
        &route,
        &ViewHost::spec().views[0],
        long,
        usage::help::Style::PLAIN,
    )
    .expect("the view should have a help page");
    assert!(page.contains("Usage: view-run"), "{page}");
    assert!(!page.contains("view-host run"), "{page}");
    assert!(page.contains("--verbose"), "{page}");
    assert_eq!(page.matches("--verbose").count(), 1, "{page}");
    assert!(!page.contains("--color"), "{page}");
    assert!(!page.contains("HOST-ONLY SURROUNDING HELP"), "{page}");

    assert!(matches!(
        ViewHost::parse_from_argv(&[
            OsStr::new("view-run"),
            OsStr::new("--color"),
            OsStr::new("build")
        ]),
        Err(usage::Error::UnknownFlag { .. })
    ));

    let bad_argv = [OsStr::new("view-run"), OsStr::new("--dry-ru")];
    let error = match ViewHost::parse_from_argv(&[OsStr::new("view-run"), OsStr::new("--dry-ru")]) {
        Err(error) => error,
        Ok(_) => panic!("strict parsing should reject the misspelled view flag"),
    };
    let diagnostic = usage::render_failure_view(
        ViewHost::spec(),
        &bad_argv,
        &error,
        &ViewHost::spec().views[0],
    );
    assert!(diagnostic.contains("Usage: view-run"), "{diagnostic}");
    assert!(!diagnostic.contains("view-host run"), "{diagnostic}");
    assert!(diagnostic.contains("--dry-run"), "{diagnostic}");

    let omitted_argv = [OsStr::new("view-run"), OsStr::new("--root-token")];
    let omitted_error = match ViewHost::parse_from_argv(&omitted_argv) {
        Err(error) => error,
        Ok(_) => panic!("an omitted host global must remain outside the view"),
    };
    let omitted_diagnostic = usage::render_failure_view(
        ViewHost::spec(),
        &omitted_argv,
        &omitted_error,
        &ViewHost::spec().views[0],
    );
    assert_eq!(
        omitted_diagnostic.matches("--root-token").count(),
        1,
        "an omitted host flag must not be suggested by view diagnostics: {omitted_diagnostic}"
    );
    assert!(
        omitted_diagnostic.contains("Usage: view-run"),
        "{omitted_diagnostic}"
    );

    let help_all_argv = [OsStr::new("view-run"), OsStr::new("--help-all")];
    let help_all_cmd = match ViewHost::parse_from_argv(&help_all_argv) {
        Err(usage::Error::HelpAll { cmd }) => cmd,
        _ => panic!("the view should return recursive help"),
    };
    let route = usage::help::route_to_view(
        ViewHost::command(),
        &help_all_argv,
        help_all_cmd,
        &ViewHost::spec().views[0],
    )
    .expect("the view help-all route should be recoverable");
    let all = usage::help::render_all_view_at_styled(
        ViewHost::spec(),
        &route,
        &ViewHost::spec().views[0],
        usage::help::Style::PLAIN,
    )
    .expect("the view should have recursive help");
    assert!(all.contains("Usage: view-run"), "{all}");
    assert!(!all.contains("view-host run"), "{all}");
    assert!(!all.contains("HOST-ONLY SURROUNDING HELP"), "{all}");

    assert!(matches!(
        ViewHost::parse_from_argv(&[OsStr::new("view-run"), OsStr::new("--version")]),
        Err(usage::Error::Version { long: true })
    ));
}

#[cfg(feature = "completions")]
#[test]
fn executable_views_generate_scripts_for_their_binary() {
    let script = ViewHost::completion_script_for("view-run", usage::complete::Shell::Bash)
        .expect("the view is declared");
    assert!(script.contains("view-run"), "{script}");

    let answer = ViewHost::completion_request(&[
        "__complete_word__".into(),
        "--shell".into(),
        "bash".into(),
        "--line".into(),
        "view-run --".into(),
    ])
    .expect("the hidden completion request should be recognized");
    assert!(answer.contains("--dry-run"), "{answer}");
    assert!(answer.contains("--verbose"), "{answer}");
    assert!(answer.contains("--flat-required"), "{answer}");
    assert!(!answer.contains("--color"), "{answer}");
}

#[cfg(feature = "completions")]
#[test]
fn completion_scripts_can_register_a_shell_alias() {
    let script = ViewHost::completion_script_for_alias("vh", usage::complete::Shell::Bash);
    assert!(
        script.contains("complete -F _usage_complete_vh 'vh'"),
        "{script}"
    );
    assert!(
        script.contains("command 'view-host' __complete_word__"),
        "{script}"
    );

    let embedded = ViewHost::spec()
        .view()
        .completion_app()
        .completion_script_for_alias("vh", usage::complete::Shell::Fish);
    assert!(embedded.contains("complete -c 'vh'"), "{embedded}");
    assert!(
        embedded.contains("command 'view-host' __complete_word__"),
        "{embedded}"
    );
}

#[cfg(feature = "completions")]
#[test]
fn an_install_plan_names_this_clis_own_binary() {
    use usage::install::{Env, Platform};

    // A described environment, so this asks where the script goes without a filesystem being
    // involved and without the test having an opinion about the machine it runs on.
    let env = Env::new(Platform::Linux, [("HOME".to_string(), "/home/u".into())]);

    let plan = ViewHost::completion_install_plan(usage::complete::Shell::Zsh, &env).unwrap();
    assert_eq!(plan.path.file_name().unwrap(), "_view-host");

    // An alias installs as its own file, the way it registers as its own name.
    let alias =
        ViewHost::completion_install_plan_for_alias("vh", usage::complete::Shell::Zsh, &env)
            .unwrap();
    assert_eq!(alias.path.file_name().unwrap(), "_vh");
    assert_eq!(alias.path.parent(), plan.path.parent());

    // And the same answer through the embedded view API, which is what a multicall binary uses.
    let embedded = ViewHost::spec()
        .view()
        .completion_app()
        .completion_install_plan(usage::complete::Shell::Fish, &env)
        .unwrap();
    assert_eq!(embedded.path.file_name().unwrap(), "view-host.fish");
}

#[cfg(feature = "completions")]
#[test]
fn an_install_plan_follows_the_runtime_identity_and_not_the_portable_one() {
    use usage::install::{Env, Platform};

    // The same split `runtime_program_identity_is_separate_from_the_portable_spec` pins for the
    // script: what gets installed is named for the binary that will run, not for the spec.
    let env = Env::new(Platform::Linux, [("HOME".to_string(), "/home/u".into())]);
    let plan =
        RuntimeIdentityEx::completion_install_plan(usage::complete::Shell::Zsh, &env).unwrap();
    assert_eq!(plan.path.file_name().unwrap(), "_runtime-ex");
}

#[derive(Cli)]
#[usage(
    bin = "nested-view-host",
    unknown_flags = "error",
    completion,
    view("nested-run", root = "admin run")
)]
struct NestedViewHost {
    #[command(flatten)]
    host: NestedViewHostArgs,
    #[command(subcommand)]
    command: NestedViewTop,
}

#[derive(Args)]
struct NestedViewHostArgs {
    #[arg(long, complete = host_probe_words)]
    probe: Option<String>,
}

#[cfg(feature = "completions")]
fn host_probe_words(
    _partial: &<NestedViewHostArgs as usage_argv::spec::CommandArgs>::Partial,
    _ctx: &usage::complete::CompleteCtx<'_>,
) -> Vec<usage::complete::Candidate<'static>> {
    vec![usage::complete::Candidate::new("host-probe")]
}

#[derive(Subcommands)]
enum NestedViewTop {
    Admin(NestedViewAdmin),
}

#[derive(Args)]
struct NestedViewAdmin {
    #[arg(long, global)]
    intermediate: bool,
    #[arg(long)]
    intermediate_required: String,
    #[arg(long)]
    intermediate_number: u32,
    #[arg(long, default_value = "parent-default")]
    intermediate_default: Option<String>,
    #[arg(long, env = "PATH")]
    intermediate_env: Option<String>,
    #[command(subcommand)]
    command: NestedViewAdminCommand,
}

#[derive(Subcommands)]
enum NestedViewAdminCommand {
    Run(NestedViewRun),
}

#[derive(Args)]
struct NestedViewRun {
    #[arg(long)]
    leaf: bool,
    #[arg(long, complete = nested_view_words)]
    probe: Option<String>,
}

#[cfg(feature = "completions")]
fn nested_view_words(
    _partial: &<NestedViewRun as usage_argv::spec::CommandArgs>::Partial,
    ctx: &usage::complete::CompleteCtx<'_>,
) -> Vec<usage::complete::Candidate<'static>> {
    vec![usage::complete::Candidate::new(format!(
        "{}@{}",
        ctx.words.join("|"),
        ctx.cword
    ))]
}

#[test]
fn nested_views_drop_intermediate_command_globals() {
    let parsed = NestedViewHost::parse_from_argv(&[OsStr::new("nested-run"), OsStr::new("--leaf")])
        .expect("the promoted leaf command should parse");
    let NestedViewTop::Admin(admin) = parsed.command;
    assert!(!admin.intermediate);
    assert_eq!(admin.intermediate_required, "");
    assert_eq!(admin.intermediate_number, 0);
    assert_eq!(admin.intermediate_default, None);
    assert_eq!(admin.intermediate_env, None);
    let NestedViewAdminCommand::Run(run) = admin.command;
    assert!(run.leaf);
    assert_eq!(run.probe, None);

    assert!(matches!(
        NestedViewHost::parse_from_argv(&[OsStr::new("nested-run"), OsStr::new("--intermediate")]),
        Err(usage::Error::UnknownFlag { .. })
    ));

    let portable: usage_parser::Spec = NestedViewHost::to_kdl().parse().unwrap();
    let view = portable.for_view("nested-run").unwrap();
    assert!(!view
        .cmd
        .flags
        .iter()
        .any(|flag| flag.name == "intermediate"));
}

#[cfg(feature = "completions")]
#[test]
fn view_completers_receive_the_users_original_line() {
    let answer = NestedViewHost::completion_request(&[
        "__complete_word__".into(),
        "--shell".into(),
        "bash".into(),
        "--line".into(),
        "nested-run --probe ".into(),
        "--candidates".into(),
        "probe".into(),
    ])
    .expect("the hidden completion request should be recognized");
    assert_eq!(answer, "nested-run|--probe|@2\n");
}

#[derive(Cli)]
#[allow(clippy::duplicated_attributes)]
#[usage(
    bin = "view-group-host",
    unknown_flags = "error",
    group("mode", required),
    view("view-group-run", root = "run", global = "--left", global = "--right"),
    view("view-group-left", root = "run", global = "--left")
)]
struct ViewGroupHost {
    #[arg(long, global, group = "mode")]
    left: bool,
    #[arg(long, global, group = "mode")]
    right: bool,
    #[command(subcommand)]
    command: ViewGroupCommand,
}

#[derive(Subcommands)]
enum ViewGroupCommand {
    Run,
}

struct ViewLeafValue(String);

impl std::str::FromStr for ViewLeafValue {
    type Err = std::convert::Infallible;

    fn from_str(value: &str) -> Result<Self, Self::Err> {
        Ok(Self(value.to_owned()))
    }
}

#[derive(Args)]
struct ViewLeafArgs {
    #[usage(long)]
    value: ViewLeafValue,
}

#[derive(Subcommands)]
#[allow(dead_code)]
enum ViewLeafCommand {
    Run(ViewLeafArgs),
    Other {
        #[usage(long)]
        value: ViewLeafValue,
    },
}

#[derive(Cli)]
#[usage(bin = "view-leaf-host", view("view-leaf", root = "run"))]
struct ViewLeafHost {
    #[usage(subcommand)]
    command: ViewLeafCommand,
}

#[test]
fn a_direct_view_does_not_require_leaf_or_sibling_values_to_default() {
    let parsed = ViewLeafHost::parse_from_argv(&[
        OsStr::new("view-leaf"),
        OsStr::new("--value"),
        OsStr::new("kept"),
    ])
    .expect("the promoted leaf parses its own required value");
    let ViewLeafCommand::Run(args) = parsed.command else {
        panic!("the view should select run")
    };
    assert_eq!(args.value.0, "kept");
}

#[test]
fn executable_views_enforce_required_groups_of_carried_globals() {
    assert!(matches!(
        ViewGroupHost::parse_from_argv(&[OsStr::new("view-group-run")]),
        Err(usage::Error::MissingGroup {
            group: "mode",
            members: &["--left", "--right"],
        })
    ));
    assert!(matches!(
        ViewGroupHost::parse_from_argv(&[OsStr::new("view-group-left")]),
        Err(usage::Error::MissingGroup {
            group: "mode",
            members: &["--left"],
        })
    ));

    let parsed =
        ViewGroupHost::parse_from_argv(&[OsStr::new("view-group-run"), OsStr::new("--left")])
            .expect("a carried member should satisfy its required group");
    assert!(parsed.left);
    assert!(!parsed.right);

    let spec = ViewGroupHost::spec();
    let route = [spec.root.cmd, spec.root.subcommands[0].cmd];
    let page = usage::help::render_view_at_styled(
        spec,
        &route,
        &spec.views[1],
        false,
        usage::help::Style::PLAIN,
    )
    .expect("the singleton projection should render");
    assert!(page.contains("<--left>"), "{page}");
    assert!(!page.contains("--right"), "{page}");
}

#[derive(Subcommands)]
enum Command {
    Show(Show),
    /// Print version information
    Version,
}

#[derive(Args)]
struct RepeatedGlobalChild {
    #[arg(long)]
    cd: Option<String>,
}

#[derive(Subcommands)]
enum RepeatedGlobalCommand {
    Run(RepeatedGlobalChild),
}

#[derive(Cli)]
#[usage(bin = "repeated-global")]
struct RepeatedGlobal {
    #[arg(long, global, default_value = "/default")]
    cd: Option<String>,
    #[command(subcommand)]
    command: RepeatedGlobalCommand,
}

#[test]
fn a_redeclared_global_value_reaches_the_ancestor_and_child() {
    let parsed = RepeatedGlobal::parse_from(&[OsStr::new("run"), OsStr::new("--cd=/tmp")])
        .expect("the child spelling should bind at both levels like clap");
    assert_eq!(parsed.cd.as_deref(), Some("/tmp"));
    let RepeatedGlobalCommand::Run(run) = parsed.command;
    assert_eq!(run.cd.as_deref(), Some("/tmp"));
}

#[derive(Args)]
struct FlattenedGlobal {
    #[arg(long, global)]
    cd: Option<String>,
}

#[derive(Cli)]
#[usage(bin = "flattened-repeated-global")]
struct FlattenedRepeatedGlobal {
    #[command(flatten)]
    global: FlattenedGlobal,
    #[command(subcommand)]
    command: RepeatedGlobalCommand,
}

#[test]
fn a_redeclared_global_value_reaches_a_flattened_ancestor_field() {
    let parsed =
        FlattenedRepeatedGlobal::parse_from(&[OsStr::new("run"), OsStr::new("--cd=/flattened")])
            .expect("the child spelling should mirror through a flattened global group");
    assert_eq!(parsed.global.cd.as_deref(), Some("/flattened"));
    let RepeatedGlobalCommand::Run(run) = parsed.command;
    assert_eq!(run.cd.as_deref(), Some("/flattened"));
}

#[derive(Args)]
struct GlobalAliasChild {
    #[arg(long = "work-dir", alias = "dir")]
    work_dir: Option<String>,
}

#[derive(Subcommands)]
enum GlobalAliasCommand {
    Run(GlobalAliasChild),
}

#[derive(Cli)]
#[usage(bin = "global-alias")]
struct GlobalAliasCli {
    #[arg(long = "directory", alias = "dir", global)]
    directory: Option<String>,
    #[command(subcommand)]
    command: GlobalAliasCommand,
}

#[test]
fn a_shared_alias_does_not_fill_an_unrelated_global() {
    let parsed = GlobalAliasCli::parse_from(&[OsStr::new("run"), OsStr::new("--dir=/tmp")])
        .expect("the nearer child alias should parse");
    assert_eq!(parsed.directory, None);
    let GlobalAliasCommand::Run(run) = parsed.command;
    assert_eq!(run.work_dir.as_deref(), Some("/tmp"));
}

#[derive(Subcommands)]
enum IncompatibleGlobalCommand {
    Run {
        #[arg(long)]
        mode: bool,
        #[arg(long)]
        jobs: String,
    },
}

#[derive(Cli)]
#[usage(bin = "incompatible-global")]
struct IncompatibleGlobalCli {
    #[arg(long, global, default_value = "fast", value_parser = ["fast", "slow"])]
    mode: String,
    #[arg(long, global, default_value = "1")]
    jobs: u32,
    #[command(subcommand)]
    command: IncompatibleGlobalCommand,
}

#[test]
fn incompatible_child_bindings_do_not_fill_ancestor_globals() {
    let parsed = IncompatibleGlobalCli::parse_from(&[
        OsStr::new("run"),
        OsStr::new("--mode"),
        OsStr::new("--jobs"),
        OsStr::new("many"),
    ])
    .expect("the child declarations own their values");
    assert_eq!(parsed.mode, "fast");
    assert_eq!(parsed.jobs, 1);
    let IncompatibleGlobalCommand::Run { mode, jobs } = parsed.command;
    assert!(mode);
    assert_eq!(jobs, "many");
}

#[derive(Cli)]
#[usage(
    bin = "metadata",
    author = env!("CARGO_PKG_AUTHORS"),
    license = env!("CARGO_PKG_LICENSE"),
    repository = env!("CARGO_PKG_REPOSITORY"),
    source_code_link_template = "https://github.com/jdx/usage/blob/main/src/{{path}}.rs"
)]
struct PackageMetadata;

#[test]
fn package_metadata_survives_spec_emission() {
    let kdl = PackageMetadata::to_kdl();
    assert!(kdl.contains("author \"Jeff Dickey @jdx\""), "{kdl}");
    assert!(kdl.contains("license MIT"), "{kdl}");
    assert!(
        kdl.contains("repository \"https://github.com/jdx/usage\""),
        "{kdl}"
    );
    // Not package metadata, but the same shape of claim: one string about the whole spec,
    // which used to have to be appended to the emitted KDL by hand.
    assert!(
        kdl.contains(
            "source_code_link_template \"https://github.com/jdx/usage/blob/main/src/{{path}}.rs\""
        ),
        "{kdl}"
    );

    let spec: usage_parser::Spec = kdl.parse().expect("usage-lib should read derive output");
    assert_eq!(spec.author.as_deref(), Some("Jeff Dickey @jdx"));
    assert_eq!(spec.license.as_deref(), Some("MIT"));
    assert_eq!(
        spec.repository.as_deref(),
        Some("https://github.com/jdx/usage")
    );
    assert_eq!(
        spec.source_code_link_template.as_deref(),
        Some("https://github.com/jdx/usage/blob/main/src/{{path}}.rs")
    );
}

#[test]
fn required_root_subcommand_survives_spec_emission() {
    let kdl = Ex::to_kdl();
    assert!(kdl.contains("subcommand_required #true"), "{kdl}");

    let spec: usage_parser::Spec = kdl.parse().expect("usage-lib should read derive output");
    assert!(spec.cmd.subcommand_required);
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

#[derive(Cli)]
#[command(bin = "missing-positional", allow_missing_positional)]
struct MissingPositional {
    #[arg()]
    optional: Option<String>,
    #[arg()]
    required: String,
}

#[derive(Cli)]
#[command(bin = "hidden-help")]
struct HiddenHelp {
    #[arg(
        long,
        default = "fast",
        hide_default_value,
        hide_env,
        hide_env_values,
        hide_possible_values,
        hide_short_help,
        hide_long_help
    )]
    mode: String,
}

#[derive(Cli)]
#[usage(bin = "optional-value")]
struct OptionalValue {
    #[usage(long)]
    bump: Option<Option<u32>>,
}

#[derive(Debug, Cli)]
#[usage(bin = "help-optional-value")]
struct HelpOptionalValue {
    #[usage(long, value_optional)]
    bump: Option<u32>,
}

#[derive(Debug, Cli)]
#[usage(bin = "explicit-bool", args_override_self = false)]
struct ExplicitBool {
    #[usage(long, negate = "no-color", bool_value)]
    color: bool,
    #[usage(arg)]
    rest: Option<String>,
}

#[derive(Debug, Cli)]
#[usage(bin = "sized-help", term_width = 36, max_term_width = 20)]
#[allow(dead_code)]
struct SizedHelp {
    /// A description long enough to wrap at the command's declared help width.
    #[usage(long)]
    output: Option<String>,
}

#[derive(Debug, Cli)]
#[usage(bin = "next-help", next_line_help)]
#[allow(dead_code)]
struct NextLineHelp {
    /// Config file.
    #[usage(long)]
    config: Option<String>,
    #[arg(long, env = "NEXT_HELP_MODE", default = "fast")]
    mode: String,
}

#[derive(Debug, Cli)]
#[usage(bin = "flat-help", flatten_help)]
#[allow(dead_code)]
struct FlatHelp {
    #[usage(subcommand)]
    command: FlatCommand,
}

#[derive(Debug, Subcommands)]
#[allow(dead_code)]
enum FlatCommand {
    /// Run a task.
    Run(FlatRun),
}

#[derive(Debug, Args)]
#[allow(dead_code)]
struct FlatRun {
    /// Task name.
    task: String,
}

#[derive(Cli)]
#[command(
    bin = "presented",
    subcommand_help_heading = "Actions",
    subcommand_value_name = "ACTION"
)]
#[allow(dead_code)]
struct PresentedSubcommands {
    #[command(subcommand)]
    command: Option<ArgumentConflictCommand>,
}

#[derive(Args)]
#[command(
    visible_alias = "go",
    alias = "secret-run",
    hide,
    after_long_help = "More details."
)]
struct StructCommandMetadata;

#[derive(Subcommands)]
enum StructMetadataCommands {
    Run(StructCommandMetadata),
}

#[derive(Cli)]
#[command(bin = "struct-metadata", about)]
#[allow(dead_code)]
struct StructMetadataCli {
    #[command(subcommand)]
    command: StructMetadataCommands,
}

#[derive(Args)]
#[group(required = true, multiple = false)]
#[allow(dead_code)]
struct ClapImplicitGroup {
    #[arg(long)]
    left: bool,
    #[arg(long)]
    right: bool,
}

#[derive(Cli)]
#[command(bin = "implicit-group")]
#[allow(dead_code)]
struct ClapImplicitGroupCli {
    #[arg(flatten)]
    choice: ClapImplicitGroup,
}

#[derive(Args)]
#[group(required = true)]
#[allow(dead_code)]
struct SingleClapImplicitGroup {
    #[arg(long)]
    only: bool,
}

#[derive(Args)]
#[group(id = "all", required = true)]
#[group(multiple = false)]
#[allow(dead_code)]
struct SplitClapImplicitGroup {
    #[arg(long, group = "explicit")]
    left: bool,
    #[arg(long)]
    middle: bool,
    #[arg(long, group = "explicit")]
    right: bool,
}

#[derive(Cli)]
#[command(bin = "split-implicit-group")]
#[allow(dead_code)]
struct SplitClapImplicitGroupCli {
    #[arg(flatten)]
    choice: SplitClapImplicitGroup,
}

#[derive(Cli)]
#[command(bin = "single-implicit-group")]
#[allow(dead_code)]
struct SingleClapImplicitGroupCli {
    #[arg(flatten)]
    choice: SingleClapImplicitGroup,
}

#[derive(Args)]
#[group(id = "renamed")]
#[allow(dead_code)]
struct NoopClapImplicitGroup {
    #[arg(long)]
    left: bool,
    #[arg(long)]
    right: bool,
}

#[derive(Cli)]
#[command(bin = "noop-implicit-group")]
#[allow(dead_code)]
struct NoopClapImplicitGroupCli {
    #[arg(flatten)]
    choice: NoopClapImplicitGroup,
}

#[derive(Cli)]
#[command(bin = "ordered")]
#[allow(dead_code)]
struct OrderedHelp {
    /// Shown second.
    #[arg(long, global, display_order = 20)]
    second: bool,
    /// Shown first.
    #[arg(long, global, display_order = 10)]
    first: bool,
    #[command(subcommand)]
    command: Option<OrderedCommand>,
}

#[derive(Subcommands)]
#[allow(dead_code)]
enum OrderedCommand {
    /// Shown second.
    #[command(display_order = 20)]
    Second,
    /// Shown first.
    #[command(display_order = 10)]
    First,
}

#[derive(Cli)]
#[command(bin = "grouped")]
#[allow(dead_code)]
struct GroupedHelp {
    #[command(subcommand)]
    command: Option<GroupedCommand>,
}

#[derive(Subcommands)]
#[allow(dead_code)]
enum GroupedCommand {
    /// Run the application.
    #[command(help_heading = "Core commands")]
    Run,
    /// Remove old state.
    #[command(help_heading = "Maintenance")]
    Clean,
    /// Show the current status.
    #[command(help_heading = "Commands")]
    Status,
}

#[derive(Cli)]
#[command(
    bin = "custom-builtins",
    version,
    disable_help_flag,
    disable_help_subcommand,
    disable_version_flag
)]
#[allow(dead_code)]
struct CustomBuiltins {
    /// Show the concise help page.
    #[arg(long = "assist", action = usage::ArgAction::HelpShort)]
    assist: bool,
    /// Show help selected by spelling.
    #[arg(short = '?', long = "help-all", action = usage::ArgAction::Help)]
    help_all: bool,
    /// Show the full help page.
    #[arg(long = "manual", action = usage::ArgAction::HelpLong)]
    manual: bool,
    /// Show version information.
    #[arg(long = "release", action = usage::ArgAction::Version)]
    release: bool,
    #[command(subcommand)]
    command: Option<ArgumentConflictCommand>,
}

#[test]
fn custom_builtin_actions_replace_synthetic_entries() {
    let kdl = CustomBuiltins::to_kdl();
    assert!(kdl.contains("disable_help_flag #true"), "{kdl}");
    assert!(kdl.contains("disable_help_subcommand #true"), "{kdl}");
    assert!(kdl.contains("disable_version_flag #true"), "{kdl}");
    assert!(kdl.contains("action=help_short"), "{kdl}");
    assert!(kdl.contains("action=help_long"), "{kdl}");
    assert!(kdl.contains("action=version"), "{kdl}");
    let portable: usage_parser::Spec = kdl.parse().expect("usage-lib should read derive output");
    assert!(portable.cmd.disable_help_flag);
    assert!(portable.cmd.disable_help_subcommand);
    assert!(portable.cmd.disable_version_flag);
    assert_eq!(
        portable.cmd.flags[0].action,
        usage_parser::SpecFlagAction::HelpShort
    );

    let assist: &[&OsStr] = &[OsStr::new("custom-builtins"), OsStr::new("--assist")];
    let release: &[&OsStr] = &[OsStr::new("custom-builtins"), OsStr::new("--release")];
    let short_help: &[&OsStr] = &[OsStr::new("custom-builtins"), OsStr::new("-?")];
    let long_help: &[&OsStr] = &[OsStr::new("custom-builtins"), OsStr::new("--help-all")];
    let manual: &[&OsStr] = &[OsStr::new("custom-builtins"), OsStr::new("--manual")];
    let help: &[&OsStr] = &[OsStr::new("custom-builtins"), OsStr::new("--help")];
    let version: &[&OsStr] = &[OsStr::new("custom-builtins"), OsStr::new("--version")];
    assert!(matches!(
        CustomBuiltins::try_parse_from(assist),
        Err(usage::Error::Help { long: false, .. })
    ));
    assert!(matches!(
        CustomBuiltins::try_parse_from(release),
        Err(usage::Error::Version { .. })
    ));
    assert!(matches!(
        CustomBuiltins::try_parse_from(short_help),
        Err(usage::Error::Help { long: false, .. })
    ));
    assert!(matches!(
        CustomBuiltins::try_parse_from(long_help),
        Err(usage::Error::Help { long: true, .. })
    ));
    assert!(matches!(
        CustomBuiltins::try_parse_from(manual),
        Err(usage::Error::Help { long: true, .. })
    ));
    assert!(CustomBuiltins::try_parse_from(help).is_err());
    assert!(CustomBuiltins::try_parse_from(version).is_err());
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

#[deny(dead_code)]
#[derive(Cli)]
#[command(bin = "parse-only-field")]
struct ParseOnlyField {
    /// Accepted for compatibility even though the application intentionally ignores it.
    #[arg(long)]
    compatibility: bool,
}

#[derive(Cli)]
#[command(bin = "clap-override-id")]
struct ClapOverrideId {
    #[arg(long, overrides_with = "installed_tool")]
    reset: bool,
    #[arg(id = "installed_tool", long = "installed")]
    tool: Option<String>,
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

#[derive(Debug, PartialEq, Eq)]
struct NonDefaultValue(String);

impl std::str::FromStr for NonDefaultValue {
    type Err = std::convert::Infallible;

    fn from_str(value: &str) -> Result<Self, Self::Err> {
        Ok(Self(value.to_owned()))
    }
}

#[derive(Args)]
struct NonDefaultArgs {
    #[usage(long)]
    value: NonDefaultValue,
}

#[derive(Subcommands)]
enum NonDefaultCommand {
    Run(NonDefaultArgs),
}

#[derive(Cli)]
#[usage(bin = "non-default")]
struct NonDefaultCli {
    #[usage(subcommand)]
    command: NonDefaultCommand,
}

#[derive(Cli)]
#[usage(bin = "completion-dedup")]
struct CompletionDedup {
    #[usage(long, value_name = "PATH", value_hint = usage::ValueHint::FilePath)]
    input: Option<PathBuf>,
    #[usage(long, value_name = "PATH", value_hint = usage::ValueHint::FilePath)]
    output: Option<PathBuf>,
}

#[derive(Cli)]
#[usage(bin = "value-hints")]
#[allow(dead_code)]
struct ValueHints {
    #[usage(long, value_hint = usage::ValueHint::Unknown)]
    unknown: Option<String>,
    #[usage(long, value_hint = usage::ValueHint::Other)]
    other: Option<String>,
    #[usage(long, value_hint = usage::ValueHint::Username)]
    username: Option<String>,
    #[usage(long, value_hint = usage::ValueHint::Hostname)]
    hostname: Option<String>,
    #[usage(long, value_hint = usage::ValueHint::Url)]
    url: Option<String>,
    #[usage(long, value_hint = usage::ValueHint::EmailAddress)]
    email: Option<String>,
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

fn computed_long_version() -> &'static str {
    "1.2.3+runtime\ncommit abc123"
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
    long_version = computed_long_version(),
    long_version_spec = "1.2.3\ncommit portable",
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
fn the_full_value_hint_vocabulary_reaches_portable_completion_types() {
    let kdl = ValueHints::to_kdl();
    for (name, type_) in [
        ("unknown", "unknown"),
        ("other", "none"),
        ("username", "username"),
        ("hostname", "hostname"),
        ("url", "url"),
        ("email", "email"),
    ] {
        assert!(
            kdl.contains(&format!("complete {name} type={type_}")),
            "{kdl}"
        );
    }
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
    let cli = ChoiceEx::parse_from(&[OsStr::new("--shell"), OsStr::new("Z-SHELL")])
        .expect("every declared alias should bind without a separate FromStr");
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
fn parse_only_fields_do_not_trigger_dead_code() {
    ParseOnlyField::parse_from(&[OsStr::new("--compatibility")])
        .expect("the compatibility flag should still parse");
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
fn clap_override_ids_emit_portable_flag_selectors() {
    let parsed = ClapOverrideId::parse_from(&[
        OsStr::new("--installed"),
        OsStr::new("tool"),
        OsStr::new("--reset"),
    ])
    .expect("the later override should displace the clap-id target");
    assert!(parsed.reset);
    assert_eq!(parsed.tool, None);

    let kdl = ClapOverrideId::to_kdl();
    assert!(kdl.contains("overrides=--installed"), "{kdl}");
    assert!(!kdl.contains("overrides=installed_tool"), "{kdl}");
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
fn typed_parser_can_reserve_a_word_for_a_required_positional() {
    let parsed = MissingPositional::parse_from(&[OsStr::new("required")])
        .expect("the last word belongs to the later required positional");
    assert_eq!(parsed.optional, None);
    assert_eq!(parsed.required, "required");

    let parsed = MissingPositional::parse_from(&[OsStr::new("optional"), OsStr::new("required")])
        .expect("an extra word still fills the optional positional first");
    assert_eq!(parsed.optional.as_deref(), Some("optional"));
    assert_eq!(parsed.required, "required");
    assert!(MissingPositional::to_kdl().contains("allow_missing_positional #true"));
}

#[test]
fn typed_granular_help_hides_reach_the_portable_spec() {
    let kdl = HiddenHelp::to_kdl();
    for property in [
        "hide_default_value",
        "hide_env",
        "hide_env_values",
        "hide_possible_values",
        "hide_short_help",
        "hide_long_help",
    ] {
        assert!(kdl.contains(&format!("{property}=#true")), "{kdl}");
    }
    assert_eq!(HiddenHelp::parse_from(&[]).unwrap().mode, "fast");
}

#[test]
fn nested_option_distinguishes_absent_bare_and_valued_flags() {
    assert_eq!(OptionalValue::parse_from(&[]).unwrap().bump, None);
    assert_eq!(
        OptionalValue::parse_from(&[OsStr::new("--bump")])
            .unwrap()
            .bump,
        Some(None)
    );
    assert_eq!(
        OptionalValue::parse_from(&[OsStr::new("--bump=5")])
            .unwrap()
            .bump,
        Some(Some(5))
    );
    let kdl = OptionalValue::to_kdl();
    assert!(kdl.contains("flag --bump"), "{kdl}");
    assert!(kdl.contains("[BUMP]"), "{kdl}");
}

#[test]
fn help_only_optional_values_still_require_a_typed_value() {
    assert!(HelpOptionalValue::parse_from(&[OsStr::new("--bump")]).is_err());
    assert_eq!(
        HelpOptionalValue::parse_from(&[OsStr::new("--bump=5")])
            .unwrap()
            .bump,
        Some(5)
    );
    let kdl = HelpOptionalValue::to_kdl();
    assert!(kdl.contains("[BUMP]"), "{kdl}");
}

#[test]
fn boolean_flags_can_opt_into_attached_values() {
    for (token, expected) in [
        ("--color", true),
        ("--color=true", true),
        ("--color=false", false),
        ("--no-color", false),
        ("--no-color=false", true),
    ] {
        let parsed = ExplicitBool::parse_from(&[OsStr::new(token)]).unwrap();
        assert_eq!(parsed.color, expected, "{token}");
    }
    let parsed =
        ExplicitBool::parse_from(&[OsStr::new("--color=false"), OsStr::new("positional")]).unwrap();
    assert_eq!(parsed.rest.as_deref(), Some("positional"));
    assert!(ExplicitBool::parse_from(&[OsStr::new("--color=maybe")]).is_err());
    assert!(
        ExplicitBool::parse_from(&[OsStr::new("--color=false"), OsStr::new("--color=true"),])
            .is_err()
    );
    assert!(
        ExplicitBool::parse_from(&[OsStr::new("--color=false"), OsStr::new("--no-color=false"),])
            .unwrap()
            .color
    );

    let kdl = ExplicitBool::to_kdl();
    assert!(kdl.contains("bool_value=#true"), "{kdl}");
}

#[test]
fn typed_subcommand_presentation_reaches_help_and_the_spec() {
    let kdl = PresentedSubcommands::to_kdl();
    assert!(kdl.contains("subcommand_help_heading Actions"), "{kdl}");
    assert!(kdl.contains("subcommand_value_name ACTION"), "{kdl}");
    let spec = PresentedSubcommands::spec();
    let page = usage::argv::help::short_help(spec, &["presented"], &[spec.root]);
    assert!(page.contains("<ACTION>"), "{page}");
    assert!(page.contains("Actions:"), "{page}");
}

#[test]
fn clap_command_metadata_can_stay_on_the_args_struct() {
    let spec = StructMetadataCli::spec();
    let meta = spec.root.subcommands[0];
    assert_eq!(meta.cmd.aliases, ["go", "secret-run"]);
    assert_eq!(meta.hidden_aliases, ["secret-run"]);
    assert!(meta.hide);
    assert_eq!(meta.after_long_help, Some("More details."));

    assert!(StructMetadataCli::parse_from(&[OsStr::new("go")]).is_ok());
    assert!(StructMetadataCli::parse_from(&[OsStr::new("secret-run")]).is_ok());
}

#[test]
fn clap_implicit_groups_apply_to_the_args_struct_fields() {
    assert!(ClapImplicitGroupCli::parse_from(&[]).is_err());
    assert!(ClapImplicitGroupCli::parse_from(&[OsStr::new("--left")]).is_ok());
    assert!(
        ClapImplicitGroupCli::parse_from(&[OsStr::new("--left"), OsStr::new("--right")]).is_err()
    );
    let kdl = ClapImplicitGroupCli::to_kdl();
    assert!(
        kdl.contains("group ClapImplicitGroup --left --right required=#true"),
        "{kdl}"
    );
    assert!(matches!(
        SingleClapImplicitGroupCli::parse_from(&[]),
        Err(usage::Error::MissingRequired { name: "only" })
    ));
    assert!(SingleClapImplicitGroupCli::parse_from(&[OsStr::new("--only")]).is_ok());
    let single_kdl = SingleClapImplicitGroupCli::to_kdl();
    assert!(
        single_kdl.contains("flag --only required=#true"),
        "{single_kdl}"
    );
    assert!(
        !single_kdl.contains("group SingleClapImplicitGroup"),
        "{single_kdl}"
    );

    let kdl = SplitClapImplicitGroupCli::to_kdl();
    assert!(
        kdl.contains("group all --left --middle --right required=#true"),
        "{kdl}"
    );
    assert!(kdl.contains("group explicit --left --right"), "{kdl}");
    assert!(SplitClapImplicitGroupCli::parse_from(&[]).is_err());
    assert!(SplitClapImplicitGroupCli::parse_from(&[OsStr::new("--middle")]).is_ok());
    assert!(
        SplitClapImplicitGroupCli::parse_from(&[OsStr::new("--left"), OsStr::new("--middle")])
            .is_err()
    );

    assert!(
        NoopClapImplicitGroupCli::parse_from(&[OsStr::new("--left"), OsStr::new("--right")])
            .is_ok()
    );
    let noop_kdl = NoopClapImplicitGroupCli::to_kdl();
    assert!(!noop_kdl.contains("group renamed"), "{noop_kdl}");
}

#[test]
fn explicit_display_order_reaches_help_and_the_portable_spec() {
    let spec = OrderedHelp::spec();
    let page = usage::argv::help::short_help(spec, &["ordered"], &[spec.root]);
    let flags = page.split_once("\nFlags:\n").unwrap().1;
    assert!(
        flags.find("--first").unwrap() < flags.find("--second").unwrap(),
        "{page}"
    );
    assert!(
        page.find("first  Shown first.").unwrap() < page.find("second  Shown second.").unwrap(),
        "{page}"
    );
    let child = &spec.root.subcommands[0];
    let child_page =
        usage::argv::help::short_help(spec, &["ordered", child.cmd.name], &[spec.root, child]);
    let globals = child_page.split_once("\nGlobal flags:\n").unwrap().1;
    assert!(
        globals.find("--first").unwrap() < globals.find("--second").unwrap(),
        "{child_page}"
    );

    let kdl = OrderedHelp::to_kdl();
    assert!(kdl.contains("display_order=10"), "{kdl}");
    assert!(kdl.contains("display_order=20"), "{kdl}");
    let portable: usage_parser::Spec = kdl.parse().unwrap();
    assert_eq!(portable.cmd.flags[0].display_order, Some(20));
    assert_eq!(portable.cmd.flags[1].display_order, Some(10));
    assert_eq!(portable.cmd.subcommands[0].display_order, Some(20));
    assert_eq!(portable.cmd.subcommands[1].display_order, Some(10));
}

#[test]
fn subcommand_help_headings_reach_help_and_the_portable_spec() {
    let spec = GroupedHelp::spec();
    for page in [
        usage::argv::help::short_help(spec, &["grouped"], &[spec.root]),
        usage::argv::help::long_help(spec, &["grouped"], &[spec.root]),
    ] {
        let commands = page.find("\nCommands:\n").expect("default command section");
        assert_eq!(page.matches("\nCommands:\n").count(), 1, "{page}");
        let core = page.find("\nCore commands:\n").expect("core section");
        let maintenance = page.find("\nMaintenance:\n").expect("maintenance section");
        assert!(commands < core && commands < maintenance, "{page}");
        let default_end = core.min(maintenance);
        assert!(page[commands..default_end].contains("status"), "{page}");
        assert!(page[commands..default_end].contains("help"), "{page}");
        let core_end = page[core + 1..]
            .find("\n\n")
            .map_or(page.len(), |offset| core + 1 + offset);
        assert!(page[core..core_end].contains("run"), "{page}");
        let maintenance_end = page[maintenance + 1..]
            .find("\n\n")
            .map_or(page.len(), |offset| maintenance + 1 + offset);
        assert!(
            page[maintenance..maintenance_end].contains("clean"),
            "{page}"
        );
    }

    let kdl = GroupedHelp::to_kdl();
    assert!(kdl.contains("help_heading=\"Core commands\""), "{kdl}");
    assert!(kdl.contains("help_heading=Maintenance"), "{kdl}");
    let portable: usage_parser::Spec = kdl.parse().unwrap();
    assert_eq!(
        portable.cmd.subcommands[0].help_heading.as_deref(),
        Some("Core commands")
    );
    assert_eq!(
        portable.cmd.subcommands[1].help_heading.as_deref(),
        Some("Maintenance")
    );
    assert_eq!(
        portable.cmd.subcommands[2].help_heading.as_deref(),
        Some("Commands")
    );
}

#[test]
fn typed_help_width_reaches_help_and_the_portable_spec() {
    let spec = SizedHelp::spec();
    assert_eq!(spec.root.term_width, Some(36));
    assert_eq!(spec.root.max_term_width, Some(20));
    let page = usage::argv::help::long_help(spec, &["sized-help"], &[spec.root]);
    assert!(
        page.contains("                         description\n"),
        "fixed width should wrap and override the lower maximum: {page}"
    );

    let kdl = SizedHelp::to_kdl();
    assert!(kdl.contains("term_width 36"), "{kdl}");
    assert!(kdl.contains("max_term_width 20"), "{kdl}");
    let portable: usage_parser::Spec = kdl.parse().unwrap();
    assert_eq!(portable.cmd.term_width, Some(36));
    assert_eq!(portable.cmd.max_term_width, Some(20));
}

#[test]
fn typed_next_line_help_reaches_help_and_the_portable_spec() {
    let spec = NextLineHelp::spec();
    assert!(spec.root.next_line_help);
    let page = usage::argv::help::short_help(spec, &["next-help"], &[spec.root]);
    assert!(
        page.contains("--config <CONFIG>\n    Config file."),
        "{page}"
    );
    assert!(
        page.contains("--mode <MODE>\n    [env: NEXT_HELP_MODE]\n    (default: fast)"),
        "{page}"
    );

    let kdl = NextLineHelp::to_kdl();
    assert!(kdl.contains("next_line_help #true"), "{kdl}");
    let portable: usage_parser::Spec = kdl.parse().unwrap();
    assert!(portable.cmd.next_line_help);
}

#[test]
fn typed_flatten_help_reaches_help_and_the_portable_spec() {
    let spec = FlatHelp::spec();
    assert!(spec.root.flatten_help);
    let page = usage::argv::help::short_help(spec, &["flat-help"], &[spec.root]);
    assert!(page.contains("Usage: flat-help run"), "{page}");
    assert!(!page.contains("\nCommands:\n"), "{page}");
    assert!(page.contains("\nrun:\nRun a task."), "{page}");
    assert!(page.contains("<TASK>"), "{page}");

    let kdl = FlatHelp::to_kdl();
    assert!(kdl.contains("flatten_help #true"), "{kdl}");
    let portable: usage_parser::Spec = kdl.parse().unwrap();
    assert!(portable.cmd.flatten_help);
}

#[test]
fn args_without_views_do_not_require_value_defaults() {
    let parsed =
        NonDefaultCli::parse_from(&[OsStr::new("run"), OsStr::new("--value"), OsStr::new("kept")])
            .unwrap();
    let NonDefaultCommand::Run(args) = parsed.command;
    assert_eq!(args.value, NonDefaultValue("kept".to_owned()));
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
    assert!(
        kdl.contains("long_version \"1.2.3\\ncommit portable\""),
        "{kdl}"
    );
    assert!(kdl.contains("default=\"7\""), "{kdl}");
    assert!(kdl.contains("default=\"0\""), "{kdl}");
    assert!(kdl.contains(DYNAMIC_ABOUT), "{kdl}");
    assert!(kdl.contains(DYNAMIC_AFTER_HELP), "{kdl}");
    let spec: usage_parser::Spec = kdl.parse().expect("the static values should be portable");
    assert_eq!(spec.version.as_deref(), Some("1.2.3"));
    assert_eq!(spec.long_version.as_deref(), Some("1.2.3\ncommit portable"));
    assert!(matches!(
        DynamicEx::parse_from(&[OsStr::new("-V")]),
        Err(usage::Error::Version { long: false })
    ));
    assert!(matches!(
        DynamicEx::parse_from(&[OsStr::new("--version")]),
        Err(usage::Error::Version { long: true })
    ));
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

/// A CLI whose declarations have all four shapes of deprecation on them.
///
/// Version `2.0.0`, so the milestones below are on both sides of where this CLI actually is.
#[derive(Cli)]
#[usage(bin = "deprecated-ex", version = "2.0.0")]
struct DeprecatedEx {
    /// Where to write the result
    #[usage(long, deprecated = "use --out", deprecated_remove_at = "3.0.0")]
    output: Option<String>,
    #[usage(long)]
    out: Option<String>,
    /// Deprecated in a release this CLI has not reached
    #[usage(long, deprecated = "use --out", deprecated_warn_at = "9.0.0")]
    outfile: Option<String>,
    /// A milestone that has passed, with no message beside it
    #[usage(long, deprecated_warn_at = "1.0.0")]
    legacy: bool,
    // Declared here so the alias is part of the shape these cases parse, but exercised in
    // `usage-conformance`'s `deprecation.rs` rather than here: a variable is process-wide, and
    // setting one in this binary would race every other case in it that parses this CLI.
    #[usage(
        long,
        env = "DEPRECATED_EX_TOKEN",
        deprecated_env = "DEPRECATED_EX_OLD_TOKEN"
    )]
    token: Option<String>,
    #[usage(long, default = "quiet")]
    mode: Option<String>,
    #[usage(subcommand)]
    command: Option<DeprecatedExCommand>,
}

#[derive(Subcommands)]
// Selected, not read: what these cases check is what a selection reports.
#[allow(dead_code)]
enum DeprecatedExCommand {
    #[usage(deprecated = "use build")]
    Compile(DeprecatedExCompile),
    Build(DeprecatedExBuild),
}

#[derive(Args)]
struct DeprecatedExCompile {
    #[usage(long, deprecated = "no longer read")]
    incremental: bool,
}

#[derive(Args)]
struct DeprecatedExBuild {
    #[usage(long)]
    release: bool,
}

/// The kinds and names of what a command line reported, in order.
fn deprecations_of(words: &[&str]) -> Vec<(usage::warn::WarningKind, String)> {
    let owned: Vec<&OsStr> = words.iter().map(OsStr::new).collect();
    let mut warnings = Vec::new();
    DeprecatedEx::parse_from_with_warnings(&owned, &mut warnings).expect("a valid command line");
    warnings
        .iter()
        .map(|warning| (warning.kind, warning.name.to_string()))
        .collect()
}

#[test]
fn a_deprecated_flag_that_was_typed_is_reported() {
    assert_eq!(
        deprecations_of(&["--output", "a.txt"]),
        [(usage::warn::WarningKind::DeprecatedFlag, "--output".into())],
    );
}

#[test]
fn a_flag_nobody_typed_reports_nothing() {
    assert!(deprecations_of(&["--out", "a.txt"]).is_empty());
    // A default fills the field without anybody having asked for it, so it is not a use.
    assert!(deprecations_of(&[]).is_empty());
}

#[test]
fn a_milestone_the_cli_has_not_reached_stays_quiet() {
    assert!(deprecations_of(&["--outfile", "a.txt"]).is_empty());
    assert_eq!(
        deprecations_of(&["--legacy"]),
        [(usage::warn::WarningKind::DeprecatedFlag, "--legacy".into())],
    );
}

#[test]
fn a_deprecated_command_reports_itself_and_its_own_flags() {
    assert_eq!(
        deprecations_of(&["compile", "--incremental"]),
        [
            (
                usage::warn::WarningKind::DeprecatedCommand,
                "compile".into()
            ),
            (
                usage::warn::WarningKind::DeprecatedFlag,
                "--incremental".into()
            ),
        ],
    );
    assert!(deprecations_of(&["build", "--release"]).is_empty());
}

#[test]
fn the_entry_points_that_were_not_asked_still_parse() {
    let words = [OsStr::new("--output"), OsStr::new("a.txt")];
    assert!(DeprecatedEx::parse_from(&words).is_ok());
    let with_argv0 = [
        OsStr::new("deprecated-ex"),
        OsStr::new("--output"),
        OsStr::new("a.txt"),
    ];
    assert!(DeprecatedEx::try_parse_from(&with_argv0).is_ok());
    let mut warnings = Vec::new();
    DeprecatedEx::try_parse_from_with_warnings(&with_argv0, &mut warnings)
        .expect("a valid command line");
    assert_eq!(warnings.len(), 1, "{warnings:?}");
}

#[test]
fn what_a_user_reads_says_what_to_do_about_it() {
    let mut warnings = Vec::new();
    DeprecatedEx::parse_from_with_warnings(
        &[OsStr::new("--output"), OsStr::new("a.txt")],
        &mut warnings,
    )
    .expect("a valid command line");
    assert_eq!(
        usage::warn::render_warnings(&warnings),
        "warning: --output is deprecated, removed at 3.0.0: use --out\n",
    );
}

/// The facade is the one package an application depends on, so the dispatch traits reach it
/// through the same `usage::` path as the derives — a CLI that hands its commands to their
/// implementations does not learn the crate split to do it.
#[derive(Args)]
struct DispatchLeaf {
    #[usage(long)]
    force: bool,
}

#[derive(Subcommands)]
#[usage(run, run_with)]
enum DispatchCommand {
    /// Do the one thing
    Go(DispatchLeaf),
}

#[derive(Cli)]
#[usage(bin = "dispatch-ex")]
struct DispatchEx {
    #[usage(subcommand)]
    command: DispatchCommand,
}

impl usage::Run for DispatchLeaf {
    type Output = bool;
    fn run(self) -> Self::Output {
        self.force
    }
}

impl usage::RunWith<&mut usize> for DispatchLeaf {
    type Output = bool;
    fn run_with(self, calls: &mut usize) -> Self::Output {
        *calls += 1;
        self.force
    }
}

#[test]
fn the_facade_exposes_the_dispatch_traits() {
    use usage::{Run, RunWith};

    let ex = DispatchEx::parse_from(&[OsStr::new("go"), OsStr::new("--force")])
        .expect("valid command line");
    assert!(ex.command.run());

    let mut calls = 0;
    let ex = DispatchEx::parse_from(&[OsStr::new("go")]).expect("valid command line");
    assert!(!ex.command.run_with(&mut calls));
    assert_eq!(calls, 1);
}

/// A CLI declaring nothing about the endpoint, which is how most of them will look.
#[derive(Cli)]
#[usage(bin = "endpoint-ex", version = "1.0.0")]
struct EndpointEx {
    #[usage(long)]
    force: bool,
    #[usage(subcommand)]
    command: EndpointCommand,
}

#[derive(Subcommands)]
enum EndpointCommand {
    /// Build the thing.
    Build {
        #[usage(long)]
        release: bool,
    },
}

#[test]
fn a_spec_request_is_answered_from_the_binary_s_own_tables() {
    let answer = EndpointEx::spec_request(&[OsStr::new(usage::SPEC_REQUEST)])
        .expect("the endpoint is on unless a CLI says otherwise");
    assert_eq!(answer, EndpointEx::to_kdl());

    // What a tool receives has to be a spec, not merely a string: this is the whole contract
    // of the endpoint, so it is checked through usage-lib rather than by grepping.
    let spec: usage_parser::Spec = answer.parse().expect("the endpoint should emit a spec");
    assert_eq!(spec.bin, "endpoint-ex");
    assert!(spec.cmd.subcommands.contains_key("build"), "{spec:?}");
}

#[test]
fn an_ordinary_command_line_is_not_a_spec_request() {
    assert!(EndpointEx::spec_request(&[]).is_none());
    assert!(EndpointEx::spec_request(&[OsStr::new("--force")]).is_none());

    // And what is not a request still parses as it always did, which is the half worth
    // checking on a CLI that gained an entry point it never declared.
    let argv = ["--force", "build", "--release"].map(OsStr::new);
    let cli = EndpointEx::parse_from(&argv).expect("valid command line");
    assert!(cli.force);
    let EndpointCommand::Build { release } = cli.command;
    assert!(release);

    // Not the first word, so it is an ordinary value — which is what keeps the endpoint from
    // eating an argument of a CLI whose arguments are arbitrary text.
    let later = ["build", usage::SPEC_REQUEST].map(OsStr::new);
    assert!(EndpointEx::spec_request(&later).is_none());
}

/// A CLI that declares the spelling itself. Contrived, and the point: a declaration wins.
#[derive(Cli)]
#[usage(bin = "declares-ex")]
struct DeclaresEndpointEx {
    #[usage(subcommand)]
    command: DeclaresEndpointCommand,
}

#[derive(Subcommands)]
enum DeclaresEndpointCommand {
    /// Whatever this CLI meant by it.
    #[usage(name = "__usage_spec__")]
    UsageSpec {
        #[usage(long)]
        mine: bool,
    },
}

#[test]
fn a_declared_command_of_that_name_outranks_the_endpoint() {
    assert!(DeclaresEndpointEx::spec_request(&[OsStr::new(usage::SPEC_REQUEST)]).is_none());
    // And it still parses as the command it declared, which is the behavior being protected.
    let cli =
        DeclaresEndpointEx::parse_from(&[OsStr::new(usage::SPEC_REQUEST), OsStr::new("--mine")])
            .expect("the declared command should still parse");
    let DeclaresEndpointCommand::UsageSpec { mine } = cli.command;
    assert!(mine);
}

/// A CLI that does not want to carry the KDL writer at all.
#[derive(Cli)]
#[usage(bin = "opted-out-ex", spec_endpoint = false)]
struct OptedOutEx {
    #[usage(long)]
    force: bool,
}

#[test]
fn opting_out_leaves_the_spec_itself_alone() {
    // No `spec_request` is generated — absence is a compile-time property, so what is checked
    // here is that opting out costs nothing else: the spec and the parse are unchanged.
    let kdl = OptedOutEx::to_kdl();
    assert!(kdl.contains("name opted-out-ex"), "{kdl}");
    let _: usage_parser::Spec = kdl.parse().expect("opting out should not change the spec");
    let cli = OptedOutEx::parse_from(&[OsStr::new("--force")]).expect("valid command line");
    assert!(cli.force);
}

/// The nodes no attribute carries yet, appended from a file beside this test.
#[derive(Cli)]
#[usage(bin = "extra-ex", spec_extra = "tests/spec-extra.usage.kdl")]
struct SpecExtraEx {
    #[usage(long)]
    force: bool,
}

#[test]
fn spec_extra_joins_the_document_the_endpoint_hands_over() {
    let kdl = SpecExtraEx::to_kdl();
    assert!(kdl.contains("name extra-ex"), "{kdl}");
    assert!(kdl.contains("example \"extra --help\""), "{kdl}");

    // One document, so a tool asking the binary sees the appended nodes too — that is the
    // reason the hook is on `to_kdl` rather than on the endpoint alone.
    let answer = SpecExtraEx::spec_request(&[OsStr::new(usage::SPEC_REQUEST)])
        .expect("the endpoint should answer");
    assert_eq!(answer, kdl);

    // And the join has to leave something usage-lib can still read, since nothing validated
    // the appended text at compile time.
    let spec: usage_parser::Spec = answer.parse().expect("the joined document should parse");
    assert_eq!(spec.examples.len(), 1);
    assert_eq!(spec.examples[0].code, "extra --help");

    // Appending nodes to the document changes nothing about the declaration that produced it.
    let cli = SpecExtraEx::parse_from(&[OsStr::new("--force")]).expect("valid command line");
    assert!(cli.force);
}

/// mise's root shape: an unmatched word is a task name rather than a mistake.
///
/// This is the one CLI shape where the endpoint changes what a command line means, so the
/// boundary is pinned rather than argued about. `__complete_word__` already shadows the same
/// task-name space in the same CLI, which is the precedent the spelling was chosen against.
#[derive(Cli)]
#[usage(bin = "task-ex", default_subcommand = "run")]
struct DefaultSubcommandEx {
    #[usage(subcommand)]
    command: DefaultSubcommandCommand,
}

#[derive(Subcommands)]
enum DefaultSubcommandCommand {
    /// Run a task by name.
    Run { task: String },
}

#[test]
fn the_endpoint_wins_over_default_subcommand_routing() {
    // Any other word still routes to `run`, which is what makes the next assertion a statement
    // about the endpoint rather than about a CLI that routes nothing.
    let cli = DefaultSubcommandEx::parse_from(&[OsStr::new("build")]).expect("valid command line");
    let DefaultSubcommandCommand::Run { task } = cli.command;
    assert_eq!(task, "build");

    // The request is answered instead of being routed to `run` as a task of that name. A CLI
    // that wants the word back declares it, or sets `spec_endpoint = false`.
    assert!(DefaultSubcommandEx::spec_request(&[OsStr::new(usage::SPEC_REQUEST)]).is_some());
}

// A struct flattened into more than one command: the shape mise has 200-odd times over, and
// the reason the emitted spec used to repeat itself.
#[derive(Args)]
#[allow(dead_code)]
struct SharedFlags {
    /// Print more.
    #[arg(long, short)]
    verbose: bool,
    /// How many at once.
    #[arg(long)]
    jobs: Option<usize>,
}

#[derive(Args)]
#[allow(dead_code)]
struct BuildCmd {
    #[arg(long)]
    release: bool,
    #[usage(flatten)]
    shared: SharedFlags,
    #[arg(long)]
    target: Option<String>,
}

#[derive(Args)]
#[allow(dead_code)]
struct TestCmd {
    #[usage(flatten)]
    shared: SharedFlags,
}

#[derive(Subcommands)]
#[allow(dead_code)]
enum FlagsetCommand {
    Build(BuildCmd),
    Test(TestCmd),
}

#[derive(Cli)]
#[usage(bin = "flagset-ex")]
#[allow(dead_code)]
struct FlagsetEx {
    #[usage(subcommand)]
    command: FlagsetCommand,
}

#[test]
fn a_struct_flattened_twice_is_written_once_as_a_flagset() {
    let kdl = FlagsetEx::to_kdl();
    assert!(kdl.contains("flagset shared-flags {"), "{kdl}");
    // Two commands, one set, and the flags written once rather than under each.
    assert_eq!(kdl.matches("use shared-flags").count(), 2, "{kdl}");
    assert_eq!(kdl.matches("flag \"-v --verbose\"").count(), 1, "{kdl}");

    // The set stands where the struct did, so help order is still declaration order.
    let spec: usage_parser::Spec = kdl.parse().expect("the emitted set should parse back");
    let build = spec.cmd.subcommands.get("build").expect("build");
    let flags: Vec<&str> = build.flags.iter().map(|f| f.name.as_str()).collect();
    assert_eq!(flags, ["release", "verbose", "jobs", "target"], "{kdl}");
    let test = spec.cmd.subcommands.get("test").expect("test");
    let flags: Vec<&str> = test.flags.iter().map(|f| f.name.as_str()).collect();
    assert_eq!(flags, ["verbose", "jobs"], "{kdl}");

    // Help text and values survive the trip, not just the names: a set that lost them
    // would still pass the assertions above.
    let verbose = build
        .flags
        .iter()
        .find(|f| f.name == "verbose")
        .expect("verbose");
    assert_eq!(verbose.help.as_deref(), Some("Print more."));
    assert_eq!(verbose.short, vec!['v']);
    let jobs = build.flags.iter().find(|f| f.name == "jobs").expect("jobs");
    assert!(jobs.arg.is_some(), "{kdl}");
}

#[test]
fn what_the_flagset_expands_to_is_what_the_typed_parse_binds() {
    // The emitted spec is what docs, completions and other implementations read, so the
    // question is not only whether it parses but whether it means the same thing.
    let spec: usage_parser::Spec = FlagsetEx::to_kdl().parse().expect("parses");
    let words = ["flagset-ex", "build", "--verbose", "--jobs", "4"].map(String::from);
    let parsed = usage_parser::parse(&spec, &words).expect("the reference parser binds it");
    assert_eq!(
        parsed.as_env().get("usage_jobs").map(String::as_str),
        Some("4")
    );

    let typed = FlagsetEx::parse_from(&[
        OsStr::new("build"),
        OsStr::new("--verbose"),
        OsStr::new("--jobs"),
        OsStr::new("4"),
    ])
    .expect("the typed parse binds it too");
    let FlagsetCommand::Build(build) = typed.command else {
        panic!("build should have been selected");
    };
    assert!(build.shared.verbose);
    assert_eq!(build.shared.jobs, Some(4));
}

// A set assembled from a smaller one, which is the composition the spec node allows.
#[derive(Args)]
#[allow(dead_code)]
struct InnerFlags {
    #[arg(long)]
    inner: bool,
}

#[derive(Args)]
#[allow(dead_code)]
struct OuterFlags {
    #[usage(flatten)]
    inner: InnerFlags,
    #[arg(long)]
    outer: bool,
}

#[derive(Args)]
#[allow(dead_code)]
struct NestedOne {
    #[usage(flatten)]
    outer: OuterFlags,
}

#[derive(Args)]
#[allow(dead_code)]
struct NestedTwo {
    #[usage(flatten)]
    outer: OuterFlags,
}

#[derive(Subcommands)]
#[allow(dead_code)]
enum NestedFlagsetCommand {
    One(NestedOne),
    Two(NestedTwo),
}

#[derive(Cli)]
#[usage(bin = "nested-flagset-ex")]
#[allow(dead_code)]
struct NestedFlagsetEx {
    #[usage(subcommand)]
    command: NestedFlagsetCommand,
}

#[test]
fn a_struct_that_flattens_another_is_a_set_that_uses_a_set() {
    let kdl = NestedFlagsetEx::to_kdl();
    assert!(kdl.contains("flagset inner-flags {"), "{kdl}");
    assert!(kdl.contains("flagset outer-flags {"), "{kdl}");
    // `--inner` is declared once, inside its own set, and the outer set uses it rather
    // than holding a copy.
    assert_eq!(kdl.matches("flag --inner").count(), 1, "{kdl}");
    assert_eq!(kdl.matches("use inner-flags").count(), 1, "{kdl}");
    assert_eq!(kdl.matches("use outer-flags").count(), 2, "{kdl}");

    let spec: usage_parser::Spec = kdl.parse().expect("composed sets should parse back");
    for name in ["one", "two"] {
        let cmd = spec.cmd.subcommands.get(name).expect(name);
        let flags: Vec<&str> = cmd.flags.iter().map(|f| f.name.as_str()).collect();
        assert_eq!(flags, ["inner", "outer"], "{name}: {kdl}");
    }
}

// A flattened struct holding only positionals. There is no set of flags to name, and the
// arguments stay on each command that flattens it.
#[derive(Args)]
#[allow(dead_code)]
struct PositionalsOnly {
    script: String,
}

#[derive(Args)]
#[allow(dead_code)]
struct RunsAScript {
    #[usage(flatten)]
    script: PositionalsOnly,
}

#[derive(Cli)]
#[usage(bin = "args-only-ex")]
#[allow(dead_code)]
struct ArgsOnlyEx {
    #[usage(flatten)]
    script: PositionalsOnly,
}

#[test]
fn a_flattened_struct_with_no_flags_declares_no_set() {
    let kdl = ArgsOnlyEx::to_kdl();
    assert!(!kdl.contains("flagset"), "{kdl}");
    assert!(!kdl.contains("use "), "{kdl}");
    assert!(kdl.contains("arg <SCRIPT>"), "{kdl}");
}

mod first {
    #[derive(usage_rs::Args)]
    #[allow(dead_code)]
    pub struct Collides {
        #[arg(long)]
        pub left: bool,
    }

    /// Holds a colliding struct, and collides itself.
    #[derive(usage_rs::Args)]
    #[allow(dead_code)]
    pub struct Nested {
        #[usage(flatten)]
        pub inner: Collides,
        #[arg(long)]
        pub one: bool,
    }
}

mod second {
    #[derive(usage_rs::Args)]
    #[allow(dead_code)]
    pub struct Collides {
        #[arg(long)]
        pub right: bool,
    }

    #[derive(usage_rs::Args)]
    #[allow(dead_code)]
    pub struct Nested {
        #[usage(flatten)]
        pub inner: Collides,
        #[arg(long)]
        pub two: bool,
    }
}

#[derive(Args)]
#[allow(dead_code)]
struct UsesFirst {
    #[usage(flatten)]
    inner: first::Collides,
}

#[derive(Args)]
#[allow(dead_code)]
struct UsesSecond {
    #[usage(flatten)]
    inner: second::Collides,
}

#[derive(Subcommands)]
#[allow(dead_code)]
enum CollidingCommand {
    One(UsesFirst),
    Two(UsesSecond),
}

#[derive(Cli)]
#[usage(bin = "colliding-ex")]
#[allow(dead_code)]
struct CollidingEx {
    #[usage(subcommand)]
    command: CollidingCommand,
}

#[test]
fn two_structs_whose_names_end_the_same_way_get_no_set_at_all() {
    // A set named for one of them would put its flags on the command that asked for the
    // other's, so neither gets the name and both are written inline — which is what every
    // flatten did before sets existed, and is never wrong, only longer.
    let kdl = CollidingEx::to_kdl();
    assert!(!kdl.contains("flagset collides"), "{kdl}");
    assert!(!kdl.contains("use collides"), "{kdl}");

    let spec: usage_parser::Spec = kdl.parse().expect("parses");
    let one = spec.cmd.subcommands.get("one").expect("one");
    assert_eq!(
        one.flags
            .iter()
            .map(|f| f.name.as_str())
            .collect::<Vec<_>>(),
        ["left"],
        "{kdl}"
    );
    let two = spec.cmd.subcommands.get("two").expect("two");
    assert_eq!(
        two.flags
            .iter()
            .map(|f| f.name.as_str())
            .collect::<Vec<_>>(),
        ["right"],
        "{kdl}"
    );
}

#[derive(Args)]
#[allow(dead_code)]
struct NestsFirst {
    #[usage(flatten)]
    nested: first::Nested,
}

#[derive(Args)]
#[allow(dead_code)]
struct NestsSecond {
    #[usage(flatten)]
    nested: second::Nested,
}

#[derive(Subcommands)]
#[allow(dead_code)]
enum NestedCollidingCommand {
    One(NestsFirst),
    Two(NestsSecond),
}

#[derive(Cli)]
#[usage(bin = "nested-colliding-ex")]
#[allow(dead_code)]
struct NestedCollidingEx {
    #[usage(subcommand)]
    command: NestedCollidingCommand,
}

#[test]
fn a_collision_does_not_hide_the_one_inside_it() {
    // Both `Nested` types collide, and so do the `Collides` types they hold. Stopping at the
    // outer collision left the inner pair uncompared, so whichever parent was walked first
    // gave its `Collides` the name — and which command got a `use` came down to traversal
    // order rather than to anything either command said.
    let kdl = NestedCollidingEx::to_kdl();
    assert!(!kdl.contains("flagset nested"), "{kdl}");
    assert!(!kdl.contains("flagset collides"), "{kdl}");
    assert!(!kdl.contains("use collides"), "{kdl}");

    let spec: usage_parser::Spec = kdl.parse().expect("parses");
    let one = spec.cmd.subcommands.get("one").expect("one");
    assert_eq!(
        one.flags
            .iter()
            .map(|f| f.name.as_str())
            .collect::<Vec<_>>(),
        ["left", "one"],
        "{kdl}"
    );
    let two = spec.cmd.subcommands.get("two").expect("two");
    assert_eq!(
        two.flags
            .iter()
            .map(|f| f.name.as_str())
            .collect::<Vec<_>>(),
        ["right", "two"],
        "{kdl}"
    );
}

/// mise's shape, which is the one the vocabulary has to fit without changing a
/// spelling: a counted `-v`, three pinning switches, a level-valued flag, and the
/// mutual `overrides` lattice that means at most one of them survives a parse.
#[derive(Debug, Cli)]
#[usage(bin = "loud")]
struct Loud {
    #[usage(
        long,
        short = 'v',
        global,
        count,
        verbosity = "verbose",
        overrides("--quiet", "--silent", "--debug", "--log-level")
    )]
    verbose: u8,
    #[usage(
        long,
        short = 'q',
        global,
        verbosity = "error",
        overrides("--verbose", "--silent", "--debug", "--log-level")
    )]
    quiet: bool,
    #[usage(
        long,
        global,
        verbosity = "silent",
        overrides("--verbose", "--quiet", "--debug", "--log-level")
    )]
    silent: bool,
    #[usage(
        long,
        global,
        hide,
        verbosity = "debug",
        overrides("--verbose", "--quiet", "--silent", "--log-level")
    )]
    debug: bool,
    #[usage(
        long,
        global,
        hide,
        verbosity = "level",
        value_name = "LEVEL",
        // mise's own list, `warning` included: the scale reads it as `warn`.
        choices("trace", "debug", "info", "warning", "error"),
        overrides("--verbose", "--quiet", "--silent", "--debug")
    )]
    log_level: Option<String>,
    /// mise's `mise watch --color`, and aube's pair, in one CLI.
    #[usage(
        long,
        color = "choice",
        value_name = "WHEN",
        choices("auto", "always", "never")
    )]
    color: Option<String>,
    #[usage(long, color = "never")]
    no_color: bool,
}

/// aube's `--loglevel`, whose values are a `ValueEnum` rather than words on the field.
#[derive(Debug, PartialEq, Eq, ValueEnum)]
enum LogLevelValue {
    Trace,
    Debug,
    Info,
    Warn,
    Error,
    Silent,
}

#[derive(Cli)]
#[usage(bin = "aubish")]
struct Aubish {
    #[usage(long, short = 'v', global, verbosity = "debug")]
    verbose: bool,
    #[usage(long, global, value_enum, verbosity = "level", value_name = "LEVEL")]
    loglevel: Option<LogLevelValue>,
    #[usage(long, global, verbosity = "silent")]
    silent: bool,
}

fn level_of(argv: &[&str]) -> usage::Verbosity {
    let words: Vec<&OsStr> = argv.iter().map(OsStr::new).collect();
    let cli = Loud::parse_from(&words).expect("valid command line");
    usage::VerbosityPolicy::verbosity(&cli)
}

fn colour_of(argv: &[&str]) -> usage::ColorChoice {
    let words: Vec<&OsStr> = argv.iter().map(OsStr::new).collect();
    let cli = Loud::parse_from(&words).expect("valid command line");
    usage::ColorPolicy::color(&cli)
}

#[test]
fn a_declared_verbosity_resolves_to_a_level() {
    assert_eq!(level_of(&[]), usage::Verbosity::Info);
    assert_eq!(level_of(&["-v"]), usage::Verbosity::Debug);
    assert_eq!(level_of(&["-vv"]), usage::Verbosity::Trace);
    assert_eq!(level_of(&["-vvvv"]), usage::Verbosity::Trace);
    assert_eq!(level_of(&["--quiet"]), usage::Verbosity::Error);
    assert_eq!(level_of(&["--silent"]), usage::Verbosity::Silent);
    assert_eq!(level_of(&["--debug"]), usage::Verbosity::Debug);
    assert_eq!(level_of(&["--log-level", "trace"]), usage::Verbosity::Trace);
    // mise spells it `warning`; the scale spells it `warn`.
    assert_eq!(
        level_of(&["--log-level", "warning"]),
        usage::Verbosity::Warn
    );
}

#[test]
fn the_overrides_lattice_settles_a_contradiction_before_the_level_does() {
    // Last one wins, which is the CLI's own declaration doing the work: the
    // resolver never sees two of these at once.
    assert_eq!(level_of(&["-vv", "--quiet"]), usage::Verbosity::Error);
    assert_eq!(level_of(&["--quiet", "-vv"]), usage::Verbosity::Trace);
    assert_eq!(level_of(&["--silent", "--debug"]), usage::Verbosity::Debug);
}

#[test]
fn a_level_can_come_from_a_value_enum() {
    let words = [OsStr::new("--loglevel"), OsStr::new("silent")];
    let cli = Aubish::parse_from(&words).unwrap();
    assert_eq!(
        usage::VerbosityPolicy::verbosity(&cli),
        usage::Verbosity::Silent
    );
    // aube's `-v` is a shortcut for `debug`, and `--loglevel` outranks it.
    let words = [
        OsStr::new("-v"),
        OsStr::new("--loglevel"),
        OsStr::new("trace"),
    ];
    let cli = Aubish::parse_from(&words).unwrap();
    assert_eq!(
        usage::VerbosityPolicy::verbosity(&cli),
        usage::Verbosity::Trace
    );
    // With no lattice to settle it, the most restrictive switch wins.
    let words = [OsStr::new("-v"), OsStr::new("--silent")];
    let cli = Aubish::parse_from(&words).unwrap();
    assert_eq!(
        usage::VerbosityPolicy::verbosity(&cli),
        usage::Verbosity::Silent
    );
}

#[test]
fn a_declared_colour_resolves_to_a_choice() {
    assert_eq!(colour_of(&[]), usage::ColorChoice::Auto);
    assert_eq!(colour_of(&["--color", "never"]), usage::ColorChoice::Never);
    assert_eq!(colour_of(&["--color=always"]), usage::ColorChoice::Always);
    assert_eq!(colour_of(&["--no-color"]), usage::ColorChoice::Never);
    // A refusal beats a request, whichever order they arrive in.
    assert_eq!(
        colour_of(&["--color", "always", "--no-color"]),
        usage::ColorChoice::Never
    );
    assert_eq!(
        colour_of(&["--no-color", "--color", "always"]),
        usage::ColorChoice::Never
    );
}

#[test]
fn a_command_line_can_be_asked_about_colour_before_anything_is_bound() {
    // What help and diagnostics read: they render on a path where the struct was
    // never built.
    let read = |argv: &[&str]| {
        let words: Vec<&OsStr> = argv.iter().map(OsStr::new).collect();
        usage::policy::color_from_argv(Loud::spec(), &words)
    };
    assert_eq!(read(&[]), None);
    assert_eq!(read(&["--no-color"]), Some(usage::ColorChoice::Never));
    assert_eq!(read(&["--color=never"]), Some(usage::ColorChoice::Never));
    assert_eq!(
        read(&["--color", "always"]),
        Some(usage::ColorChoice::Always)
    );
    // Last one wins, as a repeated scalar flag does.
    assert_eq!(
        read(&["--color", "always", "--color", "never"]),
        Some(usage::ColorChoice::Never)
    );
    // A word after the separator is somebody's argument, not a request.
    assert_eq!(read(&["--", "--no-color"]), None);
    // And a detached value that happens to spell one is that flag's value.
    assert_eq!(read(&["--log-level", "--no-color"]), None);
}

#[test]
fn a_declared_colour_flag_turns_off_the_colour_in_usage_own_output() {
    // The bug this pays for: before the declaration existed, a CLI's `--no-color`
    // could not reach the help page usage renders on its behalf.
    let coloured = usage::help::Style::for_choice(usage::ColorChoice::Always, false);
    let plain = usage::help::Style::for_choice(usage::ColorChoice::Never, true);
    let page = |style| {
        usage::help::render_styled(Loud::spec(), Loud::command(), false, style)
            .expect("a help page")
    };
    assert!(page(coloured).contains('\u{1b}'));
    assert!(!page(plain).contains('\u{1b}'));

    let words = [OsStr::new("--no-color"), OsStr::new("--nonsense")];
    let err = Loud::parse_from(&words).unwrap_err();
    let rendered = usage::render_failure(Loud::spec(), &words, &err);
    assert!(!rendered.contains('\u{1b}'), "{rendered}");
}

#[test]
fn roles_reach_the_emitted_spec() {
    let kdl = Loud::to_kdl();
    assert!(kdl.contains("verbosity=verbose"), "{kdl}");
    assert!(kdl.contains("verbosity=error"), "{kdl}");
    assert!(kdl.contains("verbosity=silent"), "{kdl}");
    assert!(kdl.contains("verbosity=level"), "{kdl}");
    assert!(kdl.contains("color=choice"), "{kdl}");
    assert!(kdl.contains("color=never"), "{kdl}");
    // And back again, through the interpreter that reads them.
    let spec: usage_parser::Spec = kdl.parse().expect("a spec usage-lib can read");
    let flag = |name: &str| {
        spec.cmd
            .flags
            .iter()
            .find(|f| f.name == name)
            .unwrap_or_else(|| panic!("no flag {name}"))
            .clone()
    };
    assert_eq!(
        flag("verbose").verbosity,
        Some(usage_parser::SpecVerbosityRole::Verbose)
    );
    assert_eq!(
        flag("log-level").verbosity,
        Some(usage_parser::SpecVerbosityRole::Level)
    );
    assert_eq!(
        flag("no-color").color,
        Some(usage_parser::SpecColorRole::Never)
    );
}
