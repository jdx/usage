use std::fs;
use std::fs::File;
use std::io::Write;
use std::iter::once;
use std::path::{Path, PathBuf};

use clap::Args;
use miette::{IntoDiagnostic, Result};

use usage::{SchemaCmd, Spec};

use crate::cli::generate::file_or_spec;

#[derive(Args)]
#[clap(visible_alias = "md")]
pub struct Markdown {
    #[clap(short, long)]
    file: Option<PathBuf>,

    #[clap(short, long, value_hint = clap::ValueHint::DirPath)]
    dir: Option<PathBuf>,

    #[clap(short, long, required_unless_present = "dir", value_hint = clap::ValueHint::FilePath)]
    inject: Option<PathBuf>,

    #[clap(short, long, required_unless_present = "file", overrides_with = "file")]
    spec: Option<String>,
}

impl Markdown {
    pub fn run(&self) -> miette::Result<()> {
        let spec = file_or_spec(&self.file, &self.spec)?;
        for cmd in spec.cmd.subcommands.values() {
            self.print(&spec, self.dir.as_ref().unwrap(), &[&spec.cmd, cmd])?;
        }
        Ok(())
    }

    fn print(&self, spec: &Spec, dir: &Path, cmds: &[&SchemaCmd]) -> Result<()> {
        let cmd = cmds.last().unwrap();
        let mut out = vec![];
        let cmd_path = cmds
            .iter()
            .map(|c| c.name.as_str())
            .collect::<Vec<_>>()
            .join(" ");
        out.push(format!("# {cmd_path}"));
        out.push(format!("## Usage"));
        out.push(format!("```"));
        out.push(format!("{cmd_path} [flags] [args]"));
        out.push(format!("```"));

        let args = cmd.args.iter().filter(|a| !a.hide).collect::<Vec<_>>();
        if !args.is_empty() {
            out.push(format!("## Args"));
            for arg in args {
                let name = &arg.usage();
                if let Some(about) = &arg.long_help {
                    out.push(format!("### {name}"));
                    out.push(format!("{about}"));
                } else if let Some(about) = &arg.help {
                    out.push(format!("- `{name}`: {about}"));
                } else {
                    out.push(format!("- `{name}`"));
                }
            }
        }
        let flags = cmd.flags.iter().filter(|f| !f.hide).collect::<Vec<_>>();
        if !flags.is_empty() {
            out.push(format!("## Flags"));
            for flag in flags {
                let name = flag.usage();
                if let Some(about) = &flag.long_help {
                    out.push(format!("### {name}"));
                    out.push(format!("{about}"));
                } else if let Some(about) = &flag.help {
                    out.push(format!("- `{name}`: {about}"));
                } else {
                    out.push(format!("- `{name}`"));
                }
            }
        }
        let subcommands = cmd
            .subcommands
            .values()
            .filter(|c| !c.hide)
            .collect::<Vec<_>>();
        if !subcommands.is_empty() {
            out.push(format!("## Commands"));
            for cmd in subcommands {
                let name = cmd.name.as_str();
                if let Some(about) = &cmd.help {
                    out.push(format!("- [`{name}`](./{name}): {about}"));
                } else {
                    out.push(format!("- [`{name}`](./{name})"));
                }
            }
        }

        let dir = dir.join(&cmd.name);
        let file = if cmd.subcommands.is_empty() {
            let dir = dir.parent().unwrap();
            fs::create_dir_all(dir).into_diagnostic()?;
            dir.join(format!("{}.md", cmd.name))
        } else {
            fs::create_dir_all(&dir).into_diagnostic()?;
            dir.join(format!("index.md"))
        };
        let mut file = File::create(file).into_diagnostic()?;
        writeln!(file, "{}", out.join("\n")).into_diagnostic()?;

        for cmd in cmd.subcommands.values() {
            let cmds = cmds.iter().cloned().chain(once(cmd)).collect::<Vec<_>>();
            self.print(spec, &dir, &cmds)?;
        }
        Ok(())
    }
}
