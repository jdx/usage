use std::path::PathBuf;

use usage::go::GoOptions;
use usage::miette::Result;
use usage_rs::Args;

use crate::cli::generate;

/// Generate Go parse tables from a usage spec
///
/// The tables are read by github.com/jdx/usage/go/argv. Go has no macros, so what
/// a Rust CLI gets from a derive at compile time, a Go CLI gets from this at build
/// time — typically from a `go:generate` line:
///
///   //go:generate usage generate go -f mycli.usage.kdl -o tables.go
#[derive(Args)]
#[usage(effect = "read")]
pub struct Go {
    /// A usage spec taken in as a file, use "-" to read from stdin
    #[usage(short, long)]
    file: Option<PathBuf>,

    /// File path where the generated Go source will be saved, or "-" for stdout
    #[usage(
        short,
        long,
        value_hint = usage_rs::ValueHint::FilePath,
        effect = "write"
    )]
    out_file: Option<PathBuf>,

    /// Go package clause for the generated file (defaults to the spec's bin name)
    #[usage(short, long)]
    package: Option<String>,

    /// Raw string spec input
    #[usage(long, required_unless = "--file", overrides = "--file")]
    spec: Option<String>,
}

impl usage_rs::Run for Go {
    type Output = Result<()>;

    fn run(self) -> Self::Output {
        // Checked here rather than sanitized, because this one came from a person:
        // quietly turning `--package my-pkg` into `mypkg` is a surprise waiting in
        // somebody's build script, and the file would not compile if it were not
        // sanitized at all.
        if let Some(package) = &self.package {
            if !usage::go::is_valid_package(package) {
                usage::miette::bail!(
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
