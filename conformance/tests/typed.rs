//! Fields that are not text.
//!
//! The parse binds words; this is the layer where a word becomes the thing the field holds.
//! What matters here is that it covers the types a real CLI is written with — mise names
//! `PathBuf` 227 times and a tool-version type 83 times in its command structs — and that a
//! value which will not convert says which value and why.

use std::ffi::OsStr;
use std::path::PathBuf;
use std::str::FromStr;

use usage::Spec as LibSpec;
use usage_argv::Error;
use usage_derive::Cli;

fn argv<const N: usize>(tokens: [&str; N]) -> [&OsStr; N] {
    tokens.map(OsStr::new)
}

/// A type of the adopter's own, as `ToolArg` is in mise.
#[derive(Debug, PartialEq)]
struct Tool {
    name: String,
    version: Option<String>,
}

impl FromStr for Tool {
    type Err = String;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        if s.is_empty() {
            return Err("a tool needs a name".into());
        }
        let (name, version) = match s.split_once('@') {
            Some((name, version)) => (name, Some(version.to_string())),
            None => (s, None),
        };
        Ok(Tool {
            name: name.to_string(),
            version,
        })
    }
}

/// A CLI whose fields are the types a real one uses.
#[derive(Cli)]
#[usage(bin = "typed")]
struct Typed {
    /// How many at once
    #[usage(short = 'j', long)]
    jobs: Option<usize>,
    /// Where to write
    #[usage(long)]
    out: Option<PathBuf>,
    /// A number that must be given
    ///
    /// Bare rather than `Option`, which is how a required value is declared: the type says
    /// there is nowhere to put "absent".
    #[usage(long)]
    port: u16,
    /// Tools to use
    #[usage(long, var)]
    tool: Vec<Tool>,
    /// Directories to search, absent when the flag was never given
    #[usage(long, var)]
    search: Option<Vec<PathBuf>>,
    /// What to act on
    #[usage(arg, name = "TARGET")]
    target: String,
}

#[test]
fn values_arrive_as_the_types_the_fields_hold() {
    let a = argv([
        "-j", "4", "--out", "/tmp/x", "--port", "8080", "--tool", "node@20", "--tool", "python",
        "--search", "/a", "--search", "/b", "thing",
    ]);
    let typed = Typed::parse_from(&a).expect("should parse");

    assert_eq!(typed.jobs, Some(4));
    assert_eq!(typed.out, Some(PathBuf::from("/tmp/x")));
    assert_eq!(typed.port, 8080);
    assert_eq!(
        typed.tool,
        [
            Tool {
                name: "node".into(),
                version: Some("20".into())
            },
            Tool {
                name: "python".into(),
                version: None
            },
        ]
    );
    assert_eq!(
        typed.search,
        Some(vec![PathBuf::from("/a"), PathBuf::from("/b")])
    );
    assert_eq!(typed.target, "thing");
}

#[test]
fn an_optional_collection_tells_absent_from_empty() {
    // What `Option<Vec<T>>` is for, and what a `Vec` cannot say. mise's root draws this
    // distinction three times.
    let a = argv(["--port", "1", "thing"]);
    let typed = Typed::parse_from(&a).expect("should parse");
    assert_eq!(typed.search, None, "never given");
    assert!(typed.tool.is_empty(), "a plain Vec is simply empty");
}

#[test]
fn a_value_that_will_not_convert_says_which_and_why() {
    let a = argv(["--port", "1", "--jobs", "lots", "thing"]);
    match Typed::parse_from(&a) {
        Err(Error::InvalidValue(bad)) => {
            assert_eq!(bad.name, "jobs");
            assert_eq!(bad.value, "lots");
            // Whatever the type itself said, rather than a message of ours that would be
            // worse: `usize` explains this better than "invalid value" would.
            assert!(
                bad.reason.contains("invalid digit"),
                "unhelpful reason: {}",
                bad.reason
            );
        }
        Err(other) => panic!("wrong error: {other:?}"),
        Ok(_) => panic!("this should not have parsed"),
    }
}

#[test]
fn a_custom_types_own_error_is_what_the_user_sees() {
    let a = argv(["--port", "1", "--tool", "", "thing"]);
    match Typed::parse_from(&a) {
        Err(Error::InvalidValue(bad)) => {
            assert_eq!(bad.name, "tool");
            assert_eq!(bad.reason, "a tool needs a name");
        }
        Err(other) => panic!("wrong error: {other:?}"),
        Ok(_) => panic!("this should not have parsed"),
    }
}

