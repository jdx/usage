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

func TestLongHelpEndsWithAuthorshipAndLicense(t *testing.T) {
	root := &Command{Name: "ex", Key: 1}
	spec := HelpSpec{Bin: "ex", Author: "A. Person", License: "MIT"}
	page := LongHelp(spec, []string{"ex"}, []*Command{root}, HelpTable{{Key: 1}})
	if !strings.HasSuffix(page, "Author: A. Person\nLicense: MIT\n") {
		t.Fatalf("missing long-page footer:\n%s", page)
	}
	if strings.Contains(ShortHelp(spec, []string{"ex"}, []*Command{root}, HelpTable{{Key: 1}}), "Author:") {
		t.Fatal("short help should not print the long-page footer")
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

func TestRepeatableFlagUsesOrdinarySpelling(t *testing.T) {
	flag := &Flag{Key: 2, Name: "allow", Longs: []string{"allow"}, Shorts: []byte{'A'}, TakesValue: true}
	root := &Command{Name: "ex", Key: 1, Flags: []*Flag{flag}}
	help := HelpTable{{Key: 1}, {Key: 2, Repeatable: true, ValueName: "NAME", ValueDemanded: true}}

	for _, page := range []string{
		ShortHelp(HelpSpec{Name: "ex", Bin: "ex"}, []string{"ex"}, []*Command{root}, help),
		LongHelp(HelpSpec{Name: "ex", Bin: "ex"}, []string{"ex"}, []*Command{root}, help),
	} {
		if !strings.Contains(page, "-A, --allow <NAME>") {
			t.Errorf("repeatable flag should use ordinary spelling:\n%s", page)
		}
		if strings.Contains(page, "--allow…") {
			t.Errorf("repeatability marker should be omitted:\n%s", page)
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

func TestCommandDeprecationAppearsInListingsAndFlattenedHelp(t *testing.T) {
	sub := &Command{Name: "old", Key: 2}
	root := &Command{Name: "ex", Key: 1, Subcommands: []*Command{sub}}
	help := HelpTable{
		{Key: 1},
		{Key: 2, Short: "old command", DeprecatedWarnAt: "6.1"},
	}
	for _, page := range []string{
		ShortHelp(HelpSpec{Bin: "ex"}, []string{"ex"}, []*Command{root}, help),
		LongHelp(HelpSpec{Bin: "ex"}, []string{"ex"}, []*Command{root}, help),
	} {
		if !strings.Contains(page, "[deprecated: warns at 6.1]") {
			t.Fatalf("command listing omitted deprecation:\n%s", page)
		}
	}

	help[0].NextLineHelp = true
	nextLinePage := ShortHelp(HelpSpec{Bin: "ex"}, []string{"ex"}, []*Command{root}, help)
	// In a command list the label trails the summary rather than taking a line of its own,
	// in either layout: it wraps with the text instead of pushing it out of the column.
	if !strings.Contains(nextLinePage, "old\n    old command [deprecated: warns at 6.1]") {
		t.Fatalf("next-line command listing misplaced the deprecation:\n%s", nextLinePage)
	}
	help[0].NextLineHelp = false

	help[0].FlattenHelp = true
	shortPage := ShortHelp(HelpSpec{Bin: "ex"}, []string{"ex"}, []*Command{root}, help)
	if !strings.Contains(shortPage, "old command\n[deprecated: warns at 6.1]") {
		t.Fatalf("flattened command glued deprecation to its description:\n%s", shortPage)
	}
	for _, page := range []string{
		shortPage,
		LongHelp(HelpSpec{Bin: "ex"}, []string{"ex"}, []*Command{root}, help),
	} {
		if !strings.Contains(page, "old command") || !strings.Contains(page, "[deprecated: warns at 6.1]") {
			t.Fatalf("flattened command omitted deprecation:\n%s", page)
		}
	}
}

func TestSubcommandHelpHeadings(t *testing.T) {
	run := &Command{Name: "run", Key: 2}
	clean := &Command{Name: "clean", Key: 3}
	status := &Command{Name: "status", Key: 4}
	root := &Command{Name: "ex", Key: 1, Subcommands: []*Command{run, clean, status}}
	help := HelpTable{
		{Key: 1},
		{Key: 2, Short: "run it", Heading: "Core commands"},
		{Key: 3, Short: "remove old state", Heading: "Maintenance"},
		{Key: 4, Short: "show status", Heading: "Commands"},
	}
	for _, page := range []string{
		ShortHelp(HelpSpec{Bin: "ex"}, []string{"ex"}, []*Command{root}, help),
		LongHelp(HelpSpec{Bin: "ex"}, []string{"ex"}, []*Command{root}, help),
	} {
		commands := strings.Index(page, "\nCommands:\n")
		core := strings.Index(page, "\nCore commands:\n")
		maintenance := strings.Index(page, "\nMaintenance:\n")
		if commands < 0 || core < commands || maintenance < commands {
			t.Fatalf("command headings are missing or out of order:\n%s", page)
		}
		if strings.Count(page, "\nCommands:\n") != 1 {
			t.Fatalf("default-equivalent heading was duplicated:\n%s", page)
		}
		defaultEnd := min(core, maintenance)
		if !strings.Contains(page[commands:defaultEnd], "status") || !strings.Contains(page[commands:defaultEnd], "help") ||
			!strings.Contains(page[core:], "run") || !strings.Contains(page[maintenance:], "clean") {
			t.Fatalf("commands are in the wrong sections:\n%s", page)
		}
	}
}

func TestExplicitDisplayOrder(t *testing.T) {
	second := &Flag{Key: 2, Name: "second", Longs: []string{"second"}}
	first := &Flag{Key: 3, Name: "first", Longs: []string{"first"}}
	secondCmd := &Command{Name: "second", Key: 4}
	firstCmd := &Command{Name: "first", Key: 5}
	root := &Command{
		Name:        "ex",
		Key:         1,
		Flags:       []*Flag{second, first},
		Subcommands: []*Command{secondCmd, firstCmd},
	}
	help := HelpTable{
		{Key: 1},
		{Key: 2, Short: "shown second", DisplayOrder: 20, DisplayOrderSet: true},
		{Key: 3, Short: "shown first", DisplayOrder: 10, DisplayOrderSet: true},
		{Key: 4, Short: "shown second", DisplayOrder: 20, DisplayOrderSet: true},
		{Key: 5, Short: "shown first", DisplayOrder: 10, DisplayOrderSet: true},
	}

	for _, page := range []string{
		ShortHelp(HelpSpec{Bin: "ex"}, []string{"ex"}, []*Command{root}, help),
		LongHelp(HelpSpec{Bin: "ex"}, []string{"ex"}, []*Command{root}, help),
	} {
		commands := strings.SplitN(page, "\nCommands:\n", 2)[1]
		if strings.Index(commands, "first") > strings.Index(commands, "second") {
			t.Fatalf("commands ignored display order:\n%s", page)
		}
		flags := strings.SplitN(page, "\nFlags:\n", 2)[1]
		if strings.Index(flags, "--first") > strings.Index(flags, "--second") {
			t.Fatalf("flags ignored display order:\n%s", page)
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
		{Key: 3, Short: "Enable verbose output", Deprecated: "use --log-level"},
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
		if count := strings.Count(page, "[deprecated: use --log-level]"); count != 1 {
			t.Fatalf("deprecation should appear once, got %d:\n%s", count, page)
		}
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

func TestFlattenHelp(t *testing.T) {
	arg := &Arg{Name: "task", Key: 3, Required: true}
	flag := &Flag{Name: "extraordinarily-long-flag", Key: 4, Longs: []string{"extraordinarily-long-flag"}}
	deep := &Flag{Name: "deep", Key: 6, Longs: []string{"deep"}}
	nested := &Command{Name: "nested", Key: 5, Flags: []*Flag{deep}}
	sub := &Command{Name: "run", Key: 2, Args: []*Arg{arg}, Flags: []*Flag{flag}, Subcommands: []*Command{nested}}
	root := &Command{Name: "ex", Key: 1, Subcommands: []*Command{sub}}
	help := HelpTable{
		{Key: 1, FlattenHelp: true},
		{Key: 2, Short: "Run it", FlattenHelp: true, NextLineHelp: true},
		{Key: 3, Short: "Task name", Demanded: true},
		{Key: 4, Short: "Only show changes", Deprecated: "use --mode"},
		{Key: 5, Short: "Nested operation"},
		{Key: 6, Short: "Deep option"},
	}

	for _, page := range []string{
		ShortHelp(HelpSpec{Bin: "ex"}, []string{"ex"}, []*Command{root}, help),
		LongHelp(HelpSpec{Bin: "ex"}, []string{"ex"}, []*Command{root}, help),
	} {
		for _, want := range []string{
			"Usage: ex\n       ex run",
			"\nrun:\nRun it",
			"<task>",
			"--extraordinarily-long-flag",
			"\nrun nested:\nNested operation",
			"--deep\n    Deep option",
		} {
			if !strings.Contains(page, want) {
				t.Fatalf("missing %q in:\n%s", want, page)
			}
		}
		if strings.Contains(page, "\nCommands:\n") {
			t.Fatalf("flattened help still has a Commands section:\n%s", page)
		}
		if !strings.Contains(page, "  <task>  Task name") {
			t.Fatalf("a long flag stretched the argument column:\n%s", page)
		}
	}
}

func TestALongFlagOnlyMovesItsOwnHelpBelow(t *testing.T) {
	short := &Flag{Name: "short", Key: 2, Longs: []string{"short"}}
	long := &Flag{Name: "this-flag-name-is-far-beyond-the-column-cap", Key: 3, Longs: []string{"this-flag-name-is-far-beyond-the-column-cap"}}
	root := &Command{Name: "ex", Key: 1, Flags: []*Flag{short, long}}
	help := HelpTable{
		{Key: 1},
		{Key: 2, Short: "ordinary help"},
		{Key: 3, Short: "alpha beta"},
	}

	for _, page := range []string{
		ShortHelp(HelpSpec{Bin: "ex"}, []string{"ex"}, []*Command{root}, help),
		LongHelp(HelpSpec{Bin: "ex"}, []string{"ex"}, []*Command{root}, help),
	} {
		if !strings.Contains(page, "      --short                     ordinary help") {
			t.Fatalf("the ordinary row did not retain a readable description column:\n%s", page)
		}
		lines := strings.Split(page, "\n")
		var stacked bool
		for i, line := range lines[:len(lines)-1] {
			if strings.HasPrefix(strings.TrimSpace(line), "--this-flag-name-is-far-beyond-the-column-cap") {
				stacked = strings.TrimSpace(lines[i+1]) == "alpha beta"
				break
			}
		}
		if !stacked {
			t.Fatalf("the overflowing row did not move its help below:\n%s", page)
		}
	}
}

func TestFlattenedNextLineHelpKeepsDeprecationSeparate(t *testing.T) {
	flag := &Flag{Name: "old", Key: 3, Longs: []string{"old"}}
	sub := &Command{Name: "run", Key: 2, Flags: []*Flag{flag}}
	root := &Command{Name: "ex", Key: 1, Subcommands: []*Command{sub}}
	help := HelpTable{
		{Key: 1, FlattenHelp: true, NextLineHelp: true},
		{Key: 2, Short: "Run it"},
		{Key: 3, Short: "Old mode", Deprecated: "use --new"},
	}
	page := ShortHelp(HelpSpec{Bin: "ex"}, []string{"ex"}, []*Command{root}, help)
	if !strings.Contains(page, "--old\n    Old mode\n    [deprecated: use --new]") {
		t.Fatalf("flattened next-line help glued deprecation to its description:\n%s", page)
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
		{Key: 2, Short: "run it\n", Long: "run it\n\nExamples:\n\n    $ ex run\n"},
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

	shortParent := ShortHelp(spec, []string{"ex"}, []*Command{root}, help)
	if strings.Contains(shortParent, "run it\n\n  help") {
		t.Errorf("short command help should not add a blank line either:\n%q", shortParent)
	}
}

func TestAllHelpWalksVisibleDescendants(t *testing.T) {
	leaf := &Command{Key: 3, Name: "leaf"}
	hidden := &Command{Key: 4, Name: "hidden"}
	parent := &Command{Key: 2, Name: "parent", Subcommands: []*Command{hidden, leaf}}
	early := &Command{Key: 5, Name: "early"}
	zulu := &Command{Key: 6, Name: "zulu"}
	root := &Command{Key: 1, Name: "recursive", Subcommands: []*Command{zulu, parent, early}}
	help := make(HelpTable, 6)
	help[0] = Help{Key: 1}
	help[1] = Help{Key: 2, Short: "A visible parent"}
	help[2] = Help{Key: 3, Short: "A visible leaf"}
	help[3] = Help{Key: 4, Hide: true}
	help[4] = Help{Key: 5, DisplayOrder: 1, DisplayOrderSet: true}
	help[5] = Help{Key: 6}
	page := AllHelp(HelpSpec{Name: "recursive", Bin: "recursive"}, []string{"recursive"}, []*Command{root}, help)
	for _, usage := range []string{"Usage: recursive", "Usage: recursive early", "Usage: recursive parent", "Usage: recursive parent leaf", "Usage: recursive zulu"} {
		if !strings.Contains(page, usage) {
			t.Fatalf("missing %q:\n%s", usage, page)
		}
	}
	if strings.Contains(page, "Usage: recursive parent hidden") {
		t.Fatalf("hidden command was rendered:\n%s", page)
	}
	if strings.Index(page, "Usage: recursive early") > strings.Index(page, "Usage: recursive parent") {
		t.Fatalf("explicit display order followed unordered commands:\n%s", page)
	}
}

func TestHeadingProse(t *testing.T) {
	root := &Command{
		Name: "ex",
		Key:  1,
		Flags: []*Flag{
			{Name: "allow", Longs: []string{"allow"}, Shorts: []byte{'A'}, Key: 2},
			{Name: "quiet", Longs: []string{"quiet"}, Key: 3},
		},
	}
	help := HelpTable{
		{Key: 1, Headings: []Heading{
			{Title: "Filters", Help: "Filters accumulate from left to right.\nFor example: `-A no-debugger`."},
			{Title: "Flags", Help: "Should not appear: the default title is not a declared heading."},
		}},
		{Key: 2, Short: "allow a rule", Heading: "Filters"},
		{Key: 3, Short: "say less"},
	}

	long := LongHelp(HelpSpec{Bin: "ex"}, []string{"ex"}, []*Command{root}, help)
	want := "\nFilters:\n  Filters accumulate from left to right.\n  For example: `-A no-debugger`.\n\n"
	if !strings.Contains(long, want) {
		t.Fatalf("prose does not introduce its section:\n%s", long)
	}
	// Within the section, not the usage line above it: the prose opens the block and the
	// entries still follow.
	section := long[strings.Index(long, "\nFilters:\n"):]
	if i, j := strings.Index(section, "For example"), strings.Index(section, "--allow"); j < i {
		t.Fatalf("prose replaced the section instead of introducing it:\n%s", long)
	}
	// The default title names the entries that asked for no section, and it is not one
	// title — the same flags read as `Flags` here and as `Global Flags` elsewhere.
	if strings.Contains(long, "Should not appear") {
		t.Fatalf("an undeclared default title took prose:\n%s", long)
	}

	// A summary stays a summary, as it does for admonitions.
	short := ShortHelp(HelpSpec{Bin: "ex"}, []string{"ex"}, []*Command{root}, help)
	if strings.Contains(short, "accumulate from left to right") {
		t.Fatalf("prose reached the short page:\n%s", short)
	}
}

func TestSubcommandHeadingProse(t *testing.T) {
	compile := &Command{Name: "compile", Key: 2}
	root := &Command{Name: "ex", Key: 1, Subcommands: []*Command{compile}}
	help := HelpTable{
		{Key: 1, Headings: []Heading{{Title: "Build Commands", Help: "These write to the target directory."}}},
		{Key: 2, Short: "compile it", Heading: "Build Commands"},
	}

	long := LongHelp(HelpSpec{Bin: "ex"}, []string{"ex"}, []*Command{root}, help)
	if !strings.Contains(long, "\nBuild Commands:\n  These write to the target directory.\n\n") {
		t.Fatalf("a subcommand heading took no prose:\n%s", long)
	}
	short := ShortHelp(HelpSpec{Bin: "ex"}, []string{"ex"}, []*Command{root}, help)
	if strings.Contains(short, "These write to") {
		t.Fatalf("prose reached the short page:\n%s", short)
	}
}
