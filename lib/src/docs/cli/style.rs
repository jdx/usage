use crate::docs::models::{SpecCommand, SpecFlag};

/// Whether terminal help is rendered with ANSI styling.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Style {
    pub(super) coloured: bool,
}

impl Style {
    /// Plain text, suitable for a pipe or generated artifact.
    ///
    /// ANSI CSI escapes already present in authored help are removed as well.
    pub const PLAIN: Style = Style { coloured: false };
    /// ANSI-coloured text, regardless of the output destination.
    pub const COLOURED: Style = Style { coloured: true };

    /// Colour when stdout is a terminal and the environment permits it.
    pub fn auto() -> Style {
        use std::io::IsTerminal as _;
        Self::auto_for(std::io::stdout().is_terminal())
    }

    fn auto_for(is_terminal: bool) -> Style {
        let forced = std::env::var_os("CLICOLOR_FORCE").is_some_and(|value| value != "0");
        let refused = std::env::var_os("NO_COLOR").is_some_and(|value| !value.is_empty());
        if refused {
            Style::PLAIN
        } else if forced || is_terminal {
            Style::COLOURED
        } else {
            Style::PLAIN
        }
    }

    fn semantic(self, specification: &str, text: &str) -> String {
        crate::help_template::semantic(specification, text, self.coloured)
    }

    fn inline(self, text: &str) -> String {
        if self.coloured {
            styled_inline(text, None)
        } else {
            text.to_string()
        }
    }
}

pub(super) struct Styling {
    headings: Vec<String>,
    command_usages: Vec<String>,
    flag_usages: Vec<String>,
    arg_usages: Vec<String>,
    synopsis: Vec<String>,
}

impl Styling {
    pub(super) fn new(
        command: &SpecCommand,
        global_flags: &[SpecFlag],
        usage_section: &str,
        show_help_subcommand: bool,
    ) -> Self {
        let mut headings = vec!["Examples".to_string()];
        headings.extend(command.subcommand_groups.iter().map(|group| {
            group
                .heading
                .clone()
                .or_else(|| command.subcommand_help_heading.clone())
                .unwrap_or_else(|| "Commands".to_string())
        }));
        headings.extend(command.arg_groups.iter().map(|group| {
            group
                .heading
                .clone()
                .unwrap_or_else(|| "Arguments".to_string())
        }));
        headings.extend(
            command
                .flag_groups
                .iter()
                .map(|group| group.heading.clone().unwrap_or_else(|| "Flags".to_string())),
        );
        if !global_flags.is_empty() {
            headings.push("Global flags".to_string());
        }

        let mut command_usages: Vec<String> = if command.flatten_help {
            Vec::new()
        } else {
            command
                .subcommand_groups
                .iter()
                .flat_map(|group| group.items.iter())
                .map(|command| command.name.clone())
                .collect()
        };
        if !command_usages.is_empty() && show_help_subcommand {
            command_usages.push("help".to_string());
        }
        command_usages.sort_unstable_by_key(|usage| std::cmp::Reverse(usage.len()));
        let mut flag_usages = command
            .flag_groups
            .iter()
            .flat_map(|group| group.items.iter())
            .map(|flag| flag.display_usage.clone())
            .chain(global_flags.iter().map(|flag| flag.display_usage.clone()))
            .collect();
        let mut arg_usages = command
            .arg_groups
            .iter()
            .flat_map(|group| group.items.iter())
            .map(|arg| arg.usage.trim().to_string())
            .collect();
        collect_flattened(
            &command.flattened_subcommands,
            &mut headings,
            &mut flag_usages,
            &mut arg_usages,
        );
        flag_usages.sort_unstable_by_key(|usage| std::cmp::Reverse(usage.len()));
        arg_usages.sort_unstable_by_key(|usage| std::cmp::Reverse(usage.len()));
        Self {
            headings,
            command_usages,
            flag_usages,
            arg_usages,
            synopsis: usage_section.lines().map(str::to_string).collect(),
        }
    }

