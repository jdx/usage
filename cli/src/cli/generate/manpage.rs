use std::path::PathBuf;

use super::parse_file_or_stdin;
use clap::Args;
use miette::IntoDiagnostic;
use usage::docs::manpage::ManpageRenderer;

#[derive(Args)]
#[clap(visible_alias = "man")]
pub struct Manpage {
    /// A usage spec taken in as a file, use "-" to read from stdin
    #[clap(short, long)]
    file: PathBuf,

    /// Output file path (defaults to stdout)
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

        if let Some(out_file) = &self.out_file {
            println!("writing to {}", out_file.display());
            if let Some(parent) = out_file.parent() {
                std::fs::create_dir_all(parent).into_diagnostic()?;
            }
            std::fs::write(out_file, &manpage).into_diagnostic()?;
        } else {
            print!("{}", manpage);
        }

        Ok(())
    }
}
