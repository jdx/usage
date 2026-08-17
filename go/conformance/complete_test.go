package conformance

import (
	"os/exec"
	"path/filepath"
	"strings"
	"testing"

	"github.com/jdx/usage/go/argv"
)

// Does the cursor get the same answer here as it does from usage-lib?
//
// The same standard the pages are held to, and for the same reason: these rules
// were reimplemented, and reimplemented rules drift. `usage complete-word` is the
// reference a shell would have called before any of this existed, so it is the
// oracle — over mise's real spec, which is the largest one there is.
//
// This exists because a divergence got through. `Candidates` offered every flag
// at a bare cursor; the reference offers none until a `-` is typed, and nothing
// noticed until a reviewer read the two. A hand-written test asserts what its
// author believed; this asks.

// completeWord is the reference's answer for a partial word at a command path.
//
// Only the lines the CLI itself produced: where the reference has nothing to
// offer it falls back to listing the working directory, and those lines describe
// the machine rather than the spec. The Go side says `\x01files` there instead,
// which is compared separately below.
func completeWord(t *testing.T, usageBin, kdl string, words []string, cword int) []string {
	t.Helper()
	args := []string{"complete-word", "--shell", "bash", "-f", kdl,
		"--cword", itoa(cword), "--"}
	args = append(args, words...)
	out, err := exec.Command(usageBin, args...).Output()
	if err != nil {
		t.Fatalf("the reference should answer %v: %v", words, err)
	}
	var lines []string
	for _, line := range strings.Split(strings.TrimRight(string(out), "\n"), "\n") {
		if line != "" {
			lines = append(lines, line)
		}
	}
	return lines
}

func TestTheCursorGetsTheReferencesAnswer(t *testing.T) {
	usageBin := findUsage(t)
	kdl := filepath.Join("..", "..", "benches", "mise.usage.kdl")
	lowered := lowerFile(t, usageBin, kdl)
	root, meta, help := lowered.BuildAll()

	// Lines where the reference answers from the spec alone. Two kinds are left
	// out on purpose, because they are answers this side does not claim to give:
	// a position where the reference lists the working directory (compared as a
	// marker below), and one where it *runs* a spec's `complete` block — `mise ⌶`
	// and `mise settings ⌶` both do, and shelling out on a Tab is the piece this
	// package has deliberately not built.
	//
	// What is left covers the branches: subcommands, aliases, a nested command,
	// both flag forms, a long that narrows, and a word that matches nothing.
	for _, line := range []string{
		"mise plug",
		"mise config ",
		"mise config l",
		"mise -",
		"mise --",
		"mise --log-",
		"mise use -",
		"mise wat",
		"mise plugins ",
		"mise plugins install -",
	} {
		split := argv.Split(line, len(line), argv.Bash)
		want := completeWord(t, usageBin, kdl, split.Words, split.Cword)

		answer := argv.Request{Shell: argv.Bash, Line: line, Cursor: len(line)}.
			Answer(root, help, meta)
		var got []string
		for _, c := range answer.Candidates {
			got = append(got, c.Value)
		}

		if !sameSet(got, want) {
			t.Errorf("%q:\n  ours: %v\n   lib: %v", line, got, want)
		}
	}
}

// And where the reference falls back to the filesystem, the Go side says so
// rather than listing it.
//
// The two are the same answer said differently: usage-lib prints the directory
// because it is answering a shell that has already asked; the tables say "paths
// belong here" and let the script call the shell's own path completion, which
// knows about `~`, variables and remote paths.
func TestAPathFallbackIsAMarkerRatherThanAListing(t *testing.T) {
	usageBin := findUsage(t)
	kdl := filepath.Join("..", "..", "benches", "mise.usage.kdl")
	lowered := lowerFile(t, usageBin, kdl)
	root, meta, help := lowered.BuildAll()

	// `mise edit` takes a file, and neither side has anything else to offer there.
	const line = "mise edit "
	split := argv.Split(line, len(line), argv.Bash)
	want := completeWord(t, usageBin, kdl, split.Words, split.Cword)
	if len(want) == 0 {
		t.Skip("the reference listed nothing here, so there is no fallback to compare")
	}

	answer := argv.Request{Shell: argv.Bash, Line: line, Cursor: len(line)}.
		Answer(root, help, meta)
	if answer.Files != argv.AnyFile {
		t.Errorf("the reference fell back to paths at %q, and this did not: %v",
			line, answer.Files)
	}
	if len(answer.Candidates) != 0 {
		t.Errorf("nothing of the spec's own belongs there: %v", answer.Candidates)
	}
}

func sameSet(a, b []string) bool {
	if len(a) != len(b) {
		return false
	}
	seen := map[string]int{}
	for _, s := range a {
		seen[s]++
	}
	for _, s := range b {
		seen[s]--
		if seen[s] < 0 {
			return false
		}
	}
	return true
}
