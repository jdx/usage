use crate::docs::markdown::tera::TERA;
use crate::docs::models::Spec;
use crate::error::UsageErr;
use itertools::Itertools;
use regex::Regex;
use std::sync::LazyLock;

/// One of the templates used to generate Markdown documentation.
///
/// A renderer starts with a complete built-in template set. Replacing one member keeps the
/// others available, including through Tera's `{% include %}` directive. This lets an adopter
/// change the document shell or the presentation of one kind of item without copying the whole
/// theme.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum MarkdownTemplate {
    /// A single-file document containing the root and every subcommand.
    Spec,
    /// The landing page generated in multi-file mode.
    Index,
    /// A command and its arguments, flags, outputs, exits, and examples.
    Command,
    /// The details beneath one positional argument.
    Argument,
    /// The details beneath one flag.
    Flag,
    /// The configuration reference appended to a spec or written in multi-file mode.
    Config,
}

impl MarkdownTemplate {
    fn name(self) -> &'static str {
        match self {
            Self::Spec => "spec_template.md.tera",
            Self::Index => "index_template.md.tera",
            Self::Command => "cmd_template.md.tera",
            Self::Argument => "arg_template.md.tera",
            Self::Flag => "flag_template.md.tera",
            Self::Config => "config_template.md.tera",
        }
    }
}

/// An ANSI escape sequence: what `color_print::cstr!` leaves in help text.
static SGR: LazyLock<Regex> = LazyLock::new(|| Regex::new(r"\x1b\[[0-?]*[ -/]*[@-~]").unwrap());
/// A backtick span, or a bare `<` outside one.
static CODE_SPAN_OR_LT: LazyLock<Regex> = LazyLock::new(|| Regex::new(r"(`[^`]*`)|(<)").unwrap());

fn escape_md(value: &str, html_encode: bool) -> String {
    let mut in_fenced_code_block = false;
    // Help text is allowed to contain terminal styling. clap-era applications commonly build
    // their examples with `color_print::cstr!`, which embeds SGR sequences even when color is
    // disabled at runtime. Terminal styling has no meaning in generated Markdown, and leaving
    // it here publishes literal escape bytes in docs and downstream static sites.
    let value = SGR.replace_all(value, "");

    value
        .lines()
        .map(|line| {
            if !html_encode {
                return line.to_string();
            }
            // Indented code is handled before fence state. This is safe because
            // `replace_code_fences` always emits closing fences at column zero.
            if line.starts_with("    ") {
                return line.to_string();
            }
            if in_fenced_code_block {
                if line.trim_end() == "```" {
                    in_fenced_code_block = false;
                }
                return line.to_string();
            }
            // Support the conventional fence shape emitted by `replace_code_fences`
            // without attempting to parse the full Markdown specification.
            if line
                .strip_prefix("```")
                .is_some_and(|suffix| !suffix.starts_with('`'))
            {
                in_fenced_code_block = true;
                return line.to_string();
            }
            // replace '<' with '&lt;' but not inside code blocks
            CODE_SPAN_OR_LT
                .replace_all(line, |caps: &regex::Captures| {
                    if caps.get(1).is_some() {
                        caps.get(1).unwrap().as_str().to_string()
                    } else {
                        "&lt;".to_string()
                    }
                })
                .to_string()
        })
        .join("\n")
}

#[derive(Debug, Clone)]
pub struct MarkdownRenderer {
    pub(crate) spec: Spec,
    /// The config block as the spec wrote it, before any rendering.
    ///
    /// `new` renders the whole docs model eagerly, which happens *before* the builder methods
    /// that set `replace_pre_with_code_fences` and friends — and rendering marks each item
    /// done, so a later pass no-ops. `render_cmd` avoids this by re-deriving from the raw
    /// command its caller hands it; config has no such argument, so the raw form is kept here
    /// instead. Without it, `--replace-pre-with-code-fences` silently did nothing to a
    /// setting's long help.
    pub(crate) raw_config: crate::spec::config::SpecConfig,
    pub(crate) header_level: usize,
    pub(crate) multi: bool,
    url_prefix: Option<String>,
    html_encode: bool,
    replace_pre_with_code_fences: bool,
    templates: Vec<(MarkdownTemplate, String)>,
}

