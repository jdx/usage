use std::fs::File;
use std::io::Write;
use std::iter::once;
use std::path::{Path, PathBuf};
use std::str::FromStr;
use std::sync::Mutex;
use std::{env, fs};

use clap::Args;
use kdl::{KdlDocument, KdlNode};
use miette::{IntoDiagnostic, Result, SourceSpan};
use thiserror::Error;

use usage::{SchemaCmd, Spec};

#[derive(Args)]
#[clap(visible_alias = "md")]
pub struct Markdown {
    // /// A usage spec taken in as a file
    // #[clap()]
    // file: Option<PathBuf>,
    // /// Pass a usage spec in an argument instead of a file
    // #[clap(short, long, required_unless_present = "file", overrides_with = "file")]
    // spec_str: Option<String>,
    /// A markdown file taken as input
    /// This file should have a comment like this:
    /// <!-- usage file="path/to/usage.kdl" -->
    #[clap(required_unless_present = "out_dir", value_hint = clap::ValueHint::FilePath)]
    inject: Option<PathBuf>,

    /// Output markdown files to this directory
    #[clap(short, long, value_hint = clap::ValueHint::DirPath)]
    out_dir: Option<PathBuf>,
}

impl Markdown {
    pub fn run(&self) -> miette::Result<()> {
        if let Some(inject) = &self.inject {
            self.inject_file(inject)?;
        }
        // let spec = file_or_spec(&self.file, &self.spec_str)?;
        // for cmd in spec.cmd.subcommands.values() {
        //     self.print(&spec, self.out_dir.as_ref().unwrap(), &[&spec.cmd, cmd])?;
        // }
        Ok(())
    }

    fn inject_file(&self, inject: &Path) -> Result<()> {
        let raw = fs::read_to_string(inject).into_diagnostic()?;
        println!("{}", raw);
        let root = inject.parent().unwrap().to_path_buf();
        let cwd = env::current_dir().into_diagnostic()?;
        env::set_current_dir(&root).into_diagnostic()?;
        let out = raw
            .lines()
            .map(|line| line.parse())
            .collect::<Result<Vec<UsageMdDirective>>>()?
            .into_iter()
            .try_fold(UsageMdContext::new(root), |ctx, d| d.run(ctx))?
            .out
            .lock()
            .unwrap()
            .join("\n");
        env::set_current_dir(cwd).into_diagnostic()?;
        println!("{}", out);
        fs::write(inject, out).into_diagnostic()?;
        Ok(())
    }

    fn _print(&self, spec: &Spec, dir: &Path, cmds: &[&SchemaCmd]) -> Result<()> {
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
            self._print(spec, &dir, &cmds)?;
        }
        Ok(())
    }
}

enum UsageMdDirective {
    Load { file: PathBuf },
    UsageOverview,
    GlobalArgs,
    GlobalFlags,
    CommandIndex,
    Commands,
    EndToken,
    Plain(String),
}

struct UsageMdContext {
    _root: PathBuf,
    plain: bool,
    spec: Option<Spec>,
    out: Mutex<Vec<String>>,
}

impl UsageMdContext {
    fn new(_root: PathBuf) -> Self {
        Self {
            _root,
            plain: true,
            spec: None,
            out: Mutex::new(vec![]),
        }
    }

    fn push(&self, line: String) {
        self.out.lock().unwrap().push(line);
    }
}

