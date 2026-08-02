use std::path::PathBuf;

use super::{parse_file_or_stdin, write_or_stdout};
use clap::Args;
use usage::docs::manpage::ManpageRenderer;

#[derive(Args)]
#[clap(visible_alias = "man")]
pub struct Manpage {
    /// A usage spec taken in as a file, use "-" to read from stdin
    #[clap(short, long)]
    file: PathBuf,

    /// Output file path, or "-" for stdout (default)
    #[clap(short, long, value_hint = clap::ValueHint::FilePath)]
    out_file: Option<PathBuf>,

    /// Manual section number (default: 1)
    ///
    /// Common sections:
    /// - 1: User commands
    /// - 5: File formats
    /// - 7: Miscellaneous
    /// - 8: System administration commands
    #[clap(short, long, default_value = "1")]
    section: u8,
}

impl Manpage {
    pub fn run(&self) -> miette::Result<()> {
        let spec = parse_file_or_stdin(&self.file)?;
        let renderer = ManpageRenderer::new(spec).with_section(self.section);
        let manpage = renderer.render()?;

        write_or_stdout(self.out_file.as_deref(), &manpage)?;

        Ok(())
    }
}
