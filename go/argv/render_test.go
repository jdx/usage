package argv

import (
	"strings"
	"testing"
)

// A rendered failure is judged on three things rather than on bytes: it says what
// went wrong, it shows the command the user was actually in, and it says what to
// try next. There is no reference to match here — see render.go.
func TestRenderSaysWhatWentWrongAndWhatToTry(t *testing.T) {
	sub := &Command{Name: "run", Key: 2, Flags: []*Flag{
		{Key: 3, Name: "force", Longs: []string{"force"}, Shorts: []byte{'f'}},
	}}
	root := &Command{Name: "ex", Key: 1, Subcommands: []*Command{sub}}
	help := HelpTable{{Key: 1}, {Key: 2}, {Key: 3}}
	path, chain := []string{"ex", "run"}, []*Command{root, sub}

	got := Render(&Error{Code: CodeUnknownFlag, Token: "--wat"}, path, chain, help)

	for _, want := range []string{
		"error: unknown flag `--wat`",
		// The command the user was in, not the program.
		"Usage: ex run [-f --force]",
		"try `--help`",
	} {
		if !strings.Contains(got, want) {
			t.Errorf("missing %q in:\n%s", want, got)
		}
	}
}

// Every code renders something specific. A failure that falls through to "could
// not be parsed" tells the user nothing, so the test is that none of them does.
func TestEveryCodeRendersSomethingSpecific(t *testing.T) {
	flag := &Flag{Key: 1, Name: "jobs", Longs: []string{"jobs"}}
	arg := &Arg{Key: 2, Name: "FILE"}
	cases := []*Error{
		{Code: CodeUnknownFlag, Token: "--wat"},
		{Code: CodeMissingFlagValue, Flag: flag},
		{Code: CodeUnexpectedArg, Token: "extra"},
		{Code: CodeArgRequiresDoubleDash, Arg: arg},
		{Code: CodeTooDeep},
		{Code: CodeMissingRequiredFlag, Name: "file"},
		{Code: CodeMissingRequiredArg, Name: "FILE"},
		{Code: CodeInvalidChoice, Name: "shell", Choices: []string{"bash", "zsh"}},
		{Code: CodeVarTooFew, Name: "files", Bound: 2, Got: 1},
		{Code: CodeVarTooMany, Name: "tag", Bound: 1, Got: 3},
		{Code: CodeConflictingFlags, Name: "file", Other: "stdin"},
	}
	for _, e := range cases {
		got := explain(e, nil)
		if got == "" || strings.Contains(got, "could not be parsed") {
			t.Errorf("%v renders nothing useful: %q", e.Code, got)
		}
	}
}

// Help and version are not failures. A caller that gets one should print the
// page, and rendering an error for it would be the wrong thing twice over.
func TestHelpAndVersionRenderNothing(t *testing.T) {
	for _, code := range []Code{CodeHelp, CodeVersion} {
		if got := Render(&Error{Code: code}, nil, nil, nil); got != "" {
			t.Errorf("%v should render nothing, got %q", code, got)
		}
	}
	if got := Render(nil, nil, nil, nil); got != "" {
		t.Errorf("no error should render nothing, got %q", got)
	}
}

// The messages name things the way a user typed them, which is the whole reason
// for the backticks.
func TestMessagesQuoteWhatTheUserWouldType(t *testing.T) {
	for _, c := range []struct {
		err  *Error
		want string
	}{
		{&Error{Code: CodeInvalidChoice, Name: "shell", Choices: []string{"bash", "zsh"}},
			"expected one of: bash, zsh"},
		{&Error{Code: CodeConflictingFlags, Name: "file", Other: "stdin"},
			"`--file` and `--stdin` cannot be given together"},
		{&Error{Code: CodeVarTooFew, Name: "files", Bound: 2, Got: 1},
			"at least 2 values, got 1"},
		{&Error{Code: CodeVarTooMany, Name: "tag", Bound: 1, Got: 3},
			"at most 1 time, given 3"},
	} {
		if got := explain(c.err, nil); !strings.Contains(got, c.want) {
			t.Errorf("want %q in %q", c.want, got)
		}
	}
}
