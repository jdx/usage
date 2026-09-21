//! Where a spec's logo goes on the page, and what happens when it does not fit.
//!
//! A logo is art, and art that pushes the help off the screen is worse than no art. So the
//! spec says *what* the logo is and this module says *where* it goes, from the one thing the
//! spec cannot know: how wide the terminal is right now.
//!
//! Three outcomes, in order of preference:
//!
//! 1. **Beside the page.** The art sits in the right margin, its right edge at the right edge
//!    of the terminal, starting at the first line. Chosen when every line it would sit beside
//!    ends at least [`GUTTER`] columns short of where the art starts, so nothing collides.
//! 2. **Above the page.** The art is printed as a banner with a blank line under it. Chosen
//!    when the page is too wide to share a line but the terminal is still wider than the art.
//! 3. **Not at all.** The terminal is narrower than the art itself, and a wrapped logo is not
//!    a logo. The page renders as though the spec declared none.
//!
//! The rule is the page's, not the author's, because the same spec renders on an 80-column
//! ssh session and a 200-column window and should look deliberate on both.
//!
//! The twins of this module are `usage_argv::help`'s `place_logo` and Go's `placeLogo`, and
//! `conformance/tests/render.rs` is what says the three still agree. Width is counted in
//! characters, as every other column on the page is counted — art built from double-width
//! characters will not line up, in any of the three.

use crate::docs::layout::visible_width;
use crate::docs::strip_ansi;

/// Blank columns kept between the widest line of the page and the art beside it.
pub const GUTTER: usize = 2;

/// The narrowest a page will make itself to keep a logo in its margin.
///
/// Below this the art is not worth what it costs: a page wrapped into fifty columns on a
/// terminal that has more of them looks like a rendering fault, and the banner is the better
/// use of a narrow window.
pub const MIN_PAGE: usize = 50;

/// The columns a page keeps for itself when a logo takes the margin, and where the art starts.
///
/// A logo beside the page is a *reservation*, made before anything is laid out, not a decision
/// taken about a finished page: help wraps to whatever width it is given, so a page rendered
/// at the full width fills the full width and there is never any margin left to put art in.
/// Every implementation therefore narrows the page first and places the art second, and the
/// two halves have to agree about the same number — which is why this is one function.
///
/// `None` when the page keeps the whole width: there is no art after trimming, the width is
/// unbounded, or narrowing it would leave less than [`MIN_PAGE`].
///
/// The empty case is not a formality. A `logo ""`, or one that is nothing but blank lines,
/// has a width of zero and would otherwise reserve the gutter alone — wrapping help two
/// columns short of the terminal to make room for a picture that is never drawn.
pub fn margin(logo: &str, width: usize) -> Option<(usize, usize)> {
    let art = trimmed_lines(logo);
    if art.is_empty() || width == usize::MAX {
        return None;
    }
    let art_width = art_width(&art);
    let page = width.checked_sub(art_width + GUTTER)?;
    (page >= MIN_PAGE).then_some((page, width - art_width))
}

fn art_width(art: &[&str]) -> usize {
    art.iter().copied().map(shown).max().unwrap_or(0)
}

/// A page with the logo placed on it, or the page unchanged when there is no room.
///
/// `page` is the finished page, already coloured or already plain, and already trimmed: the
/// art is put on last precisely so that trimming cannot eat the indentation that holds it in
/// its column.
pub fn place(
    page: &str,
    logo: &str,
    style: Option<&str>,
    width: usize,
    reserved: Option<usize>,
    coloured: bool,
) -> String {
    let art = trimmed_lines(logo);
    if art.is_empty() {
        return page.to_string();
    }
    // `reserved` is the column the page was laid out to leave clear, which is normally the
    // whole answer. It is still checked against what was actually rendered: a line nothing can
    // wrap — a synopsis, an example's command, a long URL — can overrun the margin it was
    // given, and art printed over it would be worse than the banner.
    let column = reserved.filter(|column| fits_beside(page, &art, *column));
    match column {
        Some(column) => beside(page, &art, style, column, coloured),
        None if art_width(&art) <= width => above(page, &art, style, coloured),
        None => page.to_string(),
    }
}

/// Whether every page line the art would share is short enough to leave the gutter clear.
fn fits_beside(page: &str, art: &[&str], column: usize) -> bool {
    page.lines()
        .take(art.len())
        .all(|line| shown(line) + GUTTER <= column)
}

