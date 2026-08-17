package conformance

import (
	"encoding/json"
	"os/exec"
	"path/filepath"
	"strings"
	"testing"

	"github.com/jdx/usage/go/argv"
	"github.com/jdx/usage/go/internal/spec"
)

// Does the page `-h` prints match usage-lib's, at mise's scale?
//
// The same standard as the usage line, over the whole document: all 211 of mise's
// pages, byte for byte. This is the test that decides whether an adopter's help
// output changes, so it compares the text rather than a summary of it.
//
// The reference comes from `xtask help-pages`, which renders usage-lib's own
// pages — unwrapped, in one pass. See that command for why the CLI's output is
// not used directly.

type pages struct {
	Short string `json:"short"`
	Long  string `json:"long"`
}

func referencePages(t *testing.T) map[string]pages {
	t.Helper()
	kdl := filepath.Join("..", "..", "benches", "mise.usage.kdl")
	out, err := exec.Command("cargo", "run", "-q", "-p", "xtask", "--",
		"help-pages", kdl).Output()
	if err != nil {
		if ee, ok := err.(*exec.ExitError); ok {
			t.Fatalf("rendering the reference pages: %v\n%s", err, ee.Stderr)
		}
		t.Fatalf("rendering the reference pages: %v", err)
	}
	var got map[string]pages
	if err := json.Unmarshal(out, &got); err != nil {
		t.Fatalf("the reference pages would not decode: %v", err)
	}
	return got
}

func TestEveryShortPageMatchesTheReference(t *testing.T) {
	usageBin := findUsage(t)
	lowered := lowerFile(t, usageBin, filepath.Join("..", "..", "benches", "mise.usage.kdl"))
	root, _, help := lowered.BuildAll()
	reference := referencePages(t)
	spec := lowered.HelpSpec()

	var checked int
	var differences []string
	var walk func(chain []*argv.Command, path []string)
	walk = func(chain []*argv.Command, path []string) {
		key := strings.Join(path[1:], " ")
		want, ok := reference[key]
		if !ok {
			differences = append(differences, key+": no reference page")
			return
		}
		got := argv.ShortHelp(spec, path, chain, help)
		if got != want.Short {
			differences = append(differences, key+"\n"+firstDiff(got, want.Short))
		}
		checked++
		cmd := chain[len(chain)-1]
		for _, sub := range cmd.Subcommands {
			walk(append(append([]*argv.Command{}, chain...), sub),
				append(append([]string{}, path...), sub.Name))
		}
	}
	walk([]*argv.Command{root}, []string{"mise"})

	if checked < 200 {
		t.Errorf("only %d pages checked; mise's tree is larger", checked)
	}
	if len(differences) > 0 {
		// Two is enough to work from; the whole set would bury the count.
		shown := differences
		if len(shown) > 2 {
			shown = shown[:2]
		}
		t.Fatalf("%d of %d pages differ from usage-lib:\n%s",
			len(differences), checked, strings.Join(shown, "\n"))
	}
	t.Logf("%d short pages match usage-lib exactly", checked)
}

// firstDiff shows the first line that differs, with a little context — a whole
// help page twice over is not something anyone reads.
func firstDiff(ours, theirs string) string {
	mine, ref := strings.Split(ours, "\n"), strings.Split(theirs, "\n")
	for i := 0; i < len(mine) && i < len(ref); i++ {
		if mine[i] != ref[i] {
			return "  line " + itoa(i+1) + ":\n    ours: " + quote(mine[i]) +
				"\n     lib: " + quote(ref[i])
		}
	}
	return "  same for " + itoa(min(len(mine), len(ref))) + " lines, then ours has " +
		itoa(len(mine)) + " and the reference " + itoa(len(ref))
}

func quote(s string) string { b, _ := json.Marshal(s); return string(b) }
func itoa(n int) string     { b, _ := json.Marshal(n); return string(b) }
func min(a, b int) int {
	if a < b {
		return a
	}
	return b
}

var _ = spec.Spec{}

func TestEveryLongPageMatchesTheReference(t *testing.T) {
	usageBin := findUsage(t)
	lowered := lowerFile(t, usageBin, filepath.Join("..", "..", "benches", "mise.usage.kdl"))
	root, _, help := lowered.BuildAll()
	reference := referencePages(t)
	spec := lowered.HelpSpec()

	var checked int
	var differences []string
	var walk func(chain []*argv.Command, path []string)
	walk = func(chain []*argv.Command, path []string) {
		key := strings.Join(path[1:], " ")
		want, ok := reference[key]
		if !ok {
			// A page the reference does not have is a difference, not a page to
			// skip: a comparison that quietly drops what it cannot compare passes
			// loudest when the oracle is empty.
			differences = append(differences, key+": no reference page")
			return
		}
		if got := argv.LongHelp(spec, path, chain, help); got != want.Long {
			differences = append(differences, key+"\n"+firstDiff(got, want.Long))
		}
		checked++
		for _, sub := range chain[len(chain)-1].Subcommands {
			walk(append(append([]*argv.Command{}, chain...), sub),
				append(append([]string{}, path...), sub.Name))
		}
	}
	walk([]*argv.Command{root}, []string{"mise"})

	// The floor the short page's test has, for the same reason: "every page
	// matched" means nothing without a count of what every page was.
	if checked < 200 {
		t.Errorf("only %d pages checked; mise's tree is larger", checked)
	}
	if len(differences) > 0 {
		shown := differences
		if len(shown) > 2 {
			shown = shown[:2]
		}
		t.Fatalf("%d of %d long pages differ from usage-lib:\n%s",
			len(differences), checked, strings.Join(shown, "\n"))
	}
	t.Logf("%d long pages match usage-lib exactly", checked)
}
