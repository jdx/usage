//! Lint hygiene for code emitted into adopter crates.

#![deny(clippy::allow_attributes, clippy::pub_underscore_fields)]

use std::ffi::OsStr;

use usage_derive::Cli;

mod reusable {
    use usage_derive::Args;

    #[derive(Args)]
    pub struct Shared {
        #[usage(long)]
        pub verbose: bool,
    }
}

#[derive(Cli)]
struct App {
    #[usage(flatten)]
    shared: reusable::Shared,
}

#[test]
fn generated_partial_state_is_lint_clean() {
    let parsed = App::parse_from(&[OsStr::new("--verbose")]).expect("parses");
    assert!(parsed.shared.verbose);
}
