use clap::Args;
use std::path::PathBuf;
use usage::complete::CompleteOptions;
use usage::Spec;

/// Generate shell completion scripts for bash, fish, powershell, or zsh
#[derive(Args)]
#[clap(visible_alias = "c", aliases = ["complete", "completions"])]
pub struct Completion {
    /// Shell to generate completions for
    #[clap(value_parser = ["bash", "fish", "nu", "powershell", "zsh"])]
    shell: String,

    /// The CLI which we're generating completions for
    bin: String,

    /// A .usage.kdl spec file to use for generating completions
    #[clap(short, long)]
    file: Option<PathBuf>,

    /// A cache key to use for storing the results of calling the CLI with --usage-cmd
    #[clap(long, requires = "usage_cmd")]
    cache_key: Option<String>,

    /// Include https://github.com/scop/bash-completion
    ///
    /// This is required for usage completions to work in bash, but the user may already provide it
    #[clap(long, verbatim_doc_comment)]
    include_bash_completion_lib: bool,

    /// Override the bin used for calling back to usage-cli
    ///
    /// You may need to set this if you have a different bin named "usage"
    #[clap(long, default_value = "usage", env = "JDX_USAGE_BIN")]
    usage_bin: String,

    /// A command which generates a usage spec
    /// e.g.: `mycli --usage` or `mycli completion usage`
    /// Defaults to "$bin --usage"
    #[clap(long, required_unless_present = "file")]
    usage_cmd: Option<String>,
}

impl Completion {
    pub fn run(&self) -> miette::Result<()> {
        // TODO: refactor this
        let spec = match &self.file {
            Some(file) => Spec::parse_file(file)?,
            None => Spec::default(),
        };
        let spec = match self.file.is_some() {
            true => Some(spec),
            false => None,
        };
        let opts = CompleteOptions {
            usage_bin: self.usage_bin.clone(),
            shell: self.shell.clone(),
            bin: self.bin.clone(),
            cache_key: self.cache_key.clone(),
            spec,
            usage_cmd: self.usage_cmd.clone(),
            include_bash_completion_lib: self.include_bash_completion_lib,
            source_file: self.file.as_ref().map(|f| f.to_string_lossy().to_string()),
        };

        println!("{}", usage::complete::complete(&opts)?.trim());
        Ok(())
    }
}
