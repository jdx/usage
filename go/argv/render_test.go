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
		{&Error{Code: CodeConflictingFlags, Name: "file", Spelling: "--file",
			Other: "stdin", OtherSpelling: "--stdin"},
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

// A short-only flag is named the way it can be typed.
//
// `--f` is not a flag anybody can enter, and the advice that comes with a missing
// value has to be followable: telling someone with `-j` to write `--j=-1` sends
// them to an unknown-flag error.
func TestAShortOnlyFlagIsNamedAsItIsTyped(t *testing.T) {
	short := &Flag{Key: 1, Name: "j", Shorts: []byte{'j'}, TakesValue: true}
	got := explain(&Error{Code: CodeMissingFlagValue, Flag: short}, nil)
	if strings.Contains(got, "--j") {
		t.Errorf("a short-only flag should not be named `--j`: %s", got)
	}
	for _, want := range []string{"`-j`", "`-j=-x`"} {
		if !strings.Contains(got, want) {
			t.Errorf("want %s in: %s", want, got)
		}
	}

	long := &Flag{Key: 2, Name: "jobs", Longs: []string{"jobs"}, Shorts: []byte{'j'}}
	if got := explain(&Error{Code: CodeMissingFlagValue, Flag: long}, nil); !strings.Contains(got, "`--jobs`") {
		t.Errorf("a flag with a long form is named by it: %s", got)
	}

	// The post-binding failures never see a flag, so they carry the spelling the
	// tables worked out — see TestAOneCharacterLongFormIsNotMistakenForAShort.
	if got := explain(&Error{Code: CodeMissingRequiredFlag, Name: "f", Spelling: "-f"}, nil); !strings.Contains(got, "`-f`") {
		t.Errorf("the carried spelling should be used: %s", got)
	}
	if got := explain(&Error{Code: CodeMissingRequiredFlag, Name: "file", Spelling: "--file"}, nil); !strings.Contains(got, "`--file`") {
		t.Errorf("the carried spelling should be used: %s", got)
	}
}

// An error quotes back what the user typed, and what the user typed can contain
// escape sequences. Rendering a rejected value is not a reason to execute it.
func TestControlCharactersDoNotReachTheTerminal(t *testing.T) {
	got := explain(&Error{Code: CodeUnknownFlag, Token: "--x\x1b[31mred\r\nerror: forged"}, nil)
	for _, forbidden := range []string{"\x1b", "\r", "\n"} {
		if strings.Contains(got, forbidden) {
			t.Errorf("a control character survived into %q", got)
		}
	}
	// Still legible: the escaping shows what was there rather than dropping it.
	for _, want := range []string{`\x1b`, `\r`, `\n`, "red", "forged"} {
		if !strings.Contains(got, want) {
			t.Errorf("want %q kept visible in %q", want, got)
		}
	}
}

// Every failure that names an entry names it the way it is typed.
//
// The spelling is carried on the metadata, so the rules that judge an entry after
// binding can pass it on — they never see a *Flag. Missing this on three of the
// codes left a short-only flag reported as `j`, which is not something anyone can
// type, and the whole point of carrying it was to stop printing those.
func TestEveryPostBindingFailureNamesTheFlagAsTyped(t *testing.T) {
	short := &Meta{Name: "jobs", Flag: true, Spelling: "-j"}
	for _, c := range []struct {
		what string
		err  *Error
	}{
		{"a choice", Check(&Meta{Name: short.Name, Flag: true, Spelling: short.Spelling,
			Choices: []string{"a"}}, []string{"b"}, 1)},
		{"too few", Check(&Meta{Name: short.Name, Flag: true, Spelling: short.Spelling,
			VarMin: 2}, []string{"a"}, 1)},
		{"too many", Check(&Meta{Name: short.Name, Flag: true, Spelling: short.Spelling,
			VarMax: 1}, []string{"a", "b"}, 2)},
		{"required", Check(&Meta{Name: short.Name, Flag: true, Spelling: short.Spelling,
			Required: true}, nil, 0)},
	} {
		if c.err == nil {
			t.Fatalf("%s should fail", c.what)
		}
		if got := explain(c.err, nil); !strings.Contains(got, "`-j`") {
			t.Errorf("%s should name the flag as `-j`: %s", c.what, got)
		}
	}

	// An argument has no spelling to carry, and its bare name is what it is
	// called: `<file>` is not typed with dashes.
	arg := Check(&Meta{Name: "file", Choices: []string{"a"}}, []string{"b"}, 0)
	if got := explain(arg, nil); !strings.Contains(got, "`file`") {
		t.Errorf("an argument keeps its name: %s", got)
	}
}

// The same for the message Go's error interface hands out.
//
// Render is the page a CLI prints, but an error is a value: it gets logged,
// wrapped, printed by a caller that never calls Render at all. That string
// reaches a terminal too, and it was quoting the command line raw.
func TestAnErrorValueIsSafeToPrintToo(t *testing.T) {
	for _, e := range []*Error{
		{Code: CodeUnknownFlag, Token: "--x\x1b[31m\r\nerror: forged"},
		{Code: CodeUnexpectedArg, Token: "wat\x1b[31m\r\nerror: forged"},
		{Code: CodeInvalidValue, Name: "jobs", Value: "8\x1b[31m\r\nerror: forged"},
	} {
		got := e.Error()
		for _, forbidden := range []string{"\x1b", "\r", "\n"} {
			if strings.Contains(got, forbidden) {
				t.Errorf("a control character survived into %q", got)
			}
		}
		if !strings.Contains(got, "forged") {
			t.Errorf("the token should still be shown: %q", got)
		}
	}
}

// The advice names the form the user typed, where the parser knows it.
//
// A flag answers to several spellings, and the first is not always the one in
// front of them: an inherited `--jobs --workers` whose `--jobs` a nearer command
// has taken still binds through `--workers`, and advising `--jobs=-x` there sends
// them to a different flag entirely.
func TestTheAttachedFormIsSpelledTheWayItWasTyped(t *testing.T) {
	global := &Flag{Key: 2, Name: "jobs", Longs: []string{"jobs", "workers"},
		TakesValue: true, Global: true}
	local := &Flag{Key: 3, Name: "jobs", Longs: []string{"jobs"}}
	sub := &Command{Name: "run", Key: 4, Flags: []*Flag{local}}
	root := &Command{Name: "ex", Key: 1, Flags: []*Flag{global}, Subcommands: []*Command{sub}}

	p := New(root, []string{"run", "--workers"})
	for p.Next() {
	}
	err, _ := p.Err().(*Error)
	if err == nil || err.Code != CodeMissingFlagValue {
		t.Fatalf("a value-taking flag with nothing after it should fail: %v", p.Err())
	}
	got := explain(err, nil)
	if !strings.Contains(got, "`--workers=-x`") {
		t.Errorf("the advice should use the form that bound: %s", got)
	}
	if strings.Contains(got, "--jobs") {
		t.Errorf("`--jobs` is the nearer command's flag here: %s", got)
	}
}

// The spelling is carried, not guessed.
//
// A one-character *long* form and a short form are both one character, so a
// heuristic on the name renders `--a` as `-a` — a form that does not exist, and
// one that may belong to a different flag.
func TestAOneCharacterLongFormIsNotMistakenForAShort(t *testing.T) {
	long := explain(&Error{Code: CodeMissingRequiredFlag, Name: "a", Spelling: "--a"}, nil)
	if !strings.Contains(long, "`--a`") {
		t.Errorf("want --a, got %s", long)
	}
	short := explain(&Error{Code: CodeMissingRequiredFlag, Name: "a", Spelling: "-a"}, nil)
	if !strings.Contains(short, "`-a`") || strings.Contains(short, "--a") {
		t.Errorf("want -a, got %s", short)
	}
	// Nothing carried: the bare name rather than a form the user cannot type.
	bare := explain(&Error{Code: CodeMissingRequiredFlag, Name: "a"}, nil)
	if strings.Contains(bare, "-a") {
		t.Errorf("no spelling means no prefix invented: %s", bare)
	}
	// Both sides of a conflict get their own.
	both := explain(&Error{Code: CodeConflictingFlags, Name: "a", Spelling: "--a",
		Other: "f", OtherSpelling: "-f"}, nil)
	if !strings.Contains(both, "`--a`") || !strings.Contains(both, "`-f`") {
		t.Errorf("want both spellings, got %s", both)
	}
}
