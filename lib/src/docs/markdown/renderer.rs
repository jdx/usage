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

/// The built-in presentation used for generated Markdown.
#[derive(Debug, Default, Clone, Copy, PartialEq, Eq)]
pub enum MarkdownTheme {
    /// Dense grouped lists intended for scanning a large command reference.
    #[default]
    Compact,
    /// Give every argument and flag its own addressable heading and detail block.
    Detailed,
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

/// A backtick span, or a bare `<` outside one.
static CODE_SPAN_OR_LT: LazyLock<Regex> = LazyLock::new(|| Regex::new(r"(`[^`]*`)|(<)").unwrap());

fn escape_md_with_indent(value: &str, html_encode: bool, indent: bool) -> String {
    let mut in_fenced_code_block = false;
    // Help text is allowed to contain terminal styling. clap-era applications commonly build
    // their examples with `color_print::cstr!`, which embeds SGR sequences even when color is
    // disabled at runtime. Terminal styling has no meaning in generated Markdown, and leaving
    // it here publishes literal escape bytes in docs and downstream static sites.
    let value = crate::docs::strip_ansi(value);

    value
        .lines()
        .enumerate()
        .map(|(index, line)| {
            let line = if !html_encode {
                line.to_string()
            } else {
                // Indented code is handled before fence state. This is safe because
                // `fence_indented_blocks` always emits closing fences at column zero.
                if line.starts_with("    ") {
                    line.to_string()
                } else if in_fenced_code_block {
                    if line.trim_end() == "```" {
                        in_fenced_code_block = false;
                    }
                    line.to_string()
                // Support the conventional fence shape emitted by `fence_indented_blocks`
                // without attempting to parse the full Markdown specification.
                } else if line
                    .strip_prefix("```")
                    .is_some_and(|suffix| !suffix.starts_with('`'))
                {
                    in_fenced_code_block = true;
                    line.to_string()
                } else {
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
                }
            };
            if indent && index > 0 && !line.is_empty() {
                format!("  {line}")
            } else {
                line
            }
        })
        .join("\n")
}

fn escape_md(value: &str, html_encode: bool) -> String {
    escape_md_with_indent(value, html_encode, false)
}

#[derive(Debug, Clone)]
pub struct MarkdownRenderer {
    /// The spec as its author wrote it, before any rendering.
    ///
    /// Kept because rendering depends on the builder options, and the builders run *after*
    /// [`Self::new`]. Rendering eagerly in `new` read options nobody had set yet, and because
    /// rendering marks each item done, the later pass with the real options no-opped — which
    /// is how `--indented-blocks-to-code-fences` came to do nothing at all on single-file
    /// output. The rendered model is derived from this on first use instead, so it always sees
    /// the options the caller actually asked for.
    raw: crate::Spec,
    /// The rendered docs model, derived from [`Self::raw`] on first use by [`Self::spec`].
    ///
    /// Every builder clears this, so an option set after a render still takes effect.
    ///
    /// A `OnceLock` rather than a `OnceCell` so the renderer stays `Sync` and `RefUnwindSafe`
    /// for callers that hold one across threads — a `OnceCell` here would take those away.
    spec: std::sync::OnceLock<Spec>,
    pub(crate) header_level: usize,
    pub(crate) multi: bool,
    url_prefix: Option<String>,
    html_encode: bool,
    indented_blocks_to_code_fences: bool,
    theme: MarkdownTheme,
    templates: Vec<(MarkdownTemplate, String)>,
}

impl MarkdownRenderer {
    pub fn new(spec: crate::Spec) -> Self {
        Self {
            raw: spec,
            spec: std::sync::OnceLock::new(),
            header_level: 1,
            multi: false,
            url_prefix: None,
            html_encode: true,
            indented_blocks_to_code_fences: false,
            theme: MarkdownTheme::default(),
            templates: Vec::new(),
        }
    }

    /// The rendered docs model, built on first use from the options set by then.
    pub(crate) fn spec(&self) -> &Spec {
        self.spec.get_or_init(|| {
            // By reference: `From<&Spec>` already clones internally, so handing it an owned
            // spec clones the whole tree twice.
            let mut spec = Spec::from(&self.raw);
            spec.render_md(self);
            spec
        })
    }

    /// Apply a builder option and drop any model rendered under the old options.
    ///
    /// Every `with_*` goes through this. A builder that set its field directly would leave a
    /// model rendered without its option behind, which is the bug this type already had once.
    fn with(mut self, set: impl FnOnce(&mut Self)) -> Self {
        set(&mut self);
        self.spec = std::sync::OnceLock::new();
        self
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
        self.spec()
            .cmd
            .subcommands
            .values()
            .find(|cmd| !cmd.hide && cmd.full_cmd == [stem])
            .map(|cmd| cmd.name.as_str())
    }

