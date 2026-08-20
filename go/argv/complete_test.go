package argv

import (
	"strings"
	"testing"
)

// A CLI with the shapes completion has to get right: a global, a subcommand that
// shadows it, a flag with choices, and an argument with choices.
func completionFixture() (*Command, HelpTable, Metadata) {
	// Keys dense from 1, because both cold tables are indexed by them — a sparse
	// fixture looks up nothing and every assertion about hiding or choices passes
	// for the wrong reason. Which is exactly what the first draft of this did.
	verbose := &Flag{Key: 4, Name: "verbose", Longs: []string{"verbose"}, Shorts: []byte{'v'}, Global: true}
	color := &Flag{Key: 5, Name: "color", Longs: []string{"color"}, Negate: "no-color"}
	shell := &Flag{Key: 6, Name: "shell", Longs: []string{"shell"}, TakesValue: true}
	hidden := &Flag{Key: 7, Name: "secret", Longs: []string{"secret"}}
	mode := &Arg{Key: 8, Name: "MODE"}

	run := &Command{Name: "run", Key: 2, Flags: []*Flag{shell}, Args: []*Arg{mode}}
	list := &Command{Name: "list", Key: 3}
	buried := &Command{Name: "buried", Key: 9}
	root := &Command{Name: "ex", Key: 1,
		Flags:       []*Flag{verbose, color, hidden},
		Subcommands: []*Command{run, list, buried},
	}

	help := HelpTable{
		{Key: 1}, {Key: 2, Short: "run it", VisibleAliases: []string{"r"}},
		{Key: 3, Short: "list them"}, {Key: 4, Short: "be loud"}, {Key: 5},
		{Key: 6}, {Key: 7, Hide: true}, {Key: 8}, {Key: 9, Hide: true},
	}
	meta := Metadata{
		{Key: 1}, {Key: 2}, {Key: 3}, {Key: 4}, {Key: 5},
		{Key: 6, Name: "shell", Flag: true, Choices: []string{"bash", "zsh"}},
		{Key: 7}, {Key: 8, Name: "MODE", Choices: []string{"fast", "slow"}}, {Key: 9},
	}
	return root, help, meta
}

func values(cs []Candidate) []string {
	out := make([]string, len(cs))
	for i, c := range cs {
		out[i] = c.Value
	}
	return out
}

func offered(list []string, want string) bool {
	for _, s := range list {
		if s == want {
			return true
		}
	}
	return false
}

func complete(words []string, partial string) []string {
	root, help, meta := completionFixture()
	return values(Candidates(Walk(root, words), partial, help, meta))
}

func TestCompletionOffersCommandsFlagsAndAliases(t *testing.T) {
	// A bare cursor is asking which command to run. The reference offers no flags
	// there — checked against `usage complete-word`, which answers `install` and
	// `run` for a spec whose root also has `-v --verbose`.
	got := complete(nil, "")
	for _, want := range []string{"run", "list", "r"} {
		if !offered(got, want) {
			t.Errorf("want %q offered, got %v", want, got)
		}
	}
	for _, unwanted := range []string{"--verbose", "-v"} {
		if offered(got, unwanted) {
			t.Errorf("a bare cursor is not asking for flags: %v", got)
		}
	}

	// A dash is: a lone one offers both forms.
	got = complete(nil, "-")
	for _, want := range []string{"--verbose", "-v", "--color", "--no-color"} {
		if !offered(got, want) {
			t.Errorf("want %q offered, got %v", want, got)
		}
	}
	// A hidden flag binds and is not advertised.
	if offered(got, "--secret") {
		t.Errorf("a hidden flag should not be offered: %v", got)
	}
}

func TestCompletionFiltersByThePartialWord(t *testing.T) {
	got := complete(nil, "--co")
	if !offered(got, "--color") {
		t.Errorf("want --color, got %v", got)
	}
	for _, unwanted := range []string{"run", "--verbose"} {
		if offered(got, unwanted) {
			t.Errorf("%q does not start with the partial word: %v", unwanted, got)
		}
	}
}

