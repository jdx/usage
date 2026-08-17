//! Show the companies sponsoring usage and the jdx.dev open source tools.
//!
//! A command that takes nothing, so it is a bare variant of the command enum rather than a
//! struct with no fields: the derive writes the struct such a variant implies, and the help
//! text and `effect` are declared on the variant.

pub fn run() -> miette::Result<()> {
    println!(
        "usage and the jdx.dev open source tools are sponsored by:\n\n  entire.io - https://entire.io\n  37signals - https://37signals.com\n\nView all sponsors: https://jdx.dev/sponsors.html"
    );
    Ok(())
}
