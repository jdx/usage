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
/// | `after_help` | examples, `after_help`, and the author/license footer on a long page |
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
pub const STYLES: [&str; 23] = [
    "heading",
    "option",
    "metavar",
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
    if check_styles(template).is_err() {
        return substitute_sections_only(template, section);
    }
    let mut out = String::with_capacity(template.len());
    let mut rest = template;
    loop {
        let placeholder = rest.find("{{").map(|at| (at, 0));
        let style = next_style_event(rest).map(|(at, event)| (at, event as u8 + 1));
        let Some((at, kind)) = [placeholder, style]
            .into_iter()
            .flatten()
            .min_by_key(|(at, _)| *at)
        else {
            out.push_str(rest);
            break;
        };
        out.push_str(&rest[..at]);
        rest = &rest[at..];
        match kind {
            0 => {
                let after = &rest[2..];
                let Some(end) = after.find("}}") else {
                    out.push_str(rest);
                    break;
                };
                match section(after[..end].trim()) {
                    Some(text) => out.push_str(&text),
                    None => out.push_str(&rest[..2 + end + 2]),
                }
                rest = &after[end + 2..];
            }
            1 => {
                let Some(end) = rest.find('}') else {
                    out.push_str(rest);
                    break;
                };
                rest = &rest[end + 1..];
            }
            2 => rest = &rest[4..],
            3 => {
                out.push_str("{$");
                rest = &rest[3..];
            }
            _ => {
                out.push_str("{/$}");
                rest = &rest[5..];
            }
        }
    }
    collapse_blank_runs(&out)
}

fn check_styles(template: &str) -> Result<(), String> {
    let mut rest = template;
    let mut depth = 0usize;
    while let Some((at, event)) = next_style_event(rest) {
        let tag = &rest[at..];
        match event {
            StyleEvent::EscapeOpen => rest = &tag[3..],
            StyleEvent::EscapeClose => rest = &tag[5..],
            StyleEvent::Open => {
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
            StyleEvent::Close => {
                if depth == 0 {
                    return Err("help_template has a `{/$}` with no open style tag".to_string());
                }
                depth -= 1;
                rest = &tag[4..];
            }
        }
    }
    if depth == 0 {
        Ok(())
    } else {
        Err("help_template has a style tag with no `{/$}` after it".to_string())
    }
}

#[derive(Clone, Copy)]
enum StyleEvent {
    Open = 0,
    Close = 1,
    EscapeOpen = 2,
    EscapeClose = 3,
}

fn next_style_event(template: &str) -> Option<(usize, StyleEvent)> {
    [
        ("{$$", StyleEvent::EscapeOpen),
        ("{/$$}", StyleEvent::EscapeClose),
        ("{$", StyleEvent::Open),
        ("{/$}", StyleEvent::Close),
    ]
    .into_iter()
    .enumerate()
    .filter_map(|(priority, (token, event))| template.find(token).map(|at| ((at, priority), event)))
    .min_by_key(|(position, _)| *position)
    .map(|((at, _), event)| (at, event))
}

fn substitute_sections_only(template: &str, section: impl Fn(&str) -> Option<String>) -> String {
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
