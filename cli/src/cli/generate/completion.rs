use clap::Args;
use std::path::PathBuf;
use usage::Spec;

#[derive(Args)]
#[clap(visible_alias = "c", aliases = ["complete", "completions"])]
pub struct Completion {
    #[clap(value_parser = ["bash", "fish", "zsh"])]
    shell: String,

    /// The CLI which we're generates completions for
    bin: String,

    /// A command which generates a usage spec
    /// e.g.: `mycli --usage` or `mycli completion usage`
    /// Defaults to "$bin --usage"
    #[clap(long)]
    usage_cmd: Option<String>,
    #[clap(short, long)]
    file: Option<PathBuf>,
    // #[clap(short, long, required_unless_present = "file", overrides_with = "file")]
    // spec: Option<String>,
}

impl Completion {
    pub fn run(&self) -> miette::Result<()> {
        // TODO: refactor this
        let (spec, _) = match &self.file {
            Some(file) => Spec::parse_file(file)?,
            None => (Spec::default(), "".to_string()),
        };
        let spec = match self.file.is_some() {
            true => Some(&spec),
            false => None,
        };

        let bin = &self.bin;
        let usage_cmd = self.usage_cmd.as_deref();

        let script = match self.shell.as_str() {
            "bash" => usage::complete::bash::complete_bash(bin, usage_cmd, spec),
            "fish" => usage::complete::fish::complete_fish(bin, usage_cmd, spec),
            "zsh" => usage::complete::zsh::complete_zsh(bin, usage_cmd, spec),
            _ => unreachable!(),
        };
        println!("{}", script.trim());
        Ok(())
    }
}
