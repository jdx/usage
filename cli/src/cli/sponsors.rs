/// Show the companies sponsoring usage and the jdx.dev open source tools
#[derive(clap::Args)]
pub struct Sponsors;

impl Sponsors {
    pub fn run(&self) -> miette::Result<()> {
        println!(
            "usage and the jdx.dev open source tools are sponsored by:\n\n  entire.io - https://entire.io\n  37signals - https://37signals.com\n\nView all sponsors: https://jdx.dev/sponsors.html"
        );
        Ok(())
    }
}
