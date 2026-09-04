//! The named sections a `help_template` may place, and how one is filled in.
//!
//! A spec can say what order its help sections come in — `help_template "{{about}}{{usage}}…"`
//! — and nothing more than that. The template holds a closed vocabulary of *pre-rendered*
//! sections rather than the metadata behind them, which is what lets an interpreter, a compiled
//! parser and a generated Go program agree: they agree on where each section starts and ends and
//! on a small colour-tag vocabulary, not on the metadata behind a section.
//!
//! The twins of this module are `usage_argv::help`'s `SECTIONS` and `Sections`, and Go's
//! `helpSections`. `conformance/tests/render.rs` is what says the three still agree.

/// The sections a template may name, and nothing else.
///
/// | section      | content                                                             |
/// | ------------ | ------------------------------------------------------------------- |
/// | `about`      | `before_help`, the version banner, and the description              |
/// | `usage`      | the `Usage:` synopsis, however many lines it takes                  |
/// | `commands`   | the subcommand list, or the flattened bodies under `flatten_help`   |
/// | `args`       | every argument group, each under its heading                        |
/// | `flags`      | this command's flag groups, then the globals it inherits            |
/// | `grouped_args` | arguments with a declared help heading                            |
/// | `ungrouped_args` | arguments under the default `Arguments` heading                 |
/// | `grouped_flags` | flags with a declared help heading                               |
/// | `ungrouped_flags` | flags under `Flags`, plus inherited global flags                |
/// | `after_help` | examples, `after_help`, and the root long page's package footer      |
pub const SECTIONS: [&str; 10] = [
    "about",
    "usage",
    "commands",
    "args",
    "flags",
    "grouped_args",
    "ungrouped_args",
    "grouped_flags",
    "ungrouped_flags",
    "after_help",
];

/// The styles a template may apply to its own text or to a rendered section.
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

const MARK: char = '\u{2}';
const END: char = '\u{3}';

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
            match fragment {
                "heading" => {
                    self.foreground = Some(33);
                    self.bold = true;
                }
                "option" => {
                    self.foreground = Some(32);
                    self.bold = true;
                }
                "metavar" => {
                    self.foreground = Some(35);
                    self.bold = true;
                }
                "command" => {
                    self.foreground = Some(32);
                    self.bold = true;
                }
                "black" => self.foreground = Some(30),
                "red" => self.foreground = Some(31),
                "green" => self.foreground = Some(32),
                "yellow" => self.foreground = Some(33),
                "blue" => self.foreground = Some(34),
                "magenta" => self.foreground = Some(35),
                "cyan" => self.foreground = Some(36),
                "white" => self.foreground = Some(37),
                "bright-black" => self.foreground = Some(90),
                "bright-red" => self.foreground = Some(91),
                "bright-green" => self.foreground = Some(92),
                "bright-yellow" => self.foreground = Some(93),
                "bright-blue" => self.foreground = Some(94),
                "bright-magenta" => self.foreground = Some(95),
                "bright-cyan" => self.foreground = Some(96),
                "bright-white" => self.foreground = Some(97),
                "bold" => self.bold = true,
                "dim" => self.dim = true,
                "italic" => self.italic = true,
                "underline" => self.underline = true,
                _ => return None,
            }
        }
        Some(self)
    }

    fn write(self, out: &mut String) {
        let mut separator = "";
        out.push_str("\u{1b}[");
        for (enabled, code) in [
            (self.bold, 1),
            (self.dim, 2),
            (self.italic, 3),
            (self.underline, 4),
        ] {
            if enabled {
                out.push_str(separator);
                out.push_str(&code.to_string());
                separator = ";";
            }
        }
        if let Some(foreground) = self.foreground {
            out.push_str(separator);
            out.push_str(&foreground.to_string());
            separator = ";";
        }
        if separator.is_empty() {
            out.push('0');
        }
        out.push('m');
    }
}