    pub fn with_header_level(self, header_level: usize) -> Self {
        self.with(|r| r.header_level = header_level)
    }

    pub fn with_multi(self, index: bool) -> Self {
        self.with(|r| r.multi = index)
    }

    pub fn with_url_prefix<S: Into<String>>(self, url_prefix: S) -> Self {
        self.with(|r| r.url_prefix = Some(url_prefix.into()))
    }

    pub fn with_html_encode(self, html_encode: bool) -> Self {
        self.with(|r| r.html_encode = html_encode)
    }

    /// Turn four-space indented blocks in help text into fenced code blocks.
    pub fn with_indented_blocks_to_code_fences(self, indented_blocks_to_code_fences: bool) -> Self {
        self.with(|r| r.indented_blocks_to_code_fences = indented_blocks_to_code_fences)
    }

    /// The former name of [`Self::with_indented_blocks_to_code_fences`]. Prefer that one.
    ///
    /// A misnomer from the start: no `<pre>` tag was ever involved. Kept, rather than renamed
    /// out from under callers, so an existing docs build kept compiling — and left in the
    /// public API rather than `#[doc(hidden)]`, because hiding it is itself the kind of
    /// removal that costs downstreams a major version.
    pub fn with_replace_pre_with_code_fences(self, indented_blocks_to_code_fences: bool) -> Self {
        self.with_indented_blocks_to_code_fences(indented_blocks_to_code_fences)
    }

    /// Select a built-in Markdown presentation.
    pub fn with_theme(self, theme: MarkdownTheme) -> Self {
        self.with(|r| r.theme = theme)
    }

    /// Replace one built-in Markdown template.
    ///
    /// Templates use [Tera](https://keats.github.io/tera/). A replacement may include any of the
    /// templates it did not replace; for example, a custom [`MarkdownTemplate::Spec`] can still
    /// contain `{% include "cmd_template.md.tera" %}`. Replacing the same member more than once
    /// uses the last value. Syntax and include errors are returned by the render method.
    pub fn with_template(self, template: MarkdownTemplate, source: impl Into<String>) -> Self {
        let source = source.into();
        self.with(|r| {
            if let Some((_, current)) = r
                .templates
                .iter_mut()
                .find(|(current, _)| *current == template)
            {
                *current = source;
            } else {
                r.templates.push((template, source));
            }
        })
    }

    fn tera_ctx(&self) -> tera::Context {
        let mut ctx = tera::Context::new();
        ctx.insert("spec", self.spec());
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
        let mut tera = match self.theme {
            MarkdownTheme::Compact => TERA.clone(),
            MarkdownTheme::Detailed => crate::docs::markdown::tera::DETAILED_TERA.clone(),
        };

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
        tera.register_filter(
            "escape_md_indented",
            move |value: &tera::Value,
                  _: tera::Kwargs,
                  _: &tera::State|
                  -> tera::TeraResult<String> {
                let value = value.as_str().unwrap();
                Ok(escape_md_with_indent(value, html_encode, true))
            },
        );

        let mut ctx = self.tera_ctx();
        enrich(&mut ctx);
        Ok(tera.render(template_name, &ctx)?)
    }

    /// Rewrite four-space indented blocks as fenced ones, when the caller asked for it.
    pub(crate) fn fence_indented_blocks(&self, md: String) -> String {
        if !self.indented_blocks_to_code_fences {
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
        assert!(page.contains("- **`--force`**"), "{page}");
    }

    #[test]
    fn the_last_replacement_of_a_template_wins() {
        let spec = "bin \"ex\"\n".parse().unwrap();
        let page = MarkdownRenderer::new(spec)
            .with_template(MarkdownTemplate::Spec, "{{")
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

    #[test]
    fn the_detailed_theme_keeps_addressable_entry_headings() {
        let spec = "bin \"ex\"\nflag \"--force\" help=\"Do it anyway\"\n"
            .parse()
            .unwrap();
        let page = MarkdownRenderer::new(spec)
            .with_theme(super::MarkdownTheme::Detailed)
            .render_spec()
            .unwrap();

        assert!(page.contains("### `--force`"), "{page}");
    }

    #[test]
    fn entry_template_overrides_apply_to_the_compact_theme() {
        let spec = "bin \"ex\"\narg \"<file>\"\nflag \"--force\"\n"
            .parse()
            .unwrap();
        let page = MarkdownRenderer::new(spec)
            .with_template(MarkdownTemplate::Argument, "argument: {{ arg.usage }}")
            .with_template(MarkdownTemplate::Flag, "flag: {{ flag.usage }}")
            .render_spec()
            .unwrap();

        assert!(page.contains("argument: <file>"), "{page}");
        assert!(page.contains("flag: --force"), "{page}");
    }
}
