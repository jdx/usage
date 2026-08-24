use std::fmt::Write as _;

#[derive(Debug)]
pub(crate) struct FormatConfig<'a> {
    pub(crate) indent_level: usize,
    pub(crate) indent: &'a str,
    pub(crate) no_comments: bool,
    pub(crate) entry_autoformate_keep: bool,
}

impl Default for FormatConfig<'_> {
    fn default() -> Self {
        Self {
            indent_level: 0,
            indent: "    ",
            no_comments: false,
            entry_autoformate_keep: false,
        }
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