impl MarkdownRenderer {
    pub fn new(spec: crate::Spec) -> Self {
        let mut renderer = Self {
            raw_config: spec.config.clone(),
            spec: spec.into(),
            header_level: 1,
            multi: false,
            url_prefix: None,
            html_encode: true,
            replace_pre_with_code_fences: false,
            templates: Vec::new(),
        };
        let mut spec = renderer.spec.clone();
        spec.render_md(&renderer);
        renderer.spec = spec;
        renderer
    }

    /// The file name the settings page gets in `--multi` mode.
    ///
    /// One name, always. `settings.md` was the obvious choice and the wrong one: mise has a
    /// `settings` command, whose own page lands there, and config is written second — so for
    /// the very CLI this feature is aimed at, the command's page was silently replaced and the
    /// index linked both entries to the same file.
    ///
    /// Choosing between two names by whether a command is in the way would fix that and
    /// introduce something worse: the name would move when a command is added or removed
    /// between runs, leaving the abandoned one behind as a stale page nothing links but the
    /// docs site still serves. A fixed name cannot do that, and `configuration` is a name CLIs
    /// give to a *file*, not to a command — the command is called `config`.
    pub fn config_page(&self) -> &'static str {
        "configuration.md"
    }

    /// A visible top-level command whose own page would be written to [`Self::config_page`].
    ///
    /// Vanishingly unlikely, and silence is what made the `settings.md` collision hard to see,
    /// so the one case left is reported rather than guessed at.
    pub fn config_page_collision(&self) -> Option<&str> {
        let stem = self.config_page().trim_end_matches(".md");
        self.spec
            .cmd
            .subcommands
            .values()
            .find(|cmd| !cmd.hide && cmd.full_cmd == [stem])
            .map(|cmd| cmd.name.as_str())
    }

    pub fn with_header_level(mut self, header_level: usize) -> Self {
        self.header_level = header_level;
        self
    }

    pub fn with_multi(mut self, index: bool) -> Self {
        self.multi = index;
        self
    }

    pub fn with_url_prefix<S: Into<String>>(mut self, url_prefix: S) -> Self {
        self.url_prefix = Some(url_prefix.into());
        self
    }

    pub fn with_html_encode(mut self, html_encode: bool) -> Self {
        self.html_encode = html_encode;
        self
    }

    pub fn with_replace_pre_with_code_fences(mut self, replace_pre_with_code_fences: bool) -> Self {
        self.replace_pre_with_code_fences = replace_pre_with_code_fences;
        self
    }

    /// Replace one built-in Markdown template.
    ///
    /// Templates use [Tera](https://keats.github.io/tera/). A replacement may include any of the
    /// templates it did not replace; for example, a custom [`MarkdownTemplate::Spec`] can still
    /// contain `{% include "cmd_template.md.tera" %}`. Replacing the same member more than once
    /// uses the last value. Syntax and include errors are returned by the render method.
    pub fn with_template(mut self, template: MarkdownTemplate, source: impl Into<String>) -> Self {
        self.templates.push((template, source.into()));
        self
    }

    fn tera_ctx(&self) -> tera::Context {
        let mut ctx = tera::Context::new();
        ctx.insert("spec", &self.spec);
        ctx.insert("header_level", &self.header_level);
        ctx.insert("multi", &self.multi);
        ctx.insert("url_prefix", &self.url_prefix);
        ctx.insert("html_encode", &self.html_encode);
        ctx
    }

    /// Render with values that belong only to this page.
    ///
    /// A page used to clone the whole renderer — including the complete command tree — merely
    /// to insert one local into its stored context. Multi-page output paid that deep clone once
    /// per command. The context already has to be materialized for Tera, so enrich that directly.
    pub(crate) fn render_with(
        &self,
        template_name: &str,
        enrich: impl FnOnce(&mut tera::Context),
    ) -> Result<String, UsageErr> {
        let mut tera = TERA.clone();

        for (template, source) in &self.templates {
            tera.add_raw_template(template.name(), source)?;
        }

        let html_encode = self.html_encode;
        tera.register_filter(
            "escape_md",
            move |value: &tera::Value,
                  _: tera::Kwargs,
                  _: &tera::State|
                  -> tera::TeraResult<String> {
                let value = value.as_str().unwrap();
                let value = escape_md(value, html_encode);
                Ok(value)
            },
        );

        let mut ctx = self.tera_ctx();
        enrich(&mut ctx);
        Ok(tera.render(template_name, &ctx)?)
    }

    pub(crate) fn replace_code_fences(&self, md: String) -> String {
        if !self.replace_pre_with_code_fences {
            return md;
        }
        // TODO: handle fences inside of <pre> or <code>
        let mut in_code_block = false;
        let mut new_md = String::new();
        for line in md.lines() {
            if let Some(line) = line.strip_prefix("    ") {
                if in_code_block {
                    new_md.push_str(&format!("{line}\n"));
                } else {
                    new_md.push_str(&format!("```\n{line}\n"));
                    in_code_block = true;
                }
            } else {
                if in_code_block {
                    new_md.push_str("```\n");
                    in_code_block = false;
                }
                new_md.push_str(&format!("{line}\n"));
            }
        }
        if in_code_block {
            new_md.push_str("```\n");
        }
        new_md.replace("```\n\n```\n", "\n")
    }
}

