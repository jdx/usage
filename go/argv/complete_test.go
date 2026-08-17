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
	got := complete(nil, "")
	for _, want := range []string{"run", "list", "r", "--verbose", "-v", "--color", "--no-color"} {
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
	got := complete([]string{"run"}, "")
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

// A description is one line: a shell shows one line beside a candidate, and a
// wrapped description turns a completion menu into a wall.
func TestDescriptionsAreOneLine(t *testing.T) {
	root, help, meta := completionFixture()
	help[1].Short = "run it\nand keep running it"
	for _, c := range Candidates(Walk(root, nil), "run", help, meta) {
		if strings.Contains(c.Describe, "\n") {
			t.Errorf("description should be one line: %q", c.Describe)
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

	got := values(Candidates(Walk(root, []string{"run"}), "", help, meta))
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

func count(list []string, want string) int {
	n := 0
	for _, s := range list {
		if s == want {
			n++
		}
	}
	return n
}