// A global is offered inside a subcommand, because the parser accepts it there.
func TestAGlobalIsOfferedInsideASubcommand(t *testing.T) {
	got := complete([]string{"run"}, "-")
	if !offered(got, "--verbose") {
		t.Errorf("an inherited global should be offered: %v", got)
	}
	// And the subcommand's own.
	if !offered(got, "--shell") {
		t.Errorf("the command's own flags should be offered: %v", got)
	}
	// A flag declared only on the root and not global is not in scope here.
	if offered(got, "--color") {
		t.Errorf("a non-global root flag is not accepted here, so should not be offered: %v", got)
	}
}

// A flag waiting for its value takes the position entirely.
func TestAWaitingValueOffersItsChoicesAndNothingElse(t *testing.T) {
	got := complete([]string{"run", "--shell"}, "")
	for _, want := range []string{"bash", "zsh"} {
		if !offered(got, want) {
			t.Errorf("want %q, got %v", want, got)
		}
	}
	for _, unwanted := range []string{"--verbose", "list"} {
		if offered(got, unwanted) {
			t.Errorf("nothing else belongs where a value is expected: %v", got)
		}
	}
}

func TestAPositionalOffersItsChoices(t *testing.T) {
	if got := complete([]string{"run"}, ""); !offered(got, "fast") {
		t.Errorf("the pending argument's choices should be offered: %v", got)
	}
}

// Past a `--` there is no flag of this CLI to offer.
func TestPastASeparatorNoFlagsAreOffered(t *testing.T) {
	got := complete([]string{"run", "--"}, "-")
	for _, unwanted := range []string{"--verbose", "--shell"} {
		if offered(got, unwanted) {
			t.Errorf("flag interpretation has stopped: %v", got)
		}
	}
}

// `help` asks which command to read about; nothing else belongs there.
func TestAHelpTopicOffersOnlyCommands(t *testing.T) {
	got := complete([]string{"help"}, "")
	if !offered(got, "run") || !offered(got, "list") {
		t.Errorf("want the commands, got %v", got)
	}
	for _, unwanted := range []string{"--verbose", "--color"} {
		if offered(got, unwanted) {
			t.Errorf("a topic takes no flags: %v", got)
		}
	}
}

// A candidate carries the whole description; putting it on one line is the
// renderer's job.
//
// Collapsing there rather than truncating here keeps both halves of a two-line
// description — see TestADescriptionIsCollapsedNotTruncated.
func TestACandidateCarriesTheWholeDescription(t *testing.T) {
	root, help, meta := completionFixture()
	help[1].Short = "run it\nand keep running it"
	for _, c := range Candidates(Walk(root, nil), "run", help, meta) {
		if c.Value != "run" {
			continue
		}
		if !strings.Contains(c.Describe, "and keep running it") {
			t.Errorf("the second half should survive: %q", c.Describe)
		}
	}
}

// A hidden command binds and is not advertised — the rule `hide` exists for, and
// the one the help pages already follow.
func TestAHiddenCommandIsNotOffered(t *testing.T) {
	if got := complete(nil, ""); offered(got, "buried") {
		t.Errorf("a hidden command should not be offered: %v", got)
	}
	// Including after `help`, where it would otherwise be most discoverable.
	if got := complete([]string{"help"}, ""); offered(got, "buried") {
		t.Errorf("a hidden command is not a topic either: %v", got)
	}
}

// `findNamed` resolves a topic by name or alias, so a topic completion that hides
// aliases makes accepted spellings undiscoverable where they are most useful.
func TestAHelpTopicOffersAliasesToo(t *testing.T) {
	got := complete([]string{"help"}, "")
	if !offered(got, "r") {
		t.Errorf("`run`'s alias should be a topic too: %v", got)
	}
}

// An argument that reads only after a `--` must not be advertised before one:
// those words come back as `arg_requires_double_dash`, which is the exact failure
// this design exists to prevent.
func TestAnArgumentNeedingASeparatorWaitsForIt(t *testing.T) {
	after := &Arg{Key: 2, Name: "REST", DoubleDash: DoubleDashRequired}
	root := &Command{Name: "ex", Key: 1, Args: []*Arg{after}}
	help := HelpTable{{Key: 1}, {Key: 2}}
	meta := Metadata{{Key: 1}, {Key: 2, Name: "REST", Choices: []string{"one", "two"}}}

	if got := values(Candidates(Walk(root, nil), "", help, meta)); offered(got, "one") {
		t.Errorf("nothing should be offered before the separator: %v", got)
	}
	if got := values(Candidates(Walk(root, []string{"--"}), "", help, meta)); !offered(got, "one") {
		t.Errorf("after the separator it is the argument's turn: %v", got)
	}
}

