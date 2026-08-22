//! Merging a command line into a value that already exists.
//!
//! `update_from` is clap's name for parsing twice: a REPL reading a line at a time, a daemon
//! reconfigured while it runs. The rules cannot be inherited from a fresh parse, because a
//! parse cannot be run backwards — a `String` field says nothing about the word it was made
//! from — so what the caller already holds is read from the struct itself. These tests state
//! each rule once: relationships see the standing value, the environment and declared defaults
//! do not overwrite it, a collection is replaced only when this argv mentions it, and a
//! subcommand word naming a different variant replaces it rather than merging into fields the
//! new command does not have.

use std::ffi::OsStr;

use usage_argv::Error;
use usage_derive::{ArgGroup, Args, Cli, Subcommands};

fn argv<const N: usize>(tokens: [&str; N]) -> [&OsStr; N] {
    tokens.map(OsStr::new)
}

/// A CLI whose `--file` is required and whose `--quiet` excludes `--verbose`.
#[derive(Cli, Debug, PartialEq)]
#[usage(bin = "upd")]
struct Upd {
    /// The file to work on
    #[usage(long)]
    file: String,
    /// Say less
    #[usage(long, conflicts = "verbose")]
    quiet: bool,
    /// Say more
    #[usage(long)]
    verbose: bool,
    /// How many times to try
    #[usage(long)]
    retries: Option<u32>,
}

#[test]
fn a_standing_required_flag_need_not_be_given_again() {
    let a = argv(["--file", "a.txt"]);
    let mut upd = Upd::parse_from(&a).expect("the first parse supplies it");

    // The second command line says nothing about `--file`, which a fresh parse would refuse.
    let a = argv(["--retries", "3"]);
    upd.try_update_from(&a).expect("--file already stands");

    assert_eq!(upd.file, "a.txt");
    assert_eq!(upd.retries, Some(3));
}

/// A CLI whose required declaration is a collection, which can be empty.
#[derive(Cli, Debug, PartialEq)]
#[usage(bin = "reqvec")]
struct ReqVec {
    /// Files to read
    #[usage(long, required)]
    include: Vec<String>,
    /// Something to change, so the update has a reason to run
    #[usage(long)]
    tag: Option<String>,
}

#[test]
fn a_required_collection_neither_side_filled_is_still_missing() {
    // An update does not weaken a requirement, it widens what can satisfy one: a collection
    // that is empty on both sides is a value nobody supplied.
    let mut req = ReqVec {
        include: Vec::new(),
        tag: None,
    };
    let a = argv(["--tag", "second"]);
    assert!(matches!(
        req.try_update_from(&a),
        Err(Error::MissingRequired { name: "include" })
    ));

    let a = argv(["--include", "a"]);
    req.try_update_from(&a).expect("now it stands");
    let a = argv(["--tag", "second"]);
    req.try_update_from(&a).expect("and keeps standing");
    assert_eq!(req.include, ["a"]);
}

#[test]
fn a_standing_flag_conflicts_with_a_new_one() {
    let a = argv(["--file", "a.txt", "--quiet"]);
    let mut upd = Upd::parse_from(&a).expect("valid on its own");

    let a = argv(["--verbose"]);
    assert!(
        matches!(
            upd.try_update_from(&a),
            Err(Error::ConflictingFlags {
                name: "quiet",
                other: "verbose",
            })
        ),
        "the conflict is between what stands and what arrived",
    );
}

#[test]
fn a_failed_update_changes_nothing() {
    let a = argv(["--file", "a.txt", "--quiet"]);
    let mut upd = Upd::parse_from(&a).expect("valid");

    let a = argv(["--file", "b.txt", "--verbose"]);
    upd.try_update_from(&a).expect_err("conflicts with --quiet");

    assert_eq!(upd.file, "a.txt", "the rejected --file was not merged");
    assert!(!upd.verbose);
}

/// A CLI whose `--out` reads the environment and whose `--level` has a declared default.
///
/// The two environment tests below use distinct variable names so parallel cargo
/// threads cannot race on one shared process env.
#[derive(Cli, Debug, PartialEq)]
#[usage(bin = "envupd")]
struct EnvUpd {
    /// Where to write
    #[usage(long, env = "UPDATE_FROM_OUT")]
    out: Option<String>,
    /// How loud to be
    #[usage(long, default = "info")]
    level: String,
    /// Something to change, so the update has a reason to run
    #[usage(long)]
    tag: Option<String>,
}

