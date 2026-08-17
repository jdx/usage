use std::path::PathBuf;

use miette::IntoDiagnostic;

use crate::cli::generate;
use crate::schema::{config_schema, SchemaOptions};
use crate::Result;

/// Generate a JSON Schema for a CLI's config file from its usage spec
#[derive(usage_derive::Args)]
#[usage(effect = "read")]
pub struct JsonSchema {
    /// A usage spec taken in as a file, use "-" to read from stdin
    #[usage(short, long)]
    file: Option<PathBuf>,

    /// Write the schema here instead of to stdout
    #[usage(
        long,
        value_hint = usage_argv::ValueHint::FilePath,
        effect = "write"
    )]
    out_file: Option<PathBuf>,

    /// raw string spec input
    #[usage(long, required_unless = "--file", overrides = "--file")]
    spec: Option<String>,

    /// The schema's title, shown by editors
    #[usage(long)]
    title: Option<String>,

    /// Where the schema is published, for its `$id`
    #[usage(long)]
    url: Option<String>,
}

impl JsonSchema {
    pub fn run(&self) -> Result<()> {
        let spec = generate::file_or_spec(&self.file, &self.spec)?;
        let options = SchemaOptions {
            title: self
                .title
                .clone()
                .or_else(|| Some(format!("{} configuration", spec.name))),
            url: self.url.clone(),
        };
        let schema = config_schema(&spec.config, &options);
        // A schema with no properties and `unevaluatedProperties: false` rejects every config
        // file there is, which is worse than saying there is nothing to describe. Asked of
        // the schema rather than of the spec, because a spec whose settings are *all*
        // `scope="env"` declares props and still has nothing a file may hold.
        if schema
            .get("properties")
            .and_then(serde_json::Value::as_object)
            .is_none_or(serde_json::Map::is_empty)
        {
            miette::bail!(
                "this spec declares nothing a config file can hold, so there is no schema to write"
            );
        }
        let json = serde_json::to_string_pretty(&schema).into_diagnostic()?;
        // Through the shared writer, like every other generator: it takes `-` for stdout and
        // reports a broken pipe instead of panicking on one.
        generate::write_or_stdout(self.out_file.as_deref(), &format!("{json}\n"))?;
        Ok(())
    }
}
