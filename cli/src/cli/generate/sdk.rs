use std::path::PathBuf;

use usage_rs::Args;

use crate::cli::generate;

use usage::sdk::{SdkLanguage, SdkOptions};

/// Generate a type-safe SDK from a usage spec
///
/// The SDK is a subprocess wrapper: typed arguments, flags, and choices for every command,
/// and a client that builds the argument list and runs the binary.
// The only generator whose output flag is required: it cannot print an SDK to stdout, so
// every invocation writes a directory, and the effect belongs on the command rather than on
// a flag that raises it.
#[derive(Args)]
#[usage(effect = "write")]
pub struct Sdk {
    /// A usage spec file, or a script with a usage shebang; "-" reads stdin
    #[usage(short, long)]
    file: Option<PathBuf>,

    /// Target language for the SDK
    #[usage(short, long, choices("typescript", "python"))]
    language: String,

    /// Directory to write the SDK into
    #[usage(short, long)]
    output: PathBuf,

    /// Override the package/module name (defaults to spec bin name)
    #[usage(short, long)]
    package_name: Option<String>,

    /// The spec itself, as a string, instead of a file
    #[usage(long, required_unless = "--file", overrides = "--file")]
    spec: Option<String>,
}

impl usage_rs::Run for Sdk {
    type Output = usage::miette::Result<()>;

    fn run(self) -> Self::Output {
        let spec = generate::file_or_spec(&self.file, &self.spec)?;

        let language = match self.language.as_str() {
            "typescript" => SdkLanguage::TypeScript,
            "python" => SdkLanguage::Python,
            other => {
                return Err(usage::miette::miette!("unsupported language: {other}"));
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
            .map_err(|e| usage::miette::miette!("failed to create output directory: {e}"))?;

        for file in &output.files {
            let path = self.output.join(&file.path);
            eprintln!("writing to {}", path.display());
            super::write_file(&path, &file.content)?;
        }

        Ok(())
    }
}
