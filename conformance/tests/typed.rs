//! Fields that are not text.
//!
//! The parse binds words; this is the layer where a word becomes the thing the field holds.
//! What matters here is that it covers the types a real CLI is written with — mise names
//! `PathBuf` 227 times and a tool-version type 83 times in its command structs — and that a
//! value which will not convert says which value and why.

use std::ffi::OsStr;
use std::path::PathBuf;
use std::str::FromStr;

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