/// How many columns a line takes up, counting what is printed and not the escapes that
/// colour it. A page arrives here already styled, so its widest line is only knowable this
/// way.
fn shown(line: &str) -> usize {
    visible_width(&strip_ansi(line))
}

fn beside(page: &str, art: &[&str], style: Option<&str>, column: usize, coloured: bool) -> String {
    let mut lines: Vec<String> = page.lines().map(str::to_string).collect();
    // Art taller than the page keeps going below it rather than being cut off: a logo with its
    // bottom sliced away looks like a rendering fault, and a short page has the room.
    if lines.len() < art.len() {
        lines.resize(art.len(), String::new());
    }
    for (line, art) in lines.iter_mut().zip(art) {
        if art.is_empty() {
            continue;
        }
        let padding = column.saturating_sub(shown(line));
        line.push_str(&" ".repeat(padding));
        line.push_str(&paint(art, style, coloured));
    }
    finish(lines.join("\n"))
}

fn above(page: &str, art: &[&str], style: Option<&str>, coloured: bool) -> String {
    let mut out = String::new();
    for art in art {
        out.push_str(&paint(art, style, coloured));
        out.push('\n');
    }
    out.push('\n');
    out.push_str(page);
    finish(out)
}

/// One escape pair per line rather than one around the whole logo.
///
/// A sequence left open across a newline is a sequence open across whatever the terminal puts
/// on the next line, which beside a page is the page.
fn paint(line: &str, style: Option<&str>, coloured: bool) -> String {
    if !coloured {
        // The page had its own escapes taken out before it arrived here, and art carrying
        // escapes of its own has to lose them by the same rule: a plain page is plain all
        // the way across, or a redirected `--help` writes control bytes into a file.
        return strip_ansi(line).into_owned();
    }
    match style {
        Some(style) if !line.trim().is_empty() => {
            crate::help_template::semantic(style, line, coloured)
        }
        _ => line.to_string(),
    }
}

/// The art's lines, without the blank ones above and below it and without trailing spaces.
///
/// An author writes a logo as an indented block in KDL or a raw string in Rust, and both
/// commonly arrive with a leading newline and a trailing one. Neither is part of the picture.
fn trimmed_lines(logo: &str) -> Vec<&str> {
    let mut lines: Vec<&str> = logo.lines().map(|line| line.trim_end()).collect();
    while lines.first().is_some_and(|line| line.is_empty()) {
        lines.remove(0);
    }
    while lines.last().is_some_and(|line| line.is_empty()) {
        lines.pop();
    }
    lines
}

