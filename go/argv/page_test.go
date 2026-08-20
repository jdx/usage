package argv

import (
	"strings"
	"testing"
)

// Examples declared once at the root appear on a page that declares none.
//
// The same fallback `BeforeHelp` and `AfterHelp` get. mise declares no root
// examples, so the 211-page parity suite cannot see this in either direction —
// it is checked here against the reference's rule instead.
func TestExamplesFallBackToTheRoot(t *testing.T) {
	sub := &Command{Name: "run", Key: 2}
	root := &Command{Name: "ex", Key: 1, Subcommands: []*Command{sub}}
	help := HelpTable{
		{Key: 1, Examples: []Example{{Header: "Build it", Code: "ex build"}}},
		{Key: 2, Short: "run it"},
	}
	spec := HelpSpec{Name: "ex", Bin: "ex"}

	for _, page := range []string{
		ShortHelp(spec, []string{"ex", "run"}, []*Command{root, sub}, help),
		LongHelp(spec, []string{"ex", "run"}, []*Command{root, sub}, help),
	} {
		if !strings.Contains(page, "$ ex build") {
			t.Errorf("a page with no examples of its own should show the root's:\n%s", page)
		}
	}

	// And a command's own win where it has them.
	help[1].Examples = []Example{{Code: "ex run --now"}}
	page := ShortHelp(spec, []string{"ex", "run"}, []*Command{root, sub}, help)
	if strings.Contains(page, "ex build") || !strings.Contains(page, "ex run --now") {
		t.Errorf("its own examples should win:\n%s", page)
	}
}

func TestHiddenFlagAliasesStayOutOfHelp(t *testing.T) {
	flag := &Flag{
		Key: 2, Name: "output",
		Longs: []string{"output", "quietly"}, HiddenLongs: []string{"quietly"},
		Shorts: []byte{'o', 'q'}, HiddenShorts: []byte{'q'},
	}
	root := &Command{Name: "ex", Key: 1, Flags: []*Flag{flag}}
	help := HelpTable{{Key: 1}, {Key: 2, Short: "write output"}}
	page := LongHelp(HelpSpec{Name: "ex", Bin: "ex"}, []string{"ex"}, []*Command{root}, help)
	for _, visible := range []string{"--output", "-o"} {
		if !strings.Contains(page, visible) {
			t.Errorf("visible spelling %s should appear:\n%s", visible, page)
		}
	}
	for _, hidden := range []string{"--quietly", "-q"} {
		if strings.Contains(page, hidden) {
			t.Errorf("hidden alias %s should not appear:\n%s", hidden, page)
		}
	}
}

func TestFixedArityHelpKeepsDistinctValueNames(t *testing.T) {
	flag := &Flag{Key: 2, Name: "range", Longs: []string{"range"}, TakesValue: true, Variadic: true}
	arg := &Arg{Key: 3, Name: "PAIR", Var: true}
	root := &Command{Name: "ex", Key: 1, Flags: []*Flag{flag}, Args: []*Arg{arg}}
	help := HelpTable{
		{Key: 1},
		{Key: 2, ValueDemanded: true, ValueNames: []string{"START", "END"}},
		{Key: 3, Demanded: true, ValueNames: []string{"LEFT", "RIGHT"}},
	}
	page := ShortHelp(HelpSpec{Name: "ex", Bin: "ex"}, []string{"ex"}, []*Command{root}, help)
	for _, want := range []string{"--range <START> <END>", "<LEFT> <RIGHT>"} {
		if !strings.Contains(page, want) {
			t.Errorf("missing %q in:\n%s", want, page)
		}
	}
}

func TestFixedArityHelpRepeatsOneValueName(t *testing.T) {
	flag := &Flag{Key: 2, Name: "pair", Longs: []string{"pair"}, TakesValue: true, Variadic: true}
	arg := &Arg{Key: 3, Name: "ITEM", Var: true}
	root := &Command{Name: "ex", Key: 1, Flags: []*Flag{flag}, Args: []*Arg{arg}}
	help := HelpTable{
		{Key: 1},
		{Key: 2, ValueName: "ITEM", ValueArity: 2, ValueDemanded: true},
		{Key: 3, ValueArity: 2, Demanded: true},
	}
	page := ShortHelp(HelpSpec{Name: "ex", Bin: "ex"}, []string{"ex"}, []*Command{root}, help)
	for _, want := range []string{"--pair <ITEM> <ITEM>", "<ITEM> <ITEM>"} {
		if !strings.Contains(page, want) {
			t.Errorf("missing %q in:\n%s", want, page)
		}
	}
	if strings.Contains(page, "<ITEM>…") {
		t.Errorf("exact arity must not render as variadic:\n%s", page)
	}
}

