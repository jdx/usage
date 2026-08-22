//! Handing a parsed command to the code that carries it out.
//!
//! The `match` over a subcommand enum is the one part of a CLI that every adopter writes and
//! nobody varies: one arm per command, each calling the one function that command exists for.
//! `#[usage(run)]` generates it from the same declaration the parser and the spec come from,
//! so an arm cannot route to the wrong handler and a new command cannot be forgotten — the
//! match is exhaustive because it is generated.
//!
//! Nothing about it reaches the spec, which is the point these tests hold: a dispatched CLI
//! emits exactly the KDL an undispatched one does.

use std::ffi::OsStr;

use usage_argv::{Run, RunWith};
use usage_derive::{Args, Cli, Subcommands};

/// Install a tool
#[derive(Args)]
struct Install {
    /// Overwrite what is there
    #[usage(long)]
    force: bool,
    /// What to install
    tools: Vec<String>,
}

/// Show who pays for this
#[derive(Args)]
struct Sponsors;

/// List the configuration
#[derive(Args)]
struct ConfigLs {
    /// Leave the header off
    #[usage(long)]
    no_header: bool,
}

/// Work with the configuration
#[derive(Subcommands)]
#[usage(run, run_with)]
enum ConfigCommand {
    /// List the configuration
    Ls(ConfigLs),
}

/// Work with the configuration
#[derive(Args)]
#[usage(run, run_with)]
struct Config {
    #[usage(subcommand)]
    command: ConfigCommand,
}

#[derive(Subcommands)]
#[usage(run, run_with)]
enum Command {
    /// Install a tool
    Install(Box<Install>),
    /// Show who pays for this
    Sponsors(Sponsors),
    /// Work with the configuration
    Config(Config),
}

/// A tool that does things
#[derive(Cli)]
#[usage(bin = "ex")]
struct Ex {
    /// Print more
    #[usage(short = 'v', long, global)]
    verbose: bool,
    #[usage(subcommand)]
    command: Command,
}

// What each command does when it is run. The output type is the first variant's and every
// other command has to agree, which is what makes one of these returning something else a
// compile error rather than a mismatch inside a generated arm.
impl Run for Install {
    type Output = Result<String, String>;
    fn run(self) -> Self::Output {
        if self.force {
            Ok(format!("install --force {}", self.tools.join(",")))
        } else {
            Err(format!("refusing to install {}", self.tools.join(",")))
        }
    }
}

impl Run for Sponsors {
    type Output = Result<String, String>;
    fn run(self) -> Self::Output {
        Ok("sponsors".to_string())
    }
}

impl Run for ConfigLs {
    type Output = Result<String, String>;
    fn run(self) -> Self::Output {
        Ok(format!("config ls no_header={}", self.no_header))
    }
}

/// What a CLI hands its commands, when it hands them anything.
#[derive(Default)]
struct Log {
    lines: Vec<String>,
}

impl RunWith<&mut Log> for Install {
    type Output = Result<String, String>;
    fn run_with(self, log: &mut Log) -> Self::Output {
        log.lines.push("install".to_string());
        Ok(format!("install force={}", self.force))
    }
}

impl RunWith<&mut Log> for Sponsors {
    type Output = Result<String, String>;
    fn run_with(self, log: &mut Log) -> Self::Output {
        log.lines.push("sponsors".to_string());
        Ok("sponsors".to_string())
    }
}

impl RunWith<&mut Log> for ConfigLs {
    type Output = Result<String, String>;
    fn run_with(self, log: &mut Log) -> Self::Output {
        log.lines.push("config ls".to_string());
        Ok(format!("config ls no_header={}", self.no_header))
    }
}

fn parse(words: &[&str]) -> Ex {
    let argv: Vec<&OsStr> = words.iter().map(OsStr::new).collect();
    Ex::parse_from(&argv).expect("valid command line")
}

#[test]
fn the_selected_command_is_the_one_that_runs() {
    let ex = parse(&["install", "--force", "node", "python"]);
    assert_eq!(
        ex.command.run(),
        Ok("install --force node,python".to_string())
    );
}

