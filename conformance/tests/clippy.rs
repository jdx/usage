//! Lint hygiene for code emitted into adopter crates.

#![deny(clippy::allow_attributes, clippy::pub_underscore_fields)]

use std::ffi::OsStr;

use usage_derive::{Args, Cli};

mod reusable {
    use usage_derive::Args;

    #[derive(Args)]
    pub struct Shared {
        #[usage(long)]
        pub verbose: bool,
    }
}

// Keep this type private: its generated `Partial` must fulfill the lint expectation too.
#[derive(Args)]
struct PrivateShared {
    #[usage(long)]
    quiet: bool,
}

#[derive(Cli)]
struct App {
    #[usage(flatten)]
    shared: reusable::Shared,
    #[usage(flatten)]
    private: PrivateShared,
}

#[test]
fn generated_partial_state_is_lint_clean() {
    let parsed =
        App::parse_from(&[OsStr::new("--verbose"), OsStr::new("--quiet")]).expect("parses");
    assert!(parsed.shared.verbose);
    assert!(parsed.private.quiet);
}
