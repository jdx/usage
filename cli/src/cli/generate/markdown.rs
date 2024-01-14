use std::fmt::{Display, Formatter};
use std::path::{Path, PathBuf};
use std::sync::Mutex;
use std::{env, fs};

use clap::Args;
use contracts::requires;
use kdl::{KdlDocument, KdlNode};
use miette::{IntoDiagnostic, NamedSource, SourceOffset, SourceSpan};
use strum::EnumIs;
use tera::Tera;
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
            .try_fold(UsageMdContext::new(inject), |ctx, d| d.run(ctx))?
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

const USAGE_TITLE_TEMPLATE: &str = r#"
# {{spec.name}}
"#;

const USAGE_OVERVIEW_TEMPLATE: &str = r#"
## Usage

```bash
{{spec.usage}}
```
"#;

const CONFIG_TEMPLATE: &str = r#"
### `!KEY!`

!ENV!
!DEFAULT!

!HELP!
!LONG_HELP!
"#;

const COMMANDS_INDEX_TEMPLATE: &str = r#"
## Commands Index

{% for cmd in commands -%}
* [`{{ cmd.full_cmd | join(sep=" ") }}`](#{{ cmd.full_cmd | join(sep=" ") | slugify }})
{% endfor %}
"#;

const COMMANDS_TEMPLATE: &str = r#"
### `{{ cmd.full_cmd | join(sep=" ") }}`

{% if cmd.before_long_help -%}
{{ cmd.before_long_help }}
{% elif cmd.before_help -%}
{{ cmd.before_help }}
{% endif -%}

{% if cmd.aliases -%}
#### Aliases

{% for alias in cmd.aliases -%}
* `{{ alias }}`
{% endfor -%}
{% endif -%}

{% if cmd.args -%}
#### Args

{% for arg in cmd.args -%}
* `{{ arg.usage }}` – {{ arg.long_help | default(value=arg.help) }}
{% endfor -%}
{% endif %}

{% if cmd.flags -%}
#### Flags

{% for flag in cmd.flags -%}
* `{{ flag.usage }}` – {{ flag.long_help | default(value=flag.help) }}
{% endfor -%}
{% endif -%}

{% if cmd.long_help -%}
{{ cmd.long_help }}
{% elif cmd.help -%}
{{ cmd.help }}
{% endif -%}

{% if cmd.after_long_help -%}
{{ cmd.after_long_help }}
{% elif cmd.after_help -%}
{{ cmd.after_help }}
{% endif -%}
"#;

#[derive(Debug, EnumIs)]
#[strum(serialize_all = "snake_case")]
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
    root: PathBuf,
    spec: Option<Spec>,
    out: Mutex<Vec<String>>,
    tera: tera::Context,
}

impl UsageMdContext {
    fn new(inject: &Path) -> Self {
        Self {
            plain: true,
            spec: None,
            out: Mutex::new(vec![]),
            tera: tera::Context::new(),
            root: inject.parent().unwrap().to_path_buf(),
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
                let file = match file.is_relative() {
                    true => ctx.root.join(file),
                    false => file.to_path_buf(),
                };
                let spec: Spec = Spec::parse_file(&file)?.0;
                ctx.tera.insert("spec", &spec.clone());
                let commands: Vec<_> = gather_subcommands(&[&spec.cmd])
                    .into_iter()
                    .filter(|c| !c.hide)
                    .collect();
                ctx.tera.insert("commands", &commands);
                ctx.spec = Some(spec);
                ctx.push(self.to_string());
            }
            UsageMdDirective::Title => {
                ensure!(ctx.spec.is_some(), "spec must be loaded before title");
                ctx.plain = false;
                ctx.push(self.to_string());
                ctx.push(render_template(USAGE_TITLE_TEMPLATE, &ctx.tera)?);
                ctx.push("<!-- [USAGE] -->".to_string());
            }
            UsageMdDirective::UsageOverview => {
                ctx.plain = false;
                ctx.push(self.to_string());
                ctx.push(render_template(USAGE_OVERVIEW_TEMPLATE, &ctx.tera)?);
                ctx.push("<!-- [USAGE] -->".to_string());
            }
            UsageMdDirective::GlobalArgs => {
                ctx.plain = false;
                let spec = ctx.spec.as_ref().unwrap();

                ctx.push(self.to_string());
                let args = spec.cmd.args.iter().filter(|a| !a.hide).collect::<Vec<_>>();
                if !args.is_empty() {
                    for arg in args {
                        // let name = &arg.usage();
                        let name = "USAGE";
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
                ctx.push(self.to_string());
                ctx.push(render_template(COMMANDS_INDEX_TEMPLATE, &ctx.tera)?);
                ctx.push("<!-- [USAGE] -->".to_string());
            }
            UsageMdDirective::Commands => {
                ctx.plain = false;
                let spec = ctx.spec.as_ref().unwrap();
                ctx.push(self.to_string());
                let commands = gather_subcommands(&[&spec.cmd]);
                for cmd in &commands {
                    let mut tctx = ctx.tera.clone();
                    tctx.insert("cmd", &cmd);
                    ctx.push(render_template(COMMANDS_TEMPLATE, &tctx)?);
                }
                ctx.push("<!-- [USAGE] -->".to_string());
            }
            UsageMdDirective::Config => {
                ctx.plain = false;
                let spec = ctx.spec.as_ref().unwrap();
                ctx.push(self.to_string());
                ctx.push(print_config(&spec.config)?);
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

fn render_template(template: &str, tctx: &tera::Context) -> miette::Result<String> {
    let out = Tera::one_off(template, tctx, false).into_diagnostic()?;
    Ok(out)
}

fn gather_subcommands(cmds: &[&SchemaCmd]) -> Vec<SchemaCmd> {
    let mut subcommands = vec![];
    for cmd in cmds {
        if cmd.hide {
            continue;
        }
        if !cmd.name.is_empty() {
            subcommands.push((*cmd).clone());
        }
        let more = gather_subcommands(&cmd.subcommands.values().collect::<Vec<_>>());
        subcommands.extend(more);
    }
    subcommands
}

fn print_config(config: &SpecConfig) -> miette::Result<String> {
    let mut all = vec![];
    for (key, prop) in &config.props {
        let mut out = CONFIG_TEMPLATE.to_string();
        let mut tmpl = |k, d: String| {
            out = out.replace(k, &d);
        };
        tmpl("!KEY!", key.to_string());
        // out = out.replace("!KEY!", &format!("### `{key}`"));
        if let Some(env) = &prop.env {
            tmpl("!ENV!", format!("* env: `{env}`"));
            // out = out.replace("!ENV!", &format!("* env: `{env}`"));
        }
        if let Some(default) = prop.default_note.clone().or_else(|| prop.default.clone()) {
            tmpl("!DEFAULT!", format!("* default: `{default}`"));
            // out = out.replace("!DEFAULT!", &format!("* default: `{default}`"));
        }
        if let Some(help) = prop.long_help.clone().or(prop.help.clone()) {
            // out = out.replace("!HELP!", &format!("* help: `{help}`"));
            tmpl("!HELP!", help);
        }
        out = regex!(r#"!.+!\n"#)
            .replace_all(&out, "")
            .trim_start()
            .trim_end()
            .to_string()
            + "\n";
        all.push(out)
        // TODO: data type
        // TODO: show which commands use this prop ctx.push("Used by commnds: global|*".to_string());
    }
    Ok(all.join("\n"))
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
                "config" => UsageMdDirective::Config,
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
