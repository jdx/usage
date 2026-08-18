package argv

import (
	"reflect"
	"strings"
	"testing"
)

// The cursor is written as `⌶` in these cases, and cut out before the split, so a
// case reads as the line the user is looking at.
func at(line string, shell Shell) SplitLine {
	cursor := strings.Index(line, "⌶")
	if cursor < 0 {
		cursor = len(line)
	}
	return Split(strings.Replace(line, "⌶", "", 1), cursor, shell)
}

func TestSplittingALine(t *testing.T) {
	for _, c := range []struct {
		line   string
		words  []string
		cword  int
		prefix string
	}{
		// The plain cases: a word being typed, and a gap after one.
		{"ex inst⌶", []string{"ex", "inst"}, 1, "inst"},
		{"ex ⌶", []string{"ex", ""}, 1, ""},
		{"ex install ⌶", []string{"ex", "install", ""}, 2, ""},
		// A cursor inside a word ignores the rest of it: what follows is a tail the
		// user has not decided on, and it should not narrow what can be typed.
		{"ex ins⌶tall", []string{"ex", "install"}, 1, "ins"},
		// A cursor in the gap *before* a word is completing a word that is not
		// there yet, so one is made for it.
		{"ex ⌶install", []string{"ex", "", "install"}, 1, ""},
		// Quotes hold a word together, and an unclosed one is a word being typed.
		{`ex "two words⌶`, []string{"ex", "two words"}, 1, "two words"},
		{`ex 'two words' ⌶`, []string{"ex", "two words", ""}, 2, ""},
		// An empty quoted string is a word, not a gap.
		{`ex "" ⌶`, []string{"ex", "", ""}, 2, ""},
		// An escape takes the character after it, and a trailing one is a line
		// still being typed rather than a mistake.
		{`ex two\ wo⌶`, []string{"ex", "two wo"}, 1, "two wo"},
		{`ex two\⌶`, []string{"ex", "two"}, 1, "two"},
		// Nothing at all is still one word: the cursor is completing it.
		{"", []string{""}, 0, ""},
		{"ex⌶", []string{"ex"}, 0, "ex"},
	} {
		got := at(c.line, Bash)
		want := SplitLine{Words: c.words, Cword: c.cword, Prefix: c.prefix}
		if !reflect.DeepEqual(got, want) {
			t.Errorf("%q:\n got %+v\nwant %+v", c.line, got, want)
		}
	}
}

// Inside double quotes a backslash escapes only what it could mean something to,
// which is what lets a Windows path survive being quoted.
func TestAnEscapeInsideQuotes(t *testing.T) {
	got := at(`ex "C:\Users\me⌶`, Bash)
	if want := `C:\Users\me`; got.Prefix != want {
		t.Errorf("want %q, got %q", want, got.Prefix)
	}
	got = at(`ex "say \"hi\"⌶`, Bash)
	if want := `say "hi"`; got.Prefix != want {
		t.Errorf("want %q, got %q", want, got.Prefix)
	}
}

// PowerShell escapes with a backtick and writes a quote by doubling it. Splitting
// its lines by POSIX rules turned a path into an escape sequence.
func TestPowerShellQuotesItsOwnWay(t *testing.T) {
	if got := at("ex C:\\Users\\me⌶", PowerShell); got.Prefix != `C:\Users\me` {
		t.Errorf("a backslash is not an escape in PowerShell: %q", got.Prefix)
	}
	if got := at("ex `\"quoted⌶", PowerShell); got.Prefix != `"quoted` {
		t.Errorf("a backtick escapes: %q", got.Prefix)
	}
	if got := at("ex 'it''s'⌶", PowerShell); got.Prefix != "it's" {
		t.Errorf("a doubled quote is one quote: %q", got.Prefix)
	}
	// And the same doubled quote is two words to bash, which is the point of
	// asking which shell sent the line.
	if got := at("ex 'it''s'⌶", Bash); got.Prefix != "its" {
		t.Errorf("bash has no doubling rule: %q", got.Prefix)
	}
}

// A cursor is a byte offset from a shell's arithmetic, and a completion request is
// not the place to be strict about it.
func TestACursorOutsideTheLine(t *testing.T) {
	if got := Split("ex run", 999, Bash); got.Prefix != "run" {
		t.Errorf("past the end is the end: %+v", got)
	}
	if got := Split("ex ünïcode", 5, Bash); got.Prefix != "ü" {
		// Byte 5 is inside `ï`; the character it belongs to starts at 4, and the
		// word so far is `ü`. Moved back rather than cutting a rune in half.
		t.Errorf("a cursor inside a character moves back to its start: %+v", got)
	}
}

// The words a parser walks: after the program name, before the word being typed.
func TestWhatTheParserIsGiven(t *testing.T) {
	got := at("ex install no⌶", Bash)
	if want := []string{"install"}; !reflect.DeepEqual(got.Argv(), want) {
		t.Errorf("want %v, got %v", want, got.Argv())
	}
	// Nothing but the program name yet, so nothing to walk.
	if got := at("ex ⌶", Bash); len(got.Argv()) != 0 {
		t.Errorf("want nothing, got %v", got.Argv())
	}
	// Not even the program name: a line that is only a cursor.
	if got := at("⌶", Bash); len(got.Argv()) != 0 {
		t.Errorf("want nothing, got %v", got.Argv())
	}
}
