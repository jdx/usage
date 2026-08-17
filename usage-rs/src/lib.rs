//! The facade for building compiled Rust CLIs with usage.
//!
//! Depend on `usage-rs` under the short crate name `usage`; the derive macros and their runtime
//! then come from one versioned package, while cold-path functionality stays behind features:
//!
//! ```toml
//! [dependencies]
//! usage = { package = "usage-rs", version = "5.1" }
//! ```
//!
//! ```
//! use usage_rs as usage;
//! use usage::Cli;
//!
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
//! ```

#![forbid(unsafe_code)]

// Procedural macros can resolve this package as `Itself`, including from rustdoc's wrapper
// crate. Give generated absolute paths one name that works in both contexts.
extern crate self as usage_rs;

// Keep the cold/runtime module path explicit; derives intentionally emit `usage::argv::...`.
pub use usage_argv as argv;
pub use usage_argv::*;
#[cfg(feature = "spec")]
pub use usage_derive::{Args, Cli, Subcommands, ValueEnum};

#[cfg(all(test, feature = "spec"))]
mod tests {
    use super::Cli;

    #[derive(Cli)]
    #[usage(bin = "inside-facade")]
    struct InsideFacade {}

    #[test]
    fn derives_can_refer_to_the_facade_from_inside_its_library_target() {
        assert!(InsideFacade::to_kdl().contains("bin \"inside-facade\""));
    }
}
