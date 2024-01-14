use std::fmt::{Display, Formatter};
use std::fs;
use std::iter::once;
use std::path::{Path, PathBuf};
use std::sync::Mutex;

use clap::Args;
use contracts::requires;
use kdl::{KdlDocument, KdlNode};
use miette::{IntoDiagnostic, NamedSource, SourceOffset, SourceSpan};
use strum::EnumIs;
use thiserror::Error;
use xx::{context, file};

use usage::parse::config::SpecConfig;
use usage::{SchemaCmd, Spec};

use crate::errors::UsageCLIError;

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

    fn inject_file(&self, inject: &Path) -> miette::Result<()> {
        let raw = file::read_to_string(inject).into_diagnostic()?;
        context::set_load_root(inject.parent().unwrap().to_path_buf());
        let out = parse_readme_directives(inject, &raw)?
            .into_iter()
            .try_fold(UsageMdContext::new(), |ctx, d| d.run(ctx))?
            .out
            .lock()
            .unwrap()
            .join("\n")
            + "\n";
        print!("{}", out);
        fs::write(inject, out).into_diagnostic()?;
        Ok(())
    }

    //noinspection RsFormatMacroWithoutFormatArguments
    // fn _print(&self, spec: &Spec, dir: &Path, cmds: &[&SchemaCmd]) -> miette::Result<()> {
    //     let cmd = cmds.last().unwrap();
    //     let mut out = vec![];
    //     let cmd_path = cmds
    //         .iter()
    //         .map(|c| c.name.as_str())
    //         .collect::<Vec<_>>()
    //         .join(" ");
    //     out.push(format!("# {cmd_path}"));
    //     out.push("## Usage".to_string());
    //     out.push("```".to_string());
    //     out.push(format!("{cmd_path} [flags] [args]"));
    //     out.push("```".to_string());
    //
    //     let args = cmd.args.iter().filter(|a| !a.hide).collect::<Vec<_>>();
    //     if !args.is_empty() {
    //         out.push("## Args".to_string());
    //         for arg in args {
    //             let name = &arg.usage();
    //             if let Some(about) = &arg.long_help {
    //                 out.push(format!("### {name}"));
    //                 out.push(about.to_string());
    //             } else if let Some(about) = &arg.help {
    //                 out.push(format!("- `{name}`: {about}"));
    //             } else {
    //                 out.push(format!("- `{name}`"));
    //             }
    //         }
    //     }
    //     let flags = cmd.flags.iter().filter(|f| !f.hide).collect::<Vec<_>>();
    //     if !flags.is_empty() {
    //         out.push("## Flags".to_string());
    //         for flag in flags {
    //             let name = flag.usage();
    //             if let Some(about) = &flag.long_help {
    //                 out.push(format!("### {name}"));
    //                 out.push(about.to_string());
    //             } else if let Some(about) = &flag.help {
    //                 out.push(format!("- `{name}`: {about}"));
    //             } else {
    //                 out.push(format!("- `{name}`"));
    //             }
    //         }
    //     }
    //     let subcommands = cmd
    //         .subcommands
    //         .values()
    //         .filter(|c| !c.hide)
    //         .collect::<Vec<_>>();
    //     if !subcommands.is_empty() {
    //         out.push("## Commands".to_string());
    //         for cmd in subcommands {
    //             let name = cmd.name.as_str();
    //             if let Some(about) = &cmd.help {
    //                 out.push(format!("- [`{name}`](./{name}): {about}"));
    //             } else {
    //                 out.push(format!("- [`{name}`](./{name})"));
    //             }
    //         }
    //     }
    //
    //     let dir = dir.join(&cmd.name);
    //     let file = if cmd.subcommands.is_empty() {
    //         let dir = dir.parent().unwrap();
    //         fs::create_dir_all(dir).into_diagnostic()?;
    //         dir.join(format!("{}.md", cmd.name))
    //     } else {
    //         fs::create_dir_all(&dir).into_diagnostic()?;
    //         dir.join("index.md")
    //     };
    //     let mut file = File::create(file).into_diagnostic()?;
    //     writeln!(file, "{}", out.join("\n")).into_diagnostic()?;
    //
    //     for cmd in cmd.subcommands.values() {
    //         let cmds = cmds.iter().cloned().chain(once(cmd)).collect::<Vec<_>>();
    //         self._print(spec, &dir, &cmds)?;
    //     }
    //     Ok(())
    // }
}

