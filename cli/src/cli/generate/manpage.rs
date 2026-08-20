use std::path::PathBuf;

use super::{parse_file_or_stdin, select_view, write_or_stdout};
use usage::docs::manpage::ManpageRenderer;
use usage_rs::Args;

/// Generate a manpage from a usage spec
#[derive(Args)]
#[usage(alias = "man", effect = "read")]
pub struct Manpage {
    /// A usage spec taken in as a file, use "-" to read from stdin
    #[usage(short, long)]
    file: PathBuf,

    /// Render one spec-declared executable view
    #[usage(long)]
    view: Option<String>,

    /// Output file path, or "-" for stdout (default)
    #[usage(
        short,
        long,
        value_hint = usage_rs::ValueHint::FilePath,
        effect = "write"
    )]
    out_file: Option<PathBuf>,

    /// Manual section number (default: 1)
    ///
    /// Common sections:
    /// - 1: User commands
    /// - 5: File formats
    /// - 7: Miscellaneous
    /// - 8: System administration commands
    #[usage(short, long, default = "1")]
    section: u8,
}

impl Manpage {
    pub fn run(&self) -> miette::Result<()> {
        let spec = select_view(parse_file_or_stdin(&self.file)?, self.view.as_deref())?;
        let renderer = ManpageRenderer::new(spec).with_section(self.section);
        let manpage = renderer.render()?;

        write_or_stdout(self.out_file.as_deref(), &manpage)?;

        Ok(())
    }
}