    pub(super) fn apply(&self, page: &str, style: Style) -> String {
        if !style.coloured {
            return page.to_string();
        }
        let mut out = String::with_capacity(page.len());
        for line in page.split_inclusive('\n') {
            let (body, newline) = line
                .strip_suffix('\n')
                .map_or((line, ""), |body| (body, "\n"));
            if self.synopsis.iter().any(|known| known == body) && body.starts_with("Usage:") {
                let usage = body.strip_prefix("Usage:").unwrap_or_default();
                out.push_str(&style.semantic("heading", "Usage:"));
                out.push_str(&styled_usage(usage, style));
            } else if self.synopsis.iter().any(|known| known == body) {
                out.push_str(&styled_usage(body, style));
            } else if body
                .strip_suffix(':')
                .is_some_and(|heading| self.headings.iter().any(|known| known == heading))
            {
                out.push_str(&style.semantic("heading", body));
            } else {
                let styled = body.strip_prefix("  ").and_then(|entry| {
                    self.flag_usages
                        .iter()
                        .find_map(|usage| style_entry(entry, usage, style))
                        .or_else(|| {
                            self.arg_usages
                                .iter()
                                .find_map(|usage| style_entry(entry, usage, style))
                        })
                        .or_else(|| {
                            self.command_usages
                                .iter()
                                .find_map(|usage| style_command_entry(entry, usage, style))
                        })
                });
                let body = styled.as_deref().unwrap_or(body);
                if body.trim_start().starts_with("$ ") {
                    out.push_str(body);
                } else {
                    out.push_str(&style.inline(body));
                }
            }
            out.push_str(newline);
        }
        out
    }
}

fn collect_flattened(
    commands: &[SpecCommand],
    headings: &mut Vec<String>,
    flags: &mut Vec<String>,
    args: &mut Vec<String>,
) {
    for command in commands {
        headings.push(command.full_cmd.join(" "));
        flags.extend(
            command
                .flag_groups
                .iter()
                .flat_map(|group| group.items.iter())
                .map(|flag| flag.display_usage.clone()),
        );
        args.extend(
            command
                .arg_groups
                .iter()
                .flat_map(|group| group.items.iter())
                .map(|arg| arg.usage.trim().to_string()),
        );
        collect_flattened(&command.flattened_subcommands, headings, flags, args);
    }
}

fn style_entry(entry: &str, usage: &str, style: Style) -> Option<String> {
    entry
        .strip_prefix(usage)
        .filter(|rest| rest.is_empty() || rest.starts_with(char::is_whitespace))
        .map(|rest| format!("  {}{rest}", styled_usage(usage, style)))
}

fn style_command_entry(entry: &str, usage: &str, style: Style) -> Option<String> {
    entry
        .strip_prefix(usage)
        // A rendered row either ends after the name or has the table's two-space column
        // separator. Group prose has the same indentation, but ordinary word spacing.
        .filter(|rest| rest.is_empty() || rest.starts_with("  "))
        .map(|rest| format!("  {}{rest}", style.semantic("command", usage)))
}

fn styled_usage(usage: &str, style: Style) -> String {
    let mut out = String::with_capacity(usage.len());
    let mut at = 0;
    while at < usage.len() {
        let rest = &usage[at..];
        let previous = usage[..at].chars().next_back();
        if rest.starts_with('-')
            && previous.is_none_or(|c| c.is_whitespace() || matches!(c, ',' | ':' | '[' | '<'))
        {
            let end = rest
                .char_indices()
                .skip(1)
                .find_map(|(index, c)| {
                    (c.is_whitespace() || matches!(c, ',' | '=' | '[' | ']' | '<' | '>'))
                        .then_some(index)
                })
                .unwrap_or(rest.len());
            out.push_str(&style.semantic("option", &rest[..end]));
            at += end;
            continue;
        }
        if rest.starts_with("<-") {
            out.push('<');
            at += 1;
            continue;
        }
        if rest.starts_with('<') {
            if let Some(end) = rest.find('>') {
                let end = end + 1;
                out.push_str(&style.semantic("metavar", &rest[..end]));
                at += end;
                continue;
            }
        }
        if let Some(value) = rest.strip_prefix("[=") {
            if let Some(end) = value.find(']') {
                out.push_str("[=");
                out.push_str(&style.semantic("metavar", &value[..end]));
                out.push(']');
                at += end + 3;
                continue;
            }
        }
        if let Some(value) = rest.strip_prefix('=') {
            out.push('=');
            at += 1;
            if !value.starts_with('<') {
                let end = value
                    .find(|c: char| c.is_whitespace() || matches!(c, ',' | ']' | '>'))
                    .unwrap_or(value.len());
                if end > 0 {
                    out.push_str(&style.semantic("metavar", &value[..end]));
                    at += end;
                }
            }
            continue;
        }
        if previous == Some('[') && !rest.starts_with('-') {
            let end = rest.find(']').unwrap_or(rest.len());
            if end > 0 {
                out.push_str(&style.semantic("metavar", &rest[..end]));
                at += end;
                continue;
            }
        }
        if rest.starts_with(|c: char| c.is_ascii_uppercase())
            && previous.is_none_or(|c| c.is_whitespace() || matches!(c, '=' | '[' | '<'))
        {
            let end = rest
                .find(|c: char| {
                    !(c.is_ascii_uppercase() || c.is_ascii_digit() || matches!(c, '_' | '-' | '@'))
                })
                .unwrap_or(rest.len());
            let boundary = rest[end..].chars().next();
            if boundary.is_none_or(|c| {
                c.is_whitespace() || matches!(c, ',' | '=' | '[' | ']' | '<' | '>' | '.')
            }) {
                out.push_str(&style.semantic("metavar", &rest[..end]));
                at += end;
                continue;
            }
        }
        let ch = rest.chars().next().expect("at is on a character boundary");
        out.push(ch);
        at += ch.len_utf8();
    }
    out
}

