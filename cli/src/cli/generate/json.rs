use crate::cli::generate;
use std::path::PathBuf;
use usage::miette::IntoDiagnostic;
use usage::miette::Result;

/// Outputs a usage spec in json format
#[derive(usage_rs::Args)]
#[usage(effect = "read")]
pub struct Json {
    /// A usage spec taken in as a file, use "-" to read from stdin
    #[usage(short, long)]
    file: Option<PathBuf>,

    /// raw string spec input
    #[usage(long, required_unless = "--file", overrides = "--file")]
    spec: Option<String>,

    /// Render one spec-declared executable view
    #[usage(long)]
    view: Option<String>,
}

impl usage_rs::Run for Json {
    type Output = Result<()>;

    fn run(self) -> Self::Output {
        let spec = generate::select_view(
            generate::file_or_spec(&self.file, &self.spec)?,
            self.view.as_deref(),
        )?;
        let json = serde_json::to_string_pretty(&spec).into_diagnostic()?;
        println!("{json}");
        Ok(())
    }
}
