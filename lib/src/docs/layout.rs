/// Calculate terminal width from environment or use default
pub fn get_terminal_width() -> usize {
    std::env::var("COLUMNS")
        .ok()
        .and_then(|s| s.parse().ok())
        .unwrap_or(80)
}

/// Minimum useful room for prose beside an entry wider than the shared usage column.
pub const MIN_INLINE_HELP_WIDTH: usize = 30;

/// Resolve a command's help width using clap-compatible precedence.
pub fn help_width(term_width: Option<usize>, max_term_width: Option<usize>) -> usize {
    if let Some(width) = term_width {
        return if width == 0 { usize::MAX } else { width };
    }
    let detected = get_terminal_width();
    match max_term_width {
        Some(0) | None => detected,
        Some(max) => detected.min(max),
    }
}

/// Calculate maximum usage string width across items
pub fn max_usage_width<'a>(items: impl Iterator<Item = &'a str>) -> usize {
    items.map(visible_width).max().unwrap_or(0)
}

/// Calculate a readable usage column for a terminal-width help page.
///
/// The names get at most two fifths of the width left after the two-space indent and gap. A
/// longer outlier is rendered as a block by its caller instead of narrowing every description
/// on the page.
pub fn usage_column_width<'a>(
    items: impl Iterator<Item = &'a str>,
    terminal_width: usize,
) -> usize {
    let longest = max_usage_width(items);
    if terminal_width == usize::MAX {
        return longest;
    }
    let available = terminal_width.saturating_sub(4);
    let cap = available / 5 * 2 + available % 5 * 2 / 5;
    longest.min(cap)
}

/// Calculate visible width of a string (ignoring ANSI codes)
pub fn visible_width(s: &str) -> usize {
    // Simple implementation - counts chars
    // TODO: Handle ANSI escape codes if needed
    s.chars().count()
}

/// Wrap text to fit within a given width
pub fn wrap_text(text: &str, width: usize) -> Vec<String> {
    if width == 0 {
        return vec![text.to_string()];
    }

    let mut lines = Vec::new();

    // Keep explicit line boundaries. Indented lines are preformatted; list items get a
    // hanging indent while ordinary prose is folded at word boundaries.
    for paragraph in text.split('\n') {
        if paragraph.is_empty() {
            lines.push(String::new());
            continue;
        }

        if paragraph.starts_with("    ") || paragraph.starts_with('\t') {
            lines.push(paragraph.to_string());
            continue;
        }

        let paragraph = paragraph.trim();
        let (prefix, body) = list_prefix(paragraph).unwrap_or(("", paragraph));
        let body_width = width.saturating_sub(visible_width(prefix));
        let mut line_prefix = prefix.to_string();

        let mut current_line = String::new();
        let mut current_width = 0;

        for word in body.split_whitespace() {
            let word_width = visible_width(word);

            // If adding this word would exceed width, start a new line
            if current_width > 0 && current_width + 1 + word_width > body_width {
                lines.push(format!("{line_prefix}{current_line}"));
                line_prefix = " ".repeat(visible_width(prefix));
                current_line = String::new();
                current_width = 0;
            }

            // Add space between words (but not at start of line)
            if current_width > 0 {
                current_line.push(' ');
                current_width += 1;
            }

            current_line.push_str(word);
            current_width += word_width;
        }

        if !current_line.is_empty() {
            lines.push(format!("{line_prefix}{current_line}"));
        }
    }

    // If input was empty or only whitespace, return empty vec
    if lines.is_empty() {
        vec![String::new()]
    } else {
        lines
    }
}

fn list_prefix(line: &str) -> Option<(&str, &str)> {
    for marker in ["* ", "- ", "+ "] {
        if let Some(body) = line.strip_prefix(marker) {
            return Some((marker, body));
        }
    }
    let digits = line.bytes().take_while(u8::is_ascii_digit).count();
    if digits > 0 && line.as_bytes().get(digits..digits + 2) == Some(b". ") {
        return Some(line.split_at(digits + 2));
    }
    None
}

/// Render help text with proper alignment and wrapping
/// Returns (rendered_text, is_multiline)
pub fn render_help_text(
    help: &str,
    terminal_width: usize,
    usage_col_width: usize,
) -> (String, bool) {
    // Format: "  <usage>PADDING  help text"
    let indent = 2;
    let gap = 2;
    let first_line_prefix_width = indent + usage_col_width + gap;
    render_help_text_at(
        help,
        terminal_width,
        first_line_prefix_width,
        first_line_prefix_width,
    )
}