fn styled_inline(text: &str, parent: Option<&str>) -> String {
    let mut out = String::with_capacity(text.len());
    let mut at = 0;
    let mut allow_run_remainder = false;
    while at < text.len() {
        let rest = &text[at..];
        if let Some(escaped) = rest
            .strip_prefix('\\')
            .and_then(|after| after.chars().next())
        {
            if matches!(escaped, '*' | '_' | '~' | '`' | '\\') {
                out.push(escaped);
                at += 1 + escaped.len_utf8();
                allow_run_remainder = false;
                continue;
            }
        }
        let span = [
            ("***", "1;3", "22;23", false, true),
            ("___", "1;3", "22;23", true, true),
            ("**", "1", "22", false, true),
            ("__", "1", "22", true, true),
            ("~~", "9", "29", false, true),
            ("*", "3", "23", false, true),
            ("_", "3", "23", true, true),
            ("`", "36", "39", false, false),
        ]
        .into_iter()
        .find_map(|(delimiter, open, close, word_boundary, recurse)| {
            rest.strip_prefix(delimiter)?;
            let marker = delimiter.chars().next()?;
            let previous = text[..at].chars().next_back();
            if (previous == Some(marker) && !allow_run_remainder)
                || (delimiter.len() == 1 && rest[delimiter.len()..].starts_with(marker))
                || (word_boundary && previous.is_some_and(char::is_alphanumeric))
            {
                return None;
            }
            let content_start = at + delimiter.len();
            let (end, after) = closing_delimiter(text, content_start, delimiter, word_boundary, 0)?;
            Some((delimiter, open, close, recurse, content_start, end, after))
        });
        if let Some((delimiter, open, close, recurse, content_start, end, after)) = span {
            out.push_str("\u{1b}[");
            out.push_str(open);
            out.push('m');
            if recurse {
                out.push_str(&styled_inline(&text[content_start..end], Some(open)));
            } else {
                out.push_str(&text[content_start..end]);
            }
            out.push_str("\u{1b}[");
            out.push_str(close);
            out.push('m');
            if let Some(parent) = parent {
                out.push_str("\u{1b}[");
                out.push_str(parent);
                out.push('m');
            }
            let marker = delimiter.chars().next().expect("a delimiter has a marker");
            allow_run_remainder =
                text[after..].starts_with(marker) && text[..after].ends_with(marker);
            at = after;
            continue;
        }
        let ch = rest.chars().next().expect("at is on a character boundary");
        out.push(ch);
        at += ch.len_utf8();
        allow_run_remainder = false;
    }
    out
}

fn closing_delimiter(
    text: &str,
    content_start: usize,
    delimiter: &str,
    word_boundary: bool,
    reserve: usize,
) -> Option<(usize, usize)> {
    let marker = delimiter.chars().next()?;
    let width = delimiter.len();
    let mut search_at = content_start;
    while let Some(found) = text[search_at..].find(marker) {
        let run_start = search_at + found;
        let run_len = text[run_start..]
            .chars()
            .take_while(|ch| *ch == marker)
            .count();
        let run_end = run_start + run_len;
        let escaped = text[..run_start]
            .chars()
            .rev()
            .take_while(|ch| *ch == '\\')
            .count()
            % 2
            == 1;
        if escaped {
            search_at = run_start + marker.len_utf8();
            continue;
        }
        let nested_width = match (run_len, marker) {
            (1..=3, '*' | '_') if run_len != width => run_len,
            _ => 0,
        };
        if nested_width != 0 {
            let nested = &text[run_start..run_start + nested_width];
            if let Some((_, after)) =
                closing_delimiter(text, run_start + nested_width, nested, marker == '_', width)
            {
                search_at = after;
                continue;
            }
        }
        if run_len >= width {
            let after = run_start + width;
            let left_in_run = run_end - after;
            let boundary_ok = !word_boundary
                || !text[after..]
                    .chars()
                    .next()
                    .is_some_and(char::is_alphanumeric);
            if run_start > content_start
                && !text[content_start..run_start].trim().is_empty()
                && (left_in_run == 0 || left_in_run >= reserve)
                && boundary_ok
            {
                return Some((run_start, after));
            }
        }
        search_at = run_end;
    }
    None
}
