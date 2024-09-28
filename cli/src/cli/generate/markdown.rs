use std::path::PathBuf;

use clap::Args;
use usage::docs::markdown::MarkdownRenderer;
use usage::Spec;

#[derive(Args)]
#[clap(visible_alias = "md")]
pub struct Markdown {
    /// A usage spec taken in as a file
    #[clap(short, long)]
    file: PathBuf,
    // /// Pass a usage spec in an argument instead of a file
    // #[clap(short, long, required_unless_present = "file", overrides_with = "file")]
    // spec: Option<String>,
    /// Render each subcommand as a separate markdown file
    #[clap(short, long, requires = "out_dir", conflicts_with = "out_file")]
    multi: bool,

    /// Prefix to add to all URLs
    #[clap(long)]
    url_prefix: Option<String>,

    // /// Escape HTML in markdown
    // #[clap(long)]
    // html_escape: bool,
    /// Output markdown files to this directory
    #[clap(long, value_hint = clap::ValueHint::DirPath)]
    out_dir: Option<PathBuf>,

    #[clap(long, value_hint = clap::ValueHint::FilePath, required_unless_present = "multi")]
    out_file: Option<PathBuf>,
}

impl Markdown {
    pub fn run(&self) -> miette::Result<()> {
        let write = |path: &PathBuf, md: &str| -> miette::Result<()> {
            println!("writing to {}", path.display());
            xx::file::write(path, format!("{}\n", md.trim()))?;
            Ok(())
        };
        let (spec, _) = Spec::parse_file(&self.file)?;
        let mut ctx = MarkdownRenderer::new(&spec);
        if let Some(url_prefix) = &self.url_prefix {
            ctx = ctx.with_url_prefix(url_prefix);
        }
        if self.multi {
            ctx = ctx.with_multi(true);
            let commands = spec.cmd.all_subcommands().into_iter().filter(|c| !c.hide);
            for cmd in commands {
                let md = ctx.render_cmd(cmd)?;
                let dir = cmd
                    .full_cmd
                    .iter()
                    .take(cmd.full_cmd.len() - 1)
                    .map(|c| c.to_string())
                    .collect::<Vec<_>>()
                    .join("/");
                let path = self
                    .out_dir
                    .as_ref()
                    .unwrap()
                    .join(dir)
                    .join(format!("{}.md", cmd.name));
                write(&path, &md)?;
            }
            let md_idx = ctx.render_index()?;
            let path_idx = self.out_dir.as_ref().unwrap().join("index.md");
            write(&path_idx, &md_idx)?;
        } else {
            let md = ctx.render_spec()?;
            let path = self.out_file.as_ref().unwrap();
            write(path, &md)?;
        }
        Ok(())
    }
}