#[derive(Cli, Debug, PartialEq)]
#[usage(bin = "envfill")]
struct EnvFill {
    /// Where to write
    #[usage(long, env = "UPDATE_FROM_FILL")]
    out: Option<String>,
    /// Something to change, so the update has a reason to run
    #[usage(long)]
    tag: Option<String>,
}

#[test]
fn the_environment_does_not_overwrite_a_standing_value() {
    unsafe { std::env::remove_var("UPDATE_FROM_OUT") };
    unsafe { std::env::set_var("UPDATE_FROM_OUT", "from-env") };
    let a = argv(["--out", "from-argv"]);
    let mut env_upd = EnvUpd::parse_from(&a).expect("argv wins over the environment");
    assert_eq!(env_upd.out.as_deref(), Some("from-argv"));

    let a = argv(["--tag", "second"]);
    env_upd.try_update_from(&a).expect("valid");
    assert_eq!(
        env_upd.out.as_deref(),
        Some("from-argv"),
        "the variable fills an empty field, not a full one",
    );
    assert_eq!(env_upd.tag.as_deref(), Some("second"));
    unsafe { std::env::remove_var("UPDATE_FROM_OUT") };
}

#[test]
fn the_environment_still_fills_a_field_nothing_has_supplied() {
    unsafe { std::env::remove_var("UPDATE_FROM_FILL") };
    let a = argv(["--tag", "first"]);
    let mut env_upd = EnvFill::parse_from(&a).expect("valid");
    assert_eq!(env_upd.out, None);

    unsafe { std::env::set_var("UPDATE_FROM_FILL", "from-env") };
    let a = argv(["--tag", "second"]);
    env_upd.try_update_from(&a).expect("valid");
    assert_eq!(env_upd.out.as_deref(), Some("from-env"));
    unsafe { std::env::remove_var("UPDATE_FROM_FILL") };
}

#[test]
fn a_declared_default_does_not_overwrite_a_standing_value() {
    let a = argv(["--level", "debug"]);
    let mut env_upd = EnvUpd::parse_from(&a).expect("valid");

    let a = argv(["--tag", "second"]);
    env_upd.try_update_from(&a).expect("valid");
    assert_eq!(
        env_upd.level, "debug",
        "`info` is what an empty field falls back to, not what an update imposes",
    );
}

/// A CLI with two collections, so one can be mentioned while the other is not.
#[derive(Cli, Debug, PartialEq)]
#[usage(bin = "vecupd")]
struct VecUpd {
    /// Files to read
    #[usage(long)]
    include: Vec<String>,
    /// Files to skip
    #[usage(long)]
    exclude: Vec<String>,
}

#[test]
fn a_collection_this_argv_mentions_is_replaced_whole() {
    let a = argv(["--include", "a", "--include", "b", "--exclude", "x"]);
    let mut vec_upd = VecUpd::parse_from(&a).expect("valid");

    let a = argv(["--include", "c"]);
    vec_upd.try_update_from(&a).expect("valid");

    assert_eq!(vec_upd.include, ["c"], "replaced rather than appended to");
    assert_eq!(vec_upd.exclude, ["x"], "not mentioned, so not touched");
}

#[test]
fn a_collection_no_argv_mentions_survives_every_update() {
    let a = argv(["--include", "a"]);
    let mut vec_upd = VecUpd::parse_from(&a).expect("valid");
    let a = argv([]);
    vec_upd.try_update_from(&a).expect("valid");
    assert_eq!(vec_upd.include, ["a"]);
}

/// A CLI with subcommands whose variants carry different fields.
#[derive(Cli, Debug, PartialEq)]
#[usage(bin = "subupd")]
struct SubUpd {
    /// Say more
    #[usage(long, global)]
    verbose: bool,
    #[usage(subcommand)]
    command: Cmd,
}

#[derive(Subcommands, Debug, PartialEq)]
enum Cmd {
    /// Add something
    Add(AddArgs),
    /// Remove something
    Remove(RemoveArgs),
}

#[derive(Args, Debug, PartialEq)]
struct AddArgs {
    /// What to add
    name: String,
    /// Add it even if it is there
    #[usage(long)]
    force: bool,
    /// Where to put it
    #[usage(long)]
    dest: Option<String>,
}

#[derive(Args, Debug, PartialEq)]
struct RemoveArgs {
    /// What to remove
    name: String,
}

