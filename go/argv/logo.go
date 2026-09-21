package argv

import "strings"

// Where a spec's logo goes on the page, and what happens when it does not fit.
//
// A logo is art, and art that pushes the help off the screen is worse than no art.
// So the spec says what the logo is and this file says where it goes, from the one
// thing the spec cannot know: how wide the page is.
//
// Three outcomes, in order of preference:
//
//  1. Beside the page, its right edge at the page's right edge, starting at the
//     first line. Taken when every line it would share ends at least logoGutter
//     columns short of where the art starts, so nothing collides.
//  2. Above the page, as a banner with a blank line under it. Taken when the page
//     is too wide to share a line but is still wider than the art.
//  3. Not at all, when the page is narrower than the art itself: a wrapped logo is
//     not a logo, and the page renders as though the spec declared none.
//
// Ported from usage-lib's docs::logo and usage-argv's placeLogo counterpart, and
// held to the same standard as this package's other neighbours: conformance is what
// says the three still agree. Width is counted in runes, as every other column on
// this page is counted — art built from double-width characters will not line up,
// in any of the three.

// logoGutter is the blank columns kept between the widest line of the page and the
// art beside it.
const logoGutter = 2

// logoMinPage is the narrowest a page will make itself to keep a logo in its margin.
// Below this the art is not worth what it costs: a page wrapped into fifty columns on
// a terminal that has more of them looks like a rendering fault, and the banner is the
// better use of a narrow window.
const logoMinPage = 50

// logoMargin is the columns a page keeps for itself when a logo takes the margin, and
// the column the art starts in.
//
// A logo beside the page is a reservation, made before anything is laid out, not a
// decision taken about a finished page: help wraps to whatever width it is given, so a
// page rendered at the full width fills the full width and there is never any margin
// left to put art in. Every implementation narrows the page first and places the art
// second, and the two halves have to agree about the same number.
//
// ok is false when the page keeps the whole width: no logo, not the root page, no
// art left after trimming, or narrowing would leave less than logoMinPage.
//
// The empty case is not a formality. A logo that is nothing but blank lines has a
// width of zero and would otherwise reserve the gutter alone — wrapping help two
// columns short to make room for a picture that is never drawn.
func logoMargin(spec HelpSpec, root bool, total int) (page, column int, ok bool) {
	if spec.Logo == "" || !root {
		return total, 0, false
	}
	art := logoLines(spec.Logo)
	if len(art) == 0 {
		return total, 0, false
	}
	artWidth := 0
	for _, line := range art {
		if w := width(line); w > artWidth {
			artWidth = w
		}
	}
	page = total - artWidth - logoGutter
	if page < logoMinPage {
		return total, 0, false
	}
	return page, total - artWidth, true
}

// pageWidth is the width a page's own text is laid out in: the terminal, less any
// logo margin.
func pageWidth(spec HelpSpec, root bool) int {
	page, _, _ := logoMargin(spec, root, helpWidth)
	return page
}

// placeLogo returns the page with the spec's logo on it, when the spec declared one
// and the page is the program's own.
//
// Called last, on a page that is already assembled and already trimmed: the art is
// held in its column by indentation, and anything that trimmed the page afterwards
// would move it.
func placeLogo(page string, spec HelpSpec, root bool) string {
	if spec.Logo == "" || !root {
		return page
	}
	art := logoLines(spec.Logo)
	if len(art) == 0 {
		return page
	}
	artWidth := 0
	for _, line := range art {
		if w := width(line); w > artWidth {
			artWidth = w
		}
	}
	lines := strings.Split(strings.TrimSuffix(page, "\n"), "\n")
	// The reserved column is normally the whole answer. It is still checked against
	// what was actually rendered: a line nothing can wrap — a synopsis, an example's
	// command, a long URL — can overrun the margin it was given, and art printed over
	// it would be worse than the banner.
	if _, column, ok := logoMargin(spec, root, helpWidth); ok && fitsBeside(lines, len(art), column) {
		return finishLogo(beside(lines, art, column))
	}
	if artWidth > helpWidth {
		return page
	}
	return finishLogo(append(append(append([]string{}, art...), ""), lines...))
}

// fitsBeside reports whether every page line the art would share leaves the gutter
// clear.
func fitsBeside(lines []string, height, column int) bool {
	for i, line := range lines {
		if i >= height {
			break
		}
		if width(line)+logoGutter > column {
			return false
		}
	}
	return true
}

func beside(lines, art []string, column int) []string {
	// Art taller than the page keeps going below it rather than being cut off: a
	// logo with its bottom sliced away looks like a rendering fault, and a short
	// page has the room.
	for len(lines) < len(art) {
		lines = append(lines, "")
	}
	out := append([]string{}, lines...)
	for i, line := range art {
		if line == "" {
			continue
		}
		pad := max(column-width(out[i]), 0)
		out[i] += strings.Repeat(" ", pad) + line
	}
	return out
}

// logoLines is the art's lines, without the blank ones above and below it and
// without trailing spaces. An author writes a logo as an indented block, which
// commonly arrives with a leading newline and a trailing one; neither is part of
// the picture.
func logoLines(logo string) []string {
	// Escapes come out here rather than at composition time, so that what is measured is
	// what is printed. This renderer produces the portable plain page and colours nothing,
	// so art carrying its own colour would otherwise write control bytes into it.
	lines := strings.Split(stripANSISequences(logo), "\n")
	for i, line := range lines {
		lines[i] = strings.TrimRight(line, " \t\r")
	}
	for len(lines) > 0 && lines[0] == "" {
		lines = lines[1:]
	}
	for len(lines) > 0 && lines[len(lines)-1] == "" {
		lines = lines[:len(lines)-1]
	}
	return lines
}

// finishLogo joins the composed lines back into a page ending in exactly one
// newline and carrying no trailing spaces.
func finishLogo(lines []string) string {
	return strings.TrimRight(strings.Join(lines, "\n"), " \t\n") + "\n"
}
