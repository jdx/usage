//! What a command does to the world, declared beside the command.
//!
//! Not something clap can express, so a CLI that wants it keeps a side table keyed by command
//! path and applies it to the generated spec afterwards. communique's `command_effects.rs` is
//! two hundred lines of exactly that, and mise has the same file for the same reason.
//!
//! The spec's rule, which this only records: an invocation's effect is the highest of the
//! command's and of every flag supplied, so a flag can raise what a command does and never
//! lower it.

use usage::Spec as LibSpec;
use usage_derive::{Args, Cli, Subcommands};

/// Generate release notes for a git tag
#[derive(Args)]
#[usage(effect = "read")]
struct Generate {
    /// Push editorialized notes to the GitHub release
    #[usage(long, effect = "write")]
    github_release: bool,
    /// Generate notes without updating GitHub
    #[usage(long)]
    dry_run: bool,
}

/// Generate a config file in the repo root
#[derive(Args)]
#[usage(effect = "write")]
struct Init {
    /// Overwrite an existing config file
    #[usage(long, effect = "destructive")]
    force: bool,
}

/// Say nothing about what it does
#[derive(Args)]
struct Undeclared {
    #[usage(long)]
    plain: bool,
}

#[derive(Subcommands)]
enum Command {
    /// Generate release notes for a git tag
    Generate(Box<Generate>),
    /// Generate a config file in the repo root
    Init(Box<Init>),
    /// Say nothing about what it does
    Undeclared(Box<Undeclared>),
}

/// Editorialized release notes powered by AI
#[derive(Cli)]
#[usage(bin = "communique", name = "communique")]
struct Cli_ {
    #[usage(subcommand)]
    command: Option<Command>,
}

#[test]
fn a_command_says_what_it_does_to_the_world() {
    // Through the spec, because that is what a consumer reads: the point of declaring it is that
    // something downstream — a confirmation prompt, an audit, a `--dry-run` wrapper — can tell
    // `init` from `generate` without knowing either.
    let spec: LibSpec = Cli_::to_kdl().parse().expect("valid spec");
    let effect = |name: &str| {
        spec.cmd
            .subcommands
            .get(name)
            .unwrap_or_else(|| panic!("{name}"))
            .effect
    };
    assert_eq!(effect("generate"), Some(usage::SpecCommandEffect::Read));
    assert_eq!(effect("init"), Some(usage::SpecCommandEffect::Write));

    // Unsaid is not "safe". A consumer treats the absence as "ask", so leaving it off has to
    // stay distinguishable from declaring `read` — which is why this is an `Option`.
    assert_eq!(effect("undeclared"), None);
}

#[test]
fn a_flag_can_raise_what_its_command_does() {
    // `generate` only prints; `generate --github-release` writes. The spec takes an invocation's
    // effect to be the highest of the command's and of every flag given, so the flag is where
    // that difference belongs — and a table keyed by command path cannot say it at all.
    let spec: LibSpec = Cli_::to_kdl().parse().expect("valid spec");
    let generate = spec.cmd.subcommands.get("generate").expect("generate");
    let flag = |name: &str| {
        generate
            .flags
            .iter()
            .find(|f| f.name == name)
            .unwrap_or_else(|| panic!("{name}"))
            .effect
    };
    assert_eq!(
        flag("github-release"),
        Some(usage::SpecCommandEffect::Write)
    );

    // And a flag that changes nothing about what the command does says nothing.
    assert_eq!(flag("dry-run"), None);

    let init = spec.cmd.subcommands.get("init").expect("init");
    assert_eq!(
        init.flags
            .iter()
            .find(|f| f.name == "force")
            .unwrap()
            .effect,
        Some(usage::SpecCommandEffect::Destructive)
    );
}

#[test]
fn the_fields_are_bound() {
    // Keeps every declared field read, which CI requires of a test CLI.
    use std::ffi::OsStr;
    let argv = ["init", "--force"].map(OsStr::new);
    let cli = Cli_::parse_from(&argv).expect("should parse");
    let Some(Command::Init(init)) = cli.command else {
        panic!("expected init")
    };
    assert!(init.force);

    let argv = ["generate", "--github-release", "--dry-run"].map(OsStr::new);
    let cli = Cli_::parse_from(&argv).expect("should parse");
    let Some(Command::Generate(g)) = cli.command else {
        panic!("expected generate")
    };
    assert!(g.github_release && g.dry_run);

    let argv = ["undeclared", "--plain"].map(OsStr::new);
    let cli = Cli_::parse_from(&argv).expect("should parse");
    let Some(Command::Undeclared(u)) = cli.command else {
        panic!("expected undeclared")
    };
    assert!(u.plain);
}
