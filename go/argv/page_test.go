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
