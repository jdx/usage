use clap::Args;
use std::path::PathBuf;
use usage::complete::CompleteOptions;
use usage::Spec;

#[derive(Args)]
#[clap(visible_alias = "c", aliases = ["complete", "completions"])]
pub struct Completion {
    #[clap(value_parser = ["bash", "fish", "zsh"])]
    shell: String,

    /// The CLI which we're generates completions for
    bin: String,

    /// A cache key to use for storing the results of calling the CLI with --usage-cmd
    #[clap(long, requires = "usage_cmd")]
    cache_key: Option<String>,

    /// A .usage.kdl spec file to use for generating completions
    #[clap(short, long)]
    file: Option<PathBuf>,

    /// A command which generates a usage spec
    /// e.g.: `mycli --usage` or `mycli completion usage`
    /// Defaults to "$bin --usage"
    #[clap(long, required_unless_present = "file")]
    usage_cmd: Option<String>,
}

impl Completion {
    pub fn run(&self) -> miette::Result<()> {
        // TODO: refactor this
        let (spec, _) = match &self.file {
            Some(file) => Spec::parse_file(file)?,
            None => (Spec::default(), "".to_string()),
        };
        let spec = match self.file.is_some() {
            true => Some(spec),
            false => None,
        };
        let opts = CompleteOptions {
            shell: self.shell.clone(),
            bin: self.bin.clone(),
            cache_key: self.cache_key.clone(),
            spec,
            usage_cmd: self.usage_cmd.clone(),
        };

        println!("{}", usage::complete::complete(&opts).trim());
        Ok(())
    }
}