// A nearer flag reclaiming one spelling leaves the inherited flag's others
// binding, so they stay offered. Dropping the whole flag hid spellings the parser
// still accepts.
func TestOnlyTheClaimedSpellingIsWithdrawn(t *testing.T) {
	global := &Flag{Key: 3, Name: "jobs", Longs: []string{"jobs", "workers"},
		Shorts: []byte{'j'}, Global: true}
	local := &Flag{Key: 4, Name: "jobs", Longs: []string{"jobs"}}
	sub := &Command{Name: "run", Key: 2, Flags: []*Flag{local}}
	root := &Command{Name: "ex", Key: 1, Flags: []*Flag{global}, Subcommands: []*Command{sub}}
	help := HelpTable{{Key: 1}, {Key: 2}, {Key: 3}, {Key: 4}}
	meta := Metadata{{Key: 1}, {Key: 2}, {Key: 3}, {Key: 4}}

	got := values(Candidates(Walk(root, []string{"run"}), "-", help, meta))
	// `--jobs` is the subcommand's now, and still offered once.
	if n := count(got, "--jobs"); n != 1 {
		t.Errorf("--jobs should appear once, got %d: %v", n, got)
	}
	// The spellings the nearer flag did not take still bind, so they stay.
	for _, want := range []string{"--workers", "-j"} {
		if !offered(got, want) {
			t.Errorf("%s still binds, so it should be offered: %v", want, got)
		}
	}
}

func TestHiddenFlagAliasesBindWithoutBeingCompleted(t *testing.T) {
	flag := &Flag{
		Key: 2, Name: "output",
		Longs: []string{"output", "quietly"}, HiddenLongs: []string{"quietly"},
		Shorts: []byte{'o', 'q'}, HiddenShorts: []byte{'q'},
	}
	root := &Command{Name: "ex", Key: 1, Flags: []*Flag{flag}}
	help := HelpTable{{Key: 1}, {Key: 2}}
	meta := Metadata{{Key: 1}, {Key: 2}}

	for _, spelling := range []string{"--quietly", "-q"} {
		p := New(root, []string{spelling})
		if !p.Next() || p.Event().Flag != flag || p.Err() != nil {
			t.Fatalf("hidden alias %s should bind: event=%+v err=%v", spelling, p.Event(), p.Err())
		}
	}

	got := values(Candidates(Walk(root, nil), "-", help, meta))
	for _, spelling := range []string{"--output", "-o"} {
		if !offered(got, spelling) {
			t.Errorf("visible spelling %s should be offered: %v", spelling, got)
		}
	}
	for _, spelling := range []string{"--quietly", "-q"} {
		if offered(got, spelling) {
			t.Errorf("hidden alias %s should not be offered: %v", spelling, got)
		}
	}
}

// Whichever flag the parser binds a spelling to is the flag that offers it.
//
// `binds` asks the parser, rather than asserting what the scope rules ought to
// do: the whole claim of this file is that what is offered is what would be
// accepted, and the only authority on the second half is the parser.
func binds(t *testing.T, root *Command, words []string) *Flag {
	t.Helper()
	p := New(root, words)
	var last *Flag
	for p.Next() {
		if ev := p.Event(); ev.Kind == KindFlag {
			last = ev.Flag
		}
	}
	if err := p.Err(); err != nil {
		t.Fatalf("%v should bind: %v", words, err)
	}
	return last
}

