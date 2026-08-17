//! Subcommands, end to end: routing, per-command values, and the emitted spec.

use std::ffi::OsStr;

use usage::Spec as LibSpec;
use usage_derive::{Args, Cli, Subcommands};

/// A tool that does things
#[derive(Cli)]
#[usage(bin = "ex", version = "1.0")]
struct Ex {
    /// Say more
    #[usage(short = 'v', long, global)]
    verbose: bool,
    /// What to do
    #[usage(subcommand)]
    command: Option<Commands>,
}

#[derive(Subcommands)]
enum Commands {
    /// Install a tool
    Install(Install),
    /// Run a task
    #[usage(name = "run")]
    RunTask(Run),
}

/// The struct's own description, which the variant's overrides
#[derive(Args)]
struct Install {
    /// Overwrite an existing install
    #[usage(short = 'f', long)]
    force: bool,
    /// How many at once
    #[usage(short = 'j', long, default = "4")]
    jobs: Option<String>,
    /// What to install
    tools: Vec<String>,
}

/// Run a task
#[derive(Args)]
struct Run {
    /// The task to run
    task: String,
}

/// A second CLI where the variant's name and the struct's differ, and where one
/// argument struct is shared by two variants. The first version passed its tests by
/// coincidence: `RunTask(Run)` named its command `run` either way.
#[derive(Cli)]
#[usage(bin = "two")]
struct Two {
    #[usage(subcommand)]
    command: Option<TwoCommands>,
}

#[derive(Subcommands)]
enum TwoCommands {
    /// Add a thing
    #[usage(name = "add")]
    Install(AddArgs),
    /// Remove a thing
    #[usage(name = "remove")]
    Uninstall(RemoveArgs),
}

/// The struct's description, which the variant's overrides
#[derive(Args)]
struct AddArgs {
    /// What to add
    target: String,
}

/// Also overridden
#[derive(Args)]
struct RemoveArgs {
    /// What to remove
    target: String,
}

#[test]
fn the_variant_names_the_command_not_the_struct() {
    let a = argv(["add", "x"]);
    let two = Two::parse_from(&a).expect("`add` should route");
    assert!(matches!(two.command, Some(TwoCommands::Install(_))));

    // And the struct's own name is not a command at all.
    let a = argv(["shared", "x"]);
    assert!(
        Two::parse_from(&a).is_err(),
        "the struct's name should not select anything"
    );

    let spec: LibSpec = Two::to_kdl().parse().expect("valid spec");
    let mut names: Vec<&str> = spec.cmd.subcommands.keys().map(String::as_str).collect();
    names.sort();
    assert_eq!(names, ["add", "remove"]);
}

#[test]
fn each_command_collects_into_its_own_struct() {
    // Two variants wrapping one struct is refused at compile time, because a
    // command's values go into the struct that declares them: sharing one would
    // collect into whichever command was reached first. Each here has its own.
    let a = argv(["remove", "y"]);
    let two = Two::parse_from(&a).expect("`remove` should route");
    let Some(TwoCommands::Uninstall(args)) = two.command else {
        panic!("expected the remove command");
    };
    assert_eq!(args.target, "y");

    let a = argv(["add", "x"]);
    let two = Two::parse_from(&a).expect("`add` should route");
    let Some(TwoCommands::Install(args)) = two.command else {
        panic!("expected the add command");
    };
    assert_eq!(args.target, "x");
}

fn argv<const N: usize>(tokens: [&str; N]) -> [&OsStr; N] {
    tokens.map(OsStr::new)
}

#[test]
fn a_word_selects_a_command_and_its_own_fields_are_filled() {
    let a = argv(["install", "--force", "node@20", "go@1.22"]);
    let ex = Ex::parse_from(&a).expect("should parse");

    let Some(Commands::Install(install)) = ex.command else {
        panic!("expected the install command");
    };
    assert!(install.force);
    assert_eq!(install.tools, ["node@20", "go@1.22"]);
    // A declared default is in place before parsing starts, so an untouched flag
    // still has it.
    assert_eq!(install.jobs.as_deref(), Some("4"));
}

#[test]
fn another_variant_routes_by_its_own_name() {
    // `run` rather than `run-task`: the variant renames itself.
    let a = argv(["run", "build"]);
    let ex = Ex::parse_from(&a).expect("should parse");
    let Some(Commands::RunTask(run)) = ex.command else {
        panic!("expected the run command");
    };
    assert_eq!(run.task, "build");
}

#[test]
fn no_subcommand_leaves_the_field_empty() {
    let a = argv(["--verbose"]);
    let ex = Ex::parse_from(&a).expect("should parse");
    assert!(ex.verbose);
    assert!(ex.command.is_none());
}

