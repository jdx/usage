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

// A value carrying a tab or a newline would be read as more fields or more rows.
//
// Dropped rather than collapsed: a value is what gets typed onto the command
// line, so a repaired one inserts an argument nobody offered — the shell reports
// success and the CLI receives something else. Values normally come from a spec
// and contain none of this, but a `complete` script can produce anything.
func TestAValueThatCannotTravelIsNotOffered(t *testing.T) {
	a := Answer{Candidates: []Candidate{
		{Value: "one\ttwo\nthree"},
		{Value: "\x01files"}, // would be read as the marker line
		{Value: "plain"},
	}}
	for _, shell := range []Shell{Bash, Zsh, Fish, Nu, PowerShell} {
		got := RenderAnswer(a, shell)
		if strings.Count(got, "\n") != 1 {
			t.Errorf("%v: only the one that travels is offered, got %q", shell, got)
		}
		if !strings.HasPrefix(got, "plain") {
			t.Errorf("%v: the candidate that travels should survive, got %q", shell, got)
		}
	}
}

// The description column is decided over the rows that survive.
//
// A description on a candidate that gets dropped would otherwise turn the column
// on for everyone else, leaving an empty field on every row — the all-or-nothing
// rule broken by the answer it was deciding for.
func TestADroppedRowDoesNotTurnOnTheDescriptionColumn(t *testing.T) {
	a := Answer{Candidates: []Candidate{
		{Value: "bad\tvalue", Describe: "the only description"},
		{Value: "plain"},
	}}
	for _, shell := range []Shell{Fish, Nu, PowerShell} {
		if got := RenderAnswer(a, shell); got != "plain\n" {
			t.Errorf("%v: no column where nothing written has a description: %q", shell, got)
		}
	}
}

// A description is prose, and prose collapses: nothing is typed from it, and a
// two-line help still says both halves on one line.
func TestADescriptionIsCollapsedRatherThanDropped(t *testing.T) {
	a := Answer{Candidates: []Candidate{{Value: "run", Describe: "does a thing\nand another"}}}
	got := RenderAnswer(a, Zsh)
	if strings.Count(got, "\n") != 1 {
		t.Errorf("one candidate is one row, got %q", got)
	}
	for _, want := range []string{"does a thing and another", "run"} {
		if !strings.Contains(got, want) {
			t.Errorf("want %q kept in %q", want, got)
		}
	}
}
