use std::path::PathBuf;

use usage_derive::Args;

use crate::cli::generate;

use usage::sdk::{SdkLanguage, SdkOptions};

/// Generate a type-safe SDK from a usage spec
// The only generator whose output flag is required: it cannot print an SDK to stdout, so
// every invocation writes a directory, and the effect belongs on the command rather than on
// a flag that raises it.
#[derive(Args)]
#[usage(effect = "write")]
pub struct Sdk {
    /// A usage spec taken in as a file
    #[usage(short = 'f', long)]
    file: Option<PathBuf>,

    /// Target language for the SDK
    #[usage(short = 'l', long, choices("typescript", "python"))]
    language: String,

    /// Output directory for generated SDK files
    #[usage(short = 'o', long)]
    output: PathBuf,

    /// Override the package/module name (defaults to spec bin name)
    #[usage(short = 'p', long)]
    package_name: Option<String>,

    /// Raw string spec input
    #[usage(long, required_unless = "--file", overrides = "--file")]
    spec: Option<String>,
}

impl Sdk {
    pub fn run(&self) -> miette::Result<()> {
        let spec = generate::file_or_spec(&self.file, &self.spec)?;

        let language = match self.language.as_str() {
            "typescript" => SdkLanguage::TypeScript,
            "python" => SdkLanguage::Python,
            other => {
                return Err(miette::miette!("unsupported language: {other}"));
            }
        };

        let source_file = self.file.as_ref().map(|p| p.display().to_string());

        let opts = SdkOptions {
            language,
            package_name: self.package_name.clone(),
            source_file,
        };

        let output = usage::sdk::generate(&spec, &opts);

        std::fs::create_dir_all(&self.output)
            .map_err(|e| miette::miette!("failed to create output directory: {e}"))?;

        for file in &output.files {
            let path = self.output.join(&file.path);
            eprintln!("writing to {}", path.display());
            xx::file::write(&path, &file.content)?;
        }

        Ok(())
    }
}