#[test]
fn a_global_flag_works_on_either_side_of_the_command() {
    for tokens in [["--verbose", "run", "build"], ["run", "build", "--verbose"]] {
        let a = argv(tokens);
        let ex = Ex::parse_from(&a).expect("should parse");
        assert!(ex.verbose, "{tokens:?}");
        assert!(
            matches!(ex.command, Some(Commands::RunTask(_))),
            "{tokens:?}"
        );
    }
}

#[test]
fn one_command_does_not_see_another_ones_flag() {
    // `--force` belongs to `install`, so `run` does not answer to it. Leniently it
    // becomes a word, and `run` has one argument to hold it.
    let a = argv(["run", "--force"]);
    let ex = Ex::parse_from(&a).expect("should parse");
    let Some(Commands::RunTask(run)) = ex.command else {
        panic!("expected the run command");
    };
    assert_eq!(run.task, "--force");
}

#[test]
fn the_spec_carries_the_commands() {
    let kdl = Ex::to_kdl();
    let spec: LibSpec = kdl.parse().unwrap_or_else(|e| panic!("{e}\n\n{kdl}"));

    assert_eq!(spec.bin, "ex");
    let mut names: Vec<&str> = spec.cmd.subcommands.keys().map(String::as_str).collect();
    names.sort();
    assert_eq!(names, ["install", "run"]);

    let install = &spec.cmd.subcommands["install"];
    // The variant's doc comment wins: it is where a reader of the enum expects to
    // describe the command, and ignoring it would lose the description silently.
    assert_eq!(install.help.as_deref(), Some("Install a tool"));
    assert_eq!(install.flags.len(), 2);
    assert_eq!(install.args.len(), 1);
    assert!(install.args[0].var, "`tools` is a Vec, so it takes several");

    let run = &spec.cmd.subcommands["run"];
    assert_eq!(run.help.as_deref(), Some("Run a task"));
    assert_eq!(run.args[0].name, "TASK");
}

#[test]
fn keys_are_unique_across_independently_expanded_types() {
    // Each derive assigns keys without seeing the others, so a collision would bind
    // the wrong field. Checked here rather than trusted.
    let mut keys = vec![Ex::command().key];
    for cmd in Ex::command().subcommands {
        keys.push(cmd.key);
        for flag in cmd.flags {
            keys.push(flag.key);
        }
        for arg in cmd.args {
            keys.push(arg.key);
        }
    }
    for flag in Ex::command().flags {
        keys.push(flag.key);
    }

    let mut sorted = keys.clone();
    sorted.sort_unstable();
    sorted.dedup();
    assert_eq!(sorted.len(), keys.len(), "duplicate keys in {keys:?}");
}

#[test]
fn the_emitted_spec_reads_the_way_a_handwritten_one_would() {
    // The whole point of emitting KDL: what comes out is what a person would have
    // written, and `usage g markdown|manpage` reads it without knowing a derive was
    // involved.
    insta::assert_snapshot!(Ex::to_kdl());
}

/// Commands that answer to more than one name.
///
/// mise has 91 of these — `i` for `install`, `x` for `exec`, `r` for `tasks run` — and
/// until now the derive could not declare one, so a shadow of mise rejected invocations
/// the real thing accepts.
#[derive(Subcommands)]
enum AliasedCommands {
    /// Install a tool
    Install(AliasedInstallArgs),
    /// Remove a tool
    #[usage(alias = "rm")]
    Remove(AliasedRemoveArgs),
}

/// Install a tool
#[derive(Args)]
#[usage(alias = "i", alias_hidden = "add")]
struct AliasedInstallArgs {
    /// What to install
    #[usage(arg, name = "TOOL")]
    tool: Option<String>,
}

/// Remove a tool
#[derive(Args)]
#[usage(alias = "uninstall")]
struct AliasedRemoveArgs {
    /// Say nothing
    #[usage(long)]
    quiet: bool,
}

/// A tool with aliased commands
#[derive(Cli)]
#[usage(bin = "al")]
struct Aliased {
    #[usage(subcommand)]
    command: Option<AliasedCommands>,
}

#[test]
fn a_command_answers_to_its_aliases() {
    for word in ["install", "i", "add"] {
        let a = argv([word, "node@20"]);
        let Some(AliasedCommands::Install(install)) =
            Aliased::parse_from(&a).expect("should parse").command
        else {
            panic!("`{word}` should have selected install")
        };
        assert_eq!(install.tool.as_deref(), Some("node@20"));
    }

    for word in ["remove", "rm", "uninstall"] {
        let a = argv([word, "--quiet"]);
        let Some(AliasedCommands::Remove(remove)) =
            Aliased::parse_from(&a).expect("should parse").command
        else {
            panic!("`{word}` should have selected remove")
        };
        assert!(remove.quiet);
    }
}

