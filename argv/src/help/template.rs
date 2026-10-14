//! The colour markup accepted in a `help_template`.
//!
//! Tags deliberately resemble bunt's format strings, but are parsed at runtime: a template may
//! come from KDL rather than a Rust string literal. The parser touches only the template itself;
//! substituted help sections are opaque, so prose that happens to contain `{$red}` stays prose.

use std::borrow::Cow;

use super::{Palette, Style};

const MARK: char = '\u{2}';
const END: char = '\u{3}';

/// The styles accepted in a help template.
pub const STYLES: [&str; 24] = [
    "heading",
    "option",
    "metavar",
    "command",
    "black",
    "red",
    "green",
    "yellow",
    "blue",
    "magenta",
    "cyan",
    "white",
    "bright-black",
    "bright-red",
    "bright-green",
    "bright-yellow",
    "bright-blue",
    "bright-magenta",
    "bright-cyan",
    "bright-white",
    "bold",
    "dim",
    "italic",
    "underline",
];

#[derive(Clone, Copy, Default)]
struct AnsiStyle {
    foreground: Option<u8>,
    bold: bool,
    dim: bool,
    italic: bool,
    underline: bool,
}

impl AnsiStyle {
    fn apply(mut self, specification: &str) -> Option<Self> {
        if specification.is_empty() {
            return None;
        }
        for fragment in specification.split('+') {
            // Positions in `STYLES`: the four roles, then the eight colours and their bright
            // twins in ANSI order, then the four attributes.
            let index = STYLES.iter().position(|name| *name == fragment)?;
            match index {
                0..=3 => {
                    self.foreground = Some([33, 32, 35, 32][index]);
                    self.bold = true;
                }
                4..=11 => self.foreground = Some(26 + index as u8),
                12..=19 => self.foreground = Some(78 + index as u8),
                20 => self.bold = true,
                21 => self.dim = true,
                22 => self.italic = true,
                _ => self.underline = true,
            }
        }
        Some(self)
    }

    fn write(self, out: &mut String) {
        let mut separator = "";
        out.push_str("\u{1b}[");
        for (enabled, code) in [
            (self.bold, 1u8),
            (self.dim, 2),
            (self.italic, 3),
            (self.underline, 4),
        ] {
            if enabled {
                out.push_str(separator);
                push_code(out, code);
                separator = ";";
            }
        }
        if let Some(foreground) = self.foreground {
            out.push_str(separator);
            push_code(out, foreground);
            separator = ";";
        }
        if separator.is_empty() {
            out.push('0');
        }
        out.push('m');
    }
}

/// A code in decimal, without `core::fmt`.
fn push_code(out: &mut String, code: u8) {
    if code >= 10 {
        push_code(out, code / 10);
    }
    out.push(char::from(b'0' + code % 10));
}

fn mapped_fragment(fragment: &str, palette: Palette) -> &str {
    match fragment {
        "heading" => palette.heading,
        "option" => palette.option,
        "metavar" => palette.metavar,
        "command" => palette.command,
        other => other,
    }
}

fn expand_spec(specification: &str, palette: Palette) -> Cow<'_, str> {
    // One fragment, which is every role tag and nearly every other: nothing to join.
    if !specification.contains('+') {
        return Cow::Borrowed(mapped_fragment(specification, palette));
    }
    let mut out = String::with_capacity(specification.len());
    for (i, fragment) in specification.split('+').enumerate() {
        if i > 0 {
            out.push('+');
        }
        out.push_str(mapped_fragment(fragment, palette));
    }
    Cow::Owned(out)
}

pub(super) fn semantic(specification: &str, text: &str, style: Style) -> String {
    let mut out = String::with_capacity(text.len() + 16);
    semantic_into(&mut out, specification, text, style);
    out
}

/// [`semantic`], appended to `out` rather than returned.
pub(super) fn semantic_into(out: &mut String, specification: &str, text: &str, style: Style) {
    if !style.coloured {
        out.push_str(text);
        return;
    }
    let specification = expand_spec(specification, style.palette);
    AnsiStyle::default()
        .apply(specification.as_ref())
        .unwrap_or_default()
        .write(out);
    out.push_str(text);
    AnsiStyle::default().write(out);
}

