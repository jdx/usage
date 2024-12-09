use crate::Result;
use std::path::PathBuf;
use miette::IntoDiagnostic;
use crate::cli::generate;

/// Outputs a usage spec in json format
#[derive(clap::Args)]
#[clap()]
pub struct Json {
    /// A usage spec taken in as a file
    #[clap(short, long)]
    file: Option<PathBuf>,

    /// raw string spec input
    #[clap(long, required_unless_present = "file", overrides_with = "file")]
    spec: Option<String>,
}

impl Json {
    pub fn run(&self) -> Result<()> {
        let spec = generate::file_or_spec(&self.file, &self.spec)?;
        let json = serde_json::to_string_pretty(&spec).into_diagnostic()?;
        println!("{json}");
        Ok(())
    }
}
