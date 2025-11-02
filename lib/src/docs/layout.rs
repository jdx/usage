/// Layout calculation for help text rendering in CLI output

/// Calculate terminal width from environment or use default
pub fn get_terminal_width() -> usize {
    std::env::var("COLUMNS")
        .ok()
        .and_then(|s| s.parse().ok())
        .unwrap_or(80)
}

/// Calculate maximum usage string width across items
pub fn max_usage_width<'a>(items: impl Iterator<Item = &'a str>) -> usize {
    items.map(visible_width).max().unwrap_or(0)
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

    // Handle explicit newlines in the input
    for paragraph in text.split('\n') {
        if paragraph.is_empty() {
            lines.push(String::new());
            continue;
        }

        let mut current_line = String::new();
        let mut current_width = 0;

        for word in paragraph.split_whitespace() {
            let word_width = visible_width(word);

            // If adding this word would exceed width, start a new line
            if current_width > 0 && current_width + 1 + word_width > width {
                lines.push(current_line);
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
            lines.push(current_line);
        }
    }

    // If input was empty or only whitespace, return empty vec
    if lines.is_empty() {
        vec![String::new()]
    } else {
        lines
    }
}

/// Render help text with proper alignment and wrapping
/// Returns (rendered_text, is_multiline)
pub fn render_help_text(
    help: &str,
    terminal_width: usize,
    usage_col_width: usize,
) -> (String, bool) {
    // If help contains explicit newlines, use block layout (legacy behavior)
    if help.contains('\n') {
        // Return None for inline rendering - template will use block layout
        return (String::new(), false);
    }

    // Format: "  <usage>PADDING  help text"
    let indent = 2;
    let gap = 2;
    let first_line_prefix_width = indent + usage_col_width + gap;
    let continuation_indent = first_line_prefix_width;

    let available_width = terminal_width.saturating_sub(first_line_prefix_width);

    // Minimum readable width
    if available_width < 10 {
        // Terminal too narrow, use block layout
        return (String::new(), false);
    }

    // Wrap text to available width
    let wrapped_lines = wrap_text(help, available_width);

    if wrapped_lines.is_empty() || (wrapped_lines.len() == 1 && wrapped_lines[0].is_empty()) {
        return (String::new(), false);
    }

    let is_multiline = wrapped_lines.len() > 1;

    // Build rendered output
    let mut result = wrapped_lines[0].clone();
    for line in &wrapped_lines[1..] {
        result.push('\n');
        result.push_str(&" ".repeat(continuation_indent));
        result.push_str(line);
    }

    (result, is_multiline)
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
        // Help with explicit newlines should use block layout (returns empty)
        let help = "Line one\nLine two";
        let (rendered, is_multiline) = render_help_text(help, 80, 20);
        assert_eq!(rendered, "");
        assert!(!is_multiline);
    }
}