/// Validate the style tags in a help template.
pub(super) fn check(template: &str) -> Result<(), &'static str> {
    let mut rest = template;
    let mut depth = 0usize;
    while let Some((at, event)) = next_style_event(rest) {
        let tag = &rest[at..];
        match event {
            Event::EscapeOpen => rest = &tag[3..],
            Event::EscapeClose => rest = &tag[5..],
            Event::Open => {
                let Some(end) = tag.find('}') else {
                    return Err("a `{$` with no `}` after it");
                };
                let specification = &tag[2..end];
                if specification.is_empty() {
                    return Err("an empty style tag `{$}`");
                }
                if AnsiStyle::default().apply(specification).is_none() {
                    return Err("an unknown help-template style");
                }
                depth += 1;
                rest = &tag[end + 1..];
            }
            Event::Close => {
                if depth == 0 {
                    return Err("a `{/$}` with no open style tag");
                }
                depth -= 1;
                rest = &tag[4..];
            }
            Event::Placeholder => rest = &tag[2..],
        }
    }
    if depth == 0 {
        Ok(())
    } else {
        Err("a style tag with no `{/$}` after it")
    }
}

/// Substitute sections and render template-authored colour markup.
pub(super) fn substitute(
    template: &str,
    style: Style,
    mut section: impl FnMut(&str) -> Option<String>,
) -> String {
    // A template whose style markup does not hold together has none: it is sections and text,
    // and every `{$` in it is literal.
    let styled = check(template).is_ok();
    let mut marked = String::with_capacity(template.len());
    let mut rest = template;
    loop {
        let placeholder = rest.find("{{").map(|at| (at, Event::Placeholder));
        let tag = if styled { next_style_event(rest) } else { None };
        let Some((at, event)) = earliest(placeholder, tag) else {
            push_escaped(&mut marked, rest, styled);
            break;
        };
        push_escaped(&mut marked, &rest[..at], styled);
        rest = &rest[at..];
        match event {
            Event::Placeholder => {
                let after = &rest[2..];
                let Some(end) = after.find("}}") else {
                    push_escaped(&mut marked, rest, styled);
                    break;
                };
                match section(after[..end].trim()) {
                    Some(text) => push_escaped(&mut marked, &text, styled),
                    None => push_escaped(&mut marked, &rest[..2 + end + 2], styled),
                }
                rest = &after[end + 2..];
            }
            Event::Open => {
                let Some(end) = rest.find('}') else {
                    push_escaped(&mut marked, rest, styled);
                    break;
                };
                marked.push(MARK);
                marked.push('+');
                marked.push_str(&rest[2..end]);
                marked.push(END);
                rest = &rest[end + 1..];
            }
            Event::Close => {
                marked.push(MARK);
                marked.push('-');
                marked.push(END);
                rest = &rest[4..];
            }
            Event::EscapeOpen => {
                push_escaped(&mut marked, "{$", styled);
                rest = &rest[3..];
            }
            Event::EscapeClose => {
                push_escaped(&mut marked, "{/$}", styled);
                rest = &rest[5..];
            }
        }
    }
    let collapsed = collapse_blank_runs(&marked, styled);
    if styled {
        render_marked(&collapsed, style)
    } else {
        collapsed
    }
}

#[derive(Clone, Copy)]
enum Event {
    Placeholder,
    Open,
    Close,
    EscapeOpen,
    EscapeClose,
}

fn earliest(left: Option<(usize, Event)>, right: Option<(usize, Event)>) -> Option<(usize, Event)> {
    match (left, right) {
        (Some(left), Some(right)) => Some(if left.0 <= right.0 { left } else { right }),
        (left, right) => left.or(right),
    }
}

fn next_style_event(text: &str) -> Option<(usize, Event)> {
    static TOKENS: [(&str, Event); 4] = [
        ("{$$", Event::EscapeOpen),
        ("{/$$}", Event::EscapeClose),
        ("{$", Event::Open),
        ("{/$}", Event::Close),
    ];
    TOKENS
        .iter()
        .enumerate()
        .filter_map(|(priority, (token, event))| {
            text.find(token).map(|at| ((at, priority), *event))
        })
        .min_by_key(|(position, _)| *position)
        .map(|((at, _), event)| (at, event))
}

