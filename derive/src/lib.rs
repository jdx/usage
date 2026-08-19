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
//! A variant that holds nothing is a command with no flags and no arguments of its own:
//!
//! ```ignore
//! #[derive(Subcommands)]
//! enum Commands {
//!     /// Install a tool
//!     Install(Install),
//!     /// Show who pays for this
//!     #[usage(effect = "read")]
//!     Sponsors,
//! }
//! ```
//!
//! `effect` goes on the variant there because there is no struct to put it on; everywhere else
//! it belongs to the `Args`, and declaring it in both places is refused.
//!
//! A command type with no fields may keep Rust's unit-struct spelling:
//!
//! ```ignore
//! #[derive(Args)]
//! struct Sponsors;
//! ```
//!
//! Tuple structs remain ambiguous: a derive cannot infer whether their unnamed field is a
//! positional value or a flattened `Args` type. The diagnostic points wrapper migrations to a
//! named field with `#[usage(flatten)]`.
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
//! 3. **`choices`, `validate`, and `var_min`/`var_max`**, which judge a value however it
//!    arrived, including from the environment or a default.
//!
//! Only the command that actually ran is judged. A flag that `install` requires says
//! nothing about an invocation of `run`.
//!
//! Bounds constrain the values a field was *given*: an unused optional flag is
//! absent, not a violation, or `var_min` would be a second way to spell
//! required-ness and there would be no way to say "at least two, if you use it".
//!
//! `validate` is a portable [expr](https://expr-lang.org/) expression with one string
//! variable, `value`. It must return a boolean. `validate_error` supplies the message
//! shown when it returns false.
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
//! Metadata that already has a Rust source of truth may remain an expression. Command help
//! fields such as `about` and `after_long_help` accept expressions usable as `&'static str`.
//! A computed `version` additionally declares `version_spec = "..."`, and a typed field
//! default declares both `default_value_t = EXPR` and `default = "..."`: runtime behavior
//! evaluates the expression while portable KDL uses the explicit literal.
//!
//! A completer is written as
//!
//! ```ignore
//! fn tasks(partial: &<Tasks as CommandArgs>::Partial, ctx: &CompleteCtx<'_>) -> Vec<Candidate<'static>>
//! ```
//!
//! and is handed its *own command's* half-parsed struct, so `tk tasks --file other.toml <TAB>`
//! can be answered against that file — which a `run=` shelling out to a fixed command cannot see.
//! The emitted spec gets a `run=` naming this binary, so everything that reads a spec still has
//! one, generated from the function rather than declared beside it.
//!
//! `Cli::parse()` is the entry point that *is* the process: it prints a help page or a version
//! and leaves, and on a failure it prints the message to stderr and exits 2 — clap's status, so a
//! script checking for it keeps working. `Cli::parse_from(argv)` hands the error back instead,
//! for a library embedding a CLI that wants to decide for itself.
//!
//! Declaring a `version` also gives the CLI `--version` and `-V`, as clap does — supplied by
//! the parser rather than listed in the spec, exactly as `--help` is, and yielding to either
//! spelling the CLI declares for itself. clap refuses that collision by panicking at startup;
//! here the declaration simply wins and the other spelling still answers.
//!
//! On the struct itself: `bin`, `version`, `about`, `long_about`, `before_help`, `after_help`,
//! `verbatim_doc_comment` — preserve doc-comment line breaks and whitespace —
//! `default_subcommand`, `multicall` — argv[0]'s basename selects a subcommand —
//! `infer_subcommands`, `infer_long_args` — accept unambiguous prefixes and inherit that
//! policy through nested commands —
//! `min_usage_version` — the oldest `usage` that can read the emitted
//! spec, declared rather than worked out — `effect` — what running this command does to the world, on an `Args`
//! rather than on the root, which does nothing itself — `completion`, which adds the hidden command a generated shell
//! script calls, and needs usage-argv's `complete` feature enabled where it is depended on —
//! and `settings`, for a CLI whose bound flags all live in a flattened group (see [Settings]).
//!
//! [Settings]: #settings-and-the-flags-that-set-them
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
//! | `default = "x"` | the value when the command line does not supply one; a `Vec` may be given several, and starts out holding all of them |
//! | `help_heading = "x"` | the section to list this under in help output |
//! | `verbatim_doc_comment` | preserve line breaks and whitespace in the doc comment instead of flowing its first paragraph |
//! | `hide` | keep it out of help and completions |
//! | `effect = "write"` | what supplying this flag does to the world: `read`, `write` or `destructive`. Also goes on an `Args`, where it says what *running* the command does |
//! | `double_dash = "…"` | how a positional relates to `--`: `optional` (the default), `required` (fillable only after one), `preserve` (the `--` is a value), `automatic` (filling it ends flag parsing, so a wrapper forwards) |
//! | `complete = my_fn` | a function that answers for this value when a shell asks |
//! | `value_enum` | the words come from the field's type, which derives [`ValueEnum`] |
//! | `value_hint = usage::ValueHint::FilePath` | ask the shell for paths, executables, or forwarded command argv |
//! | `arg` | force a field to be positional |
//! | `overrides = "--other"` | a flag this one displaces, the last given winning |
//! | `conflicts = "--other"` | a flag this one cannot be given with |
//! | `requires = "--other"` | a flag that must also be given when this one is |
//! | `requires_if("value", "--other")` | a flag required when this one explicitly has `value` |
//! | `requires_ifs(("a", "--x"), ("b", "--y"))` | several value-conditional requirements |
//! | `group = "input"` | the group this flag is one of; see below |
//! | `exclusive` | this flag has to be given on its own, positionals included |
//! | `delimiter = ','` | one word becomes several values; the field has to be a `Vec` |
//! | `allow_hyphen_values` | a flag's detached value may look like a flag, including `--` |
//! | `require_equals` | `--flag=value` is accepted and `--flag value` is not |
//! | `default_missing = "always"` | the value when the flag is given with none |
//! | `required_if = "--other"` | a flag whose presence makes this one necessary |
//! | `required_unless = "--other"` | a flag whose presence makes this one unnecessary |
//!
//! These name a flag the way the spec does — `"--long"` or `"-s"` — and take several
//! as a list: `conflicts("--file", "--url")`. A selector naming no flag on the command
//! is a compile error, which is the advantage of declaring a relationship in code: in a
//! hand-written spec a typo'd selector is a relationship that quietly does not hold.
//!
//! A **group** is the one relationship that is not written flag-to-flag, because what it
//! says is about the set: `required` means one of them is needed, and no rule on an
//! individual flag expresses that. Membership goes on the fields and the properties on the
//! struct, which may be left out entirely when the group is a plain "at most one":
//!
//! ```ignore
//! #[derive(Cli)]
//! #[usage(bin = "ex")]
//! #[usage(group("input", required))]
//! struct Ex {
//!     #[usage(long, group = "input")]
//!     file: Option<String>,
//!     #[usage(long, group = "input")]
//!     url: Option<String>,
//! }
//! ```
//!
//! `required` means at least one member is needed and `multiple` means more than one may
//! be given, so a bare group is "at most one", `required` alone is "exactly one", and the
//! two together are "at least one" — clap's two properties, read the same way. A group
//! with one member, or a declaration no field joins, is a compile error.
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
//! A command takes `alias = "i"` for a name it should advertise and
//! `alias_hidden = "add"` for one it should answer to quietly, each accepting several as a
//! list. They may be written on the `Args` struct that owns the command or on its
//! `Subcommands` variant; when both say some, the lists are joined. The parser matches both;
//! the difference is only whether help and completions mention them.
//!
//! # Settings and the flags that set them
//!
//! `setting = "key"` says which setting a flag sets. `Cli::parse_from_with_settings` then
//! returns a `usage_config::CliLayer` beside the parsed struct — the command line as the
//! highest layer of a resolution — and `Cli::SETTINGS_BINDINGS` lists every flag it binds,
//! which `usage_config::Registry::drift` compares against the flags the *spec* declares. A
//! flag documented as setting something and read by nothing fails a test rather than a user.
//!
//! The layer is built from what the parser saw rather than from the parsed struct, because a
//! `bool` field is `false` whether the flag was left off or negated, and the command line
//! outranks every file on the machine. So `--no-colour` contributes `false`, and a flag that
//! was not given contributes nothing at all.
//!
//! A setting can be declared wherever a flag is: on the root, in a `#[usage(flatten)]` group,
//! or on a subcommand's struct. A group hands its parent what it was given in
//! `usage_argv::spec::SettingGiven` — a vocabulary that says nothing about types, since the
//! registry is what decides them — and only the root turns that into a layer, so a program
//! with no settings never mentions `usage-config`. A root that binds nothing itself but
//! flattens a group that does declares `#[usage(settings)]`; leaving it off is a compile error
//! naming the attribute, because the alternative is a documented flag that quietly sets
//! nothing.
//!
//! A word is held as the bytes it arrived as and converted once, where the struct is built.
//! So a value that is not valid UTF-8 is **reported** rather than quietly replaced with
//! `U+FFFD` — which for a `PathBuf` meant a different file, silently. On Unix, `PathBuf` and
//! `OsString` fields accept the bytes exactly through the safe `OsStringExt::from_vec`; on
//! Windows, a value that cannot be converted safely is reported rather than reconstructed with
//! `OsString::from_encoded_bytes_unchecked`.
//!
use proc_macro::TokenStream;
use syn::{parse_macro_input, DeriveInput};

