package argv

import "strings"

// A page's sections, and the template that may reorder them.
//
// Ported from usage-argv's `help::Sections`, and held to the same standard as the
// rest of this file's neighbours: the boundaries are what the three implementations
// agree on, so a section here holds exactly what the same section holds there.

// HelpSections is the vocabulary a HelpTemplate may name, and nothing else.
//
// A closed list on purpose. Handing a template the metadata behind a page instead
// would make this renderer's internals part of the spec and ask every
// implementation to expose the metadata behind a section. They agree on these
// boundaries and the small colour-tag vocabulary instead.
//
//	about       BeforeHelp, the version banner, and the description
//	usage       the Usage: synopsis, however many lines it takes
//	commands    the subcommand list, or the flattened bodies under FlattenHelp
//	args             every argument group, each under its heading
//	flags            this command's flag groups, then the globals it inherits
//	grouped_args     arguments with a declared help heading
//	ungrouped_args   arguments under the default Arguments heading
//	grouped_flags    flags with a declared help heading
//	ungrouped_flags  flags under Flags, plus inherited global flags
//	after_help       examples, AfterHelp, and the root long page's package footer
//
// An array rather than a slice, so the vocabulary cannot grow or shrink the way
// a package-level slice can. The names themselves are still assignable; nothing
// in this package writes them.
var HelpSections = [...]string{
	"about", "usage", "commands", "args", "flags",
	"grouped_args", "ungrouped_args", "grouped_flags", "ungrouped_flags",
	"after_help",
}

// HelpStyles is the closed style vocabulary accepted by HelpTemplate.
var HelpStyles = [...]string{
	"heading", "option", "metavar", "command",
	"black", "red", "green", "yellow", "blue", "magenta", "cyan", "white",
	"bright-black", "bright-red", "bright-green", "bright-yellow", "bright-blue",
	"bright-magenta", "bright-cyan", "bright-white",
	"bold", "dim", "italic", "underline",
}

// helpSections is a page under construction, cut at the boundaries a template may
// reorder. `flattened` is not a section an author can name: it is the other half of
// `commands`, since FlattenHelp replaces a command list with the subcommands' own
// bodies, and only one of the two is ever written.
type helpSections struct {
	about          strings.Builder
	usage          strings.Builder
	commands       strings.Builder
	args           strings.Builder
	flags          strings.Builder
	groupedArgs    strings.Builder
	ungroupedArgs  strings.Builder
	groupedFlags   strings.Builder
	ungroupedFlags strings.Builder
	flattened      strings.Builder
	afterHelp      strings.Builder
}

// concatenated is the default page: every section in the order it was written.
//
// A plain join, so this is the same string the renderer produced before the
// sections were separable — the blank line above a section belongs to the section.
func (s *helpSections) concatenated() string {
	var out strings.Builder
	for _, part := range []*strings.Builder{
		&s.about, &s.usage, &s.commands, &s.args, &s.flags, &s.flattened, &s.afterHelp,
	} {
		out.WriteString(part.String())
	}
	return out.String()
}

// named is one section, trimmed, so that a template owns the whitespace between
// them: a template is a layout, and a section carrying the blank line above it
// could not be moved without carrying that decision along.
func (s *helpSections) named(name string) (string, bool) {
	switch name {
	case "about":
		return strings.TrimSpace(s.about.String()), true
	case "usage":
		return strings.TrimSpace(s.usage.String()), true
	case "commands":
		list := strings.TrimSpace(s.commands.String())
		flattened := strings.TrimSpace(s.flattened.String())
		if flattened == "" {
			return list, true
		}
		if list == "" {
			return flattened, true
		}
		return list + "\n\n" + flattened, true
	case "args":
		return strings.TrimSpace(s.args.String()), true
	case "flags":
		return strings.TrimSpace(s.flags.String()), true
	case "grouped_args":
		return strings.TrimSpace(s.groupedArgs.String()), true
	case "ungrouped_args":
		return strings.TrimSpace(s.ungroupedArgs.String()), true
	case "grouped_flags":
		return strings.TrimSpace(s.groupedFlags.String()), true
	case "ungrouped_flags":
		return strings.TrimSpace(s.ungroupedFlags.String()), true
	case "after_help":
		return strings.TrimSpace(s.afterHelp.String()), true
	}
	return "", false
}

// assemble is the finished page: laid out by the spec's template where it has one.
//
// usage-lib trims the whole document and puts back one newline, which keeps the
// blank lines between sections from becoming trailing ones. That holds for a
// template's output too: a page ends in exactly one newline however it was built.
func (s *helpSections) assemble(template string) string {
	page := s.concatenated()
	if strings.TrimSpace(template) != "" {
		page = substituteSections(template, s)
	}
	page = stripANSISequences(page)
	return strings.TrimSpace(page) + "\n"
}

// stripANSISequences removes complete ANSI control-sequence introducer escapes
// from authored help. The Go renderer only produces plain pages, so bytes
// embedded before rendering, such as color_print output carried through a spec,
// have no place in the finished document.
func stripANSISequences(text string) string {
	first := strings.Index(text, "\x1b[")
	if first < 0 {
		return text
	}

	var plain strings.Builder
	plain.Grow(len(text))
	copied := 0
	for at := first; at+1 < len(text); {
		if text[at] != '\x1b' || text[at+1] != '[' {
			at++
			continue
		}

		end := at + 2
		for end < len(text) && text[end] >= 0x30 && text[end] <= 0x3f {
			end++
		}
		for end < len(text) && text[end] >= 0x20 && text[end] <= 0x2f {
			end++
		}
		if end == len(text) || text[end] < 0x40 || text[end] > 0x7e {
			at += 2
			continue
		}

		plain.WriteString(text[copied:at])
		copied = end + 1
		at = copied
	}
	plain.WriteString(text[copied:])
	return plain.String()
}

