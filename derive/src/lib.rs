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
//! # Declaring
//!
//! A field with `long` or `short` is a flag; anything else is a positional
//! argument. Help text comes from the doc comment: the first paragraph is the
//! short form, and the whole comment is the long form.
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
//! | `global` | subcommands inherit the flag |
//! | `env = "X"` | an environment variable that can supply the value |
//! | `default = "x"` | the value when the command line does not supply one |
//! | `help_heading = "x"` | the section to list this under in help output |
//! | `hide` | keep it out of help and completions |
//! | `double_dash = "required"` | a positional only fillable after `--` |
//! | `arg` | force a field to be positional |
//!
//! # What this version does not do
//!
//! Published early on purpose, so it can be used and argued with — but these are
//! real limits, not omissions from the docs.
//!
//! - **Subcommands.** One command per struct for now.
//! - **Typed values.** Fields are `bool`, `String`, `Option<String>`,
//!   `Vec<String>`, or an unsigned integer with `count`. Anything else is a
//!   compile error rather than a surprise, because converting a value is also
//!   where required-ness, `choices`, and bounds get enforced, and that layer does
//!   not exist yet.
//! - **Enforce what it records.** `default` and `env` are written into the spec
//!   and `default` is applied, but `env` is not read, and a missing required value
//!   is not reported. Same reason.

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