#[test]
fn the_spec_says_which_aliases_are_hidden() {
    let spec: LibSpec = Aliased::to_kdl().parse().expect("valid spec");
    let install = spec.cmd.subcommands.get("install").expect("install");
    assert_eq!(install.aliases, vec!["i".to_string()]);
    assert_eq!(install.hidden_aliases, vec!["add".to_string()]);

    let remove = spec.cmd.subcommands.get("remove").expect("remove");
    assert_eq!(
        remove.aliases,
        vec!["uninstall".to_string(), "rm".to_string()]
    );
    assert!(remove.hidden_aliases.is_empty());

    // And usage-lib resolves them, which is how completions follow an alias. Its
    // `subcommands` map is keyed by name only — the aliases live in a lookup built
    // alongside it.
    for word in ["install", "i", "add"] {
        assert_eq!(
            spec.cmd.find_subcommand(word).map(|c| c.name.as_str()),
            Some("install"),
            "usage-lib should resolve `{word}`"
        );
    }
}

/// A command set whose variants are boxed.
///
/// mise does this for its largest commands: a command enum is as large as its biggest
/// variant, so a thirty-flag subcommand makes every invocation move that much stack.
/// `Box` is also how a CLI that size stops running into clap's limits.
#[derive(Subcommands)]
enum BoxedCommands {
    /// Install a tool
    #[usage(alias = "i")]
    Install(Box<BoxedInstallArgs>),
    /// Something small, unboxed, so the two forms are known to work side by side
    Nudge(BoxedNudgeArgs),
}

/// Install a tool
#[derive(Args)]
struct BoxedInstallArgs {
    /// What to install
    #[usage(arg, name = "TOOL")]
    tool: Option<String>,
    /// How many at once
    #[usage(long, short = 'j')]
    jobs: Option<String>,
}

/// Nudge it
#[derive(Args)]
struct BoxedNudgeArgs {
    /// Say nothing
    #[usage(long)]
    quiet: bool,
}

/// A tool with boxed commands
#[derive(Cli)]
#[usage(bin = "bx")]
struct Boxed {
    #[usage(subcommand)]
    command: Option<BoxedCommands>,
}

#[test]
fn a_boxed_variant_parses_like_any_other() {
    let a = argv(["install", "-j", "4", "node@20"]);
    let Some(BoxedCommands::Install(install)) = Boxed::parse_from(&a).expect("parses").command
    else {
        panic!("expected install")
    };
    assert_eq!(install.tool.as_deref(), Some("node@20"));
    assert_eq!(install.jobs.as_deref(), Some("4"));

    // Its alias, since the box is nothing to do with how the command is named.
    let a = argv(["i", "node@20"]);
    assert!(matches!(
        Boxed::parse_from(&a).expect("parses").command,
        Some(BoxedCommands::Install(_))
    ));

    // And an unboxed variant in the same enum.
    let a = argv(["nudge", "--quiet"]);
    let Some(BoxedCommands::Nudge(nudge)) = Boxed::parse_from(&a).expect("parses").command else {
        panic!("expected nudge")
    };
    assert!(nudge.quiet);
}

#[test]
fn boxing_changes_nothing_about_the_spec() {
    // The box is how the variant holds the struct, not something a CLI has: a reader of
    // the spec, or of `--help`, should not be able to tell.
    let spec: LibSpec = Boxed::to_kdl().parse().expect("valid spec");
    let install = spec.cmd.subcommands.get("install").expect("install");
    assert_eq!(install.aliases, vec!["i".to_string()]);
    assert_eq!(install.flags.len(), 1);
    assert_eq!(install.args.len(), 1);
    assert!(!Boxed::to_kdl().contains("Box"), "{}", Boxed::to_kdl());
}

/// A boxed command whose struct is named through a path.
///
/// `type_name` renders only a type's last segment, so reading the box out of a rendered
/// string turned `Box<cmds::Deep>` into `Deep` and the generated code named a type that is
/// not in scope. Taking the type apart syntactically keeps the path.
mod cmds {
    /// Something deeper
    #[derive(usage_derive::Args)]
    pub struct Deep {
        /// Say nothing
        #[usage(long)]
        pub quiet: bool,
    }
}

#[derive(Subcommands)]
enum QualifiedCommands {
    /// A command declared elsewhere
    Deep(Box<cmds::Deep>),
}

/// A tool whose command lives in another module
#[derive(Cli)]
#[usage(bin = "q")]
struct Qualified {
    #[usage(subcommand)]
    command: Option<QualifiedCommands>,
}

#[test]
fn a_boxed_command_can_be_named_through_a_path() {
    let a = argv(["deep", "--quiet"]);
    let Some(QualifiedCommands::Deep(deep)) = Qualified::parse_from(&a).expect("parses").command
    else {
        panic!("expected deep")
    };
    assert!(deep.quiet);
}