/// `text` for the marked page: a `MARK` already in it doubled, so it reads as text rather than
/// as the start of a tag. A page with no tags is not read for them, and takes the text as is.
fn push_escaped(out: &mut String, text: &str, styled: bool) {
    if !styled || !text.contains(MARK) {
        out.push_str(text);
        return;
    }
    for ch in text.chars() {
        out.push(ch);
        if ch == MARK {
            out.push(MARK);
        }
    }
}

/// The page with every run of blank lines cut to one and none left at either end. `markers`
/// says the page carries style tags: a line of nothing but tags is blank, and its tags stay.
fn collapse_blank_runs(page: &str, markers: bool) -> String {
    let mut out = String::with_capacity(page.len());
    let mut blank = false;
    let mut wrote_visible = false;
    for line in page.split('\n') {
        if if markers {
            visible_is_blank(line)
        } else {
            line.trim().is_empty()
        } {
            if markers {
                push_markers(&mut out, line);
            }
            blank = wrote_visible;
            continue;
        }
        if wrote_visible {
            out.push('\n');
            if blank {
                out.push('\n');
            }
        }
        blank = false;
        out.push_str(line);
        wrote_visible = true;
    }
    out
}

fn visible_is_blank(line: &str) -> bool {
    let mut rest = line;
    while let Some(at) = rest.find(MARK) {
        if !rest[..at].trim().is_empty() {
            return false;
        }
        let after = &rest[at + MARK.len_utf8()..];
        if after.starts_with(MARK) {
            return false;
        } else if let Some(end) = after.find(END) {
            rest = &after[end + END.len_utf8()..];
        } else {
            return false;
        }
    }
    rest.trim().is_empty()
}

fn push_markers(out: &mut String, line: &str) {
    let mut rest = line;
    while let Some(at) = rest.find(MARK) {
        let marker = &rest[at..];
        let Some(end) = marker.find(END) else {
            return;
        };
        out.push_str(&marker[..=end]);
        rest = &marker[end + END.len_utf8()..];
    }
}

fn render_marked(marked: &str, style: Style) -> String {
    let mut out = String::with_capacity(marked.len());
    let mut stack = vec![AnsiStyle::default()];
    let mut rest = marked;
    while let Some(at) = rest.find(MARK) {
        push_content(
            &mut out,
            &rest[..at],
            style.coloured,
            stack.last().copied().unwrap_or_default(),
        );
        let after = &rest[at + MARK.len_utf8()..];
        if after.starts_with(MARK) {
            out.push(MARK);
            rest = &after[MARK.len_utf8()..];
            continue;
        }
        let Some(end) = after.find(END) else {
            out.push(MARK);
            out.push_str(after);
            break;
        };
        let marker = &after[..end];
        if let Some(specification) = marker.strip_prefix('+') {
            let specification = expand_spec(specification, style.palette);
            let next = stack
                .last()
                .copied()
                .unwrap_or_default()
                .apply(specification.as_ref())
                .unwrap_or_default();
            stack.push(next);
            if style.coloured {
                next.write(&mut out);
            }
        } else {
            if stack.len() > 1 {
                stack.pop();
            }
            if style.coloured {
                AnsiStyle::default().write(&mut out);
                let parent = stack.last().copied().unwrap_or_default();
                if parent.foreground.is_some()
                    || parent.bold
                    || parent.dim
                    || parent.italic
                    || parent.underline
                {
                    parent.write(&mut out);
                }
            }
        }
        rest = &after[end + END.len_utf8()..];
    }
    push_content(
        &mut out,
        rest,
        style.coloured,
        stack.last().copied().unwrap_or_default(),
    );
    out
}

fn push_content(out: &mut String, text: &str, coloured: bool, active: AnsiStyle) {
    if !coloured
        || (!active.bold
            && !active.dim
            && !active.italic
            && !active.underline
            && active.foreground.is_none())
    {
        out.push_str(text);
        return;
    }
    let mut rest = text;
    while let Some(at) = rest.find("\u{1b}[") {
        out.push_str(&rest[..at]);
        let sequence = &rest[at..];
        let Some(end) = sequence.find('m') else {
            out.push_str(sequence);
            return;
        };
        out.push_str(&sequence[..=end]);
        let parameters = &sequence[2..end];
        if parameters.split(';').any(|parameter| {
            matches!(
                parameter,
                "" | "0" | "22" | "23" | "24" | "25" | "27" | "28" | "29" | "39" | "49"
            )
        }) {
            active.write(out);
        }
        rest = &sequence[end + 1..];
    }
    out.push_str(rest);
}