#[derive(Debug, EnumIs)]
enum UsageMdDirective {
    Load { file: PathBuf },
    Title,
    UsageOverview,
    GlobalArgs,
    GlobalFlags,
    CommandIndex,
    Commands,
    Config,
    EndToken,
    Plain(String),
}

impl Display for UsageMdDirective {
    fn fmt(&self, f: &mut Formatter<'_>) -> std::fmt::Result {
        match self {
            UsageMdDirective::Load { file } => {
                write!(f, "<!-- [USAGE] load file=\"{}\" -->", file.display())
            }
            UsageMdDirective::Title => write!(f, "<!-- [USAGE] title -->"),
            UsageMdDirective::UsageOverview => write!(f, "<!-- [USAGE] usage_overview -->"),
            UsageMdDirective::GlobalArgs => write!(f, "<!-- [USAGE] global_args -->"),
            UsageMdDirective::GlobalFlags => write!(f, "<!-- [USAGE] global_flags -->"),
            UsageMdDirective::CommandIndex => write!(f, "<!-- [USAGE] command_index -->"),
            UsageMdDirective::Commands => write!(f, "<!-- [USAGE] commands -->"),
            UsageMdDirective::Config => write!(f, "<!-- [USAGE] config -->"),
            UsageMdDirective::EndToken => write!(f, "<!-- [USAGE] -->"),
            UsageMdDirective::Plain(line) => write!(f, "{}", line),
        }
    }
}

struct UsageMdContext {
    plain: bool,
    spec: Option<Spec>,
    out: Mutex<Vec<String>>,
}

