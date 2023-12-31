use std::path::PathBuf;

use clap::Args;

use usage::Spec;

#[derive(Args)]
#[clap(visible_alias = "c", aliases=["complete", "completions"])]
pub struct Completion {
    #[clap(value_parser = ["bash", "fish", "zsh"])]
    shell: String,

    #[clap(short, long)]
    file: Option<PathBuf>,

    #[clap(short, long, required_unless_present = "file", overrides_with = "file")]
    spec: Option<String>,
}

impl Completion {
    pub fn run(&self) -> miette::Result<()> {
        let spec = if let Some(file) = &self.file {
            let (spec, _) = Spec::parse_file(file)?;
            spec
        } else {
            self.spec.as_ref().unwrap().parse()?
        };
        let script = match self.shell.as_str() {
            "bash" => usage::complete::bash::complete_bash(&spec),
            "fish" => usage::complete::fish::complete_fish(&spec),
            "zsh" => usage::complete::zsh::complete_zsh(&spec),
            _ => unreachable!(),
        };
        println!("{}", script.trim());
        Ok(())
    }
}
