use std::fmt::Write as _;

/// Formatting configuration for use with [`KdlDocument::autoformat_config`](`crate::kdl::KdlDocument::autoformat_config`)
/// and [`KdlNode::autoformat_config`](`crate::kdl::KdlNode::autoformat_config`).
#[non_exhaustive]
#[derive(Debug)]
pub struct FormatConfig<'a> {
    /// How deeply to indent the overall node or document,
    /// in repetitions of [`indent`](`FormatConfig::indent`).
    /// Defaults to `0`.
    pub indent_level: usize,

    /// The indentation to use at each level. Defaults to four spaces.
    pub indent: &'a str,

    /// Whether to remove comments. Defaults to `false`.
    pub no_comments: bool,

    /// Whether to keep individual entry formatting.
    pub entry_autoformate_keep: bool,
}

/// See field documentation for defaults.
impl Default for FormatConfig<'_> {
    fn default() -> Self {
        Self::builder().build()
    }
}

impl FormatConfig<'_> {
    /// Creates a new [`FormatConfigBuilder`] with default configuration.
    pub const fn builder() -> FormatConfigBuilder<'static> {
        FormatConfigBuilder::new()
    }
}

/// A [`FormatConfig`] builder.
///
/// Note that setters can be repeated.
#[derive(Debug, Default)]
pub struct FormatConfigBuilder<'a>(FormatConfig<'a>);

impl<'a> FormatConfigBuilder<'a> {
    /// Creates a new [`FormatConfig`] builder with default configuration.
    pub const fn new() -> Self {
        Self(FormatConfig {
            indent_level: 0,
            indent: "    ",
            no_comments: false,
            entry_autoformate_keep: false,
        })
    }

    /// How deeply to indent the overall node or document,
    /// in repetitions of [`indent`](`FormatConfig::indent`).
    /// Defaults to `0` iff not specified.
    pub const fn maybe_indent_level(mut self, indent_level: Option<usize>) -> Self {
        if let Some(indent_level) = indent_level {
            self.0.indent_level = indent_level;
        }
        self
    }

    /// How deeply to indent the overall node or document,
    /// in repetitions of [`indent`](`FormatConfig::indent`).
    /// Defaults to `0` iff not specified.
    pub const fn indent_level(mut self, indent_level: usize) -> Self {
        self.0.indent_level = indent_level;
        self
    }

    /// The indentation to use at each level.
    /// Defaults to four spaces iff not specified.
    pub const fn maybe_indent<'b, 'c>(self, indent: Option<&'b str>) -> FormatConfigBuilder<'c>
    where
        'a: 'b,
        'b: 'c,
    {
        if let Some(indent) = indent {
            self.indent(indent)
        } else {
            self
        }
    }

    /// The indentation to use at each level.
    /// Defaults to four spaces if not specified.
    pub const fn indent(self, indent: &str) -> FormatConfigBuilder<'_> {
        FormatConfigBuilder(FormatConfig { indent, ..self.0 })
    }

    /// Whether to remove comments.
    /// Defaults to `false` iff not specified.
    pub const fn maybe_no_comments(mut self, no_comments: Option<bool>) -> Self {
        if let Some(no_comments) = no_comments {
            self.0.no_comments = no_comments;
        }
        self
    }

    /// Whether to remove comments.
    /// Defaults to `false` iff not specified.
    pub const fn no_comments(mut self, no_comments: bool) -> Self {
        self.0.no_comments = no_comments;
        self
    }

    /// Builds the [`FormatConfig`].
    pub const fn build(self) -> FormatConfig<'a> {
        self.0
    }
}

pub(crate) fn autoformat_leading(leading: &mut String, config: &FormatConfig<'_>) {
    let mut result = String::new();
    if !config.no_comments {
        let input = leading.trim();
        if !input.is_empty() {
            for line in input.lines() {
                let trimmed = line.trim();
                if !trimmed.is_empty() {
                    for _ in 0..config.indent_level {
                        result.push_str(config.indent);
                    }
                    writeln!(result, "{trimmed}").unwrap();
                }
            }
        }
    }
    for _ in 0..config.indent_level {
        result.push_str(config.indent);
    }
    *leading = result;
}

pub(crate) fn autoformat_trailing(decor: &mut String, no_comments: bool) {
    if decor.is_empty() {
        return;
    }
    *decor = decor.trim().to_string();
    let mut result = String::new();
    if !decor.is_empty() && !no_comments {
        if decor.trim_start() == &decor[..] {
            write!(result, " ").unwrap();
        }
        for comment in decor.lines() {
            writeln!(result, "{comment}").unwrap();
        }
    }
    *decor = result;
}