#[cfg(feature = "cli-help")]
pub(crate) fn semantic(specification: &str, text: &str, coloured: bool) -> String {
    if !coloured {
        return text.to_string();
    }
    let mut out = String::with_capacity(text.len() + 16);
    AnsiStyle::default()
        .apply(specification)
        .unwrap_or_default()
        .write(&mut out);
    out.push_str(text);
    AnsiStyle::default().write(&mut out);
    out
}

/// Whether a template is one an author wrote, rather than an empty or whitespace-only
/// string that should render as the default page.
///
/// `help_template ""` is accepted by KDL because it has no unknown placeholders, but it
/// names no layout. Treating it as unset keeps the three renderers on one page instead of
/// Rust substituting an empty string into `"\n"` while Go concatenates the default order.
pub fn is_set(template: &str) -> bool {
    !template.trim().is_empty()
}

/// Whether every `{{…}}` in a template names a section.
///
/// The check a template is held to when a spec is read, so nothing renders a page with a
/// section it cannot fill. The message names the vocabulary, and names the two clap
/// placeholders whose spellings differ, because a template being ported is where this is most
/// likely to be read.
pub fn check(template: &str) -> Result<(), String> {
    check_styles(template)?;
    let mut rest = template;
    while let Some(at) = rest.find("{{") {
        let after = &rest[at + 2..];
        let Some(end) = after.find("}}") else {
            return Err(format!(
                "help_template has a `{{{{` with no `}}}}` after it; the sections are {}",
                SECTIONS.join(", ")
            ));
        };
        let name = after[..end].trim();
        if !SECTIONS.contains(&name) {
            return Err(format!(
                "help_template names no section \"{name}\"; a page is assembled from {} — \
                 reorder, omit or wrap those, and note that clap's `{{options}}` is \
                 `{{{{flags}}}}` here and its `{{positionals}}` is `{{{{args}}}}`",
                SECTIONS.join(", ")
            ));
        }
        rest = &after[end + 2..];
    }
    Ok(())
}

/// Fill a template in, asking `section` for each name it holds.
///
/// A placeholder naming no section is left exactly as it was written: the vocabulary is checked
/// where a spec is read, so one arriving here is text an author meant literally.
///
/// Every section a template names is optional in practice — most commands have no arguments,
/// most have no examples — so a template is written with the separators a full page wants and
/// the empty sections are what [`collapse_blank_runs`] then takes back out.
pub fn substitute(template: &str, section: impl Fn(&str) -> Option<String>) -> String {
    substitute_with_style(template, false, section)
}

