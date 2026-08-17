package argv

import "strings"

// Writing an answer the way a shell reads it.
//
// One line per candidate, in the shape the shell's own completion machinery
// expects — which is where the five differ. bash reads values only; fish, nu and
// PowerShell take a description after a tab; zsh takes a third field, the text to
// insert, because what it displays and what it types are not always the same
// string.

// Shell is a completion protocol, named for the shell that reads it.
type Shell uint8

const (
	Bash Shell = iota
	Zsh
	Fish
	Nu
	PowerShell
)

// Files says whether paths belong at this position as well as the candidates.
type Files uint8

const (
	// NoFiles means the position takes only what the CLI named.
	NoFiles Files = iota
	// AnyFile means files, directories, whatever the shell shows for a path.
	AnyFile
	// Dirs means directories only.
	Dirs
)

// The line a shell reads to mean "paths belong here too".
//
// A whole line rather than a flag on the protocol, because every one of the five
// shells can already split output into lines and look at the last one. `\x01`
// opens it because no candidate can contain a control character — the parser's
// values are escaped before they are rendered anywhere — so it cannot be mistaken
// for one.
const (
	FilesMarker = "\x01files"
	DirsMarker  = "\x01dirs"
)

// Answer is everything a shell needs to resolve one Tab.
type Answer struct {
	Candidates []Candidate
	Files      Files
}

// RenderAnswer writes an answer in the protocol `shell` reads.
//
// Named for what it renders rather than just `Render`, because [Render] already
// belongs to failures. Two things in one package both turning a value into text
// for a terminal is reason enough to say which.
func RenderAnswer(a Answer, shell Shell) string {
	var out strings.Builder

	// Descriptions are all-or-nothing per answer: a column that appears on some
	// rows and not others reads as missing data rather than as an absent
	// description.
	described := false
	for _, c := range a.Candidates {
		if c.Describe != "" {
			described = true
			break
		}
	}

	for _, c := range a.Candidates {
		description := oneLine(c.Describe)
		switch shell {
		case Bash:
			out.WriteString(c.Value)
		case Zsh:
			// Display, then description, then what to type: a candidate containing
			// a space or a quote has to reach the command line intact.
			out.WriteString(c.Value + "\t" + description + "\t" + zshQuote(c.Value))
		default:
			out.WriteString(c.Value)
			if described {
				out.WriteString("\t" + description)
			}
		}
		out.WriteString("\n")
	}

	switch a.Files {
	case AnyFile:
		out.WriteString(FilesMarker + "\n")
	case Dirs:
		out.WriteString(DirsMarker + "\n")
	}
	return out.String()
}

// oneLine collapses a description onto one line, because the protocols are
// line-based: a break inside a description would look like another candidate.
//
// Collapsed rather than truncated, so a two-line description still says both
// halves. A run of breaks becomes one space, and never a leading or trailing one.
func oneLine(text string) string {
	var out strings.Builder
	spaced := false
	for _, r := range text {
		if r == '\n' || r == '\r' || r == '\t' {
			if !spaced && out.Len() > 0 {
				out.WriteByte(' ')
				spaced = true
			}
			continue
		}
		out.WriteRune(r)
		spaced = false
	}
	return strings.TrimRight(out.String(), " ")
}

// zshQuote makes a value safe to insert on the command line.
func zshQuote(value string) string {
	safe := func(r rune) bool {
		switch {
		case r >= 'a' && r <= 'z', r >= 'A' && r <= 'Z', r >= '0' && r <= '9':
			return true
		}
		return strings.ContainsRune("_-./:@+=%,", r)
	}
	if value != "" {
		all := true
		for _, r := range value {
			if !safe(r) {
				all = false
				break
			}
		}
		if all {
			return value
		}
	}
	return "'" + strings.ReplaceAll(value, "'", `'\''`) + "'"
}
