//! Flags a command inherits, listed where they can be used.
//!
//! `communique generate` accepts `--config`, `--verbose` and `--quiet` from its root, and its
//! page mentioned none of them — a flag a user can type and cannot discover, which is the worst
//! way for help to be wrong.
//!
//! Under a heading of their own rather than mixed in, which is where this differs from clap on
//! purpose: `--config` belongs to the program and not to `generate`, and a reader should be
//! able to see which is which.

use usage_argv::help;
use usage_derive::{Args, Cli, Subcommands};

/// Read something back
#[derive(Args)]
struct Get {
    /// Only this command has one
    #[usage(long)]
    plain: bool,
    /// Declared here as well as on the root, which the parser resolves in this one's favour
    #[usage(long)]
    raw: bool,
}

/// Settings, which nest
#[derive(Args)]
struct Config {
    /// A global of its own, one level down
    #[usage(long, global)]
    file: Option<String>,
    #[usage(subcommand)]
    command: Option<Inner>,
}

#[derive(Subcommands)]
enum Inner {
    /// Read something back
    Get(Box<Get>),
}

#[derive(Subcommands)]
enum Command {
    /// Settings, which nest
    Config(Box<Config>),
    /// Claims one of the root's two spellings
    Claimer(Box<Claimer>),
    /// Claims a spelling with a flag nobody can see
    Quiet(Box<Quiet>),
}

/// Claims a spelling with a flag nobody can see
#[derive(Args)]
struct Quiet {
    /// Hidden, and still binds — which is what makes it shadow
    #[usage(long = "raw", hide)]
    raw: bool,
}

/// Claims one of the root's two spellings for itself
#[derive(Args)]
struct Claimer {
    /// A `-v` of its own, which is not the root's
    #[usage(short = 'v')]
    level: Option<String>,
}

/// A tool with flags at every level
#[derive(Cli)]
#[usage(bin = "ex")]
struct Ex {
    /// Say more
    #[usage(long, short = 'v', global)]
    verbose: bool,
    /// Read and write directly
    #[usage(long, global)]
    raw: bool,
    /// Not global, so it belongs to the root alone
    #[usage(long)]
    root_only: bool,
    #[usage(subcommand)]
    command: Option<Command>,
}

/// The listing part of a page: everything from the first `Flags:` heading on.
///
/// The usage line names flags too — `Usage: ex config get [--plain] [--raw]` — and an assertion
/// about what is *listed* must not count it.
fn listing(page: &str) -> &str {
    page.split_once("\nFlags:")
        .map(|(_, rest)| rest)
        .unwrap_or(page)
}

fn page_of(names: &[&str], long: bool) -> String {
    let mut cmd = Ex::spec().root.cmd;
    for name in names {
        cmd = cmd
            .subcommands
            .iter()
            .find(|c| c.name == *name)
            .unwrap_or_else(|| panic!("no {name}"));
    }
    help::render(Ex::spec(), cmd, long).expect("a page")
}

#[test]
fn a_page_lists_what_it_inherits_under_its_own_heading() {
    for long in [false, true] {
        let page = page_of(&["config", "get"], long);
        let (own, global) = page
            .split_once("Global flags:")
            .unwrap_or_else(|| panic!("long={long}: no global section:\n{page}"));

        // The command's own, above the rule.
        assert!(own.contains("--plain"), "long={long}: {page}");

        // What it inherits, below it — from both ancestors, not just the nearest.
        assert!(global.contains("--verbose"), "long={long}: {page}");
        assert!(global.contains("--file"), "long={long}: {page}");
    }
}

#[test]
fn a_flag_the_command_declares_itself_is_not_listed_twice() {
    // The parser looks a command's own flags up before its ancestors' and takes the first
    // match, so `ex config get --raw` is *get's* `--raw` and never the root's. Listing both
    // would print two descriptions for one spelling, one of which can never apply.
    for long in [false, true] {
        let page = page_of(&["config", "get"], long);
        assert_eq!(
            listing(&page).matches("--raw").count(),
            1,
            "long={long}: `--raw` should appear once:\n{page}"
        );
        assert!(
            // A short phrase on purpose: the long page wraps this description, so anything
            // longer would be split across lines and fail for the wrong reason.
            page.contains("Declared here as well"),
            "long={long}: and it should be the command's own: {page}"
        );
    }
}

#[test]
fn a_flag_that_is_not_global_stays_where_it_was_declared() {
    // `--root-only` is the root's and is not inherited, so a descendant must not offer it —
    // the parser would refuse it there.
    for long in [false, true] {
        let page = page_of(&["config", "get"], long);
        assert!(!page.contains("--root-only"), "long={long}: {page}");
    }
}