/// Render help whose opening line and continuations begin in different columns.
pub fn render_help_text_at(
    help: &str,
    terminal_width: usize,
    first_line_prefix_width: usize,
    continuation_indent: usize,
) -> (String, bool) {
    let available_width = terminal_width.saturating_sub(first_line_prefix_width);
    let continuation_width = terminal_width.saturating_sub(continuation_indent);

    // Minimum readable width
    if available_width < 10 || continuation_width < 10 {
        // Terminal too narrow, use block layout
        return (String::new(), false);
    }

    let mut wrapped_lines = Vec::new();
    for (index, line) in help.split('\n').enumerate() {
        wrapped_lines.extend(wrap_text(
            line,
            if index == 0 {
                available_width
            } else {
                continuation_width
            },
        ));
    }

    if wrapped_lines.is_empty() || (wrapped_lines.len() == 1 && wrapped_lines[0].is_empty()) {
        return (String::new(), false);
    }

    let is_multiline = wrapped_lines.len() > 1;

    // Build rendered output
    let mut result = wrapped_lines[0].clone();
    for line in &wrapped_lines[1..] {
        result.push('\n');
        if !line.is_empty() {
            result.push_str(&" ".repeat(continuation_indent));
            result.push_str(line);
        }
    }

    (result, is_multiline)
}

/// Wrap text for the four-space block beneath an entry that overflowed its usage column.
pub fn render_block_text(help: &str, terminal_width: usize) -> String {
    render_indented_text(help, terminal_width, 4)
}

/// Wrap text that a caller will indent by `indent` columns.
pub fn render_indented_text(help: &str, terminal_width: usize, indent: usize) -> String {
    wrap_text(help, terminal_width.saturating_sub(indent)).join("\n")
}

/// Render labelled prose with continuations hanging below the text after the label.
pub fn render_labelled_text(
    label: &str,
    text: &str,
    terminal_width: usize,
    indent: usize,
) -> String {
    let prefix = format!("{label}: ");
    let lines = wrap_text(
        text,
        terminal_width.saturating_sub(indent + visible_width(&prefix)),
    );
    let Some((first, rest)) = lines.split_first() else {
        return format!("{label}:");
    };
    let mut out = format!("{prefix}{first}");
    for line in rest {
        out.push('\n');
        if !line.is_empty() {
            out.push_str(&" ".repeat(visible_width(&prefix)));
            out.push_str(line);
        }
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_visible_width() {
        assert_eq!(visible_width("hello"), 5);
        assert_eq!(visible_width(""), 0);
        assert_eq!(visible_width("hello world"), 11);
    }

    #[test]
    fn test_usage_column_width_caps_outliers() {
        assert_eq!(
            usage_column_width(["--short", "--longer"].into_iter(), 80),
            8
        );
        assert_eq!(
            usage_column_width(
                [
                    "--short",
                    "--report-unused-disable-directives-severity <SEVERITY>"
                ]
                .into_iter(),
                80,
            ),
            30,
        );
        assert_eq!(
            usage_column_width(["--an-unbounded-column"].into_iter(), usize::MAX),
            21,
        );
    }

    #[test]
    fn test_wrap_text_short() {
        let text = "short";
        let wrapped = wrap_text(text, 20);
        assert_eq!(wrapped, vec!["short"]);
    }

    #[test]
    fn test_wrap_text_long() {
        let text = "this is a very long text that should wrap";
        let wrapped = wrap_text(text, 20);
        assert!(wrapped.len() > 1);
        for line in &wrapped {
            assert!(visible_width(line) <= 20);
        }
    }

    #[test]
    fn test_wrap_text_with_newlines() {
        let text = "line one\nline two";
        let wrapped = wrap_text(text, 20);
        assert_eq!(wrapped, vec!["line one", "line two"]);
    }

    #[test]
    fn test_render_help_text_short() {
        let help = "Short help";
        let (rendered, is_multiline) = render_help_text(help, 80, 20);
        assert_eq!(rendered, "Short help");
        assert!(!is_multiline);
    }

    #[test]
    fn test_render_help_text_long() {
        let help = "This is a very long help text that should wrap to multiple lines when rendered";
        let (rendered, is_multiline) = render_help_text(help, 60, 20);
        assert!(is_multiline);
        assert!(rendered.contains('\n'));
    }

    #[test]
    fn test_render_help_text_with_newlines() {
        let help = "Line one\nLine two";
        let (rendered, is_multiline) = render_help_text(help, 80, 20);
        assert_eq!(rendered, "Line one\n                        Line two");
        assert!(is_multiline);
    }

    #[test]
    fn list_items_wrap_with_a_hanging_indent_and_code_is_preserved() {
        assert_eq!(
            wrap_text("* alpha beta gamma delta", 14),
            ["* alpha beta", "  gamma delta"]
        );
        assert_eq!(
            wrap_text("    $ ex --a-deliberately-long-example", 10),
            ["    $ ex --a-deliberately-long-example"]
        );
    }
}
