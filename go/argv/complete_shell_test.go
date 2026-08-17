package argv

import (
	"strings"
	"testing"
)

func answer() Answer {
	return Answer{Candidates: []Candidate{
		{Kind: CandidateCommand, Value: "use", Describe: "Installs a tool"},
		{Kind: CandidateFlag, Value: "--global"},
	}}
}

// Each shell reads a different shape, and the differences are the whole point of
// having five.
func TestEachShellGetsItsOwnShape(t *testing.T) {
	// bash reads values only.
	if got := RenderAnswer(answer(), Bash); got != "use\n--global\n" {
		t.Errorf("bash: got %q", got)
	}

	// zsh takes display, description, and the text to insert.
	got := RenderAnswer(answer(), Zsh)
	if !strings.HasPrefix(got, "use\tInstalls a tool\tuse\n") {
		t.Errorf("zsh: got %q", got)
	}

	// fish, nu and PowerShell take a description after a tab.
	for _, shell := range []Shell{Fish, Nu, PowerShell} {
		got := RenderAnswer(answer(), shell)
		if !strings.HasPrefix(got, "use\tInstalls a tool\n") {
			t.Errorf("%v: got %q", shell, got)
		}
	}
}

// A column that appears on some rows and not others reads as missing data rather
// than as an absent description, so it is all-or-nothing per answer.
func TestDescriptionsAreAllOrNothing(t *testing.T) {
	// One candidate has a description, so the other still gets the column.
	got := RenderAnswer(answer(), Fish)
	if !strings.Contains(got, "--global\t") && !strings.HasSuffix(got, "--global\t\n") {
		if !strings.Contains(got, "--global\t\n") {
			t.Errorf("the column should be present for every row: %q", got)
		}
	}
	// None has one, so no row gets it.
	bare := Answer{Candidates: []Candidate{{Value: "a"}, {Value: "b"}}}
	if got := RenderAnswer(bare, Fish); got != "a\nb\n" {
		t.Errorf("no descriptions means no column: %q", got)
	}
}

// A description is collapsed onto one line rather than truncated, so a two-line
// description still says both halves — the protocols are line-based, and a break
// would look like another candidate.
func TestADescriptionIsCollapsedNotTruncated(t *testing.T) {
	a := Answer{Candidates: []Candidate{
		{Value: "x", Describe: "first half\nsecond half"},
	}}
	got := RenderAnswer(a, Fish)
	if strings.Count(got, "\n") != 1 {
		t.Errorf("a candidate is one line: %q", got)
	}
	for _, want := range []string{"first half", "second half"} {
		if !strings.Contains(got, want) {
			t.Errorf("want %q kept: %q", want, got)
		}
	}
	// A run of breaks is one space, with none left at either end.
	if got := oneLine("\n\na\n\n\nb\n\n"); got != "a b" {
		t.Errorf("want %q, got %q", "a b", got)
	}
}

// A candidate containing a space or a quote has to reach the command line intact.
func TestZshQuotesWhatItMust(t *testing.T) {
	for _, c := range []struct{ in, want string }{
		{"use", "use"},
		{"a/b-c.d:e@f+g=h%i,j_k", "a/b-c.d:e@f+g=h%i,j_k"},
		{"two words", "'two words'"},
		{"it's", `'it'\''s'`},
		{"", "''"},
	} {
		if got := zshQuote(c.in); got != c.want {
			t.Errorf("zshQuote(%q): want %q, got %q", c.in, c.want, got)
		}
	}
}

// The marker is a line because every shell can already look at the last one.
func TestFilesAreAskedForOnTheirOwnLine(t *testing.T) {
	a := answer()
	a.Files = AnyFile
	if got := RenderAnswer(a, Bash); !strings.HasSuffix(got, FilesMarker+"\n") {
		t.Errorf("want the files marker last: %q", got)
	}
	a.Files = Dirs
	if got := RenderAnswer(a, Bash); !strings.HasSuffix(got, DirsMarker+"\n") {
		t.Errorf("want the dirs marker last: %q", got)
	}
	a.Files = NoFiles
	if got := RenderAnswer(a, Bash); strings.Contains(got, "\x01") {
		t.Errorf("no marker where paths do not belong: %q", got)
	}
}
