//! Show the companies sponsoring usage and the jdx.dev open source tools.
//!
//! A command that takes nothing, so it is a unit struct: nothing to declare, and a struct is
//! what the dispatched command enum hands its work to. `effect` and the description live here
//! with it, where every other command's do.

/// Show the companies sponsoring usage and the jdx.dev open source tools
#[derive(usage_rs::Args)]
#[usage(effect = "read")]
pub struct Sponsors;

impl usage_rs::Run for Sponsors {
    type Output = miette::Result<()>;

    fn run(self) -> Self::Output {
        println!(
            "usage and the jdx.dev open source tools are sponsored by:\n\n  entire.io - https://entire.io\n  37signals - https://37signals.com\n\nView all sponsors: https://jdx.dev/sponsors.html"
        );
        Ok(())
    }
}