/// A page ends in exactly one newline however it was assembled, and carries no trailing
/// spaces on the last line the art reached.
fn finish(page: String) -> String {
    let mut out = page.trim_end().to_string();
    out.push('\n');
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Three lines, four to six columns wide, with the leading blank line and the trailing
    /// newline an authored block arrives with.
    const ART: &str = "\n  /\\\n /  \\\n/____\\\n";

    /// The pair as a renderer uses it: reserve the margin from the width, lay the page out in
    /// what is left, then place the art in what was reserved.
    fn render(page: &str, logo: &str, style: Option<&str>, width: usize, coloured: bool) -> String {
        let reserved = margin(logo, width).map(|(_, column)| column);
        place(page, logo, style, width, reserved, coloured)
    }

    #[test]
    fn a_wide_terminal_reserves_the_margin_and_puts_the_art_in_it() {
        // Six columns of art and two of gutter leave 72 for the page and start it at 74.
        assert_eq!(margin(ART, 80), Some((72, 74)));
        let page = render("usage 1.0\nAbout it\n", ART, None, 80, false);
        let lines: Vec<&str> = page.lines().collect();
        assert_eq!(
            lines
                .iter()
                .map(|line| line.chars().count())
                .collect::<Vec<_>>(),
            vec![78, 79, 80]
        );
        assert!(lines[0].starts_with("usage 1.0 "), "{:?}", lines[0]);
        assert!(lines[0].ends_with("  /\\"), "{:?}", lines[0]);
        assert!(lines[2].ends_with("/____\\"), "{:?}", lines[2]);
    }

    #[test]
    fn a_line_that_overruns_the_margin_takes_the_banner_instead() {
        // 73 characters: one past what the page was given, which is a line nothing could wrap
        // rather than a page laid out too wide.
        let long = "x".repeat(73);
        let page = render(&format!("{long}\nshort\n"), ART, None, 80, false);
        assert!(page.starts_with("  /\\\n /  \\\n/____\\\n\n"), "{page:?}");
    }

    #[test]
    fn one_column_less_fits_beside_it() {
        let page = render(
            &format!("{}\nshort\n", "x".repeat(72)),
            ART,
            None,
            80,
            false,
        );
        assert!(page.lines().next().unwrap().ends_with("  /\\"));
    }

    #[test]
    fn a_terminal_with_no_room_to_spare_keeps_its_width_and_takes_the_banner() {
        // 50 columns would leave the page 42, under `MIN_PAGE`, so the page keeps all 50.
        assert_eq!(margin(ART, 50), None);
        let page = render("usage 1.0\n", ART, None, 50, false);
        assert_eq!(page, "  /\\\n /  \\\n/____\\\n\nusage 1.0\n");
    }

    #[test]
    fn a_terminal_narrower_than_the_art_gets_no_logo() {
        assert_eq!(render("hello\n", ART, None, 5, false), "hello\n");
    }

    #[test]
    fn art_taller_than_the_page_keeps_going_below_it() {
        let page = render("one line\n", ART, None, 80, false);
        assert_eq!(page.lines().count(), 3);
        assert!(page.lines().nth(2).unwrap().ends_with("/____\\"));
    }

    #[test]
    fn a_style_colours_each_line_on_its_own() {
        let page = render("hi\n", "ab\ncd\n", Some("cyan"), 80, true);
        assert_eq!(page.lines().count(), 2);
        for line in page.lines() {
            assert!(line.ends_with("\u{1b}[0m"), "{line:?}");
            assert_eq!(line.matches("\u{1b}[36m").count(), 1, "{line:?}");
        }
    }

    #[test]
    fn a_coloured_page_is_measured_by_what_it_prints() {
        // The page line is 72 printed columns wrapped in escapes; measured with them it would
        // overrun the margin, and this is the proof that it is not measured that way.
        let line = format!("\u{1b}[36m{}\u{1b}[0m", "x".repeat(72));
        let page = render(&format!("{line}\n"), ART, None, 80, true);
        assert!(page.lines().next().unwrap().ends_with("  /\\"), "{page:?}");
    }

    #[test]
    fn a_plain_page_gets_the_art_and_none_of_the_colour() {
        let page = render("hi\n", "ab\n", Some("cyan"), 80, false);
        assert!(page.ends_with("ab\n"));
        assert!(!page.contains('\u{1b}'), "{page:?}");
    }

    #[test]
    fn an_unbounded_page_has_no_right_margin_to_put_a_logo_in() {
        assert_eq!(margin("ab\n", usize::MAX), None);
        assert_eq!(
            render("hi\n", "ab\n", None, usize::MAX, false),
            "ab\n\nhi\n"
        );
    }

    #[test]
    fn a_blank_line_inside_the_art_leaves_no_trailing_spaces_behind() {
        let page = render("one\ntwo\nthree\n", "ab\n\ncd\n", None, 80, false);
        assert_eq!(page.lines().nth(1).unwrap(), "two");
    }

    #[test]
    fn nothing_but_whitespace_is_no_logo() {
        assert_eq!(render("hi\n", "\n   \n", None, 80, false), "hi\n");
    }

    #[test]
    fn nothing_but_whitespace_reserves_nothing_either() {
        // Width zero plus the gutter would still be a reservation, and the page would wrap
        // two columns short to leave room for a picture that never gets drawn.
        assert_eq!(margin("", 80), None);
        assert_eq!(margin("\n   \n\t\n", 80), None);
    }

    #[test]
    fn a_plain_page_loses_the_escapes_the_art_brought_with_it() {
        // The page is stripped before the art is placed, so art that carries its own colour
        // has to be stripped here or a redirected `--help` writes control bytes to a file.
        let page = render("hi\n", "\u{1b}[31mab\u{1b}[0m\n", None, 80, false);
        assert!(!page.contains('\u{1b}'), "{page:?}");
        assert!(page.ends_with("ab\n"), "{page:?}");
    }

    #[test]
    fn a_coloured_page_keeps_the_escapes_the_art_brought_with_it() {
        // Art with more colours than one `style` can name is the reason to write them in.
        let page = render("hi\n", "\u{1b}[31mab\u{1b}[0m\n", None, 80, true);
        assert!(page.ends_with("\u{1b}[31mab\u{1b}[0m\n"), "{page:?}");
    }
}
