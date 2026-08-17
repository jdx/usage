use std::path::PathBuf;

use clap::Args;
use miette::Result;
use usage::go::GoOptions;

use crate::cli::generate;

/// Generate Go parse tables from a usage spec
///
/// The tables are read by github.com/jdx/usage/go/argv. Go has no macros, so what
/// a Rust CLI gets from a derive at compile time, a Go CLI gets from this at build
/// time — typically from a `go:generate` line:
///
///   //go:generate usage generate go -f mycli.usage.kdl -o tables.go
#[derive(Args)]
#[clap()]
pub struct Go {
    /// A usage spec taken in as a file, use "-" to read from stdin
    #[clap(short, long)]
    file: Option<PathBuf>,

    /// File path where the generated Go source will be saved, or "-" for stdout
    #[clap(short, long, value_hint = clap::ValueHint::FilePath)]
    out_file: Option<PathBuf>,

    /// Go package clause for the generated file (defaults to the spec's bin name)
    #[clap(short, long)]
    package: Option<String>,

    /// Raw string spec input
    #[clap(long, required_unless_present = "file", overrides_with = "file")]
    spec: Option<String>,
}

impl Go {
    pub fn run(&self) -> Result<()> {
        // Checked here rather than sanitized, because this one came from a person:
        // quietly turning `--package my-pkg` into `mypkg` is a surprise waiting in
        // somebody's build script, and the file would not compile if it were not
        // sanitized at all.
        if let Some(package) = &self.package {
            if !usage::go::is_valid_package(package) {
                miette::bail!(
                    "`--package {package}` is not a Go package name. It must be \
                     letters, digits and underscores, not start with a digit, and \
                     not be one of Go's keywords."
                );
            }
        }

        let spec = generate::file_or_spec(&self.file, &self.spec)?;
        let out = usage::go::generate(
            &spec,
            &GoOptions {
                package: self.package.clone(),
            },
        );
        generate::write_or_stdout(self.out_file.as_deref(), &out)?;
        Ok(())
    }
}
