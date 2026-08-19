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

#[cfg(all(test, feature = "spec"))]
mod tests {
    #[derive(crate::Cli)]
    #[usage(bin = "internal")]
    struct Internal {}

    #[test]
    fn derives_resolve_the_facade_from_inside_the_facade() {
        assert_eq!(Internal::spec().bin, Some("internal"));
    }
}
