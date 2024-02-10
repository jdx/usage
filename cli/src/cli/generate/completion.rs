use clap::Args;

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
    // #[clap(short, long)]
    // file: Option<PathBuf>,
    //
    // #[clap(short, long, required_unless_present = "file", overrides_with = "file")]
    // spec: Option<String>,
}

impl Completion {
    pub fn run(&self) -> miette::Result<()> {
        // let spec = if let Some(file) = &self.file {
        //     let (spec, _) = Spec::parse_file(file)?;
        //     spec
        // } else {
        //     Spec::parse_spec(self.spec.as_ref().unwrap())?
        // };
        let bin = &self.bin;
        let usage_cmd = self
            .usage_cmd
            .clone()
            .unwrap_or_else(|| format!("{bin} --usage"));

        let script = match self.shell.as_str() {
            "bash" => usage::complete::bash::complete_bash(bin, &usage_cmd),
            "fish" => usage::complete::fish::complete_fish(bin, &usage_cmd),
            "zsh" => usage::complete::zsh::complete_zsh(bin, &usage_cmd),
            _ => unreachable!(),
        };
        println!("{}", script.trim());
        Ok(())
    }
}