// A negation loses to a long form anywhere in scope, so it is not offered twice.
//
// An ancestor's `--no-color` is a literal long, and the parser asks for every
// long across the whole scope before it asks for any negation — so typing it
// binds the ancestor's flag, not the nearer flag's negation. Offering it under
// both put the same word in the list twice, described two different ways, one of
// them wrong.
func TestANegationLosesToALongOfTheSameSpelling(t *testing.T) {
	global := &Flag{Key: 3, Name: "no-color", Longs: []string{"no-color"}, Global: true}
	local := &Flag{Key: 4, Name: "color", Longs: []string{"color"}, Negate: "no-color"}
	sub := &Command{Name: "run", Key: 2, Flags: []*Flag{local}}
	root := &Command{Name: "ex", Key: 1, Flags: []*Flag{global}, Subcommands: []*Command{sub}}
	help := HelpTable{{Key: 1}, {Key: 2}, {Key: 3}, {Key: 4}}
	meta := Metadata{{Key: 1}, {Key: 2}, {Key: 3}, {Key: 4}}

	if f := binds(t, root, []string{"run", "--no-color"}); f != global {
		t.Fatalf("the parser binds --no-color to %v, so the premise is wrong", f)
	}
	got := values(Candidates(Walk(root, []string{"run"}), "-", help, meta))
	if n := count(got, "--no-color"); n != 1 {
		t.Errorf("--no-color should be offered once, by the flag that binds it, got %d: %v",
			n, got)
	}
}

// And a negation is offered where it is all that is left of an inherited flag.
//
// The nearer command reclaims `--color`, so nothing of the global's own spellings
// survives — but `--no-color` still binds to the global, and dropping the flag
// for having no primary form left hid it.
func TestAnInheritedNegationSurvivesItsFlagsOtherSpellings(t *testing.T) {
	global := &Flag{Key: 3, Name: "color", Longs: []string{"color"},
		Negate: "no-color", Global: true}
	local := &Flag{Key: 4, Name: "color", Longs: []string{"color"}}
	sub := &Command{Name: "run", Key: 2, Flags: []*Flag{local}}
	root := &Command{Name: "ex", Key: 1, Flags: []*Flag{global}, Subcommands: []*Command{sub}}
	help := HelpTable{{Key: 1}, {Key: 2}, {Key: 3}, {Key: 4}}
	meta := Metadata{{Key: 1}, {Key: 2}, {Key: 3}, {Key: 4}}

	if f := binds(t, root, []string{"run", "--no-color"}); f != global {
		t.Fatalf("the parser binds --no-color to %v, so the premise is wrong", f)
	}
	got := values(Candidates(Walk(root, []string{"run"}), "-", help, meta))
	if !offered(got, "--no-color") {
		t.Errorf("--no-color still binds, so it should be offered: %v", got)
	}
	if n := count(got, "--color"); n != 1 {
		t.Errorf("--color is the subcommand's now, and offered once, got %d: %v", n, got)
	}
}

// A subcommand is offered only while the parser would still descend.
//
// Descent stops once a positional of this command has taken a word — after that a
// word matching a subcommand name is a value, or a failure. Offering one there is
// the same mistake as offering a flag past a `--`.
func TestASubcommandIsNotOfferedOnceAPositionalIsFilled(t *testing.T) {
	sub := &Command{Name: "run", Key: 2}
	root := &Command{Name: "ex", Key: 1,
		Args:        []*Arg{{Key: 3, Name: "file"}},
		Subcommands: []*Command{sub}}
	help := HelpTable{{Key: 1}, {Key: 2}, {Key: 3}}
	meta := Metadata{{Key: 1}, {Key: 2}, {Key: 3}}

	// The premise, from the parser: with the positional filled, `run` is not a
	// command any more.
	p := New(root, []string{"a.txt", "run"})
	for p.Next() {
		if ev := p.Event(); ev.Kind == KindCommand {
			t.Fatalf("the parser still descends into %q, so the premise is wrong", ev.Command.Name)
		}
	}

	if got := values(Candidates(Walk(root, nil), "", help, meta)); !offered(got, "run") {
		t.Errorf("nothing has been typed yet, so run should be offered: %v", got)
	}
	if got := values(Candidates(Walk(root, []string{"a.txt"}), "", help, meta)); offered(got, "run") {
		t.Errorf("the positional is filled, so run would not bind: %v", got)
	}
}

// A negation spelled the same as its own long form is offered once.
//
// `flag "--no-color" negate="--no-color"` is odd and it parses, and usage-lib
// prints it as `--no-color / --no-color` — so the page says it twice by design,
// and the check for that lives in the page tests. A completion is a list of
// things to type, where the same thing twice is a repeated row.
func TestANegationSpelledLikeItsOwnLongIsOfferedOnce(t *testing.T) {
	flag := &Flag{Key: 2, Name: "no-color", Longs: []string{"no-color"}, Negate: "no-color"}
	root := &Command{Name: "ex", Key: 1, Flags: []*Flag{flag}}
	help := HelpTable{{Key: 1}, {Key: 2}}
	meta := Metadata{{Key: 1}, {Key: 2}}

	got := values(Candidates(Walk(root, nil), "-", help, meta))
	if n := count(got, "--no-color"); n != 1 {
		t.Errorf("--no-color should be offered once, got %d: %v", n, got)
	}
}