pub(crate) fn substitute_with_style(
    template: &str,
    coloured: bool,
    section: impl FnMut(&str) -> Option<String>,
) -> String {
    if check_styles(template).is_err() {
        return substitute_sections_only(template, section);
    }
    let mut marked = String::with_capacity(template.len());
    let mut rest = template;
    let mut section = section;
    loop {
        let placeholder = rest.find("{{").map(|at| (at, Event::Placeholder));
        let style = next_style_event(rest);
        let Some((at, event)) = earliest(placeholder, style) else {
            push_escaped(&mut marked, rest);
            break;
        };
        push_escaped(&mut marked, &rest[..at]);
        rest = &rest[at..];
        match event {
            Event::Placeholder => {
                let after = &rest[2..];
                let Some(end) = after.find("}}") else {
                    push_escaped(&mut marked, rest);
                    break;
                };
                match section(after[..end].trim()) {
                    Some(text) => push_escaped(&mut marked, &text),
                    None => push_escaped(&mut marked, &rest[..2 + end + 2]),
                }
                rest = &after[end + 2..];
            }
            Event::Open => {
                let Some(end) = rest.find('}') else {
                    push_escaped(&mut marked, rest);
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
                push_escaped(&mut marked, "{$");
                rest = &rest[3..];
            }
            Event::EscapeClose => {
                push_escaped(&mut marked, "{/$}");
                rest = &rest[5..];
            }
        }
    }
    render_marked(&collapse_styled_blank_runs(&marked), coloured)
}

fn check_styles(template: &str) -> Result<(), String> {
    let mut rest = template;
    let mut depth = 0usize;
    while let Some((at, event)) = next_style_event(rest) {
        let tag = &rest[at..];
        match event {
            Event::EscapeOpen => rest = &tag[3..],
            Event::EscapeClose => rest = &tag[5..],
            Event::Open => {
                let Some(end) = tag.find('}') else {
                    return Err("help_template has a `{$` with no `}` after it".to_string());
                };
                let specification = &tag[2..end];
                if specification.is_empty() {
                    return Err("help_template has an empty style tag `{$}`".to_string());
                }
                if let Some(unknown) = specification
                    .split('+')
                    .find(|fragment| !STYLES.contains(fragment))
                {
                    return Err(format!(
                        "help_template names no style \"{unknown}\"; use {}",
                        STYLES.join(", ")
                    ));
                }
                depth += 1;
                rest = &tag[end + 1..];
            }
            Event::Close => {
                if depth == 0 {
                    return Err("help_template has a `{/$}` with no open style tag".to_string());
                }
                depth -= 1;
                rest = &tag[4..];
            }
            Event::Placeholder => unreachable!("style scanning does not return placeholders"),
        }
    }
    if depth == 0 {
        Ok(())
    } else {
        Err("help_template has a style tag with no `{/$}` after it".to_string())
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

fn next_style_event(template: &str) -> Option<(usize, Event)> {
    [
        ("{$$", Event::EscapeOpen),
        ("{/$$}", Event::EscapeClose),
        ("{$", Event::Open),
        ("{/$}", Event::Close),
    ]
    .into_iter()
    .enumerate()
    .filter_map(|(priority, (token, event))| template.find(token).map(|at| ((at, priority), event)))
    .min_by_key(|(position, _)| *position)
    .map(|((at, _), event)| (at, event))
}

fn push_escaped(out: &mut String, text: &str) {
    if !text.contains(MARK) {
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

fn collapse_styled_blank_runs(page: &str) -> String {
    let mut out = String::with_capacity(page.len());
    let mut blank = false;
    let mut wrote_visible = false;
    for line in page.split('\n') {
        if visible_is_blank(line) {
            push_markers(&mut out, line);
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

fn render_marked(marked: &str, coloured: bool) -> String {
    let mut out = String::with_capacity(marked.len());
    let mut stack = vec![AnsiStyle::default()];
    let mut rest = marked;
    while let Some(at) = rest.find(MARK) {
        push_content(
            &mut out,
            &rest[..at],
            coloured,
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
            let next = stack
                .last()
                .copied()
                .unwrap_or_default()
                .apply(specification)
                .unwrap_or_default();
            stack.push(next);
            if coloured {
                next.write(&mut out);
            }
        } else {
            if stack.len() > 1 {
                stack.pop();
            }
            if coloured {
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
        coloured,
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

fn substitute_sections_only(
    template: &str,
    mut section: impl FnMut(&str) -> Option<String>,
) -> String {
    let mut out = String::with_capacity(template.len());
    let mut rest = template;
    while let Some(at) = rest.find("{{") {
        out.push_str(&rest[..at]);
        let after = &rest[at + 2..];
        let Some(end) = after.find("}}") else {
            out.push_str(&rest[at..]);
            return collapse_blank_runs(&out);
        };
        match section(after[..end].trim()) {
            Some(text) => out.push_str(&text),
            None => out.push_str(&rest[at..at + 2 + end + 2]),
        }
        rest = &after[end + 2..];
    }
    out.push_str(rest);
    collapse_blank_runs(&out)
}

/// A page's runs of blank lines, each reduced to a single blank line.
///
/// What makes a section optional. `"{{flags}}\n\n{{args}}\n\n{{commands}}"` is written for a
/// command that has all three, and a command with no arguments would otherwise render the two
/// separators back to back and push its commands down the page. Collapsing means a template
/// describes an order rather than a page, so one template can serve a whole CLI.
///
/// The cost is that a template cannot open a gap wider than one blank line, which is a
/// deliberate trade: an author who wants a run of them is asking for something no help page
/// wants, and the alternative is that every optional section needs its own template.
///
/// The rule applies to a template's output and nothing else, so a page assembled in the default
/// order is untouched by it.
/// A line with only spaces on it counts as blank, since that is what an empty placeholder on an
/// indented line leaves behind. Leading and trailing blank lines go entirely; the caller puts back
/// the single newline a page ends with.
fn collapse_blank_runs(page: &str) -> String {
    let mut out = String::with_capacity(page.len());
    let mut blank = false;
    for line in page.split('\n') {
        if line.trim().is_empty() {
            blank = !out.is_empty();
            continue;
        }
        if !out.is_empty() {
            out.push('\n');
            if blank {
                out.push('\n');
            }
        }
        blank = false;
        out.push_str(line);
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn whitespace_alone_is_not_a_layout() {
        assert!(!is_set(""));
        assert!(!is_set("  \n\t"));
        assert!(is_set("{{usage}}"));
        assert!(check("").is_ok());
    }

    #[test]
    fn a_placeholder_naming_no_section_is_refused_by_name() {
        let err = check("{{about}}{{options}}").expect_err("no section is called options");
        assert!(err.contains("\"options\""), "{err}");
        // And says what to write instead, since this is what a ported clap template hits.
        assert!(err.contains("`{{flags}}`"), "{err}");
        assert!(check("{{ about }} {{usage}}").is_ok());
        assert!(check("no placeholders at all").is_ok());
        assert!(check("{{usage").is_err());
    }

    #[test]
    fn substitution_takes_only_the_names_it_is_given() {
        let filled = substitute("[{{usage}}]{{ nope }}", |name| {
            (name == "usage").then(|| "Usage: ex".to_string())
        });
        assert_eq!(filled, "[Usage: ex]{{ nope }}");
    }

    #[test]
    fn colour_markup_is_checked_and_removed_from_plain_pages() {
        assert!(check("{$heading}Usage:{/$} {{usage}}").is_ok());
        assert!(check("{$orange}no{/$}").is_err());
        assert!(check("{$red}unclosed").is_err());
        assert!(check("orphan{/$}").is_err());

        let filled = substitute("{$heading}Custom{/$}\n{{about}}", |_| {
            Some("Literal {$red} prose".to_string())
        });
        assert_eq!(filled, "Custom\nLiteral {$red} prose");

        assert!(check("{$$heading}literal{/$$}").is_ok());
        assert_eq!(
            substitute("{$$heading}literal{/$$}", |_| None),
            "{$heading}literal{/$}"
        );
        assert_eq!(
            substitute("before {$red and {{usage}}", |_| {
                Some("Usage: ex".to_string())
            }),
            "before {$red and Usage: ex"
        );
        assert!(check("{$}")
            .expect_err("an empty tag is invalid")
            .contains("empty style tag"));
    }

    #[test]
    fn a_section_that_came_out_empty_leaves_no_gap_behind() {
        // One template, two commands: the separators a full page wants do not become blank
        // lines on the page that has no arguments.
        let template = "{{usage}}\n\n{{args}}\n\n{{flags}}";
        let full = substitute(template, |name| {
            Some(match name {
                "usage" => "Usage: ex".to_string(),
                "args" => "Arguments:\n  <file>".to_string(),
                _ => "Flags:\n  --force".to_string(),
            })
        });
        assert_eq!(
            full,
            "Usage: ex\n\nArguments:\n  <file>\n\nFlags:\n  --force"
        );

        let no_args = substitute(template, |name| {
            Some(match name {
                "usage" => "Usage: ex".to_string(),
                "args" => String::new(),
                _ => "Flags:\n  --force".to_string(),
            })
        });
        assert_eq!(no_args, "Usage: ex\n\nFlags:\n  --force");
    }

    #[test]
    fn a_sections_own_indentation_survives_the_collapsing() {
        // The rule is about blank lines between sections, so the two spaces a flag's row is
        // indented by are not whitespace it may take.
        let page = substitute("  {{flags}}", |_| {
            Some("Flags:\n      --force  Do it anyway".to_string())
        });
        assert_eq!(page, "  Flags:\n      --force  Do it anyway");
    }
}