#[test]
fn the_programs_own_page_has_no_global_section() {
    // The root's flags are the root's, `global` or not: the heading is about provenance
    // relative to *this* page, and there is nowhere above the root to inherit from.
    for long in [false, true] {
        let page = page_of(&[], long);
        assert!(!page.contains("Global flags:"), "long={long}: {page}");
        assert!(page.contains("--verbose"), "long={long}: {page}");
        assert!(page.contains("--root-only"), "long={long}: {page}");
    }
}

#[test]
fn both_sections_share_one_column() {
    // So the page reads as one table with a rule through it rather than two that happen to be
    // adjacent. The width also drives where a wrapped description resumes, so it cannot be
    // decided per section.
    let page = page_of(&["config", "get"], true);
    let column = |needle: &str, help: &str| {
        let line = listing(&page)
            .lines()
            .find(|l| l.contains(needle))
            .unwrap_or_else(|| panic!("no line for {needle}:\n{page}"));
        line.find(help)
            .unwrap_or_else(|| panic!("no help on {line:?}"))
    };
    assert_eq!(
        column("--plain", "Only this command"),
        column("--verbose", "Say more"),
        "own and inherited should start in one column:\n{page}"
    );
}

#[test]
fn the_fields_are_bound() {
    use std::ffi::OsStr;
    let argv = [
        "--verbose",
        "config",
        "--file",
        "f",
        "get",
        "--plain",
        "--raw",
    ]
    .map(OsStr::new);
    let ex = Ex::parse_from(&argv).expect("should parse");
    assert!(ex.verbose && !ex.raw && !ex.root_only);
    let Some(Command::Config(config)) = ex.command else {
        panic!("expected config")
    };
    assert_eq!(config.file.as_deref(), Some("f"));
    let Some(Inner::Get(get)) = config.command else {
        panic!("expected get")
    };
    assert!(
        get.plain && get.raw,
        "the command's own `--raw` is the one that binds"
    );
}

#[test]
fn claiming_one_spelling_leaves_the_other_on_offer() {
    // `partial` declares its own `-v`; the root's global is `-v, --verbose`. The parser still
    // binds `--verbose` there, so dropping the whole inherited entry made a working name
    // undiscoverable. What survives is offered, and what was claimed is not.
    for long in [false, true] {
        let page = page_of(&["claimer"], long);
        let (own, global) = page
            .split_once("Global flags:")
            .unwrap_or_else(|| panic!("long={long}: {page}"));
        assert!(own.contains("-v <LEVEL>"), "long={long}: {page}");
        assert!(global.contains("--verbose"), "long={long}: {page}");
        assert!(
            !global.contains("-v, --verbose"),
            "long={long}: `-v` is the subcommand's here: {page}"
        );
    }

    // And the parser agrees, which is the whole point of matching it.
    use std::ffi::OsStr;
    let argv = ["claimer", "--verbose"].map(OsStr::new);
    assert!(
        Ex::parse_from(&argv).is_ok(),
        "the root's long form still binds"
    );
}

#[test]
fn a_hidden_flag_still_shadows() {
    // `hide` keeps a flag off the page; the parser still binds it. usage-lib counted hidden
    // own flags when deciding what an ancestor could still offer and this did not, so the two
    // renderers disagreed wherever a hidden local shared a spelling with an inherited global.
    // Read the *visible* own flags only, while usage-lib read all of them — so the two
    // renderers disagreed wherever a hidden local shared a spelling with an inherited global,
    // and the page offered a `--raw` that binds something the reader cannot see.
    for long in [false, true] {
        let page = page_of(&["quiet"], long);
        assert!(
            !page.contains("--raw"),
            "long={long}: a hidden local claims this spelling: {page}"
        );
        // The other inherited globals are unaffected — this is about one spelling.
        assert!(page.contains("--verbose"), "long={long}: {page}");
    }
}

#[test]
fn the_claimer_fields_are_bound() {
    use std::ffi::OsStr;
    let argv = ["claimer", "-v", "3"].map(OsStr::new);
    let ex = Ex::parse_from(&argv).expect("should parse");
    let Some(Command::Claimer(p)) = ex.command else {
        panic!("expected claimer")
    };
    assert_eq!(p.level.as_deref(), Some("3"));
}

#[test]
fn the_quiet_fields_are_bound() {
    use std::ffi::OsStr;
    let argv = ["quiet", "--raw"].map(OsStr::new);
    let ex = Ex::parse_from(&argv).expect("should parse");
    let Some(Command::Quiet(q)) = ex.command else {
        panic!("expected quiet")
    };
    assert!(q.raw, "a hidden flag still binds, which is why it shadows");
}