#[test]
fn the_same_subcommand_merges_field_by_field() {
    let a = argv(["add", "thing", "--force"]);
    let mut sub_upd = SubUpd::parse_from(&a).expect("valid");

    // `name` is required and not repeated: the standing value answers for it, and `--force`
    // survives a command line that says nothing about it.
    let a = argv(["add", "--dest", "/tmp"]);
    sub_upd.try_update_from(&a).expect("the same variant");

    let Cmd::Add(add) = &sub_upd.command else {
        panic!("still `add`");
    };
    assert_eq!(add.name, "thing");
    assert!(add.force);
    assert_eq!(add.dest.as_deref(), Some("/tmp"));
}

#[test]
fn a_different_subcommand_replaces_the_variant_whole() {
    let a = argv(["add", "thing", "--force"]);
    let mut sub_upd = SubUpd::parse_from(&a).expect("valid");

    let a = argv(["remove", "other"]);
    sub_upd.try_update_from(&a).expect("a different variant");

    assert_eq!(
        sub_upd.command,
        Cmd::Remove(RemoveArgs {
            name: "other".into(),
        }),
        "`--force` belonged to `add` and went with it",
    );
}

#[test]
fn a_root_flag_survives_a_subcommand_switch() {
    let a = argv(["--verbose", "add", "thing"]);
    let mut sub_upd = SubUpd::parse_from(&a).expect("valid");

    let a = argv(["remove", "other"]);
    sub_upd.try_update_from(&a).expect("valid");

    assert!(sub_upd.verbose, "the root's own fields merge as ever");
}

#[test]
fn a_subcommand_word_is_needed_for_the_variant_to_change() {
    let a = argv(["add", "thing"]);
    let mut sub_upd = SubUpd::parse_from(&a).expect("valid");

    // A required subcommand that no update repeats: the standing one answers for it.
    let a = argv(["--verbose"]);
    sub_upd
        .try_update_from(&a)
        .expect("a command already stands");
    assert!(sub_upd.verbose);
    let Cmd::Add(add) = &sub_upd.command else {
        panic!("unchanged");
    };
    assert_eq!(add.name, "thing");
}

/// How to print the result.
#[derive(ArgGroup, Debug, PartialEq)]
enum Format {
    /// Print JSON
    Json,
    /// Print YAML
    Yaml,
}

/// A CLI with a flattened group and an `ArgGroup`.
#[derive(Cli, Debug, PartialEq)]
#[usage(bin = "flatupd")]
struct FlatUpd {
    #[usage(flatten)]
    common: Common,
    #[usage(arg_group)]
    format: Option<Format>,
}

#[derive(Args, Debug, PartialEq)]
struct Common {
    /// The file to work on
    #[usage(long)]
    file: String,
    /// How many times to try
    #[usage(long)]
    retries: Option<u32>,
}

#[test]
fn a_flattened_group_merges_the_way_the_root_does() {
    let a = argv(["--file", "a.txt", "--json"]);
    let mut flat = FlatUpd::parse_from(&a).expect("valid");

    // `--file` is required and lives in the flattened struct, so the recursion is what makes
    // this pass at all.
    let a = argv(["--retries", "2"]);
    flat.try_update_from(&a).expect("--file already stands");

    assert_eq!(flat.common.file, "a.txt");
    assert_eq!(flat.common.retries, Some(2));
    assert_eq!(
        flat.format,
        Some(Format::Json),
        "not mentioned, not touched"
    );
}

#[test]
fn a_group_member_this_argv_gives_selects_its_variant() {
    let a = argv(["--file", "a.txt", "--json"]);
    let mut flat = FlatUpd::parse_from(&a).expect("valid");

    let a = argv(["--yaml"]);
    flat.try_update_from(&a).expect("one member");
    assert_eq!(flat.format, Some(Format::Yaml));
}

#[test]
fn two_group_members_in_one_update_still_conflict() {
    let a = argv(["--file", "a.txt"]);
    let mut flat = FlatUpd::parse_from(&a).expect("valid");

    let a = argv(["--json", "--yaml"]);
    assert!(
        matches!(
            flat.try_update_from(&a),
            Err(Error::ConflictingFlags {
                name: "yaml",
                other: "json",
            })
        ),
        "a group admits one member per command line, update or not",
    );
}

#[test]
fn update_from_argv_strips_the_program_name() {
    let a = argv(["--file", "a.txt"]);
    let mut upd = Upd::parse_from(&a).expect("valid");

    let a = argv(["upd", "--retries", "3"]);
    upd.try_update_from_argv(&a).expect("argv0 is not a flag");
    assert_eq!(upd.retries, Some(3));
    assert_eq!(upd.file, "a.txt");
}