// A variadic still collecting does not hide the flags that would end it.
//
// `--tools a ⌶` is not the same position as `--tools ⌶`: the parser refuses a
// flag-like token where a value is owed, but a variadic that already has one
// stops collecting when it meets a flag, and that flag binds. Treating the two
// the same offered only the variadic's values, so the flags were invisible in a
// place they still work.
func TestAVariadicStillCollectingOffersFlagsToo(t *testing.T) {
	tools := &Flag{Key: 2, Name: "tools", Longs: []string{"tools"},
		TakesValue: true, Variadic: true}
	force := &Flag{Key: 3, Name: "force", Longs: []string{"force"}}
	sub := &Command{Name: "run", Key: 4}
	root := &Command{Name: "ex", Key: 1, Flags: []*Flag{tools, force},
		Subcommands: []*Command{sub}}
	help := HelpTable{{Key: 1}, {Key: 2}, {Key: 3}, {Key: 4}}
	meta := Metadata{{Key: 1}, {Key: 2, Choices: []string{"node", "python"}}, {Key: 3}, {Key: 4}}

	// The premise: the flag binds after a value has been collected.
	if f := binds(t, root, []string{"--tools", "a", "--force"}); f != force {
		t.Fatalf("--force should bind after a collected value, got %v", f)
	}

	got := values(Candidates(Walk(root, []string{"--tools", "a"}), "", help, meta))
	if !offered(got, "node") {
		t.Errorf("the variadic's own values belong here too: %v", got)
	}
	// And once a dash is typed, the flags that would end the collection — the
	// same rule as anywhere else, which is the point: this position is not the
	// exclusive one a flag owed its value is.
	dashed := values(Candidates(Walk(root, []string{"--tools", "a"}), "-", help, meta))
	if !offered(dashed, "--force") {
		t.Errorf("a flag ends the collection and binds, so it belongs here: %v", dashed)
	}
	// A plain word goes to the variadic, so nothing a plain word cannot be.
	if offered(got, "run") {
		t.Errorf("a subcommand name would be collected as a value, not bound: %v", got)
	}

	// And a flag still owed its first value keeps the position to itself, dash or
	// no dash: the parser refuses a flag-like token there.
	owed := values(Candidates(Walk(root, []string{"--tools"}), "-", help, meta))
	if offered(owed, "--force") {
		t.Errorf("a flag-like token is refused where a value is owed: %v", owed)
	}
}

// A nearer flag claiming a spelling takes it from an inherited negation as well.
//
// The exemption that lets a flag spelled `--x` still offer a negation spelled
// `--x` is about its *own* forms; it must not reach past a child that has taken
// the word. The parser binds `--x` to the child, and the global was offering it
// again.
func TestANearerFlagTakesTheSpellingFromAnInheritedNegation(t *testing.T) {
	global := &Flag{Key: 3, Name: "x", Longs: []string{"x"}, Negate: "x", Global: true}
	local := &Flag{Key: 4, Name: "x", Longs: []string{"x"}}
	sub := &Command{Name: "run", Key: 2, Flags: []*Flag{local}}
	root := &Command{Name: "ex", Key: 1, Flags: []*Flag{global}, Subcommands: []*Command{sub}}
	help := HelpTable{{Key: 1}, {Key: 2}, {Key: 3}, {Key: 4}}
	meta := Metadata{{Key: 1}, {Key: 2}, {Key: 3}, {Key: 4}}

	if f := binds(t, root, []string{"run", "--x"}); f != local {
		t.Fatalf("the parser binds --x to the nearer flag, got %v", f)
	}
	got := values(Candidates(Walk(root, []string{"run"}), "-", help, meta))
	if n := count(got, "--x"); n != 1 {
		t.Errorf("--x should be offered once, by the flag that binds it, got %d: %v", n, got)
	}
}

func count(list []string, want string) int {
	n := 0
	for _, s := range list {
		if s == want {
			n++
		}
	}
	return n
}