// substituteSections fills a template in, section by section.
//
// A placeholder naming no section is left exactly as it was written: the vocabulary
// is checked where a spec is authored — KDL refuses one at parse, the Rust derive at
// compile time — so one arriving here is text an author meant literally.
//
// A section that came out empty leaves no gap behind: see collapseBlankRuns, whose
// rule is what lets one template serve a whole CLI.
func substituteSections(template string, s *helpSections) string {
	if !validStyleMarkup(template) {
		return substituteSectionsOnly(template, s)
	}
	var out strings.Builder
	rest := template
	for {
		at, kind := nextTemplateToken(rest)
		if at < 0 {
			out.WriteString(rest)
			return collapseBlankRuns(out.String())
		}
		out.WriteString(rest[:at])
		rest = rest[at:]
		switch kind {
		case "section":
			after := rest[2:]
			end := strings.Index(after, "}}")
			if end < 0 {
				out.WriteString(rest)
				return collapseBlankRuns(out.String())
			}
			if text, ok := s.named(strings.TrimSpace(after[:end])); ok {
				out.WriteString(text)
			} else {
				out.WriteString(rest[:2+end+2])
			}
			rest = after[end+2:]
		case "open":
			end := strings.IndexByte(rest, '}')
			if end < 0 {
				out.WriteString(rest)
				return collapseBlankRuns(out.String())
			}
			rest = rest[end+1:]
		case "close":
			rest = rest[4:]
		case "escape-open":
			out.WriteString("{$")
			rest = rest[3:]
		default:
			out.WriteString("{/$}")
			rest = rest[5:]
		}
	}
}

func substituteSectionsOnly(template string, s *helpSections) string {
	var out strings.Builder
	rest := template
	for {
		at := strings.Index(rest, "{{")
		if at < 0 {
			out.WriteString(rest)
			return collapseBlankRuns(out.String())
		}
		out.WriteString(rest[:at])
		after := rest[at+2:]
		end := strings.Index(after, "}}")
		if end < 0 {
			out.WriteString(rest[at:])
			return collapseBlankRuns(out.String())
		}
		if text, ok := s.named(strings.TrimSpace(after[:end])); ok {
			out.WriteString(text)
		} else {
			out.WriteString(rest[at : at+2+end+2])
		}
		rest = after[end+2:]
	}
}

func validStyleMarkup(template string) bool {
	rest := template
	depth := 0
	for {
		at, kind := nextStyleToken(rest)
		if at < 0 {
			return depth == 0
		}
		tag := rest[at:]
		switch kind {
		case "escape-open":
			rest = tag[3:]
		case "escape-close":
			rest = tag[5:]
		case "open":
			end := strings.IndexByte(tag, '}')
			if end < 0 || end == 2 {
				return false
			}
			for _, style := range strings.Split(tag[2:end], "+") {
				known := false
				for _, candidate := range HelpStyles {
					if style == candidate {
						known = true
						break
					}
				}
				if !known {
					return false
				}
			}
			depth++
			rest = tag[end+1:]
		default:
			if depth == 0 {
				return false
			}
			depth--
			rest = tag[4:]
		}
	}
}

func nextStyleToken(template string) (int, string) {
	at, kind := -1, ""
	for _, token := range []struct {
		text string
		kind string
	}{
		{"{$$", "escape-open"},
		{"{/$$}", "escape-close"},
		{"{$", "open"},
		{"{/$}", "close"},
	} {
		found := strings.Index(template, token.text)
		if found >= 0 && (at < 0 || found < at) {
			at, kind = found, token.kind
		}
	}
	return at, kind
}

func nextTemplateToken(template string) (int, string) {
	at, kind := -1, ""
	for _, token := range []struct {
		text string
		kind string
	}{
		{"{$$", "escape-open"},
		{"{/$$}", "escape-close"},
		{"{{", "section"},
		{"{$", "open"},
		{"{/$}", "close"},
	} {
		found := strings.Index(template, token.text)
		if found >= 0 && (at < 0 || found < at) {
			at, kind = found, token.kind
		}
	}
	return at, kind
}

// collapseBlankRuns reduces every run of blank lines to a single blank line.
//
// The twin of usage::help_template::collapse_blank_runs, and the reason a template
// may name a section a given command does not have: a template carries the
// separators a full page wants, and a command with no arguments would otherwise
// render two of them back to back and push its flags down the page. Most commands
// are missing most sections, so without this a template would have to be written
// per command rather than per CLI.
//
// A whitespace-only line counts as blank, since that is what an empty placeholder on
// an indented line leaves behind; a section's own indentation does not, since that is
// the page. Applies to a template's output alone, so a default page is untouched.
func collapseBlankRuns(page string) string {
	var out strings.Builder
	blank := false
	for _, line := range strings.Split(page, "\n") {
		if strings.TrimSpace(line) == "" {
			blank = out.Len() > 0
			continue
		}
		if out.Len() > 0 {
			out.WriteString("\n")
			if blank {
				out.WriteString("\n")
			}
		}
		blank = false
		out.WriteString(line)
	}
	return out.String()
}
