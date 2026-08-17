use crate::cli::generate;
use crate::Result;
use miette::IntoDiagnostic;
use std::path::PathBuf;

/// Outputs a usage spec in json format
#[derive(usage_derive::Args)]
#[usage(effect = "read")]
pub struct Json {
    /// A usage spec taken in as a file, use "-" to read from stdin
    #[usage(short, long)]
    file: Option<PathBuf>,

    /// raw string spec input
    #[usage(long, required_unless = "--file", overrides = "--file")]
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
