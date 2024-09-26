use std::collections::HashMap;
use std::path::{Path, PathBuf};

use clap::Args;
use contracts::requires;
use kdl::{KdlDocument, KdlNode};
use miette::{Context, IntoDiagnostic, NamedSource, SourceOffset, SourceSpan};
use strum::EnumIs;
use tera::Tera;
use thiserror::Error;
use xx::file;

use usage::spec::config::SpecConfig;
use usage::{Spec, SpecCommand};

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
    /// <!-- [USAGE] load file="path/to/usage.kdl" -->
    #[clap(
        required_unless_present = "out_dir", verbatim_doc_comment, value_hint = clap::ValueHint::FilePath
    )]
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
        let directives = parse_readme_directives(inject, &raw)?;
        let b = MarkdownBuilder::new(inject, directives);
        for (file, out) in b.load()?.render()? {
            print!("{}", out);
            file::write(file, &out).into_diagnostic()?;
        }
        Ok(())
    }
}

const USAGE_TITLE_TEMPLATE: &str = r#"
# {{name}}
"#;

const USAGE_OVERVIEW_TEMPLATE: &str = r#"
## Usage

```bash
{{usage}}
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
## CLI Command Reference

{% for cmd in commands -%}
{% if multi_dir -%}
* [`{{ bin }} {{ cmd.full_cmd | join(sep=" ") }}`]({{ multi_dir }}/{% for c in cmd.full_cmd %}{{ c | slugify }}{% if not loop.last %}/{% endif %}{% endfor %}.md)
{% elif cmd.deprecated -%}
* ~~[`{{ bin }} {{ cmd.full_cmd | join(sep=" ") }}`](#{{ bin | slugify }}-{{ cmd.full_cmd | join(sep=" ") | slugify }}-deprecated)~~ [deprecated]
{% else -%}
* [`{{ bin }} {{ cmd.full_cmd | join(sep=" ") }}`](#{{ bin | slugify }}-{{ cmd.full_cmd | join(sep=" ") | slugify }})
{% endif -%}
{% endfor -%}
"#;

const COMMAND_TEMPLATE: &str = r##"
{% set deprecated = "" %}{% if cmd.deprecated %}{% set deprecated = "~~" %}{% endif %}
{{ header }} {{deprecated}}`{{ bin }} {{ cmd.full_cmd | join(sep=" ") }}`{{deprecated}}{% if cmd.deprecated %} [deprecated]{% endif -%}

{% if cmd.before_long_help %}

{{ cmd.before_long_help | trim }}

{% elif cmd.before_help %}
{{ cmd.before_help | trim }}
{% endif -%}

{% if cmd.aliases %}

###### Aliases: `{{ cmd.aliases | join(sep="`, `") }}`{{""-}}
{% endif -%}

{% if cmd.long_help %}

{{ cmd.long_help | trim -}}
{% elif cmd.help %}

{{ cmd.help | trim -}}
{% endif -%}

{% for name, cmd in cmd.subcommands -%}
{% if loop.first %}
{{header}}# Subcommands
{% endif %}
* `{{ cmd.usage }}` - {{ cmd.help -}}
{% endfor -%}

{% if cmd.args -%}
{% for arg in cmd.args %}

###### Arg `{{ arg.usage }}`

{% if arg.required %}(required){% endif -%}
{{ arg.long_help | default(value=arg.help) -}}

{% endfor -%}
{% endif -%}
{% if cmd.flags -%}
{% for flag in cmd.flags %}

{% if flag.deprecated -%}
##### Flag ~~`{{ flag.usage }}`~~ [deprecated]
{% else -%}
##### Flag `{{ flag.usage }}`
{% endif %}
{{ flag.long_help | default(value=flag.help) -}}
{% endfor -%}
{% endif -%}

{% for ex in cmd.examples -%}
{% if loop.first %}

##### Examples
{% endif %}
{% if ex.header -%}
###### {{ ex.header }}
{% endif %}
```{{ ex.lang | default(value="") }}
{{ ex.code }}
```
{% if ex.help %}
{{ ex.help -}}
{% endif -%}
{% endfor -%}

{% if cmd.after_long_help %}

{{ cmd.after_long_help | trim }}
{% elif cmd.after_help %}

{{ cmd.after_help | trim }}
{% endif -%}
"##;

#[derive(Debug, EnumIs)]
#[strum(serialize_all = "snake_case")]
enum UsageMdDirective {
    Load {
        token: String,
        file: PathBuf,
    },
    Title {
        token: String,
    },
    UsageOverview {
        token: String,
    },
    GlobalArgs {
        token: String,
    },
    GlobalFlags {
        token: String,
    },
    Commands {
        token: String,
        multi_dir: Option<String>,
    },
    Config {
        token: String,
    },
    EndToken {},
    Plain {
        token: String,
    },
}

fn render_template(template: &str, ctx: &tera::Context) -> miette::Result<String> {
    let out = Tera::one_off(template, ctx, false).into_diagnostic()?;
    Ok(out)
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
            directives.push(UsageMdDirective::EndToken {});
            continue;
        }
        let directive = if let Some(x) = regex!(r#"<!-- \[USAGE\] (.*) -->"#).captures(line) {
            let doc: KdlDocument = x.get(1).unwrap().as_str().parse()?;
            if !doc.nodes().len() == 1 {
                miette::bail!("only one node allowed in usage directive");
            }
            let node = doc.nodes().first().unwrap();
            let err = |msg: String, span| MarkdownError {
                msg,
                src: doc.to_string(),
                err_span: span,
            };
            let get_prop = |node: &KdlNode, key: &'static str| {
                Ok(node.get(key).map(|v| v.value().clone()).clone())
            };
            let get_string = |node: &KdlNode, key: &'static str| match get_prop(node, key)? {
                Some(v) => v
                    .as_string()
                    .map(|s| s.to_string())
                    .ok_or_else(|| err(format!("{key} must be a string"), *node.span()))
                    .map(Some),
                None => Ok(None),
            };
            match node.name().value() {
                "load" => UsageMdDirective::Load {
                    file: PathBuf::from(
                        get_string(node, "file")
                            .with_context(|| miette!("load directive must have a file"))?
                            .ok_or_else(|| {
                                err("load directive must have a file".into(), *node.span())
                            })?,
                    ),
                    token: line.into(),
                },
                "title" => UsageMdDirective::Title { token: line.into() },
                "usage_overview" => UsageMdDirective::UsageOverview { token: line.into() },
                "global_args" => UsageMdDirective::GlobalArgs { token: line.into() },
                "global_flags" => UsageMdDirective::GlobalFlags { token: line.into() },
                "config" => UsageMdDirective::Config { token: line.into() },
                "commands" => UsageMdDirective::Commands {
                    token: line.into(),
                    multi_dir: get_string(node, "multi_dir")?,
                },
                k => Err(UsageCLIError::MarkdownParseError {
                    message: format!("unknown directive type: {k}"),
                    src: get_named_source(path, full),
                    label: get_source_span(full, line_num, k.len()),
                })?,
            }
        } else {
            UsageMdDirective::Plain { token: line.into() }
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

struct MarkdownBuilder {
    inject: PathBuf,
    root: PathBuf,
    directives: Vec<UsageMdDirective>,

    spec: Option<Spec>,
}

impl MarkdownBuilder {
    fn new(inject: &Path, directives: Vec<UsageMdDirective>) -> Self {
        let inject = inject.to_path_buf();
        Self {
            root: inject.parent().unwrap().to_path_buf(),
            inject,
            directives,
            spec: None,
        }
    }

    #[requires(self.spec.is_none())]
    fn load(mut self) -> miette::Result<Self> {
        for dct in &self.directives {
            if let UsageMdDirective::Load { file, .. } = dct {
                let file = match file.is_relative() {
                    true => self.root.join(file),
                    false => file.to_path_buf(),
                };
                let (spec, _) = Spec::parse_file(&file)?;
                self.spec = Some(spec);
            }
        }
        ensure!(self.spec.is_some(), "spec must be loaded before title");
        Ok(self)
    }

    #[requires(self.spec.is_some())]
    fn render(&self) -> miette::Result<HashMap<PathBuf, String>> {
        let spec = self.spec.as_ref().unwrap();
        let commands = gather_subcommands(&[&spec.cmd]);
        let ctx = tera::Context::from_serialize(&self.spec).into_diagnostic()?;
        let mut outputs = HashMap::new();
        let mut plain = true;
        for dct in &self.directives {
            let main = outputs
                .entry(self.inject.clone())
                .or_insert_with(std::vec::Vec::new);
            match dct {
                UsageMdDirective::Plain { .. } | UsageMdDirective::Load { .. } => {}
                UsageMdDirective::EndToken { .. } => {
                    plain = true;
                }
                _ => plain = false,
            }
            match dct {
                UsageMdDirective::Load { token, .. } => {
                    main.push(token.clone());
                }
                UsageMdDirective::Title { token } => {
                    main.push(token.clone());
                    main.push(render_template(USAGE_TITLE_TEMPLATE, &ctx)?);
                    main.push("<!-- [USAGE] -->".to_string());
                }
                UsageMdDirective::UsageOverview { token } => {
                    main.push(token.clone());
                    main.push(render_template(USAGE_OVERVIEW_TEMPLATE, &ctx)?);
                    main.push("<!-- [USAGE] -->".to_string());
                }
                UsageMdDirective::GlobalArgs { token } => {
                    main.push(token.clone());
                    let args = spec.cmd.args.iter().filter(|a| !a.hide).collect::<Vec<_>>();
                    if !args.is_empty() {
                        for arg in args {
                            // let name = &arg.usage();
                            let name = "USAGE";
                            if let Some(about) = &arg.long_help {
                                main.push(format!("### {name}", name = name));
                                main.push(about.to_string());
                            } else if let Some(about) = &arg.help {
                                main.push(format!("- `{name}`: {about}",));
                            } else {
                                main.push(format!("- `{name}`", name = name));
                            }
                        }
                    }
                    main.push("<!-- [USAGE] -->".to_string());
                }
                UsageMdDirective::GlobalFlags { token } => {
                    main.push(token.clone());
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
                                main.push(format!("### {name}"));
                                main.push(about.to_string());
                            } else if let Some(about) = &flag.help {
                                main.push(format!("- `{name}`: {about}",));
                            } else {
                                main.push(format!("- `{name}`"));
                            }
                        }
                    }
                    main.push("<!-- [USAGE] -->".to_string());
                }
                UsageMdDirective::Commands { token, multi_dir } => {
                    main.push(token.clone());
                    let mut ctx = ctx.clone();
                    ctx.insert("commands", &commands);
                    ctx.insert("multi_dir", &multi_dir);
                    main.push(render_template(COMMANDS_INDEX_TEMPLATE, &ctx)?);
                    for cmd in &commands {
                        let mut ctx = ctx.clone();
                        ctx.insert("cmd", &cmd);
                        let output_file = match &multi_dir {
                            Some(multi_dir) => {
                                ctx.insert("header", "#");
                                self.root
                                    .join(multi_dir)
                                    .join(format!("{}.md", cmd.full_cmd.join("/")))
                            }
                            None => {
                                ctx.insert("header", &"#".repeat(cmd.full_cmd.len() + 1));
                                self.inject.clone()
                            }
                        };
                        let out = outputs.entry(output_file).or_insert_with(Vec::new);
                        let s = render_template(COMMAND_TEMPLATE, &ctx)?.trim().to_string();
                        out.push(s + "\n");
                    }
                    let main = outputs.get_mut(&self.inject).unwrap();
                    main.push("<!-- [USAGE] -->".to_string());
                }
                UsageMdDirective::Config { token } => {
                    main.push(token.clone());
                    main.push(print_config(&spec.config)?);
                    main.push("<!-- [USAGE] -->".to_string());
                }
                UsageMdDirective::EndToken { .. } => {}
                UsageMdDirective::Plain { token } => {
                    if plain {
                        main.push(token.clone());
                    }
                }
            };
        }
        Ok(outputs
            .into_iter()
            .map(|(k, v)| (k, v.join("\n").trim_start().trim_end().to_string() + "\n"))
            .collect())
    }
}

fn gather_subcommands(cmds: &[&SpecCommand]) -> Vec<SpecCommand> {
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
