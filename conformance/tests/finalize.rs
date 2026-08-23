use std::ffi::OsStr;
use std::path::PathBuf;

use usage_argv::{Error, ValidationError};
use usage_derive::{Args, Cli, Subcommands};

#[derive(Debug, Cli)]
#[usage(bin = "copy", validate_with = validate_copy, try_into = CopyCommand)]
struct CopyArgs {
    #[usage(long)]
    source: PathBuf,
    #[usage(long)]
    destination: PathBuf,
}

#[derive(Debug, PartialEq, Eq)]
struct CopyCommand {
    source: PathBuf,
    destination: PathBuf,
}

fn validate_copy(args: &CopyArgs) -> Result<(), ValidationError> {
    if args.source == args.destination {
        return Err(ValidationError::field("--destination")
            .value(args.destination.display().to_string())
            .reason("must differ from --source"));
    }
    Ok(())
}

impl TryFrom<CopyArgs> for CopyCommand {
    type Error = ValidationError;

    fn try_from(args: CopyArgs) -> Result<Self, Self::Error> {
        if args.destination.extension().is_some_and(|ext| ext == "tmp") {
            return Err(ValidationError::field("--destination")
                .value(args.destination.display().to_string())
                .reason("temporary destinations are not supported"));
        }
        Ok(Self {
            source: args.source,
            destination: args.destination,
        })
    }
}

fn argv<'a>(words: &'a [&'a str]) -> Vec<&'a OsStr> {
    words.iter().map(OsStr::new).collect()
}

#[test]
fn validates_after_all_fields_are_typed() {
    let words = argv(&["--source", "same.txt", "--destination", "same.txt"]);
    let error = CopyArgs::parse_from(&words).expect_err("paths must differ");
    let Error::InvalidValue(invalid) = error else {
        panic!("expected an invalid value");
    };
    assert_eq!(invalid.name, "--destination");
    assert_eq!(invalid.value, "same.txt");
    assert_eq!(invalid.reason, "must differ from --source");
}

#[test]
fn finalizes_directly_into_the_domain_type() {
    let words = argv(&["--source", "input.txt", "--destination", "output.txt"]);
    let command = CopyArgs::parse_into_from(&words).expect("valid command");
    assert_eq!(
        command,
        CopyCommand {
            source: PathBuf::from("input.txt"),
            destination: PathBuf::from("output.txt"),
        }
    );
}

#[test]
fn finalization_failures_use_the_normal_parse_error() {
    let words = argv(&["--source", "input.txt", "--destination", "output.tmp"]);
    let error = CopyArgs::parse_into_from(&words).expect_err("temporary output");
    let Error::InvalidValue(invalid) = error else {
        panic!("expected an invalid value");
    };
    assert_eq!(invalid.name, "--destination");
    assert_eq!(invalid.value, "output.tmp");
    assert_eq!(invalid.reason, "temporary destinations are not supported");
}

#[test]
fn full_argv_finalization_strips_the_program_name() {
    let words = argv(&[
        "copy",
        "--source",
        "input.txt",
        "--destination",
        "output.txt",
    ]);
    assert!(CopyArgs::parse_into_from_argv(&words).is_ok());
    assert!(CopyArgs::try_parse_into_from(&words).is_ok());
}

#[derive(Debug, Cli)]
#[usage(bin = "nested")]
struct NestedCli {
    #[usage(subcommand)]
    command: NestedCommand,
}

#[derive(Debug, Subcommands)]
enum NestedCommand {
    Range(RangeArgs),
}

#[derive(Debug, Args)]
#[usage(validate_with = validate_range)]
struct RangeArgs {
    #[usage(long)]
    start: u16,
    #[usage(long)]
    end: u16,
}

fn validate_range(args: &RangeArgs) -> Result<(), ValidationError> {
    if args.start > args.end {
        return Err(ValidationError::field("--end")
            .value(args.end.to_string())
            .reason("must be greater than or equal to --start"));
    }
    Ok(())
}

#[test]
fn nested_argument_structs_validate_their_own_invariants() {
    let words = argv(&["range", "--start", "5", "--end", "2"]);
    assert!(matches!(
        NestedCli::parse_from(&words),
        Err(Error::InvalidValue(invalid)) if invalid.name == "--end"
    ));
}