impl UsageMdContext {
    fn new() -> Self {
        Self {
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
    //noinspection RsFormatMacroWithoutFormatArguments
    #[requires(self.requires_spec() -> ctx.spec.is_some())]
    #[requires(self.is_load() -> ctx.spec.is_none())]
    fn run(&self, mut ctx: UsageMdContext) -> miette::Result<UsageMdContext> {
        match self {
            UsageMdDirective::Load { file } => {
                let file = context::prepend_load_root(file);
                ctx.spec = Some(Spec::parse_file(&file)?.0);
                ctx.push(self.to_string());
            }
            UsageMdDirective::Title => {
                ensure!(ctx.spec.is_some(), "spec must be loaded before title");
                ctx.plain = false;
                let spec = ctx.spec.as_ref().unwrap();
                ctx.push(self.to_string());
                ctx.push(format!("# {name}", name = spec.name));
                ctx.push("<!-- [USAGE] -->".to_string());
            }
            UsageMdDirective::UsageOverview => {
                ctx.plain = false;
                let spec = ctx.spec.as_ref().unwrap();

                ctx.push(self.to_string());
                ctx.push("```".to_string());
                ctx.push(format!("{bin} [flags] [args]", bin = spec.bin));
                ctx.push("```".to_string());
                ctx.push("<!-- [USAGE] -->".to_string());
            }
            UsageMdDirective::GlobalArgs => {
                ctx.plain = false;
                let spec = ctx.spec.as_ref().unwrap();

                ctx.push(self.to_string());
                let args = spec.cmd.args.iter().filter(|a| !a.hide).collect::<Vec<_>>();
                if !args.is_empty() {
                    for arg in args {
                        let name = &arg.usage();
                        if let Some(about) = &arg.long_help {
                            ctx.push(format!("### {name}", name = name));
                            ctx.push(about.to_string());
                        } else if let Some(about) = &arg.help {
                            ctx.push(format!("- `{name}`: {about}", name = name, about = about));
                        } else {
                            ctx.push(format!("- `{name}`", name = name));
                        }
                    }
                }
                ctx.push("<!-- [USAGE] -->".to_string());
            }
            UsageMdDirective::GlobalFlags => {
                ctx.plain = false;
                let spec = ctx.spec.as_ref().unwrap();

                ctx.push(self.to_string());
                let flags = spec
                    .cmd
                    .flags
                    .iter()
                    .filter(|f| !f.hide)
                    .collect::<Vec<_>>();
                if !flags.is_empty() {
                    for flag in flags {
                        let name = flag.usage();
                        if let Some(about) = &flag.long_help {
                            ctx.push(format!("### {name}", name = name));
                            ctx.push(about.to_string());
                        } else if let Some(about) = &flag.help {
                            ctx.push(format!("- `{name}`: {about}", name = name, about = about));
                        } else {
                            ctx.push(format!("- `{name}`", name = name));
                        }
                    }
                }
                ctx.push("<!-- [USAGE] -->".to_string());
            }
            UsageMdDirective::CommandIndex => {
                ctx.plain = false;
                let spec = ctx.spec.as_ref().unwrap();
                ctx.push(self.to_string());
                print_commands_index(&ctx, &[&spec.cmd])?;
                ctx.push("<!-- [USAGE] -->".to_string());
            }
            UsageMdDirective::Commands => {
                ctx.plain = false;
                let spec = ctx.spec.as_ref().unwrap();
                ctx.push(self.to_string());
                print_commands(&ctx, &[&spec.cmd])?;
                ctx.push("<!-- [USAGE] -->".to_string());
            }
            UsageMdDirective::Config => {
                ctx.plain = false;
                let spec = ctx.spec.as_ref().unwrap();
                ctx.push(self.to_string());
                print_config(&ctx, &spec.config)?;
                ctx.push("<!-- [USAGE] -->".to_string());
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

    fn requires_spec(&self) -> bool {
        !matches!(
            self,
            UsageMdDirective::Load { .. } | UsageMdDirective::Plain(_)
        )
    }
}

fn print_commands_index(ctx: &UsageMdContext, cmds: &[&SchemaCmd]) -> miette::Result<()> {
    let subcommands = cmds[cmds.len() - 1]
        .subcommands
        .values()
        .filter(|c| !c.hide)
        .collect::<Vec<_>>();
    for cmd in subcommands {
        let cmds = cmds.iter().cloned().chain(once(cmd)).collect::<Vec<_>>();
        let full_name = cmds
            .iter()
            .skip(1)
            .map(|c| c.name.as_str())
            .collect::<Vec<_>>()
            .join(" ");
        let slug = full_name.replace(' ', "-");
        ctx.push(format!("- [`{full_name}`](#{slug})",));
        print_commands_index(ctx, &cmds)?;
    }

    Ok(())
}

fn print_commands(ctx: &UsageMdContext, cmds: &[&SchemaCmd]) -> miette::Result<()> {
    let subcommands = cmds[cmds.len() - 1]
        .subcommands
        .values()
        .filter(|c| !c.hide)
        .collect::<Vec<_>>();
    for cmd in subcommands {
        let cmds = cmds.iter().cloned().chain(once(cmd)).collect::<Vec<_>>();
        let full_name = cmds
            .iter()
            .skip(1)
            .map(|c| c.name.as_str())
            .collect::<Vec<_>>()
            .join(" ");
        ctx.push(format!("### `{full_name}`"));
        print_commands(ctx, &cmds)?;
    }

    Ok(())
}

fn print_config(ctx: &UsageMdContext, config: &SpecConfig) -> miette::Result<()> {
    for (key, prop) in &config.props {
        ctx.push(format!("### `{key}`", key = key));
        if let Some(env) = &prop.env {
            ctx.push(format!("env: `{env}`", env = env));
        }
        if !prop.default.is_null() {
            ctx.push(format!("default: `{default}`", default = prop.default));
        }
        if let Some(help) = &prop.help {
            ctx.push(help.to_string());
        }
        if let Some(long_help) = &prop.long_help {
            ctx.push(long_help.to_string());
        }
        ctx.push("Used by commnds: global|*".to_string());
    }
    Ok(())
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

fn parse_readme_directives(path: &Path, full: &str) -> miette::Result<Vec<UsageMdDirective>> {
    let mut directives = vec![];
    for (line_num, line) in full.lines().enumerate() {
        if line == "<!-- [USAGE] -->" {
            directives.push(UsageMdDirective::EndToken);
            continue;
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
                "title" => UsageMdDirective::Title,
                "usage_overview" => UsageMdDirective::UsageOverview,
                "global_args" => UsageMdDirective::GlobalArgs,
                "global_flags" => UsageMdDirective::GlobalFlags,
                "cnfig" => UsageMdDirective::Config,
                "command_index" => UsageMdDirective::CommandIndex,
                "commands" => UsageMdDirective::Commands,
                k => Err(UsageCLIError::MarkdownParseError {
                    message: format!("unknown directive type: {k}"),
                    src: get_named_source(path, full),
                    label: get_source_span(full, line_num, k.len()),
                })?,
            }
        } else {
            UsageMdDirective::Plain(line.into())
        };
        directives.push(directive);
    }
    Ok(directives)
}

fn get_named_source(path: &Path, full: &str) -> NamedSource {
    NamedSource::new(path.to_string_lossy(), full.to_string())
}

fn get_source_span(full: &str, line_num: usize, len: usize) -> SourceSpan {
    let offset = SourceOffset::from_location(full, line_num + 1, 14).offset();
    (offset, len).into()
}
