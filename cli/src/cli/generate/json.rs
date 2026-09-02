use crate::cli::generate;
use std::path::PathBuf;
use usage::miette::IntoDiagnostic;
use usage::miette::Result;

/// Print a usage spec as JSON
///
/// The same document the KDL describes, with included files merged and defaults filled in,
/// for a tool that would rather not parse KDL itself.
#[derive(usage_rs::Args)]
#[usage(effect = "read")]
pub struct Json {
    /// A usage spec file, or a script with a usage shebang; "-" reads stdin
    #[usage(short, long)]
    file: Option<PathBuf>,

    /// The spec itself, as a string, instead of a file
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