impl UsageMdDirective {
    fn run(&self, mut ctx: UsageMdContext) -> miette::Result<UsageMdContext> {
        match self {
            UsageMdDirective::Load { file } => {
                // let file = match file.is_relative() {
                //     true => ctx.root.join(file),
                //     false => file.clone(),
                // };
                ctx.spec = Some(fs::read_to_string(file).into_diagnostic()?.parse()?);
                ctx.push(format!("<!-- [USAGE] load file=\"{}\" -->", file.display()));
            }
            UsageMdDirective::UsageOverview => {
                ctx.plain = false;
                let spec = ctx.spec.as_ref().unwrap();

                ctx.push("<!-- [USAGE] usage_overview -->".to_string());
                ctx.push(format!("# {name}", name = spec.name));
                ctx.push("## Usage".to_string());
                ctx.push("```".to_string());
                ctx.push(format!("{bin} [flags] [args]", bin = spec.bin));
                ctx.push("```".to_string());
                ctx.push("\n<!-- [USAGE] -->".to_string());
            }
            UsageMdDirective::GlobalArgs => {
                ctx.plain = false;
                let spec = ctx.spec.as_ref().unwrap();

                ctx.push("<!-- [USAGE] global_args -->".to_string());
                let args = spec.cmd.args.iter().filter(|a| !a.hide).collect::<Vec<_>>();
                if !args.is_empty() {
                    ctx.push(format!("## Args"));
                    for arg in args {
                        let name = &arg.usage();
                        if let Some(about) = &arg.long_help {
                            ctx.push(format!("### {name}", name = name));
                            ctx.push(format!("{about}", about = about));
                        } else if let Some(about) = &arg.help {
                            ctx.push(format!("- `{name}`: {about}", name = name, about = about));
                        } else {
                            ctx.push(format!("- `{name}`", name = name));
                        }
                    }
                }
                ctx.push(format!("\n<!-- [USAGE] -->"));
            }
            UsageMdDirective::GlobalFlags => {
                ctx.plain = false;
                let spec = ctx.spec.as_ref().unwrap();

                ctx.push("<!-- [USAGE] global_flags -->".to_string());
                let flags = spec
                    .cmd
                    .flags
                    .iter()
                    .filter(|f| !f.hide)
                    .collect::<Vec<_>>();
                if !flags.is_empty() {
                    ctx.push(format!("## Flags"));
                    for flag in flags {
                        let name = flag.usage();
                        if let Some(about) = &flag.long_help {
                            ctx.push(format!("### {name}", name = name));
                            ctx.push(format!("{about}", about = about));
                        } else if let Some(about) = &flag.help {
                            ctx.push(format!("- `{name}`: {about}", name = name, about = about));
                        } else {
                            ctx.push(format!("- `{name}`", name = name));
                        }
                    }
                }
                ctx.push(format!("\n<!-- [USAGE] -->"));
            }
            UsageMdDirective::CommandIndex => {
                ctx.plain = false;
                let spec = ctx.spec.as_ref().unwrap();
                ctx.push(format!("<!-- [USAGE] command_index -->"));

                let subcommands = spec
                    .cmd
                    .subcommands
                    .values()
                    .filter(|c| !c.hide)
                    .collect::<Vec<_>>();
                if !subcommands.is_empty() {
                    ctx.push(format!("## Commands"));
                    for cmd in subcommands {
                        let name = cmd.name.as_str();
                        if let Some(about) = &cmd.help {
                            ctx.push(format!(
                                "- [`{name}`](./{name}): {about}",
                                name = name,
                                about = about
                            ));
                        } else {
                            ctx.push(format!("- [`{name}`](./{name})", name = name));
                        }
                    }
                }

                ctx.push(format!("\n<!-- [USAGE] -->"));
            }
            UsageMdDirective::Commands => {
                ctx.plain = false;
                let spec = ctx.spec.as_ref().unwrap();
                ctx.push(format!("<!-- [USAGE] commands -->"));
                ctx.push(format!("# {name}", name = spec.name));
                ctx.push(format!("## Usage"));
                ctx.push(format!("```"));
                ctx.push(format!("{bin} [flags] [args]", bin = spec.bin));
                ctx.push(format!("```"));
                ctx.push(format!("\n<!-- [USAGE] -->"));
            }
            UsageMdDirective::EndToken => {
                ctx.plain = true;
            }
            UsageMdDirective::Plain(line) => {
                if ctx.plain {
                    ctx.push(line.clone());
                }
            }
        };
        Ok(ctx)
    }
}

#[derive(Error, Diagnostic, Debug)]
#[error("Error parsing markdown directive")]
#[diagnostic()]
struct MarkdownError {
    msg: String,

    #[source_code]
    src: String,

    #[label("{msg}")]
    err_span: SourceSpan,
}

impl FromStr for UsageMdDirective {
    type Err = miette::Error;
    fn from_str(line: &str) -> Result<Self, Self::Err> {
        if line == "<!-- [USAGE] -->" {
            return Ok(UsageMdDirective::EndToken);
        }
        let directive = if let Some(x) = regex!(r#"<!-- \[USAGE\] (.*) -->"#).captures(line) {
            let doc: KdlDocument = x.get(1).unwrap().as_str().parse()?;
            if !doc.nodes().len() == 1 {
                bail!("only one node allowed in usage directive");
            }
            let node = doc.nodes().first().unwrap();
            let err = |msg: String, span| MarkdownError {
                msg,
                src: doc.to_string(),
                err_span: span,
            };
            let get_string = |node: &KdlNode, key: &'static str| {
                node.get(key)
                    .ok_or_else(|| err(format!("{key} is required"), *node.span()))?
                    .value()
                    .as_string()
                    .map(|s| s.to_string())
                    .ok_or_else(|| err(format!("{key} must be a string"), *node.span()))
            };
            match node.name().value() {
                "load" => UsageMdDirective::Load {
                    file: PathBuf::from(get_string(node, "file")?),
                },
                "usage_overview" => UsageMdDirective::UsageOverview,
                "global_args" => UsageMdDirective::GlobalArgs,
                "global_flags" => UsageMdDirective::GlobalFlags,
                "command_index" => UsageMdDirective::CommandIndex,
                "commands" => UsageMdDirective::Commands,
                _ => Err(err("unknown directive type".into(), *node.name().span()))?,
            }
        } else {
            UsageMdDirective::Plain(line.into())
        };
        Ok(directive)
    }
}
