//! A derive that compiles a CLI definition into parse tables and a spec.
//!
//! `#[derive(usage::Cli)]` reads a struct and emits three things: `static` parse
//! tables for [usage-argv](https://docs.rs/usage-argv), `static` metadata for
//! spec emission, and a parse function that assigns values straight into the
//! struct's fields. Nothing is constructed at run time — there is no command tree
//! to build before a parse can start — and a successful parse touches only the
//! first of the three.
//!
//! Not compiled here, because this crate deliberately does not depend on
//! usage-argv — see the note in its `Cargo.toml`. The same example runs as a test
//! in `conformance/tests/derive.rs`, as `the_crate_level_example_from_the_docs`.
//!
//! ```ignore
//! # use usage_derive::Cli;
//! /// A tool that does things
//! #[derive(Cli)]
//! #[usage(bin = "ex", version = "1.0")]
//! struct Cli {
//!     /// How many jobs to run at once
//!     #[usage(short = 'j', long, env = "EX_JOBS", default = "4")]
//!     jobs: Option<String>,
//!
//!     /// Print more
//!     #[usage(short = 'v', long, count)]
//!     verbose: u8,
//!
//!     /// Colorize output
//!     #[usage(long, negate = "--no-color", default = "true")]
//!     color: bool,
//!
//!     /// Files to process
//!     files: Vec<String>,
//! }
//!
//! let argv = ["-j8", "--no-color", "a.txt"].map(std::ffi::OsStr::new);
//! let cli = Cli::parse_from(&argv).unwrap();
//! assert_eq!(cli.jobs.as_deref(), Some("8"));
//! assert!(!cli.color);
//! assert_eq!(cli.files, ["a.txt"]);
//!
//! // The same declaration is also the spec, which is what generates docs,
//! // manpages, and completions.
//! assert!(Cli::to_kdl().contains(r#"flag "-j --jobs""#));
//! ```
//!
//! # Subcommands
//!
//! A field marked `subcommand` holds an enum whose variants each wrap a struct:
//!
//! ```ignore
//! #[derive(Cli)]
//! #[usage(bin = "ex")]
//! struct Ex {
//!     #[usage(short = 'v', long, global)]
//!     verbose: bool,
//!     #[usage(subcommand)]
//!     command: Option<Commands>,
//! }
//!
//! #[derive(Subcommands)]
//! enum Commands {
//!     /// Install a tool
//!     Install(Install),
//!     /// Run a task
//!     #[usage(name = "run")]
//!     RunTask(Run),
//! }
//!
//! /// Install a tool
//! #[derive(Args)]
//! struct Install {
//!     #[usage(short = 'f', long)]
//!     force: bool,
//!     tools: Vec<String>,
//! }
//! ```
//!
//! The three derives cannot see each other — a macro sees one item — so the tables
//! are joined through two traits, [`usage_argv::spec::CommandArgs`] and
//! [`usage_argv::spec::Subcommands`], whose associated consts a parent splices into
//! its own `static` tables. Nothing is assembled at run time.
//!
//! A command inside a command is not a special case: an `Args` struct carries a
//! `subcommand` field exactly as the root does, to any depth, and generates the same
//! code for it. mise reaches four levels, so one was never going to be enough.
//!
//! Keys carry a hash of the declaration they came from, which is how independently
//! expanded macros avoid handing two fields the same one. A key chooses which arm to
//! jump to and the arm then verifies the event came from its own table, so even two
//! identical declarations in different modules cannot misbind — the event simply goes
//! unclaimed, and `Spec::to_kdl` asserts the tree holds no duplicate keys, so a
//! collision fails a test rather than quietly doing the wrong thing.
//!
//! # What is decided after the parse
//!
//! The parser binds tokens. Whether what it bound is *acceptable* needs to know the
//! declared type, so the generated code checks that once the last token has been
//! read, in an order that is deliberate:
//!
//! 1. **The environment** fills what argv left out, for a field with `env`.
//! 2. **Required-ness**, which the type states: a `String` has nowhere to put
//!    "absent", so it must be given — unless a default or the environment already
//!    filled it.
//! 3. **`choices`** and **`var_min`/`var_max`**, which judge a value however it
//!    arrived, including from the environment or a default.
//!
//! Only the command that actually ran is judged. A flag that `install` requires says
//! nothing about an invocation of `run`.
//!
//! Bounds constrain the values a field was *given*: an unused optional flag is
//! absent, not a violation, or `var_min` would be a second way to spell
//! required-ness and there would be no way to say "at least two, if you use it".
//!
//! Contradictions are refused at compile time rather than at run time — `choices` on
//! a `bool`, a `var_min` above its `var_max`, a bound on something that is not a
//! `Vec`, or a default that is not one of the choices.
//!
//! # Declaring
//!
//! A field with `long` or `short` is a flag; anything else is a positional
//! argument. Help text comes from the doc comment: the first paragraph is the
//! short form, and the whole comment is the long form.
//!
//! A field's **type** says how many values it takes and what they become. `bool` is a
//! switch and an unsigned integer with `count` counts occurrences; everything else holds
//! values, built with `FromStr`:
//!
//! | type | means |
//! | --- | --- |
//! | `T` | one value, required — the type has nowhere to put "absent" |
//! | `Option<T>` | one value, or nothing |
//! | `Vec<T>` | several, empty when none arrived |
//! | `Option<Vec<T>>` | several, and `None` when the flag was never given at all |
//!
//! So `Option<PathBuf>`, `Vec<ToolArg>` and `Option<usize>` all work, and a type that no
//! single word could become is a compile error naming that type. The conversion's error
//! type has to implement `Display`, since what it says is what the user reads — a type
//! whose error does not is also a compile error, and also names the type. The parse itself still
//! binds text — a word's meaning is decided once, where the struct is built — and a value
//! that will not convert becomes [`Error::InvalidValue`](usage_argv::Error::InvalidValue),
//! carrying the offending text and whatever the type's own conversion said about it.
//!
//! | option | meaning |
//! | --- | --- |
//! | `long`, `long = "x"` | a long form, defaulting to the field name |
//! | `short`, `short = 'x'` | a short form, defaulting to the field's first letter |
//! | `name = "x"` | the name used in the spec and in help output |
//! | `negate = "--no-x"` | a second long form that sets a `bool` false |
//! | `count` | count occurrences instead of collecting values |
//! | `var` | the flag may be repeated, taking one value each time |
//! | `variadic` | one occurrence keeps taking values, until a flag-like token or `--` |
//! | `var_max = n` | how many values a variadic takes before the next field gets the rest |
//! | `global` | subcommands inherit the flag |
//! | `env = "X"` | an environment variable that can supply the value |
//! | `default = "x"` | the value when the command line does not supply one |
//! | `help_heading = "x"` | the section to list this under in help output |
//! | `hide` | keep it out of help and completions |
//! | `double_dash = "required"` | a positional only fillable after `--` |
//! | `arg` | force a field to be positional |
//! | `overrides = "--other"` | a flag this one displaces, the last given winning |
//! | `conflicts = "--other"` | a flag this one cannot be given with |
//! | `required_if = "--other"` | a flag whose presence makes this one necessary |
//! | `required_unless = "--other"` | a flag whose presence makes this one unnecessary |
//!
//! These name a flag the way the spec does — `"--long"` or `"-s"` — and take several
//! as a list: `conflicts("--file", "--url")`. A selector naming no flag on the command
//! is a compile error, which is the advantage of declaring a relationship in code: in a
//! hand-written spec a typo'd selector is a relationship that quietly does not hold.
//!
//! They describe relationships *between flags*, so a positional cannot declare one —
//! the spec records them on a flag and has nowhere to put them on an argument, and a
//! check the emitted spec cannot describe would leave docs and completions saying
//! something else. `required_unless` also needs somewhere to put "absent", so it takes
//! an `Option` rather than a bare `String`.
//!
//! A variant may hold its struct in a `Box`, as `Install(Box<Install>)`: an enum is as
//! large as its biggest variant, so one command with thirty flags otherwise makes every
//! invocation move that much stack. Nothing else changes — the box is how the variant
//! holds the struct, not something the CLI has, and the spec cannot tell.
//!
//! A `Subcommands` variant takes `name`, and the two ways to give a command another
//! name: `alias = "i"` for one it should advertise, `alias_hidden = "add"` for one it
//! should answer to quietly, each accepting several as a list. The parser matches both;
//! the difference is only whether help and completions mention them.
//!
//! # What this version does not do
//!
//! Published early on purpose, so it can be used and argued with — but these are
//! real limits, not omissions from the docs.
//!
//! - **Values that are not valid UTF-8.** A word reaches a field through
//!   `String::from_utf8_lossy`, so a `PathBuf` field holding a path that is not UTF-8
//!   gets the replacement character rather than the bytes. Rare, and wrong when it
//!   happens; holding what was typed rather than a lossy copy of it is the next change.
//! - **Flattening.** A struct cannot yet borrow another struct's flags, so a set of
//!   options shared by several commands has to be repeated.

