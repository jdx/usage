package argv

import (
	"strings"
	"testing"
)

// A CLI with the shapes the files rule turns on: a named path, a named
// directory, a declared set, and an argument that reads only after a separator.
func requestFixture() (*Command, HelpTable, Metadata) {
	into := &Flag{Key: 2, Name: "into", Longs: []string{"into"}, TakesValue: true}
	tool := &Flag{Key: 3, Name: "tool", Longs: []string{"tool"}, TakesValue: true}
	file := &Arg{Key: 4, Name: "FILE"}
	edit := &Command{Key: 5, Name: "edit", Flags: []*Flag{into}, Args: []*Arg{file}}
	use := &Command{Key: 6, Name: "use", Flags: []*Flag{tool},
		Args: []*Arg{{Key: 7, Name: "TOOL"}}}
	after := &Command{Key: 8, Name: "run",
		Args: []*Arg{{Key: 9, Name: "TASK", DoubleDash: DoubleDashRequired}}}
	root := &Command{Key: 1, Name: "ex", Subcommands: []*Command{edit, use, after}}

	help := HelpTable{{Key: 1}, {Key: 2}, {Key: 3}, {Key: 4}, {Key: 5, Short: "Edit a file"},
		{Key: 6, Short: "Use a tool"}, {Key: 7}, {Key: 8, Short: "Run a task"}, {Key: 9}}
	meta := Metadata{
		{Key: 1},
		{Key: 2, Name: "into", Flag: true, ValueName: "DIR"},
		{Key: 3, Name: "tool", Flag: true, ValueName: "TOOL",
			Choices: []string{"node", "python"}},
		{Key: 4, Name: "FILE"},
		{Key: 5}, {Key: 6},
		{Key: 7, Name: "TOOL", Choices: []string{"node", "python"}},
		{Key: 8},
		{Key: 9, Name: "TASK"},
	}
	return root, help, meta
}

func ask(t *testing.T, shell Shell, line string) (Answer, string) {
	t.Helper()
	root, help, meta := requestFixture()
	cursor := strings.Index(line, "⌶")
	if cursor < 0 {
		cursor = len(line)
	}
	line = strings.Replace(line, "⌶", "", 1)

	argv := []string{RequestName, "--shell", shellName(shell), "--line", line}
	out, ok := Respond(argv, root, help, meta)
	if !ok {
		t.Fatalf("%q should be a completion request", argv)
	}
	req, _ := ParseRequest(argv)
	req.Cursor = cursor
	return req.Answer(root, help, meta), out
}

func shellName(s Shell) string {
	switch s {
	case Zsh:
		return "zsh"
	case Fish:
		return "fish"
	case Nu:
		return "nu"
	case PowerShell:
		return "powershell"
	}
	return "bash"
}

// An ordinary invocation is left alone. The request is recognized before the
// parse, and a CLI that answered completions for `ex --help` would be one nobody
// could run.
func TestAnOrdinaryInvocationIsNotARequest(t *testing.T) {
	root, help, meta := requestFixture()
	for _, argv := range [][]string{{}, {"edit"}, {"--into", "x"}, {"_complete"}} {
		if _, ok := Respond(argv, root, help, meta); ok {
			t.Errorf("%v is a command line, not a completion request", argv)
		}
	}
}

// The three arguments a shell passes, and what a missing one means.
func TestReadingTheRequest(t *testing.T) {
	req, ok := ParseRequest([]string{RequestName, "--shell", "zsh",
		"--line", "ex use no", "--cursor", "6"})
	if !ok || req.Shell != Zsh || req.Line != "ex use no" || req.Cursor != 6 {
		t.Fatalf("read back wrong: %+v ok=%v", req, ok)
	}

	// No cursor is the end of the line, which is where a shell puts it when it has
	// no way to say — nushell, whose completer only ever sees the words.
	req, _ = ParseRequest([]string{RequestName, "--line", "ex use"})
	if req.Cursor != len("ex use") {
		t.Errorf("want the end of the line, got %d", req.Cursor)
	}

	// A shell passing something this version does not know about is answered, not
	// refused: a completion that errors out beeps at every keystroke.
	req, ok = ParseRequest([]string{RequestName, "--wat", "1", "--shell", "klingon",
		"--line", "ex ", "--cursor", "nope"})
	if !ok || req.Shell != Bash || req.Line != "ex " {
		t.Errorf("unknown arguments should be ignored: %+v", req)
	}
}

// What the answer says, end to end, in the shape the shell reads.
func TestAnsweringARequest(t *testing.T) {
	_, out := ask(t, Bash, "ex ⌶")
	for _, want := range []string{"edit", "use", "run"} {
		if !strings.Contains(out, want+"\n") {
			t.Errorf("want %q offered:\n%s", want, out)
		}
	}
	// zsh takes three fields, and the description comes from the same table the
	// page uses.
	_, out = ask(t, Zsh, "ex ed⌶")
	if !strings.HasPrefix(out, "edit\tEdit a file\tedit\n") {
		t.Errorf("want the zsh triple, got %q", out)
	}
}