func TestGranularHelpHides(t *testing.T) {
	mode := &Flag{Key: 2, Name: "mode", Longs: []string{"mode"}, TakesValue: true}
	shortOnly := &Flag{Key: 3, Name: "short-only", Longs: []string{"short-only"}}
	longOnly := &Flag{Key: 4, Name: "long-only", Longs: []string{"long-only"}}
	root := &Command{Name: "ex", Key: 1, Flags: []*Flag{mode, shortOnly, longOnly}}
	help := HelpTable{
		{Key: 1},
		{Key: 2, Short: "mode", Choices: []string{"fast", "slow"}, Env: "MODE", Default: []string{"fast"}, HidePossibleValues: true, HideEnv: true, HideDefaultValue: true},
		{Key: 3, Short: "short", HideLongHelp: true},
		{Key: 4, Short: "long", HideShortHelp: true},
	}
	short := ShortHelp(HelpSpec{Name: "ex", Bin: "ex"}, []string{"ex"}, []*Command{root}, help)
	if !strings.Contains(short, "--short-only") || strings.Contains(short, "--long-only") {
		t.Fatalf("wrong short-only visibility:\n%s", short)
	}
	long := LongHelp(HelpSpec{Name: "ex", Bin: "ex"}, []string{"ex"}, []*Command{root}, help)
	if !strings.Contains(long, "--long-only") || strings.Contains(long, "--short-only") {
		t.Fatalf("wrong long-only visibility:\n%s", long)
	}
	for _, page := range []string{short, long} {
		for _, hidden := range []string{"fast, slow", "MODE", "default: fast"} {
			if strings.Contains(page, hidden) {
				t.Fatalf("%q should be hidden:\n%s", hidden, page)
			}
		}
	}
}

func TestSubcommandPresentation(t *testing.T) {
	sub := &Command{Name: "run", Key: 2}
	root := &Command{Name: "ex", Key: 1, Subcommands: []*Command{sub}}
	help := HelpTable{
		{Key: 1, SubcommandHelpHeading: "Actions", SubcommandValueName: "ACTION"},
		{Key: 2, Short: "run it"},
	}
	for _, page := range []string{
		ShortHelp(HelpSpec{Name: "ex", Bin: "ex"}, []string{"ex"}, []*Command{root}, help),
		LongHelp(HelpSpec{Name: "ex", Bin: "ex"}, []string{"ex"}, []*Command{root}, help),
	} {
		if !strings.Contains(page, "Usage: ex <ACTION>") || !strings.Contains(page, "\nActions:\n") {
			t.Fatalf("subcommand presentation was not preserved:\n%s", page)
		}
	}
}

func TestNextLineHelp(t *testing.T) {
	arg := &Arg{Name: "input", Key: 2, Required: true}
	flag := &Flag{Name: "verbose", Key: 3, Longs: []string{"verbose"}}
	annotated := &Flag{Name: "mode", Key: 5, Longs: []string{"mode"}}
	sub := &Command{Name: "run", Key: 4}
	root := &Command{Name: "ex", Key: 1, Args: []*Arg{arg}, Flags: []*Flag{flag, annotated}, Subcommands: []*Command{sub}}
	help := HelpTable{
		{Key: 1, NextLineHelp: true},
		{Key: 2, Short: "Input file"},
		{Key: 3, Short: "Enable verbose output"},
		{Key: 4, Short: "Run it\n"},
		{Key: 5, Env: "MODE", Default: []string{"fast"}, Choices: []string{"fast", "slow"}},
	}

	shortPage := ShortHelp(HelpSpec{Bin: "ex"}, []string{"ex"}, []*Command{root}, help)
	if strings.Contains(shortPage, "    Run it\n\n  help") {
		t.Fatalf("trailing command help newline became a blank line:\n%s", shortPage)
	}
	for _, page := range []string{
		shortPage,
		LongHelp(HelpSpec{Bin: "ex"}, []string{"ex"}, []*Command{root}, help),
	} {
		for _, want := range []string{
			"  [input]\n    Input file",
			"  --verbose\n    Enable verbose output",
			"  run\n    Run it",
			"  --mode\n    [possible values: fast, slow]\n    [env: MODE]\n    (default: fast)",
		} {
			if !strings.Contains(page, want) {
				t.Fatalf("missing %q in:\n%s", want, page)
			}
		}
	}
}

// A description that ends in a break adds no blank line.
//
// clap's `long_about` often ends with one — a `///` block whose last line is
// empty, an examples section written with a trailing newline — and it reaches the
// spec verbatim. The blank line under a description belongs to the renderer, so
// one already in the text was a second one: a stray blank under the about, and in
// the middle of the `Commands:` list.
//
// The rule is usage-lib's and usage-argv's, and mise exercises it — `plugins
// ls-remote` writes its examples that way. It is pinned here as well because the
// parity suite says only that some page differs, not which rule was broken.
func TestADescriptionEndingInABreakAddsNoBlankLine(t *testing.T) {
	sub := &Command{Name: "run", Key: 2}
	root := &Command{Name: "ex", Key: 1, Subcommands: []*Command{sub}}
	help := HelpTable{
		{Key: 1},
		{Key: 2, Short: "run it", Long: "run it\n\nExamples:\n\n    $ ex run\n"},
	}
	spec := HelpSpec{Name: "ex", Bin: "ex"}

	// On the command's own page, above the usage line.
	page := LongHelp(spec, []string{"ex", "run"}, []*Command{root, sub}, help)
	if strings.Contains(page, "$ ex run\n\n\nUsage:") {
		t.Errorf("the description's own break should not double the blank line:\n%q", page)
	}
	if !strings.Contains(page, "$ ex run\n\nUsage:") {
		t.Errorf("one blank line between the description and the usage:\n%q", page)
	}

	// And in the list on the parent's page, where it would leave a stray blank in
	// the middle rather than at the end.
	parent := LongHelp(spec, []string{"ex"}, []*Command{root}, help)
	if strings.Contains(parent, "$ ex run\n\n\n") {
		t.Errorf("a listed command's description should not double it either:\n%q", parent)
	}
}
