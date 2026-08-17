// Package mise holds generated parse tables for mise's committed spec, and the
// tests that keep the generator honest.
//
// The tables are checked in rather than built by the test, for the same two
// reasons the Rust shadows are: a reviewer can read the diff when the emitter's
// vocabulary changes, and CI can assert that regenerating produces no diff, so a
// change to the generator that nobody meant cannot land unnoticed. `mise run
// gen-go` regenerates.
//
// mise is the fixture because it is the largest usage CLI there is — 211
// commands, 711 flags, 128 positionals, four levels deep — and because every
// shape that has been awkward to express came from it.
package mise

import (
	"strings"
	"testing"

	"github.com/jdx/usage/go/argv"
)

// bind runs a command line and renders it as one line, so a table of cases reads
// as a table.
func bind(args ...string) string {
	var out []string
	p := argv.New(Root, args)
	for p.Next() {
		ev := p.Event()
		switch ev.Kind {
		case argv.KindCommand:
			out = append(out, "cmd:"+ev.Command.Name)
		case argv.KindFlag:
			s := "flag:" + ev.Flag.Name
			if ev.HasValue {
				s += "=" + ev.Value
			}
			out = append(out, s)
		case argv.KindArg:
			out = append(out, "arg:"+ev.Arg.Name+"="+ev.Value)
		}
	}
	if err := p.Err(); err != nil {
		out = append(out, "err:"+err.(*argv.Error).Code.String())
	}
	return strings.Join(out, " ")
}

// TestRealCommandLines parses invocations out of mise's own documentation.
//
// Hand-written rather than generated: what the generator produces is only worth
// checking if it parses the words mise's users actually type.
func TestRealCommandLines(t *testing.T) {
	cases := []struct {
		name string
		argv []string
		want string
	}{
		{
			"a tool is installed globally",
			[]string{"use", "-g", "node@20"},
			"cmd:use flag:global arg:TOOL@VERSION=node@20",
		},
		{
			// The shape that made the Rust derive's validation wrong: `[ARGS]…`
			// before the separator and `[-- ARGS_LAST]…` after it.
			"a task runs with arguments after a separator",
			[]string{"tasks", "run", "build", "extra", "--dry-run", "--", "--verbose"},
			"cmd:tasks cmd:run arg:TASK=build arg:ARGS=extra flag:dry-run arg:ARGS_LAST=--verbose",
		},
		{
			// `x` is a hidden alias in the spec, and hiding is a help-output concern:
			// binding never reads it, so the alias selects the command as any other
			// would.
			"an alias selects the command it names",
			[]string{"x", "node@20", "--", "node", "-v"},
			"cmd:exec arg:TOOL@VERSION=node@20 arg:COMMAND=node arg:COMMAND=-v",
		},
		{
			"a nested command with a flag of its own",
			[]string{"config", "ls", "--no-header"},
			"cmd:config cmd:ls flag:no-header",
		},
		{
			// A global declared on the root, given after a subcommand word two levels
			// down. Scope runs downward, and the generator resolved that into the
			// table rather than leaving the parser to walk for it.
			"a root global reaches a nested command",
			[]string{"config", "ls", "--cd", "/tmp"},
			"cmd:config cmd:ls flag:cd=/tmp",
		},
		{
			// A usage spec is often parsing a command line whose flags belong to
			// something else, so an unrecognized one is data in transit rather than a
			// typo, and mise's spec does not ask for the strict reading.
			"an unknown flag becomes a word",
			[]string{"use", "--wat"},
			"cmd:use arg:TOOL@VERSION=--wat",
		},
		{
			// The same token with nothing to hold it. `run` declares no positional at
			// all in the committed spec — mise clears them and adds `mount run="mise
			// tasks --usage"`, so task names come from running that — and a mount is
			// not something binding resolves. So the word has nowhere to land.
			//
			// Pinned because it is the visible edge of a known hole rather than a
			// property worth having: when mounts are answered, this case changes, and
			// it should change loudly.
			"a word with no argument to hold it is an error",
			[]string{"run", "--wat"},
			"cmd:run err:unexpected_arg",
		},
	}

	for _, c := range cases {
		t.Run(c.name, func(t *testing.T) {
			if got := bind(c.argv...); got != c.want {
				t.Errorf("%v\n  want %s\n  got  %s", c.argv, c.want, got)
			}
		})
	}
}