mod codegen;
mod crate_name;
mod model;

/// Compile a struct into a parser and a spec. See the [crate docs](crate).
#[proc_macro_derive(Cli, attributes(usage, command))]
pub fn derive_cli(input: TokenStream) -> TokenStream {
    let input = parse_macro_input!(input as DeriveInput);
    let parsed = model::Cli::from_input(&input)
        .and_then(|cli| cli.check_position(&input.ident, true).map(|()| cli));
    match parsed {
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
#[proc_macro_derive(Args, attributes(usage, command))]
pub fn derive_args(input: TokenStream) -> TokenStream {
    let input = parse_macro_input!(input as DeriveInput);
    // `restart_token` and `mount` are per-command and belong here; `default_subcommand` is
    // declared once for the whole spec and does not.
    let parsed = model::Cli::from_input(&input)
        .and_then(|cli| cli.check_position(&input.ident, false).map(|()| cli));
    match parsed {
        Ok(cli) => codegen::emit_args(&cli).into(),
        Err(e) => e.to_compile_error().into(),
    }
}

/// Compile an enum into a set of subcommands.
///
/// Each variant may wrap a struct deriving [`Args`] or declare its fields inline,
/// clap-style. A field holding this enum is marked `#[usage(subcommand)]`.
#[proc_macro_derive(Subcommands, attributes(usage, arg))]
pub fn derive_subcommands(input: TokenStream) -> TokenStream {
    let input = parse_macro_input!(input as DeriveInput);
    match model::Subcommands::from_input(&input) {
        Ok(subs) => codegen::emit_subcommands(&subs).into(),
        Err(e) => e.to_compile_error().into(),
    }
}

/// Compile an enum into the words one value may be.
///
/// What a CLI calls an enum — `--shell bash` — and what the spec calls `choices`. The
/// variant's name in kebab-case is the word, unless `name` says otherwise:
///
/// ```ignore
/// #[derive(usage::ValueEnum)]
/// enum Shell {
///     /// Bourne Again shell.
///     Bash,
///     #[value(alias = "shell-z")]
///     Zsh,
///     #[value(name = "pwsh", visible_alias = "powershell", hide = true)]
///     PowerShell,
/// }
/// ```
///
/// `#[usage(ignore_case)]` on the enum applies to canonical words and aliases.
/// A variant's doc comment becomes its per-value help. `help = "..."` overrides
/// it, `hide` keeps the value accepted while omitting it from help and completion,
/// `alias` is hidden, and `visible_alias` is advertised alongside the canonical word.
///
/// The enum must implement [`FromStr`](std::str::FromStr); this derive owns only its CLI
/// word metadata and does not replace domain parsing. Variant `cfg` and `cfg_attr`
/// attributes are copied to their entries in the static word tables.
///
/// A field holding one says `value_enum`, which is what puts the words in the spec — so
/// help, completions and the check that rejects a wrong word all read the same list, and
/// none of them can drift from the type.
#[proc_macro_derive(ValueEnum, attributes(usage, value))]
pub fn derive_value_enum(input: TokenStream) -> TokenStream {
    let input = parse_macro_input!(input as DeriveInput);
    match model::ValueEnum::from_input(&input) {
        Ok(value_enum) => codegen::emit_value_enum(&value_enum).into(),
        Err(e) => e.to_compile_error().into(),
    }
}
