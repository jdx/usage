// Package benches is the module that holds the mise-scale shadows, and this is the one
// assertion that keeps them worth measuring: every framework has to arrive at the same
// place on the same command line.
//
// A shadow that stopped resolving `mise use -g node@20` would still be timed by
// `cmd/sweep` — the sweep refuses to report a parser that does not reach a subcommand,
// but only when somebody runs it. This runs in CI, where a generator change that broke a
// shadow shows up as a failure rather than as a benchmark nobody took that week.
package benches

import (
	"testing"

	"github.com/jdx/usage/benches/go/mise"
	misecobra "github.com/jdx/usage/benches/go/mise-cobra"
	misekong "github.com/jdx/usage/benches/go/mise-kong"
	miseurfave "github.com/jdx/usage/benches/go/mise-urfave"
	"github.com/jdx/usage/go/argv"
)

var words = []string{"use", "-g", "node@20"}

func TestEveryShadowResolvesTheBenchmarkArgv(t *testing.T) {
	for _, tc := range []struct {
		name    string
		resolve func([]string) bool
	}{
		{"cobra", misecobra.Resolve},
		{"urfave", miseurfave.Resolve},
		{"kong", misekong.Resolve},
	} {
		if !tc.resolve(words) {
			t.Errorf("%s did not reach a subcommand on `mise %v`", tc.name, words)
		}
	}
}

// The usage side, asserted the same way and in more detail, because it is the only one
// here that hands back what it bound rather than a yes.
func TestUsageShadowBindsTheBenchmarkArgv(t *testing.T) {
	cli, err := mise.Parse(words)
	if err != nil {
		t.Fatalf("mise.Parse: %v", err)
	}
	if cli.Use == nil {
		t.Fatal("mise.Parse did not reach `use`")
	}
	if !cli.Use.Global {
		t.Error("`-g` did not reach Use.Global")
	}
	if got := cli.Use.ToolVersion; len(got) != 1 || got[0] != "node@20" {
		t.Errorf("Use.ToolVersion = %v, want [node@20]", got)
	}
}

// What the sweep's second usage-go row measures, checked here so the row cannot quietly
// become a parse that binds nothing.
func TestUsageShadowBindsAsEvents(t *testing.T) {
	p := argv.New(mise.Root, words)
	commands, flags, args := 0, 0, 0
	for p.Next() {
		switch p.Event().Kind {
		case argv.KindCommand:
			commands++
		case argv.KindFlag:
			flags++
		case argv.KindArg:
			args++
		}
	}
	if err := p.Err(); err != nil {
		t.Fatalf("argv.New: %v", err)
	}
	if commands != 1 || flags != 1 || args != 1 {
		t.Errorf("bound %d commands, %d flags, %d args; want 1, 1, 1", commands, flags, args)
	}
}
