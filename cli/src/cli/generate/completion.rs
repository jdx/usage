use std::path::PathBuf;
use usage::complete::CompleteOptions;
use usage::Spec;
use usage_rs::Args;

use super::parse_file_or_stdin;

/// Generate shell completion scripts for bash, fish, nu, powershell, or zsh
#[derive(Args)]
#[usage(alias = "c", alias_hidden("complete", "completions"), effect = "read")]
pub struct Completion {
    /// Shell to generate completions for
    #[usage(choices("bash", "fish", "nu", "powershell", "zsh"))]
    shell: String,

    /// The CLI which we're generating completions for
    bin: String,

    /// Install the script where this shell looks for it, instead of printing it
    ///
    /// Writes the script file and nothing else: no shell rc file and no PowerShell profile is
    /// edited. Where a shell needs a one-time line of its own — zsh's `fpath+=`, PowerShell's
    /// dot-source — it is printed for you to add.
    #[usage(long, effect = "write")]
    install: bool,

    /// Replace a file at the target path that usage did not write
    #[usage(long, requires = "--install", effect = "write")]
    force: bool,

    /// A .usage.kdl spec file to use for generating completions, use "-" to read from stdin
    #[usage(short, long)]
    file: Option<PathBuf>,

    /// A cache key to use for storing the results of calling the CLI with --usage-cmd
    #[usage(long, requires = "--usage-cmd")]
    cache_key: Option<String>,

    /// Override the bin used for calling back to usage-cli
    ///
    /// You may need to set this if you have a different bin named "usage"
    #[usage(long, default = "usage", env = "JDX_USAGE_BIN")]
    usage_bin: String,

    /// A command which generates a usage spec
    /// e.g.: `mycli --usage` or `mycli completion usage`
    /// Defaults to "$bin --usage"
    #[usage(long, required_unless = "--file")]
    usage_cmd: Option<String>,
}

impl usage_rs::Run for Completion {
    type Output = miette::Result<()>;

    fn run(self) -> Self::Output {
        // TODO: refactor this
        let spec = match &self.file {
            Some(file) => parse_file_or_stdin(file)?,
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
            source_file: self.file.as_ref().map(|f| {
                if f.as_os_str() == "-" {
                    "stdin".to_string()
                } else {
                    f.to_string_lossy().to_string()
                }
            }),
        };

        // Trailing newline included: a script is a file, and one without a final newline is a file
        // half the tools that read it complain about.
        let script = format!("{}\n", usage::complete::complete(&opts)?.trim());
        if !self.install {
            // `write_stdout` rather than `println!`, which panics on a broken pipe — and
            // `usage g completion bash mycli | head -1` is an ordinary thing to type.
            return Ok(super::write_stdout(&script)?);
        }
        self.install(&script)
    }
}

impl Completion {
    /// Put the script where this shell looks for it, and say what is left to do.
    ///
    /// The resolver is the one a compiled binary uses to install its own script, so the location is
    /// decided in one place regardless of which side is asking.
    fn install(&self, script: &str) -> miette::Result<()> {
        use usage_rs::install::{self, OnForeign, Wrote};

        let shell = usage_rs::complete::Shell::from_name(&self.shell)
            .ok_or_else(|| miette::miette!("{} has no completion script", self.shell))?;
        // Described from this process rather than reached for inside the resolver, which is what
        // lets a test point the same code path at a directory of its own.
        let env = install::Env::from_process();
        let plan = install::plan(&self.bin, shell, &env).map_err(as_diagnostic)?;

        let done = install::write(
            &plan,
            script,
            if self.force {
                OnForeign::Overwrite
            } else {
                OnForeign::Refuse
            },
        )
        .map_err(as_diagnostic)?;

        // Everything here goes to stderr, and stdout stays empty. A note about a write is not the
        // thing written — the same reason `write_or_stdout` moved its progress line. And after the
        // write rather than before it: a refusal that had already announced an installation would
        // be describing something that did not happen.
        eprintln!("installing to {}", done.plan.path.display());
        if done.wrote == Wrote::Unchanged {
            eprintln!("already up to date");
        }
        if let Some(line) = done.plan.loading.instruction() {
            let file = match &done.plan.loading {
                install::Loading::Manual { file, .. } => file.as_str(),
                _ => "your shell's startup file",
            };
            eprintln!("\nadd this to {file}, once:\n\n{line}\n");
        }
        if let Some(note) = done.plan.note {
            eprintln!("note: {note}");
        }
        Ok(())
    }
}

/// An install failure as something the CLI can print, with the way out where there is one.
fn as_diagnostic(err: usage_rs::install::Error) -> miette::Report {
    match &err {
        usage_rs::install::Error::Foreign { .. } => {
            miette::miette!("{err}\n\nPass --force to replace it, or redirect the script yourself.")
        }
        _ => miette::miette!("{err}"),
    }
}
