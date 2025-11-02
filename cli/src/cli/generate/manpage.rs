use std::path::PathBuf;

use clap::Args;
use usage::docs::manpage::ManpageRenderer;
use usage::Spec;

#[derive(Args)]
#[clap(visible_alias = "man")]
pub struct Manpage {
    /// A usage spec taken in as a file
    #[clap(short, long)]
    file: PathBuf,

    /// Manual section number (default: 1)
    ///
    /// Common sections:
    /// - 1: User commands
    /// - 5: File formats
    /// - 7: Miscellaneous
    /// - 8: System administration commands
    #[clap(short, long, default_value = "1")]
    section: u8,

    /// Output file path (defaults to stdout)
    #[clap(short, long, value_hint = clap::ValueHint::FilePath)]
    out_file: Option<PathBuf>,
}

impl Manpage {
    pub fn run(&self) -> miette::Result<()> {
        let (spec, _) = Spec::parse_file(&self.file)?;
        let renderer = ManpageRenderer::new(spec).with_section(self.section);
        let manpage = renderer.render()?;

        if let Some(out_file) = &self.out_file {
            println!("writing to {}", out_file.display());
            xx::file::write(out_file, &manpage)?;
        } else {
            print!("{}", manpage);
        }

        Ok(())
    }
}
