//! The named sections a `help_template` may place, and how one is filled in.
//!
//! A spec can say what order its help sections come in — `help_template "{{about}}{{usage}}…"`
//! — and nothing more than that. The template holds a closed vocabulary of *pre-rendered*
//! sections rather than the metadata behind them, which is what lets an interpreter, a compiled
//! parser and a generated Go program agree: they agree on where each section starts and ends,
//! not on a template language's semantics.
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
/// | `after_help` | examples, `after_help`, and the author/license footer on a long page |
pub const SECTIONS: [&str; 6] = ["about", "usage", "commands", "args", "flags", "after_help"];

/// Whether every `{{…}}` in a template names a section.
///
/// The check a template is held to when a spec is read, so nothing renders a page with a
/// section it cannot fill. The message names the vocabulary, and names the two clap
/// placeholders whose spellings differ, because a template being ported is where this is most
/// likely to be read.
pub fn check(template: &str) -> Result<(), String> {
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