use proc_macro::TokenStream;
use syn::{parse_macro_input, DeriveInput};

mod codegen;
mod model;

/// Compile a struct into a parser and a spec. See the [crate docs](crate).
#[proc_macro_derive(Cli, attributes(usage))]
pub fn derive_cli(input: TokenStream) -> TokenStream {
    let input = parse_macro_input!(input as DeriveInput);
    match model::Cli::from_input(&input) {
        Ok(cli) => codegen::emit(&cli).into(),
        // Reporting the error as tokens rather than panicking is what puts it on
        // the offending line instead of on the derive.
        Err(e) => e.to_compile_error().into(),
    }
}

/// Compile a struct into one subcommand's flags and arguments.
///
/// Used on the struct a [`Subcommands`] variant wraps. It generates the same
/// tables and metadata as [`Cli`], minus the program-level parts a subcommand does
/// not have — a name, a version, an entry point — plus the trait a parent uses to
/// route events into it.
#[proc_macro_derive(Args, attributes(usage))]
pub fn derive_args(input: TokenStream) -> TokenStream {
    let input = parse_macro_input!(input as DeriveInput);
    match model::Cli::from_input(&input) {
        Ok(cli) => codegen::emit_args(&cli).into(),
        Err(e) => e.to_compile_error().into(),
    }
}

/// Compile an enum into a set of subcommands.
///
/// Each variant wraps a struct deriving [`Args`], which is where that command's
/// flags and arguments are declared. A field holding this enum is marked
/// `#[usage(subcommand)]`.
#[proc_macro_derive(Subcommands, attributes(usage))]
pub fn derive_subcommands(input: TokenStream) -> TokenStream {
    let input = parse_macro_input!(input as DeriveInput);
    match model::Subcommands::from_input(&input) {
        Ok(subs) => codegen::emit_subcommands(&subs).into(),
        Err(e) => e.to_compile_error().into(),
    }
}