/// A CLI reached under a second program name that promotes one of its subcommands.
#[derive(Cli, Debug, PartialEq)]
#[usage(bin = "viewupd", view("runner", root = "run", globals))]
struct ViewUpd {
    /// Say more
    #[usage(long, global)]
    verbose: bool,
    #[usage(subcommand)]
    command: Option<ViewCmd>,
}

#[derive(Subcommands, Debug, PartialEq)]
enum ViewCmd {
    /// Run something
    Run(RunArgs),
}

#[derive(Args, Debug, PartialEq)]
struct RunArgs {
    /// How many workers
    #[usage(long)]
    jobs: Option<u32>,
}

#[test]
fn a_view_program_name_promotes_its_command_on_an_update_too() {
    let a = argv(["run", "--jobs", "2"]);
    let mut view = ViewUpd::parse_from(&a).expect("valid");

    // A view builds a struct that omits the root fields it does not carry; an update has no
    // such struct to project into, so the words are rewritten and the merge is the ordinary
    // one — `--verbose` survives because this command line said nothing about it.
    let a = argv(["runner", "--jobs", "4"]);
    view.try_update_from_argv(&a).expect("the view's own name");

    assert_eq!(
        view.command,
        Some(ViewCmd::Run(RunArgs { jobs: Some(4) })),
        "`runner` reached `run` without the word",
    );
}

/// A CLI whose subcommands nest, so a merge has to recurse through the enum twice.
#[derive(Cli, Debug, PartialEq)]
#[usage(bin = "nestupd")]
struct NestUpd {
    #[usage(subcommand)]
    command: Outer,
}

#[derive(Subcommands, Debug, PartialEq)]
enum Outer {
    /// Work on a remote
    Remote(RemoteArgs),
}

#[derive(Args, Debug, PartialEq)]
struct RemoteArgs {
    /// Which remote
    #[usage(long)]
    name: Option<String>,
    #[usage(subcommand)]
    command: Inner,
}

#[derive(Subcommands, Debug, PartialEq)]
enum Inner {
    /// Add it
    Add(InnerAdd),
    /// Remove it
    Remove(InnerRemove),
}

#[derive(Args, Debug, PartialEq)]
struct InnerAdd {
    /// Where it lives
    url: String,
    /// Fetch it straight away
    #[usage(long)]
    fetch: bool,
}

#[derive(Args, Debug, PartialEq)]
struct InnerRemove {
    /// Which one
    which: String,
}

#[test]
fn a_nested_subcommand_merges_at_every_level() {
    let a = argv(["remote", "--name", "origin", "add", "git://x", "--fetch"]);
    let mut nest = NestUpd::parse_from(&a).expect("valid");

    let a = argv(["remote", "add", "git://y"]);
    nest.try_update_from(&a).expect("the same path");

    let Outer::Remote(remote) = &nest.command;
    assert_eq!(remote.name.as_deref(), Some("origin"), "outer field kept");
    assert_eq!(
        remote.command,
        Inner::Add(InnerAdd {
            url: "git://y".into(),
            fetch: true,
        }),
        "the inner command merged rather than being rebuilt",
    );
}

#[test]
fn a_nested_subcommand_switch_replaces_only_the_inner_variant() {
    let a = argv(["remote", "--name", "origin", "add", "git://x", "--fetch"]);
    let mut nest = NestUpd::parse_from(&a).expect("valid");

    let a = argv(["remote", "remove", "origin"]);
    nest.try_update_from(&a).expect("a different inner variant");

    let Outer::Remote(remote) = &nest.command;
    assert_eq!(remote.name.as_deref(), Some("origin"), "outer field kept");
    assert_eq!(
        remote.command,
        Inner::Remove(InnerRemove {
            which: "origin".into(),
        }),
    );
}

/// How to emit output — required on the holding command (bare `Mode`, not `Option`).
#[derive(ArgGroup, Debug, PartialEq)]
enum Mode {
    /// Emit JSON
    Json,
    /// Emit YAML
    Yaml,
}

/// A CLI whose arg group is required by type, plus a sibling that names a member.
#[derive(Cli, Debug, PartialEq)]
#[usage(bin = "modeupd")]
struct ModeUpd {
    #[usage(arg_group)]
    mode: Mode,
    /// Only legal beside JSON
    #[usage(long, requires = "--json")]
    pretty: bool,
    /// Cannot sit beside YAML
    #[usage(long, conflicts = "--yaml")]
    strict: bool,
    /// Something to change so an update has a reason to run
    #[usage(long)]
    tag: Option<String>,
}

