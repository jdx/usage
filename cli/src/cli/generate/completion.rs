use std::path::PathBuf;
use usage::complete::CompleteOptions;
use usage::Spec;
use usage_rs::Args;

use super::parse_file_or_stdin;

/// Generate a shell completion script for bash, fish, nu, powershell, or zsh
///
/// The script is a shim: on each Tab it hands the words typed so far to `usage complete-word`,
/// so `usage` must be installed wherever the script is. The spec comes from `--file`, or from
/// running `--usage-cmd` at completion time, which keeps a CLI that prints its own spec from
/// ever going stale.
#[derive(Args)]
#[usage(alias = "c", alias_hidden("complete", "completions"), effect = "read")]
pub struct Completion {
    /// The shell to generate the script for
    #[usage(choices("bash", "fish", "nu", "powershell", "zsh"))]
    shell: String,

    /// The name of the CLI being completed, as it is typed at the prompt
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

    /// A usage spec file, or a script with a usage shebang; "-" reads stdin
    #[usage(short, long)]
    file: Option<PathBuf>,

    /// Cache what --usage-cmd prints under this key, so it runs once per key rather than on every Tab; the CLI's version is a good key
    #[usage(long, requires = "--usage-cmd")]
    cache_key: Option<String>,

    /// The `usage` executable the script calls back to, when it is not `usage` on PATH
    #[usage(long, default = "usage", env = "JDX_USAGE_BIN")]
    usage_bin: String,

    /// A command that prints the CLI's spec, run in place of reading --file
    ///
    /// For a CLI that answers with its own spec, such as `mycli __usage_spec__`, so the script
    /// always completes the version that is installed. Required unless --file is given.
    #[usage(long, required_unless = "--file")]
    usage_cmd: Option<String>,
}

impl usage_rs::Run for Completion {
    type Output = usage::miette::Result<()>;

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
    fn install(&self, script: &str) -> usage::miette::Result<()> {
        use usage_rs::install::{self, OnForeign, Wrote};

        let shell = usage_rs::complete::Shell::from_name(&self.shell)
            .ok_or_else(|| usage::miette::miette!("{} has no completion script", self.shell))?;
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
///
/// The chain is walked rather than formatted away: `Display` on an install error names the step and
/// the path, and keeps the operating system's own words — "permission denied", "not a directory" —
/// on `source()`. A report built from `Display` alone drops exactly the half a user acts on.
fn as_diagnostic(err: usage_rs::install::Error) -> usage::miette::Report {
    let mut message = err.to_string();
    let mut cause = std::error::Error::source(&err);
    while let Some(next) = cause {
        message.push_str(&format!(": {next}"));
        cause = next.source();
    }
    match &err {
        usage_rs::install::Error::Foreign { .. } => usage::miette::miette!(
            "{message}\n\nPass --force to replace it, or redirect the script yourself."
        ),
        _ => usage::miette::miette!("{message}"),
    }
}