#[cfg(test)]
mod tests {
    use super::{escape_md, MarkdownRenderer, MarkdownTemplate};
    use pretty_assertions::assert_eq;

    #[test]
    fn escapes_html_around_fenced_code_blocks() {
        let input = "before <\n```\ninside <\n```  \nafter <";
        let expected = "before &lt;\n```\ninside <\n```  \nafter &lt;";

        assert_eq!(escape_md(input, true), expected);
    }

    #[test]
    fn supports_fence_info_strings() {
        let input = "```bash\necho <value>\n```\nafter <";
        let expected = "```bash\necho <value>\n```\nafter &lt;";

        assert_eq!(escape_md(input, true), expected);
    }

    #[test]
    fn leaves_unclosed_fences_unescaped() {
        let input = "```\necho <value>";

        assert_eq!(escape_md(input, true), input);
    }

    #[test]
    fn ignores_indented_and_longer_fences() {
        let input = "    ```\nindented <\n````\nlonger <";
        let expected = "    ```\nindented &lt;\n````\nlonger &lt;";

        assert_eq!(escape_md(input, true), expected);
    }

    #[test]
    fn leaves_markdown_unchanged_when_html_encoding_is_disabled() {
        let input = "before <\n```\ninside <\n```\nafter <";

        assert_eq!(escape_md(input, false), input);
    }

    #[test]
    fn strips_terminal_styling_from_generated_markdown() {
        let input =
            "\u{1b}[1m\u{1b}[4mExamples:\u{1b}[22m\u{1b}[24m\n\n    \u{1b}[1mmise run\u{1b}[22m";
        let expected = "Examples:\n\n    mise run";

        assert_eq!(escape_md(input, true), expected);
        assert_eq!(escape_md(input, false), expected);
    }

    #[test]
    fn one_template_can_be_replaced_without_copying_its_includes() {
        let spec = "bin \"ex\"\nflag \"--force\" help=\"Do it anyway\"\n"
            .parse()
            .unwrap();
        let page = MarkdownRenderer::new(spec)
            .with_template(
                MarkdownTemplate::Spec,
                "# Custom {{ spec.bin }}\n{% set cmd = spec.cmd %}\n{% include \"cmd_template.md.tera\" %}",
            )
            .render_spec()
            .unwrap();

        assert!(page.starts_with("# Custom ex\n"), "{page}");
        assert!(page.contains("### `--force`"), "{page}");
    }

    #[test]
    fn the_last_replacement_of_a_template_wins() {
        let spec = "bin \"ex\"\n".parse().unwrap();
        let page = MarkdownRenderer::new(spec)
            .with_template(MarkdownTemplate::Spec, "first")
            .with_template(MarkdownTemplate::Spec, "second")
            .render_spec()
            .unwrap();

        assert_eq!(page, "second");
    }

    #[test]
    fn a_bad_custom_template_is_a_render_error() {
        let spec = "bin \"ex\"\n".parse().unwrap();
        let err = MarkdownRenderer::new(spec)
            .with_template(MarkdownTemplate::Spec, "{{")
            .render_spec()
            .unwrap_err();

        assert!(err.to_string().contains("template"), "{err}");
    }
}