#[test]
fn a_number_out_of_range_is_the_types_business() {
    // `u16` rejects 99999 and says so; nothing in the derive needs to know the range.
    let a = argv(["--port", "99999", "thing"]);
    match Typed::parse_from(&a) {
        Err(Error::InvalidValue(bad)) => {
            assert_eq!(bad.name, "port");
            assert!(bad.reason.contains("too large"), "{}", bad.reason);
        }
        Err(other) => panic!("wrong error: {other:?}"),
        Ok(_) => panic!("this should not have parsed"),
    }
}

#[test]
fn the_spec_is_unchanged_by_a_fields_type() {
    // A type is a Rust-side matter: the spec describes what a CLI accepts, and `--jobs <n>`
    // accepts a word either way. Nothing about `usize` belongs in the KDL.
    let kdl = Typed::to_kdl();
    // The writer puts a flag's argument in a child block rather than in the placeholder.
    assert!(kdl.contains(r#"flag "-j --jobs""#), "{kdl}");
    assert!(kdl.contains(r#"arg "<jobs>""#), "{kdl}");
    assert!(!kdl.contains("usize"), "{kdl}");
    assert!(!kdl.contains("PathBuf"), "{kdl}");
}

mod newtype {
    use std::str::FromStr;

    /// A type whose last path segment is `String` and which is not one.
    ///
    /// The conversion takes `String` as the identity case; matching that on the last segment
    /// alone would hand this field a `std::string::String` and fail to compile.
    #[derive(Debug, PartialEq)]
    pub struct String(pub usize);

    impl FromStr for String {
        type Err = std::num::ParseIntError;

        fn from_str(s: &str) -> Result<Self, Self::Err> {
            s.parse().map(String)
        }
    }
}

/// Values on a subcommand, which is where every real command lives.
#[derive(usage_derive::Args)]
struct Deeply {
    /// Where to write
    #[usage(long)]
    out: Option<PathBuf>,
    /// How many
    #[usage(long)]
    count: Option<u8>,
    /// Not the standard `String`
    #[usage(long)]
    width: Option<newtype::String>,
    /// Tools
    #[usage(long, var)]
    tool: Vec<Tool>,
}

#[derive(usage_derive::Subcommands)]
enum NestedCommands {
    /// Do the thing
    Deeply(Box<Deeply>),
}

/// A tool whose commands hold typed values
#[derive(Cli)]
#[usage(bin = "nested")]
struct Nested {
    #[usage(subcommand)]
    command: Option<NestedCommands>,
}

#[test]
fn a_subcommands_fields_are_converted_too() {
    // Two emitters produce `build`, and only the root's was converting: a typed field on a
    // subcommand compiled in the tests and not in an adopter's crate, which is every
    // command mise has.
    let a = argv([
        "deeply", "--out", "/tmp/y", "--count", "3", "--width", "80", "--tool", "node@22",
    ]);
    let Some(NestedCommands::Deeply(deeply)) = Nested::parse_from(&a).expect("parses").command
    else {
        panic!("expected `deeply`")
    };
    assert_eq!(deeply.out, Some(PathBuf::from("/tmp/y")));
    assert_eq!(deeply.count, Some(3));
    assert_eq!(deeply.width, Some(newtype::String(80)));
    assert_eq!(deeply.tool.len(), 1);
}

#[test]
fn a_conversion_failure_on_a_subcommand_names_the_field() {
    let a = argv(["deeply", "--count", "300"]);
    match Nested::parse_from(&a) {
        Err(Error::InvalidValue(bad)) => {
            assert_eq!(bad.name, "count");
            assert_eq!(bad.value, "300");
        }
        Err(other) => panic!("wrong error: {other:?}"),
        Ok(_) => panic!("300 does not fit in a u8"),
    }
}

/// The words a value may be, declared once on the type.
///
/// mise has nine of these. What matters is that the list reaches the spec — so help and
/// completions offer it — without being written a second time on the field.
#[derive(Debug, PartialEq, usage_derive::ValueEnum)]
enum Interpreter {
    Bash,
    Zsh,
    Fish,
    /// Not `power-shell`, which is what the variant name would have given
    #[usage(name = "pwsh")]
    PowerShell,
}

/// A CLI with an enumerated value
#[derive(Cli)]
#[usage(bin = "enumerated")]
struct Enumerated {
    /// Which shell
    #[usage(short = 's', long, value_enum)]
    shell: Option<Interpreter>,
    /// Shells to generate for
    #[usage(long, var, value_enum)]
    also: Vec<Interpreter>,
}

#[test]
fn a_word_becomes_the_variant_it_names() {
    let a = argv(["-s", "zsh", "--also", "bash", "--also", "pwsh"]);
    let e = Enumerated::parse_from(&a).expect("should parse");
    assert_eq!(e.shell, Some(Interpreter::Zsh));
    assert_eq!(e.also, [Interpreter::Bash, Interpreter::PowerShell]);
}

#[test]
fn the_words_reach_the_spec_from_the_type() {
    // The point of `value_enum`: the list is declared once, on the type, and the spec has
    // it — so `usage g markdown` and the completions offer the same words the parse accepts.
    let spec: LibSpec = Enumerated::to_kdl().parse().expect("valid spec");
    let shell = spec.cmd.flags.iter().find(|f| f.name == "shell").unwrap();
    let choices = shell
        .arg
        .as_ref()
        .and_then(|a| a.choices.as_ref())
        .expect("--shell should declare choices");
    assert_eq!(choices.choices, ["bash", "zsh", "fish", "pwsh"]);
}

#[test]
#[cfg(unix)]
fn a_choice_that_is_not_utf8_reports_the_bytes_not_the_list() {
    // The checks run before the struct is built, so a value that is not UTF-8 used to be
    // compared as an empty string, match none of the choices, and come back as
    // `InvalidChoice` — a message listing words, about a value that was never a word. The
    // UTF-8 failure is the real problem and the one worth reporting.
    use std::ffi::OsStr;
    use std::os::unix::ffi::OsStrExt;

    let bad_bytes = [OsStr::new("--shell"), OsStr::from_bytes(b"ba\xffsh")];
    match Enumerated::parse_from(&bad_bytes) {
        Err(Error::InvalidValue(bad)) => {
            assert_eq!(bad.name, "shell");
            assert!(
                bad.reason.contains("utf-8") || bad.reason.contains("UTF-8"),
                "the reason should be the UTF-8 failure: {}",
                bad.reason
            );
        }
        Err(Error::InvalidChoice { choices, .. }) => {
            panic!("reported the choices {choices:?} for a value that is not a word at all")
        }
        Err(other) => panic!("wrong error: {other:?}"),
        Ok(_) => panic!("this should not have parsed"),
    }

    // A word that *is* text and is not one of the choices still gets the list, which is the
    // case this check exists for.
    let a = argv(["--shell", "csh"]);
    assert!(matches!(
        Enumerated::parse_from(&a),
        Err(Error::InvalidChoice { .. })
    ));
}

#[test]
fn a_wrong_word_lists_what_was_expected() {
    // An `InvalidChoice` carrying the list, rather than a conversion error about a type the
    // user never named.
    let a = argv(["--shell", "csh"]);
    match Enumerated::parse_from(&a) {
        Err(Error::InvalidChoice { name, choices }) => {
            assert_eq!(name, "shell");
            assert_eq!(choices, ["bash", "zsh", "fish", "pwsh"]);
        }
        Err(other) => panic!("wrong error: {other:?}"),
        Ok(_) => panic!("`csh` is not one of the words"),
    }
}

#[test]
fn the_conversion_stands_on_its_own() {
    // Whoever converts one by hand gets a message with the words in it, since the check
    // above is the parser's and not the type's.
    use std::str::FromStr;
    assert_eq!(Interpreter::from_str("fish"), Ok(Interpreter::Fish));
    let err = Interpreter::from_str("csh").expect_err("not a shell");
    assert!(err.contains("bash, zsh, fish, pwsh"), "{err}");
}

/// A CLI holding a path, which is where mangling would show
#[derive(Cli)]
#[usage(bin = "pathy")]
struct Pathy {
    /// Where to write
    #[usage(long)]
    out: Option<PathBuf>,
    /// Anything at all
    #[usage(long)]
    text: Option<String>,
}

#[test]
fn a_word_that_is_not_utf8_is_reported_rather_than_mangled() {
    // It used to arrive through `from_utf8_lossy`, so a path with a stray byte in it became
    // a path with U+FFFD in it — a different file, silently. Now the parse says so.
    use std::ffi::OsStr;
    use std::os::unix::ffi::OsStrExt;

    let bad = OsStr::from_bytes(b"/tmp/\xff");
    let argv = [OsStr::new("--out"), bad];
    match Pathy::parse_from(&argv) {
        Err(Error::InvalidValue(bad)) => {
            assert_eq!(bad.name, "out");
            assert!(
                bad.reason.contains("utf-8") || bad.reason.contains("UTF-8"),
                "the reason should say what was wrong: {}",
                bad.reason
            );
            // Rendered lossily *for the message only*, which is the one place it is right:
            // the value is being described, not used.
            assert!(bad.value.contains("/tmp/"), "{}", bad.value);
        }
        Err(other) => panic!("wrong error: {other:?}"),
        Ok(_) => panic!("a value that is not UTF-8 should not have been accepted"),
    }
}

#[test]
fn a_path_that_is_utf8_arrives_exactly() {
    let argv = argv(["--out", "/tmp/x y/z", "--text", "hello"]);
    let p = Pathy::parse_from(&argv).expect("should parse");
    assert_eq!(p.out, Some(PathBuf::from("/tmp/x y/z")));
    assert_eq!(p.text.as_deref(), Some("hello"));
}