#[test]
fn a_standing_required_arg_group_need_not_be_given_again() {
    let a = argv(["--json"]);
    let mut upd = ModeUpd::parse_from(&a).expect("first parse selects the group");

    // A bare `Mode` is required; this argv says nothing about the group. Standing must answer.
    let a = argv(["--tag", "second"]);
    upd.try_update_from(&a)
        .expect("required group already stands");

    assert_eq!(upd.mode, Mode::Json);
    assert_eq!(upd.tag.as_deref(), Some("second"));
}

#[test]
fn a_standing_group_member_satisfies_a_sibling_requires() {
    let a = argv(["--json"]);
    let mut upd = ModeUpd::parse_from(&a).expect("json stands");

    let a = argv(["--pretty"]);
    upd.try_update_from(&a)
        .expect("standing --json satisfies requires");

    assert_eq!(upd.mode, Mode::Json);
    assert!(upd.pretty);
}

/// A CLI whose value-conditional rules name a group member rather than a plain flag.
#[derive(Cli, Debug, PartialEq)]
#[usage(bin = "equpd")]
struct EqUpd {
    #[usage(arg_group)]
    mode: Mode,
    /// Needed once JSON is the mode
    #[usage(long, required_if_eq("--json", "true"))]
    out: Option<String>,
    /// Something to change so an update has a reason to run
    #[usage(long)]
    tag: Option<String>,
}

#[test]
fn a_standing_group_member_still_triggers_required_if_eq() {
    // `--json` stands and `--out` was given, so the first parse is complete.
    let a = argv(["--json", "--out", "here"]);
    let mut upd = EqUpd::parse_from(&a).expect("json with an out");
    assert_eq!(upd.out.as_deref(), Some("here"));

    // Standing `--out` keeps satisfying the condition the standing `--json` imposes.
    let a = argv(["--tag", "second"]);
    upd.try_update_from(&a).expect("both sides still stand");
    assert_eq!(upd.mode, Mode::Json);
    assert_eq!(upd.tag.as_deref(), Some("second"));

    // And the condition is read from the standing member rather than skipped: with
    // `--yaml` standing, switching the mode to JSON without an `--out` is refused.
    let a = argv(["--yaml", "--out", "here"]);
    let mut upd = EqUpd::parse_from(&a).expect("yaml needs no out");
    let a = argv(["--json"]);
    assert!(
        upd.try_update_from(&a).is_ok(),
        "the standing --out answers the new --json",
    );
    assert_eq!(upd.mode, Mode::Json);
}

#[test]
fn a_standing_group_member_conflicts_with_a_sibling_flag() {
    let a = argv(["--yaml"]);
    let mut upd = ModeUpd::parse_from(&a).expect("yaml stands");

    let a = argv(["--strict"]);
    assert!(
        matches!(
            upd.try_update_from(&a),
            Err(Error::ConflictingFlags {
                name: "strict",
                other: "yaml",
            })
        ),
        "standing --yaml still conflicts with --strict",
    );
}

/// A counted `u8` whose presence is "not the default". The generated
/// `!= Default::default()` must name `u8`, or `serde_json`'s `PartialEq<Value> for u8`
/// makes the comparison ambiguous — hk's CLI hit that on every build that pulls
/// `serde_json` into the same crate.
#[derive(Cli, Debug, PartialEq)]
#[usage(bin = "counted", completion)]
struct Counted {
    #[usage(short, long, global, count, overrides("--quiet"))]
    verbose: u8,
    #[usage(short, long, global, overrides("--verbose"))]
    quiet: bool,
    #[usage(subcommand)]
    command: CountedCmd,
}

#[derive(Subcommands, Debug, PartialEq)]
enum CountedCmd {
    Run,
}

#[test]
fn a_standing_count_compiles_beside_serde_json() {
    // Keep `serde_json` live in this module so its `PartialEq` impls stay in scope.
    let _ = serde_json::json!({"n": 1u8});

    let a = argv(["-vv", "run"]);
    let mut counted = Counted::parse_from(&a).expect("two -v");
    assert_eq!(counted.verbose, 2);

    // A second line that says nothing about `-v` must keep the standing count —
    // presence is "not the default", and that comparison has to name `u8`.
    let a = argv(["run"]);
    counted
        .try_update_from(&a)
        .expect("the standing count answers for itself");
    assert_eq!(counted.verbose, 2);
    assert!(!counted.quiet);
}
