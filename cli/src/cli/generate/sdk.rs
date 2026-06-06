use std::path::PathBuf;

use clap::Args;

use crate::cli::generate;

use usage::sdk::{SdkLanguage, SdkOptions};

#[derive(Args)]
#[clap(about = "Generate a type-safe SDK from a usage spec")]
pub struct Sdk {
    /// A usage spec taken in as a file
    #[clap(short, long)]
    file: Option<PathBuf>,

    /// Target language for the SDK
    #[clap(short, long, value_parser = ["typescript", "python", "rust"])]
    language: String,

    /// Output directory for generated SDK files
    #[clap(short, long)]
    output: PathBuf,

    /// Override the package/module name (defaults to spec bin name)
    #[clap(short, long)]
    package_name: Option<String>,

    /// Raw string spec input
    #[clap(long, required_unless_present = "file", overrides_with = "file")]
    spec: Option<String>,
}

impl Sdk {
    pub fn run(&self) -> miette::Result<()> {
        let spec = generate::file_or_spec(&self.file, &self.spec)?;

        let language = match self.language.as_str() {
            "typescript" => SdkLanguage::TypeScript,
            "python" => SdkLanguage::Python,
            "rust" => SdkLanguage::Rust,
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
            println!("writing to {}", path.display());
            xx::file::write(&path, &file.content)?;
        }

        Ok(())
    }
}
