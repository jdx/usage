//! The facade for building compiled Rust CLIs with usage.
//!
//! Depend on `usage-rs` under the short crate name `usage`. That is the one package an
//! application needs: derive macros, the argv runtime, help, and clap-shaped errors ship in the
//! defaults. Completions stay behind a feature; low-level adopters that want only the binding
//! runtime keep depending on `usage-argv` directly.
//!
//! ```toml
//! [dependencies]
//! usage = { package = "usage-rs", version = "5.1" }
//! ```
//!
//! What happens after a parse can come from the same declaration: a command implements
//! [`Run`] — or [`RunWith`], when the CLI hands its commands shared state — the subcommand enum
//! says `#[usage(run)]`, and the `match` that routes argv to the code carrying it out is
//! generated rather than written. Nothing about it reaches the spec.
//!
//! Enable portable expression validation only when a CLI declares `validate` rules:
//!
//! ```toml
//! usage = { package = "usage-rs", version = "5.1", features = ["validation"] }
//! ```
//!
//! ```
//! use usage_rs as usage;
//! # #[cfg(feature = "spec")]
//! use usage::Cli;
//!
//! # #[cfg(not(feature = "spec"))]
//! # fn main() {}
//! # #[cfg(feature = "spec")]
//! # fn main() {
//! #[derive(Cli)]
//! #[usage(bin = "ex")]
//! struct Ex {
//!     #[usage(long, value_hint = usage::ValueHint::FilePath)]
//!     file: Option<std::path::PathBuf>,
//! }
//!
//! let argv = [std::ffi::OsStr::new("--file"), std::ffi::OsStr::new("input.txt")];
//! let ex = Ex::parse_from(&argv).expect("valid command line");
//! assert_eq!(ex.file.as_deref(), Some(std::path::Path::new("input.txt")));
//! # }
//! ```

#![forbid(unsafe_code)]

// Generated absolute paths must also work if a derive is used inside this crate. Integration
// targets already receive this name through Cargo; the library target needs the self alias.
extern crate self as usage_rs;

pub use usage_argv as argv;
pub use usage_argv::*;
#[cfg(feature = "spec")]
pub use usage_derive::{Args, Cli, Subcommands, ValueEnum};
#[cfg(feature = "test")]
pub use usage_test as test;
#[cfg(feature = "validation")]
pub use usage_validation as validation;

#[cfg(all(test, feature = "spec"))]
mod tests {
    #[derive(crate::Cli)]
    #[usage(bin = "internal")]
    struct Internal {}

    #[cfg(feature = "validation")]
    #[derive(Debug, crate::Cli)]
    #[usage(bin = "validated")]
    struct Validated {
        #[usage(
            long,
            validate = "int(value) >= 1 && int(value) <= 65535",
            validate_error = "must be a valid port"
        )]
        port: Option<u16>,
    }

    #[cfg(feature = "validation")]
    #[derive(Debug, crate::Args)]
    struct ValidatedArgs {
        #[usage(long, validate = "value == 'ok'", validate_error = "must be ok")]
        token: Option<String>,
    }

    #[cfg(feature = "validation")]
    #[derive(Debug, crate::Cli)]
    #[usage(bin = "validated-args")]
    struct ValidatedArgsCli {
        #[usage(flatten)]
        args: ValidatedArgs,
    }

    #[test]
    fn derives_resolve_the_facade_from_inside_the_facade() {
        assert_eq!(Internal::spec().bin, Some("internal"));
    }

    #[cfg(feature = "validation")]
    #[test]
    fn derives_evaluate_portable_validation_expressions() {
        let valid = [
            ::std::ffi::OsStr::new("--port"),
            ::std::ffi::OsStr::new("9229"),
        ];
        assert_eq!(Validated::parse_from(&valid).unwrap().port, Some(9229));

        let invalid = [
            ::std::ffi::OsStr::new("--port"),
            ::std::ffi::OsStr::new("0"),
        ];
        let crate::Error::InvalidValue(error) = Validated::parse_from(&invalid).unwrap_err() else {
            panic!("expected invalid value");
        };
        assert_eq!(error.reason, "must be a valid port");

        let invalid_args = [
            ::std::ffi::OsStr::new("--token"),
            ::std::ffi::OsStr::new("bad"),
        ];
        let crate::Error::InvalidValue(error) =
            ValidatedArgsCli::parse_from(&invalid_args).unwrap_err()
        else {
            panic!("expected invalid value from flattened Args");
        };
        assert_eq!(error.reason, "must be ok");

        let valid_args = [
            ::std::ffi::OsStr::new("--token"),
            ::std::ffi::OsStr::new("ok"),
        ];
        assert_eq!(
            ValidatedArgsCli::parse_from(&valid_args)
                .unwrap()
                .args
                .token
                .as_deref(),
            Some("ok")
        );

        let kdl = Validated::to_kdl();
        assert!(
            kdl.contains(r#"validate="int(value) >= 1 && int(value) <= 65535""#),
            "{kdl}"
        );
    }
}
