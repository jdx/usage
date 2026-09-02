use usage::complete::complete_init;
use usage_rs::Args;

/// Generate a shell init script that completes every usage-shebang script on PATH
///
/// Source it once from the shell's rc file and Tab works on any executable whose first line
/// is a `usage` shebang, with no per-script `usage generate completion` step. bash and zsh
/// register a fallback completer that asks `usage complete-word` when the command has such a
/// shebang; fish has no fallback, so it scans PATH once at startup and registers each script
/// it finds.
#[derive(Args)]
#[usage(
    alias = "ci",
    alias_hidden("init", "completions-init"),
    effect = "read"
)]
pub struct CompletionInit {
    /// The shell to generate the script for
    #[usage(choices("bash", "fish", "zsh"))]
    shell: String,

    /// The `usage` executable the script calls back to, when it is not `usage` on PATH
    #[usage(long, default = "usage", env = "JDX_USAGE_BIN")]
    usage_bin: String,
}

impl usage_rs::Run for CompletionInit {
    type Output = usage::miette::Result<()>;

    fn run(self) -> Self::Output {
        println!("{}", complete_init(&self.shell, &self.usage_bin)?.trim());
        Ok(())
    }
}
