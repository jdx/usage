package mise

import (
	"testing"

	"github.com/jdx/usage/go/argv"
)

// The generated cold table, driving the rules it exists for.
//
// The unit tests in `argv` prove the rules against tables written by hand; the
// corpus proves them against tables built from a spec at run time. Neither
// exercises the emitter's half, which is where a field can be dropped, misnamed,
// or filed under the wrong key without anything noticing. These are the join.

// resolve runs a parse and applies the rules to one named entry, returning what
// it ended up with.
func resolve(t *testing.T, name string, args []string,
	environ map[string]string) (values []string, source argv.Source, err *argv.Error) {
	t.Helper()

	given := map[uint64][]string{}
	seen := map[uint64]bool{}
	path := []*argv.Command{Root}

	p := argv.New(Root, args)
	for p.Next() {
		ev := p.Event()
		switch ev.Kind {
		case argv.KindCommand:
			path = append(path, ev.Command)
		case argv.KindFlag:
			seen[ev.Flag.Key] = true
			if ev.HasValue {
				given[ev.Flag.Key] = append(given[ev.Flag.Key], ev.Value)
			}
		case argv.KindArg:
			seen[ev.Arg.Key] = true
			given[ev.Arg.Key] = append(given[ev.Arg.Key], ev.Value)
		}
	}
	if e := p.Err(); e != nil {
		t.Fatalf("binding failed: %v", e)
	}

	lookup := func(k string) (string, bool) { v, ok := environ[k]; return v, ok }

	for _, cmd := range path {
		for _, f := range cmd.Flags {
			v, src := argv.Fill(Meta.Lookup(f.Key), given[f.Key], lookup)
			if e := argv.Check(Meta.Lookup(f.Key), v, len(given[f.Key])); e != nil && err == nil {
				err = e
			}
			if f.Name == name {
				values, source = v, src
			}
		}
		for _, a := range cmd.Args {
			v, src := argv.Fill(Meta.Lookup(a.Key), given[a.Key], lookup)
			if e := argv.Check(Meta.Lookup(a.Key), v, 0); e != nil && err == nil {
				err = e
			}
			if a.Name == name {
				values, source = v, src
			}
		}
	}
	return values, source, err
}

// mise declares `--manager` on `bootstrap packages import` with both a default
// and the single choice that default names, which makes it the one entry that
// exercises the whole cold table at once.
func TestAGeneratedDefaultFills(t *testing.T) {
	values, source, err := resolve(t, "manager",
		[]string{"bootstrap", "packages", "import"}, nil)
	if err != nil {
		t.Fatalf("unexpected error: %v", err)
	}
	if source != argv.FromDefault {
		t.Errorf("want the default, got %v", source)
	}
	if len(values) != 1 || values[0] != "brew" {
		t.Errorf("want [brew], got %q", values)
	}
}

// The choices came from a `choices` block on the flag's *value*, which is a level
// of nesting the emitter has to read through.
func TestGeneratedChoicesAreEnforced(t *testing.T) {
	if _, _, err := resolve(t, "log-level", []string{"--log-level", "debug"}, nil); err != nil {
		t.Fatalf("a declared choice should be accepted: %v", err)
	}
	_, _, err := resolve(t, "log-level", []string{"--log-level", "chatty"}, nil)
	if err == nil {
		t.Fatal("a value outside the choices should be refused")
	}
	if err.Code != argv.CodeInvalidChoice || err.Name != "log-level" {
		t.Errorf("want invalid_choice for log-level, got %q for %q", err.Code, err.Name)
	}
}

// The command line beats the default, which is the ordering the whole fallback
// rests on.
func TestTheCommandLineBeatsAGeneratedDefault(t *testing.T) {
	values, source, err := resolve(t, "manager",
		[]string{"bootstrap", "packages", "import", "--manager", "brew"}, nil)
	if err != nil {
		t.Fatalf("unexpected error: %v", err)
	}
	if source != argv.FromArgv || len(values) != 1 || values[0] != "brew" {
		t.Errorf("want brew from argv, got %q from %v", values, source)
	}
}

// The two tables are separate data joined only by key, and the emitter writes
// them in two passes. If those ever disagree, every rule reads the wrong
// declaration — so it is checked across all 989 entries rather than sampled.
func TestEveryEntryHasMetadataDescribingItself(t *testing.T) {
	var checked int
	var walk func(*argv.Command)
	walk = func(c *argv.Command) {
		// A command takes a key and has no cold half, so its slot is empty and
		// Lookup should report nothing rather than a neighbour's entry.
		if m := Meta.Lookup(c.Key); m != nil {
			t.Errorf("command %q has metadata %+v", c.Name, m)
		}
		for _, f := range c.Flags {
			m := Meta.Lookup(f.Key)
			if m == nil {
				t.Errorf("flag %q of %q has no metadata", f.Name, c.Name)
				continue
			}
			if m.Name != f.Name || !m.Flag {
				t.Errorf("flag %q of %q got metadata for %q (flag=%v)",
					f.Name, c.Name, m.Name, m.Flag)
			}
			checked++
		}
		for _, a := range c.Args {
			m := Meta.Lookup(a.Key)
			if m == nil {
				t.Errorf("arg %q of %q has no metadata", a.Name, c.Name)
				continue
			}
			if m.Name != a.Name || m.Flag {
				t.Errorf("arg %q of %q got metadata for %q (flag=%v)",
					a.Name, c.Name, m.Name, m.Flag)
			}
			checked++
		}
		for _, sub := range c.Subcommands {
			walk(sub)
		}
	}
	walk(Root)

	if checked < 800 {
		t.Errorf("only %d entries checked: the tables are probably truncated", checked)
	}
}

// Every key a relationship points at must exist, or the rule silently does
// nothing. The emitter drops names it cannot resolve, so this is where a spec
// that names a flag which does not exist would show up.
func TestRelationshipsPointAtRealEntries(t *testing.T) {
	for i := range Meta {
		m := &Meta[i]
		for _, group := range [][]uint64{
			m.Conflicts, m.Overrides, m.RequiredUnless, m.RequiredIf,
		} {
			for _, key := range group {
				if Meta.Lookup(key) == nil {
					t.Errorf("%q points at key %d, which is not an entry", m.Name, key)
				}
			}
		}
	}
}