#[test]
fn a_commands_own_failure_is_its_output_not_a_parse_error() {
    let ex = parse(&["install", "node"]);
    assert_eq!(
        ex.command.run(),
        Err("refusing to install node".to_string())
    );
}

/// A `Box` is how the variant holds the struct, and the struct is what implements the trait,
/// so a boxed command dispatches like any other. mise boxes its largest commands.
#[test]
fn a_boxed_variant_dispatches() {
    let ex = parse(&["install", "--force"]);
    assert!(ex.command.run().is_ok());
}

/// A command with nothing to parse still has work to do.
#[test]
fn a_command_with_no_arguments_dispatches() {
    let ex = parse(&["sponsors"]);
    assert_eq!(ex.command.run(), Ok("sponsors".to_string()));
}

/// The group in the middle — `ex config ls` — where the enum's dispatch reaches a struct
/// whose own dispatch forwards to the next enum. Neither level is written by hand.
#[test]
fn a_nested_command_dispatches_through_its_group() {
    let ex = parse(&["config", "ls", "--no-header"]);
    assert_eq!(ex.command.run(), Ok("config ls no_header=true".to_string()));
}

/// The root declares a global flag, so it decides for itself what to do with it before
/// dispatching — which is why a struct with arguments of its own implements the trait rather
/// than getting a generated forward.
#[test]
fn a_root_with_flags_of_its_own_dispatches_after_reading_them() {
    let ex = parse(&["--verbose", "sponsors"]);
    assert!(ex.verbose);
    assert_eq!(ex.command.run(), Ok("sponsors".to_string()));
}

#[test]
fn a_context_reaches_the_command_that_ran() {
    let mut log = Log::default();
    let ex = parse(&["install", "--force"]);
    assert_eq!(
        ex.command.run_with(&mut log),
        Ok("install force=true".to_string())
    );
    assert_eq!(log.lines, ["install"]);
}

#[test]
fn a_context_reaches_a_nested_command() {
    let mut log = Log::default();
    let ex = parse(&["config", "ls"]);
    assert_eq!(
        ex.command.run_with(&mut log),
        Ok("config ls no_header=false".to_string())
    );
    assert_eq!(log.lines, ["config ls"]);
}

/// Both dispatches on one enum: an invocation with a context and one without are the same
/// command set, and a CLI part-way through adopting a context needs both to exist at once.
#[test]
fn one_enum_dispatches_with_and_without_a_context() {
    let mut log = Log::default();
    assert!(parse(&["sponsors"]).command.run().is_ok());
    assert!(parse(&["sponsors"]).command.run_with(&mut log).is_ok());
    assert_eq!(log.lines, ["sponsors"]);
}

/// Which Rust function carries out a command is not part of what the CLI *is*, so `run` says
/// nothing in the emitted spec — the same rule `#[usage(skip)]` follows. This is the check
/// that keeps a dispatch attribute from quietly becoming spec surface.
#[test]
fn dispatch_says_nothing_in_the_spec() {
    let kdl = Ex::to_kdl();
    assert!(kdl.contains("cmd install"), "{kdl}");
    assert!(kdl.contains("cmd config"), "{kdl}");
    assert!(!kdl.contains("run"), "{kdl}");
}

/// A unit variant and an inline variant still have a type to implement `Run` on: the derive
/// writes `{Enum}{Variant}` for them.
#[derive(Subcommands)]
#[usage(run)]
enum Shape {
    /// Greet
    Hi,
    /// Add a path
    Add { path: String },
}

impl Run for ShapeHi {
    type Output = String;
    fn run(self) -> Self::Output {
        "hi".to_string()
    }
}

impl Run for ShapeAdd {
    type Output = String;
    fn run(self) -> Self::Output {
        self.path
    }
}

/// A tool whose commands are declared inline
#[derive(Cli)]
#[usage(bin = "shape", run)]
struct ShapeCli {
    /// Print more
    #[usage(short = 'v', long)]
    verbose: bool,
    #[usage(subcommand)]
    command: Shape,
}

