package conformance

import (
	"encoding/json"
	"os"
	"path/filepath"
	"strings"
	"testing"

	"github.com/jdx/usage/go/argv"
	"github.com/jdx/usage/go/internal/spec"
)

// Does the rendered usage line match usage-lib's, at mise's scale?
//
// usage-lib builds this from a spec, through a template over a runtime model. The
// Go side has no spec at run time — only tables — so the rules are reimplemented,
// and reimplemented rules drift. The check is to run both over mise's real spec
// and compare all 211 lines, because an adopter's help text changing is a visible
// regression even when the change is one bracket.
//
// This is the same test `benches/gate/tests/help.rs` makes for usage-argv, using
// the same oracle: usage-lib's own `usage` string, which the lowering carries per
// command. Two implementations checked against one reference beats two
// implementations checked against each other.

func TestEveryUsageLineMatchesTheReference(t *testing.T) {
	usageBin := findUsage(t)
	kdl := filepath.Join("..", "..", "benches", "mise.usage.kdl")
	if _, err := os.Stat(kdl); err != nil {
		t.Fatalf("mise's spec should be in the repository: %v", err)
	}

	lowered := lowerFile(t, usageBin, kdl)
	root, _, help := lowered.BuildAll()

	// The reference line per command path, from the lowering.
	want := map[string]string{}
	var collect func(c *spec.Cmd, path []string)
	collect = func(c *spec.Cmd, path []string) {
		want[strings.Join(path, " ")] = c.Usage
		for _, sub := range c.Subcommands {
			sub := sub
			collect(&sub.Cmd, append(append([]string{}, path...), sub.Name))
		}
	}
	collect(&lowered.Cmd, nil)

	var checked int
	var differences []string
	var walk func(cmd *argv.Command, path []string)
	walk = func(cmd *argv.Command, path []string) {
		key := strings.Join(path[1:], " ")
		reference, ok := want[key]
		if !ok {
			differences = append(differences, key+": not in the spec at all")
			return
		}
		// usage-lib's `usage` omits the binary and starts at the command path, so
		// the comparison puts it back — the same string the template writes after
		// `Usage: `.
		theirs := strings.TrimSpace("mise " + reference)
		ours := argv.UsageLine(path, cmd, help)
		if ours != theirs {
			differences = append(differences,
				key+"\n     ours: "+ours+"\n      lib: "+theirs)
		}
		checked++
		for _, sub := range cmd.Subcommands {
			walk(sub, append(append([]string{}, path...), sub.Name))
		}
	}
	walk(root, []string{"mise"})

	if checked < 200 {
		t.Errorf("only %d commands checked; mise's tree is larger than that", checked)
	}
	if len(differences) > 0 {
		t.Fatalf("%d of %d usage lines differ from usage-lib:\n  - %s",
			len(differences), checked, strings.Join(differences, "\n  - "))
	}
	t.Logf("%d usage lines match usage-lib exactly", checked)
}

// One case spelled out, so a reader can see what the parity test asserts 211
// times.
func TestTheRootLineIsWhatAUserWouldRecognise(t *testing.T) {
	usageBin := findUsage(t)
	lowered := lowerFile(t, usageBin, filepath.Join("..", "..", "benches", "mise.usage.kdl"))
	root, _, help := lowered.BuildAll()

	line := argv.UsageLine([]string{"mise"}, root, help)
	if !strings.HasPrefix(line, "mise ") {
		t.Errorf("the line should start with the binary: %s", line)
	}
	if !strings.HasSuffix(line, "<SUBCOMMAND>") {
		t.Errorf("mise has subcommands, so the line should say so: %s", line)
	}
}

// Hidden entries are absent from the line as they are from the sections: help
// describes what a user is invited to type.
func TestHiddenEntriesAreNotInTheLine(t *testing.T) {
	usageBin := findUsage(t)
	s := lower(t, usageBin, `name "ex"
bin "ex"
flag "--shown"
flag "--secret" hide=#true
arg "[visible]"
arg "[buried]" hide=#true
`)
	root, _, help := s.BuildAll()
	got := argv.UsageLine([]string{"ex"}, root, help)
	if strings.Contains(got, "secret") || strings.Contains(got, "buried") {
		t.Errorf("hidden entries leaked into the line: %s", got)
	}
	if !strings.Contains(got, "--shown") || !strings.Contains(got, "visible") {
		t.Errorf("visible entries should be there: %s", got)
	}
}

// lowerFile is [lower] for a spec that lives on disk.
func lowerFile(t *testing.T, usageBin, path string) *spec.Spec {
	t.Helper()
	out, err := runUsage(usageBin, "generate", "json", "-f", path)
	if err != nil {
		t.Fatalf("lowering %s failed: %v", path, err)
	}
	var s spec.Spec
	if err := json.Unmarshal(out, &s); err != nil {
		t.Fatalf("the lowered spec would not decode: %v", err)
	}
	return &s
}

// A flag's value follows the same required-and-undefaulted test as a positional,
// independently of the flag itself.
//
// The four combinations, all checked against usage-lib's own line rather than
// against what this happens to produce. mise has none of the bracketed cases —
// every flag value it declares is required and undefaulted — so the parity test
// over its 211 commands passes either way, which is exactly why these are here.
//
// Worth recording that usage-argv angles the value unconditionally and so differs
// from usage-lib on the last three. This follows usage-lib, since that is the
// reference the help output is measured against.
func TestAFlagValueIsBracketedByItsOwnRequiredness(t *testing.T) {
	usageBin := findUsage(t)
	for _, c := range []struct{ decl, want string }{
		{`flag "--tool <TOOL>"`, "ex [--tool <TOOL>]"},
		{`flag "--v <n>" required=#true`, "ex <--v <n>>"},
		{`flag "--opt [n]"`, "ex [--opt [n]]"},
		{"flag \"--jobs <n>\" required=#true {\n  arg \"<n>\" default=\"4\"\n}", "ex <--jobs [n]>"},
	} {
		s := lower(t, usageBin, "name \"ex\"\nbin \"ex\"\n"+c.decl+"\n")
		root, _, help := s.BuildAll()

		got := argv.UsageLine([]string{"ex"}, root, help)
		if got != c.want {
			t.Errorf("%s\n  want %s\n  got  %s", c.decl, c.want, got)
		}
		// And the reference agrees, which is what makes `want` above more than an
		// assertion about my own code.
		if reference := "ex " + s.Cmd.Usage; reference != c.want {
			t.Errorf("%s: the oracle says %q, not %q", c.decl, reference, c.want)
		}
	}
}
