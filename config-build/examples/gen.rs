//! Regenerates the checked-in registry the tests compile.
//!
//! ```sh
//! cargo run -p usage-config-build --example gen > config-build/tests/golden/settings.rs
//! ```
//!
//! An example rather than a build script, because the point of checking the file in is that a
//! human reads the diff: generated code that nobody looks at is where a generator's mistakes live.

const SPEC: &str = "config-build/tests/fixtures/hk.usage.kdl";

fn main() {
    match usage_config_build::source(SPEC) {
        Ok(source) => print!("{source}"),
        Err(err) => {
            eprintln!("{SPEC}: {err}");
            std::process::exit(1);
        }
    }
}
