use std::path::PathBuf;

use clap::Args;
use itertools::Itertools;

use crate::cli::generate;

#[derive(Debug, Args)]
#[clap(visible_alias = "cw")]
pub struct CompleteWord {
    // #[clap(value_parser = ["bash", "fish", "zsh"])]
    // shell: String,
    words: Vec<String>,

    #[clap(short, long)]
    file: Option<PathBuf>,

    #[clap(short, long, required_unless_present = "file", overrides_with = "file")]
    spec: Option<String>,

    #[clap(long, allow_hyphen_values = true)]
    cword: Option<usize>,

    #[clap(long)]
    ctoken: Option<String>,
}

impl CompleteWord {
    pub fn run(&self) -> miette::Result<()> {
        let spec = generate::file_or_spec(&self.file, &self.spec)?;
        let cword = self.cword.unwrap_or(self.words.len().max(1) - 1);
        let ctoken = self
            .ctoken
            .as_ref()
            .or(self.words.get(cword))
            .cloned()
            .unwrap_or_default();
        let mut choices = vec![];
        let mut cli = clap::Command::from(&spec).ignore_errors(true);
        match cli.try_get_matches_from_mut(&self.words) {
            Ok(m) => {
                let mut cmd = &spec.cmd;
                let mut m = &m;
                loop {
                    if ctoken.starts_with('-') {
                        for flag in cmd.flags.iter().filter(|f| f.global) {
                            for short in flag.short.iter() {
                                choices.push(format!("-{}", short));
                            }
                            for long in flag.long.iter() {
                                choices.push(format!("--{}", long));
                            }
                        }
                    }
                    if let Some((name, subcommand)) = m.subcommand() {
                        cmd = cmd.subcommands.get(name).unwrap();
                        m = subcommand;
                    } else {
                        break;
                    }
                }
                if ctoken.starts_with('-') {
                    for flag in cmd.flags.iter().filter(|f| !f.global) {
                        for short in flag.short.iter() {
                            choices.push(format!("-{}", short));
                        }
                        for long in flag.long.iter() {
                            choices.push(format!("--{}", long));
                        }
                    }
                }
                for cmd in cmd.subcommands.values() {
                    choices.push(cmd.name.clone());
                    choices.extend(cmd.aliases.iter().cloned());
                }
            }
            Err(err) => {
                warn!("clap error: {}", err);
            }
        }
        // if let Ok(m) = m {
        //     for arg in spec.cmd.args.iter() {
        //         if let Some(v) = m.value_of(&arg.name) {
        //             choices.push(v.to_string());
        //         }
        //     }
        // }
        // for cmd in spec.cmd.subcommands.values() {
        //     choices.push(cmd.name.clone());
        // }
        for c in choices.iter().sorted().unique() {
            if c.starts_with(&ctoken) {
                println!("{}", c);
            }
        }
        Ok(())
    }
}