// TestDefaultSubcommandIsTheRootsOwnRun pins a pointer, because a review of the
// generated diff is what caught it being the wrong one.
//
// mise declares `default_subcommand run`, which names a subcommand of the root.
// It also has an `oci run`, and an emitter that resolved the name against the
// whole tree found that one first in depth-first order — so `mise build` would
// have descended into a command that is not the root's child at all. Nothing in
// the parse of an ordinary command line shows the difference, which is why it is
// asserted directly rather than through a binding.
func TestDefaultSubcommandIsTheRootsOwnRun(t *testing.T) {
	if Root.DefaultSubcommand != cmdRun {
		name := "<unset>"
		if Root.DefaultSubcommand != nil {
			name = Root.DefaultSubcommand.Name
		}
		t.Fatalf("default subcommand should be the root's own `run`, got %q", name)
	}
	// The one the whole-tree search used to win with, so a regression is specific
	// rather than merely "not cmdRun".
	if Root.DefaultSubcommand == cmdOciRun {
		t.Error("resolved against the whole tree again: this is `oci run`")
	}
}

// TestKeysAreUniqueAndDense checks the property generated dispatch depends on.
//
// Code switches on a Key, so two entries sharing one would bind the wrong field —
// the failure the Rust derive hashes its way around because two macro expansions
// cannot see each other. A generator sees the whole spec and can simply count, so
// there is no excuse for a collision, and this is where that is checked at mise's
// scale rather than a fixture's.
func TestKeysAreUniqueAndDense(t *testing.T) {
	seen := map[uint64]string{}
	var walk func(*argv.Command)
	claim := func(key uint64, what string) {
		if prev, ok := seen[key]; ok {
			t.Errorf("key %d is used by both %s and %s", key, prev, what)
		}
		seen[key] = what
	}
	walk = func(c *argv.Command) {
		claim(c.Key, "command "+c.Name)
		for _, f := range c.Flags {
			claim(f.Key, "flag "+f.Name+" of "+c.Name)
		}
		for _, a := range c.Args {
			claim(a.Key, "arg "+a.Name+" of "+c.Name)
		}
		for _, sub := range c.Subcommands {
			walk(sub)
		}
	}
	walk(Root)

	// Dense from 1, which is what makes a key usable as an index if anything ever
	// wants one.
	for i := uint64(1); i <= uint64(len(seen)); i++ {
		if _, ok := seen[i]; !ok {
			t.Errorf("key %d was never handed out, so the keys are not dense", i)
		}
	}
	if len(seen) < 900 {
		t.Errorf("only %d entries: mise's spec is much larger than that, so the "+
			"tables are probably truncated", len(seen))
	}
}

// TestParseAllocatesNothingAtScale is the zero-allocation claim, measured against
// a real CLI rather than a fixture with four flags.
//
// The tables being large is exactly what could break it: a lookup that collected
// flags in scope, or a scope walk that built a slice, would be invisible on a
// toy spec and obvious here.
func TestParseAllocatesNothingAtScale(t *testing.T) {
	for _, args := range [][]string{
		{"use", "-g", "node@20"},
		{"tasks", "run", "build", "extra", "--dry-run", "--", "--verbose"},
		{"config", "ls", "--cd", "/tmp"},
	} {
		args := args
		t.Run(strings.Join(args, " "), func(t *testing.T) {
			n := testing.AllocsPerRun(100, func() {
				p := argv.New(Root, args)
				for p.Next() {
					_ = p.Event()
				}
				_ = p.Err()
			})
			if n != 0 {
				t.Errorf("want 0 allocations, got %v", n)
			}
		})
	}
}

// BenchmarkParse is the number the README quotes, at mise's scale.
func BenchmarkParse(b *testing.B) {
	args := []string{"use", "-g", "node@20"}
	b.ReportAllocs()
	b.ResetTimer()
	for i := 0; i < b.N; i++ {
		p := argv.New(Root, args)
		for p.Next() {
			_ = p.Event()
		}
	}
}
