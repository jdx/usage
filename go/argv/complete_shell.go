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

// doublesQuotes reports whether a quote inside a quoted string is written by
// doubling it, which is PowerShell's rule and nobody else's.
func (s Shell) doublesQuotes() bool { return s == PowerShell }

// backtickEscapes reports whether an escape is written with a backtick rather
// than a backslash — PowerShell again.
func (s Shell) backtickEscapes() bool { return s == PowerShell }

// ShellNamed is the shell a `--shell` argument names, and whether it named one.
//
// A completion request comes from a script this package wrote, so the name is one
// of five — but it arrives as text off a command line, and a shell that sends
// something else should get an answer rather than a crash.
func ShellNamed(name string) (Shell, bool) {
	switch name {
	case "bash":
		return Bash, true
	case "zsh":
		return Zsh, true
	case "fish":
		return Fish, true
	case "nu", "nushell":
		return Nu, true
	case "powershell", "pwsh":
		return PowerShell, true
	}
	return Bash, false
}

// Files says whether paths belong at this position as well as the candidates.
type Files uint8

const (
	// NoFiles means the position takes only what the CLI named.
	NoFiles Files = iota
	// AnyFile means files, directories, whatever the shell shows for a path.
	AnyFile
	// Dirs means directories only.
	Dirs
	// Executables means commands and executable paths.
	Executables
)

// The line a shell reads to mean "paths belong here too".
//
// A whole line rather than a flag on the protocol, because every one of the five
// shells can already split output into lines and look at the last one. `\x01`
// opens it because no candidate can contain a control character — the parser's
// values are escaped before they are rendered anywhere — so it cannot be mistaken
// for one.
const (
	FilesMarker       = "\x01files"
	DirsMarker        = "\x01dirs"
	ExecutablesMarker = "\x01commands"
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
	//
	// Over the rows that will actually be written, not over every candidate. A
	// description sitting on a row that gets dropped below would otherwise put an
	// empty column on all the survivors — the rule broken by the answer it was
	// deciding for.
	described := false
	for _, c := range a.Candidates {
		if c.Describe != "" && travels(c.Value) {
			described = true
			break
		}
	}

	for _, c := range a.Candidates {
		// The protocols are lines with tab-separated fields, so a value carrying
		// either would be read as more rows or more fields. A candidate normally
		// comes from a spec and contains neither, but a `complete` script can
		// produce anything.
		//
		// Such a candidate is dropped rather than repaired. A value is the text
		// that gets typed onto the command line, so collapsing a tab inside it
		// would insert an argument nobody offered — the shell would report success
		// while the CLI received something else, which is the confusing half of
		// the two. A missing candidate is the honest failure: the user types the
		// value themselves and it works.
		if !travels(c.Value) {
			continue
		}
		value := c.Value
		// The description is prose, and collapsing prose onto one line is what a
		// one-line protocol asks for. Nothing is typed from it.
		description := oneLine(c.Describe)
		switch shell {
		case Bash:
			out.WriteString(value)
		case Zsh:
			// Display, then description, then what to type: a candidate containing
			// a space or a quote has to reach the command line intact.
			out.WriteString(value + "\t" + description + "\t" + zshQuote(value))
		default:
			out.WriteString(value)
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
	case Executables:
		out.WriteString(ExecutablesMarker + "\n")
	}
	return out.String()
}

// travels reports whether a value can be written into these protocols as itself.
//
// Every control character, not only the three that delimit: the marker lines
// that say "files belong here too" open with `\x01`, and a candidate beginning
// with one would be read as a marker rather than as a candidate. The comment on
// FilesMarker says no candidate can contain a control character; this is what
// makes that true.
func travels(value string) bool {
	for _, r := range value {
		if r < 0x20 || r == 0x7f {
			return false
		}
	}
	return true
}

// oneLine collapses text onto one line, because the protocols are line-based
// with tab-separated fields: a break or a tab inside either field would be read
// as another row or another column.
//
// Collapsed rather than truncated, so a two-line description still says both
// halves. A run of breaks becomes one space, and never a leading or trailing one.
// Every other control character goes the same way: a description is displayed by
// the shell, and an escape sequence displayed is an escape sequence run.
func oneLine(text string) string {
	var out strings.Builder
	spaced := false
	for _, r := range text {
		if r < 0x20 || r == 0x7f {
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
