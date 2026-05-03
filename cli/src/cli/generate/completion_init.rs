use clap::Args;
use usage::complete::complete_init;

/// Generate a shell init script that auto-completes any usage shebang script on $PATH
///
/// Source the output once from your shell rc (e.g. ~/.bashrc) to enable
/// tab-completion for any executable whose first line is a `usage` shebang —
/// no per-script `usage g completion` step required.
#[derive(Args)]
#[clap(visible_alias = "ci", aliases = ["init", "completions-init"])]
pub struct CompletionInit {
    /// Shell to generate the init script for
    #[clap(value_parser = ["bash", "fish", "zsh"])]
    shell: String,

    /// Override the bin used for calling back to usage-cli
    ///
    /// You may need to set this if you have a different bin named "usage"
    #[clap(long, default_value = "usage", env = "JDX_USAGE_BIN")]
    usage_bin: String,
}

impl CompletionInit {
    pub fn run(&self) -> miette::Result<()> {
        println!("{}", complete_init(&self.shell, &self.usage_bin)?.trim());
        Ok(())
    }
}