#[cfg(test)]
mod tests {
    use super::*;

    fn paint(
        template: &str,
        coloured: bool,
        section: impl FnMut(&str) -> Option<String>,
    ) -> String {
        let style = if coloured {
            Style::COLOURED
        } else {
            Style::PLAIN
        };
        substitute(template, style, section)
    }

    #[test]
    fn nested_styles_restore_the_parent_and_plain_output_strips_tags() {
        let template = "{$red}before {$bold}strong{/$} after{/$}: {{usage}}";
        assert_eq!(
            paint(template, false, |_| Some("Usage: ex".to_string())),
            "before strong after: Usage: ex"
        );
        assert_eq!(
            paint(template, true, |_| Some("Usage: ex".to_string())),
            "\u{1b}[31mbefore \u{1b}[1;31mstrong\u{1b}[0m\u{1b}[31m after\u{1b}[0m: Usage: ex"
        );
    }

    #[test]
    fn markup_inside_a_substituted_section_is_opaque() {
        assert_eq!(
            paint("{$heading}Title{/$}\n{{about}}", false, |_| {
                Some("The literal {$red} word".to_string())
            }),
            "Title\nThe literal {$red} word"
        );
    }

    #[test]
    fn an_inner_style_close_restores_the_template_style() {
        assert_eq!(
            paint("{$red}{{about}}{/$}", true, |_| {
                Some("before \u{1b}[36mcode\u{1b}[39m after".to_string())
            }),
            "\u{1b}[31mbefore \u{1b}[36mcode\u{1b}[39m\u{1b}[31m after\u{1b}[0m"
        );
    }

    #[test]
    fn style_only_lines_do_not_keep_an_empty_section_gap_open() {
        assert_eq!(
            paint(
                "{{usage}}\n\n{$red}{{args}}{/$}\n\n{{flags}}",
                false,
                |name| {
                    Some(
                        match name {
                            "usage" => "Usage: ex",
                            "flags" => "Flags:\n  --force",
                            _ => "",
                        }
                        .to_string(),
                    )
                }
            ),
            "Usage: ex\n\nFlags:\n  --force"
        );
    }

    #[test]
    fn validation_rejects_unknown_and_unbalanced_styles() {
        assert!(check("{$heading}yes{/$}").is_ok());
        assert!(check("{$orange}no{/$}").is_err());
        assert!(check("{$red}no").is_err());
        assert!(check("no{/$}").is_err());
    }

    #[test]
    fn tags_on_lines_of_their_own_keep_a_balanced_style_stack() {
        let template = "{$heading}\nMY TOOL\n{/$}\n\n{{usage}}";
        assert_eq!(
            paint(template, false, |_| Some("Usage: ex".to_string())),
            "MY TOOL\n\nUsage: ex"
        );
        assert_eq!(
            paint(template, true, |_| Some("Usage: ex".to_string())),
            "\u{1b}[1;33mMY TOOL\u{1b}[0m\n\nUsage: ex"
        );

        assert_eq!(
            paint("{$dim}fine print\n{/$}", true, |_| None),
            "\u{1b}[2mfine print\u{1b}[0m"
        );
        assert_eq!(
            paint("before\n{$red}\nafter\n{/$}", false, |_| None),
            "before\n\nafter"
        );
    }

    #[test]
    fn malformed_markup_is_literal_and_escaped_tags_can_be_documented() {
        assert_eq!(
            paint("before {$red and {{usage}}", true, |_| {
                Some("Usage: ex".to_string())
            }),
            "before {$red and Usage: ex"
        );
        assert_eq!(
            paint("{$$heading}literal{/$$}", true, |_| None),
            "{$heading}literal{/$}"
        );
        assert!(check("{$$heading}literal{/$$}").is_ok());
    }

    #[test]
    fn a_palette_remaps_role_tags_in_a_template() {
        let style = Style::COLOURED.palette(Palette::DEFAULT.heading("cyan+bold"));
        assert_eq!(
            substitute("{$heading}Title{/$}", style, |_| None),
            "\u{1b}[1;36mTitle\u{1b}[0m"
        );
    }
}