#[test]
fn a_unit_variant_dispatches_through_the_struct_written_for_it() {
    let argv = [OsStr::new("hi")];
    let cli = ShapeCli::parse_from(&argv).expect("valid command line");
    assert!(!cli.verbose);
    assert_eq!(cli.run_command(), "hi");
}

#[test]
fn an_inline_variant_dispatches_through_the_struct_written_for_it() {
    let argv = [OsStr::new("add"), OsStr::new("src")];
    let cli = ShapeCli::parse_from(&argv).expect("valid command line");
    assert_eq!(cli.run_command(), "src");
}

#[test]
fn a_root_with_flags_reads_them_then_run_command() {
    let argv = [OsStr::new("--verbose"), OsStr::new("hi")];
    let cli = ShapeCli::parse_from(&argv).expect("valid command line");
    assert!(cli.verbose);
    assert_eq!(cli.run_command(), "hi");
}

fn fallback(argv: Vec<String>) -> String {
    format!("ext {}", argv.join(" "))
}

/// Ping
#[derive(Args)]
struct Ping;

impl Run for Ping {
    type Output = String;
    fn run(self) -> Self::Output {
        "pong".to_string()
    }
}

#[derive(Subcommands)]
#[usage(run, external = fallback, output = String)]
enum Catch {
    /// Ping
    Ping(Ping),
    #[usage(external_subcommand)]
    Other(Vec<String>),
}

/// A tool with a catch-all
#[derive(Cli)]
#[usage(bin = "catch")]
struct CatchCli {
    #[usage(subcommand)]
    command: Catch,
}

#[test]
fn an_external_subcommand_calls_the_named_fallback() {
    let ping = [OsStr::new("ping")];
    let cli = CatchCli::parse_from(&ping).expect("valid command line");
    assert_eq!(cli.command.run(), "pong");

    let extra = [OsStr::new("git"), OsStr::new("status")];
    let cli = CatchCli::parse_from(&extra).expect("valid command line");
    assert_eq!(cli.command.run(), "ext git status");
}

/// Show the version
#[derive(Args)]
struct Version;

/// Get a value
#[derive(Args)]
struct Get {
    /// What to get
    key: String,
}

impl Run for Version {
    type Output = String;
    fn run(self) -> Self::Output {
        "1".to_string()
    }
}

impl RunWith<&str> for Get {
    type Output = String;
    fn run_with(self, ctx: &str) -> Self::Output {
        format!("{ctx}:{}", self.key)
    }
}

#[derive(Subcommands)]
#[usage(run_with)]
enum MixedCtx {
    /// Show the version
    #[usage(no_ctx)]
    Version(Version),
    /// Get a value
    Get(Get),
}

/// A tool that loads context only for some commands
#[derive(Cli)]
#[usage(bin = "mixed-ctx")]
struct MixedCtxCli {
    #[usage(subcommand)]
    command: MixedCtx,
}

#[test]
fn a_command_can_skip_the_context_the_rest_of_the_enum_takes() {
    let argv = [OsStr::new("version")];
    let cli = MixedCtxCli::parse_from(&argv).expect("valid command line");
    assert_eq!(cli.command.run_with("loaded"), "1");

    let argv = [OsStr::new("get"), OsStr::new("k")];
    let cli = MixedCtxCli::parse_from(&argv).expect("valid command line");
    assert_eq!(cli.command.run_with("loaded"), "loaded:k");
}

#[test]
fn a_skipped_context_is_not_built_when_loaded_lazily() {
    let argv = [OsStr::new("version")];
    let cli = MixedCtxCli::parse_from(&argv).expect("valid command line");
    let mut built = false;
    assert_eq!(
        cli.command.run_with_lazy(|| {
            built = true;
            "loaded"
        }),
        "1"
    );
    assert!(!built);

    let argv = [OsStr::new("get"), OsStr::new("k")];
    let cli = MixedCtxCli::parse_from(&argv).expect("valid command line");
    built = false;
    assert_eq!(
        cli.command.run_with_lazy(|| {
            built = true;
            "loaded"
        }),
        "loaded:k"
    );
    assert!(built);
}
