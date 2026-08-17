//! Where a derive can be written, and what it may be sitting next to.
//!
//! The generated tables used to live in a `mod` of their own, and a module cannot see the scope
//! it was written in — so every reference to the user's own type said `super::`, and the derive
//! carried a small path-rewriting pass to put it there (`self::` shifted by one, `super::` by
//! two, `crate::` and `::` left alone).
//!
//! Two things followed from that, and both are gone now that the tables are emitted in a
//! `const _: () = { … }` instead — a const block *is* the surrounding scope, so a name means
//! what the author meant:
//!
//! 1. A derive in a function body could not compile at all. `super` from a module nested in a
//!    `fn` is the enclosing *module*, not the body, so the function-local types were invisible
//!    and the error named a generated identifier the author never wrote.
//! 2. The rewriting had to be right at every call site, and getting it wrong was not an error
//!    here but in the adopter's crate, at a line they did not write.
//!
//! What replaced it needs its own guard, because a const block shares the scope rather than
//! escaping it: whatever the generated code names unqualified, it must name a user type. So the
//! tables are written in fully-qualified form and import nothing.

use std::ffi::OsStr;

use usage_derive::{Args, Cli, Subcommands};

/// A completer at module scope, named unqualified and through `self::` by `Paint` below
fn colours(
    _partial: &<Paint as usage_argv::spec::CommandArgs>::Partial,
    _ctx: &usage_argv::complete::CompleteCtx<'_>,
) -> Vec<usage_argv::complete::Candidate<'static>> {
    vec![usage_argv::complete::Candidate::new("red")]
}

#[test]
fn a_derive_may_be_written_in_a_function_body() {
    // Every shape at once, all local: the flattened struct, the subcommand enum, both variant
    // forms, and the root. Before the const block this did not compile — the first error was
    // `cannot find type __Usage5LocalInner in module super`.
    #[derive(Args)]
    struct Common {
        #[usage(long)]
        verbose: bool,
    }

    #[derive(Args)]
    struct Inner {
        #[usage(long)]
        thing: bool,
    }

    #[derive(Subcommands)]
    enum Local {
        /// Wrapping
        Inner(Inner),
        /// Bare
        Bare,
    }

    #[derive(Cli)]
    #[usage(bin = "lo", version = "1.0")]
    struct Lo {
        #[usage(flatten)]
        common: Common,
        #[usage(subcommand)]
        command: Option<Local>,
    }

    assert_eq!(Lo::spec().name, "lo");
    let names: Vec<&str> = Lo::command().subcommands.iter().map(|c| c.name).collect();
    assert_eq!(names, ["inner", "bare"]);

    let argv = [
        OsStr::new("--verbose"),
        OsStr::new("inner"),
        OsStr::new("--thing"),
    ];
    let parsed = Lo::parse_from(&argv).expect("a local CLI parses");
    assert!(parsed.common.verbose, "the flattened struct bound");
    assert!(
        matches!(parsed.command, Some(Local::Inner(ref i)) if i.thing),
        "the local enum routed"
    );
}

/// A subcommand enum named exactly like one of usage-argv's own types.
///
/// The tables are emitted beside the user's types now, so anything the generated code imported
/// would shadow them. `Command` is the sharpest case — it is what the tables are *made of* —
/// and it resolved to `::usage_argv::Command` the moment the two shared a scope. Every other
/// name in the tables (`Flag`, `Arg`, `DoubleDash`, `Spec`, the three `*Meta`s) is one an
/// adopter could equally have taken.
#[derive(Args)]
struct Run {
    #[usage(long)]
    quiet: bool,
}

#[derive(Subcommands)]
enum Command {
    /// Run it
    Run(Run),
}

#[allow(dead_code)]
struct Flag;
#[allow(dead_code)]
struct Arg;
#[allow(dead_code)]
struct Spec;
#[allow(dead_code)]
struct DoubleDash;

#[derive(Cli)]
#[usage(bin = "shadow")]
struct Shadow {
    #[usage(subcommand)]
    command: Option<Command>,
}

#[test]
fn a_user_type_may_be_named_after_one_of_ours() {
    let argv = [OsStr::new("run"), OsStr::new("--quiet")];
    let parsed = Shadow::parse_from(&argv).expect("parses");
    assert!(matches!(parsed.command, Some(Command::Run(ref r)) if r.quiet));
}

#[derive(Args)]
struct Paint {
    /// Completed by a function this file declares, named the way the author wrote it
    #[usage(long, complete = colours)]
    colour: Option<String>,
    /// And one named through `self::`, which used to be shifted to `super::`
    #[usage(long, complete = self::colours)]
    other: Option<String>,
}

#[derive(Subcommands)]
enum FarCommands {
    /// Paint it
    Paint(Paint),
}

#[derive(Cli)]
#[usage(bin = "far", completion)]
struct Far {
    #[usage(subcommand)]
    command: Option<FarCommands>,
}

#[test]
fn a_path_means_what_it_meant_where_it_was_written() {
    // The rewriting pass existed for exactly these two spellings; identity is now the whole
    // rule, so what proves it is that both still resolve and answer.
    let ask = |line: &str| {
        let argv: Vec<std::ffi::OsString> =
            ["__complete_word__", "--shell", "bash", "--line", line]
                .iter()
                .map(std::ffi::OsString::from)
                .collect();
        Far::completion_request(&argv).expect("a completion request")
    };
    assert!(ask("far paint --colour ").contains("red"), "unqualified");
    assert!(ask("far paint --other ").contains("red"), "self-qualified");

    // And the same flags bind, which is also what keeps the fields read: a completer runs
    // against a partial and never builds the struct, so without this the fixture is dead code
    // in the adopter's crate — a warning nobody there can silence.
    let argv = [
        OsStr::new("paint"),
        OsStr::new("--colour"),
        OsStr::new("red"),
        OsStr::new("--other"),
        OsStr::new("blue"),
    ];
    let parsed = Far::parse_from(&argv).expect("parses");
    let Some(FarCommands::Paint(paint)) = parsed.command else {
        panic!("routed to paint")
    };
    assert_eq!(paint.colour.as_deref(), Some("red"));
    assert_eq!(paint.other.as_deref(), Some("blue"));
}