// Whether the shell should offer paths as well — the question a CLI answers by
// saying nothing about it.
func TestWhenPathsBelongAtTheCursor(t *testing.T) {
	for _, c := range []struct {
		line string
		want Files
		why  string
	}{
		{"ex ⌶", NoFiles, "subcommands were offered, so the position is answered"},
		{"ex -⌶", NoFiles, "a dash-prefixed word is a flag or nothing"},
		{"ex edit ⌶", AnyFile, "the argument is called FILE"},
		{"ex edit --into ⌶", Dirs, "the value is called DIR"},
		{"ex use ⌶", NoFiles, "the argument declares its own set"},
		{"ex use --tool ⌶", NoFiles, "so does the value"},
		{"ex run ⌶", NoFiles, "that argument is not readable until after a --"},
		{"ex run -- ⌶", AnyFile, "and past the separator it is anything at all"},
	} {
		answer, _ := ask(t, Bash, c.line)
		if answer.Files != c.want {
			t.Errorf("%q: want %v, got %v — %s", c.line, c.want, answer.Files, c.why)
		}
	}
}

// A command having flags does not close the position.
//
// `Candidates` offers flags only for a dash-prefixed word, which is what the
// reference does — and what keeps the file fallback working, since the fallback
// asks whether *anything* was offered. Offering every flag at a bare cursor made
// every position on every command with a flag look answered.
func TestFlagsDoNotCloseThePositionToPaths(t *testing.T) {
	root := &Command{Key: 1, Name: "ex",
		Flags: []*Flag{{Key: 2, Name: "verbose", Longs: []string{"verbose"}}},
		Args:  []*Arg{{Key: 3, Name: "INPUT"}}}
	help := HelpTable{{Key: 1}, {Key: 2}, {Key: 3}}
	meta := Metadata{{Key: 1}, {Key: 2, Name: "verbose", Flag: true}, {Key: 3, Name: "INPUT"}}

	answer := Request{Shell: Bash, Line: "ex ", Cursor: 3}.Answer(root, help, meta)
	if answer.Files != AnyFile {
		t.Errorf("nothing describes INPUT, so the shell should offer paths: %v", answer.Files)
	}
	if len(answer.Candidates) != 0 {
		t.Errorf("a bare cursor is not asking for flags: %v", values(answer.Candidates))
	}
	// And a dash asks for them, which closes the position — no path starts with
	// one.
	dashed := Request{Shell: Bash, Line: "ex -", Cursor: 4}.Answer(root, help, meta)
	if dashed.Files != NoFiles || len(dashed.Candidates) == 0 {
		t.Errorf("a dash asks for flags and nothing else: %v %v",
			dashed.Files, values(dashed.Candidates))
	}
}

// A variadic still collecting is on the flag's value, not on the argument behind
// it — so an argument owing a separator has nothing to say about this position.
func TestASeparatorOwedBehindACollectingFlag(t *testing.T) {
	tools := &Flag{Key: 2, Name: "tools", Longs: []string{"tools"},
		TakesValue: true, Variadic: true}
	root := &Command{Key: 1, Name: "ex", Flags: []*Flag{tools},
		Args: []*Arg{{Key: 3, Name: "TASK", DoubleDash: DoubleDashRequired}}}
	help := HelpTable{{Key: 1}, {Key: 2}, {Key: 3}}
	meta := Metadata{{Key: 1}, {Key: 2, Name: "tools", Flag: true, ValueName: "PATH"},
		{Key: 3, Name: "TASK"}}

	const line = "ex --tools a "
	if got := (Request{Shell: Bash, Line: line, Cursor: len(line)}).
		Answer(root, help, meta).Files; got != AnyFile {
		t.Errorf("the cursor is on the flag's value, which is called PATH: %v", got)
	}
}

// A cursor too large to hold falls back to the end of the line.
//
// Wrapping would be worse than refusing: a small number describes a position near
// the start of the line, and the completion would answer confidently about the
// wrong word.
func TestACursorTooLargeToHold(t *testing.T) {
	for _, huge := range []string{"18446744073709551616", "99999999999999999999"} {
		req, ok := ParseRequest([]string{RequestName, "--line", "ex use", "--cursor", huge})
		if !ok || req.Cursor != len("ex use") {
			t.Errorf("%s should fall back to the end of the line, got %d", huge, req.Cursor)
		}
	}
}

// A completer's declared type outranks the name, because the author wrote it.
func TestADeclaredCompleterTypeDecidesIt(t *testing.T) {
	root := &Command{Key: 1, Name: "ex", Args: []*Arg{{Key: 2, Name: "INPUT"}}}
	help := HelpTable{{Key: 1}, {Key: 2}}
	meta := Metadata{{Key: 1}, {Key: 2, Name: "INPUT", CompleteType: "dir"}}

	req := Request{Shell: Bash, Line: "ex ", Cursor: 3}
	if got := req.Answer(root, help, meta).Files; got != Dirs {
		t.Errorf("`complete type=\"dir\"` says directories, got %v", got)
	}
	// And with nothing declared and nothing known, the position is open: a name
	// nobody described could be anything, so the shell should offer paths.
	meta[1].CompleteType = ""
	if got := req.Answer(root, help, meta).Files; got != AnyFile {
		t.Errorf("an undescribed position takes anything, got %v", got)
	}
}
