package spec

import (
	"reflect"
	"testing"

	"github.com/jdx/usage/go/argv"
)

// Built from a Spec literal rather than from lowered JSON, so these run without
// the `usage` CLI. The corpus is where the lowering itself is exercised.
func build(s *Spec) (*argv.Command, argv.Metadata) { return s.Build() }

func metaFor(t *testing.T, meta argv.Metadata, root *argv.Command, name string) *argv.Meta {
	t.Helper()
	for _, f := range root.Flags {
		if f.Name == name {
			m := meta.Lookup(f.Key)
			if m == nil {
				t.Fatalf("no metadata for flag %q", name)
			}
			return m
		}
	}
	for _, a := range root.Args {
		if a.Name == name {
			m := meta.Lookup(a.Key)
			if m == nil {
				t.Fatalf("no metadata for arg %q", name)
			}
			return m
		}
	}
	t.Fatalf("no entry named %q", name)
	return nil
}

// A default can be written on the flag or on the value the flag takes, and
// usage-lib falls back to the second. A Go CLI generated from the same spec has
// to agree, or the two disagree about what a bare invocation means.
func TestAFlagsDefaultCanBeDeclaredOnItsValue(t *testing.T) {
	root, meta := build(&Spec{
		Name: "ex", Bin: "ex",
		Cmd: Cmd{Name: "ex", Flags: []Flag{
			{Name: "jobs", Long: []string{"jobs"}, Arg: &Arg{Name: "n", Default: []string{"4"}}},
			{Name: "level", Long: []string{"level"}, Default: []string{"info"},
				Arg: &Arg{Name: "l", Default: []string{"ignored"}}},
			{Name: "plain", Long: []string{"plain"}, Arg: &Arg{Name: "p"}},
		}},
	})

	if got := metaFor(t, meta, root, "jobs").Default; !reflect.DeepEqual(got, []string{"4"}) {
		t.Errorf("a default on the value should be read: got %q", got)
	}
	// The flag's own wins where both are written, which is the narrower
	// declaration.
	if got := metaFor(t, meta, root, "level").Default; !reflect.DeepEqual(got, []string{"info"}) {
		t.Errorf("the flag's own default should win: got %q", got)
	}
	if got := metaFor(t, meta, root, "plain").Default; got != nil {
		t.Errorf("nothing declared should stay nothing: got %q", got)
	}
}

// The other half of the same question, and the answer is the opposite one:
// usage-lib does not read a nested `env`, so neither does this. Verified against
// it rather than assumed — `arg "<m>" env="EX_MODE"` inside a flag leaves the
// flag unset when EX_MODE is set.
func TestANestedEnvIsNotRead(t *testing.T) {
	root, meta := build(&Spec{
		Name: "ex", Bin: "ex",
		Cmd: Cmd{Name: "ex", Flags: []Flag{
			{Name: "mode", Long: []string{"mode"}, Arg: &Arg{Name: "m", Env: "EX_MODE"}},
			{Name: "shell", Long: []string{"shell"}, Env: "EX_SHELL", Arg: &Arg{Name: "s"}},
		}},
	})

	if got := metaFor(t, meta, root, "mode").Env; got != "" {
		t.Errorf("a nested env should not be read, got %q", got)
	}
	if got := metaFor(t, meta, root, "shell").Env; got != "EX_SHELL" {
		t.Errorf("the flag's own env should be read, got %q", got)
	}
}

// choices are only ever written on the value, so they are always read through.
func TestChoicesAreReadThroughTheValue(t *testing.T) {
	root, meta := build(&Spec{
		Name: "ex", Bin: "ex",
		Cmd: Cmd{Name: "ex", Flags: []Flag{
			{Name: "shell", Long: []string{"shell"},
				Arg: &Arg{Name: "s", Choices: &Choices{Choices: []string{"bash", "zsh"}}}},
		}},
	})
	want := []string{"bash", "zsh"}
	if got := metaFor(t, meta, root, "shell").Choices; !reflect.DeepEqual(got, want) {
		t.Errorf("want %q, got %q", want, got)
	}
}

// The two tables are separate data tied together by key, so the tie is what is
// worth testing: every entry's metadata must describe that entry and no other.
func TestMetadataLinesUpWithTheParseTables(t *testing.T) {
	root, meta := build(&Spec{
		Name: "ex", Bin: "ex",
		Cmd: Cmd{Name: "ex",
			Flags: []Flag{{Name: "verbose", Long: []string{"verbose"}}},
			Args:  []Arg{{Name: "file", Required: true}},
			Subcommands: map[string]Cmd{
				"install": {Name: "install",
					Flags: []Flag{{Name: "force", Long: []string{"force"}}},
					Args:  []Arg{{Name: "pkg"}}},
			},
		},
	})

	var walk func(*argv.Command)
	walk = func(c *argv.Command) {
		for _, f := range c.Flags {
			m := meta.Lookup(f.Key)
			if m == nil || m.Name != f.Name || !m.Flag {
				t.Errorf("flag %q of %q has metadata %+v", f.Name, c.Name, m)
			}
		}
		for _, a := range c.Args {
			m := meta.Lookup(a.Key)
			if m == nil || m.Name != a.Name || m.Flag {
				t.Errorf("arg %q of %q has metadata %+v", a.Name, c.Name, m)
			}
		}
		for _, sub := range c.Subcommands {
			walk(sub)
		}
	}
	walk(root)
}

// A relationship may name a flag the declaring command does not own.
//
// A global is in scope for everything beneath it, so `conflicts="--quiet"` on a
// subcommand flag names the root's `--quiet`. Searching only the local flags left
// the key unresolved and the rule silently unenforced — while usage-lib enforced
// it, which is the kind of difference a generated CLI cannot afford.
func TestARelationshipCanNameAnInheritedGlobal(t *testing.T) {
	root, meta := build(&Spec{
		Name: "ex", Bin: "ex",
		Cmd: Cmd{Name: "ex",
			Flags: []Flag{
				{Name: "quiet", Long: []string{"quiet"}, Global: true},
				// Not global, so not in scope below, and naming it must resolve to
				// nothing rather than to something that will never be enforced.
				{Name: "local", Long: []string{"local"}},
			},
			Subcommands: map[string]Cmd{
				"run": {Name: "run", Flags: []Flag{
					{Name: "loud", Long: []string{"loud"}, Conflicts: []string{"--quiet"}},
					{Name: "solo", Long: []string{"solo"}, Conflicts: []string{"--local"}},
				}},
			},
		},
	})

	var quiet uint64
	for _, f := range root.Flags {
		if f.Name == "quiet" {
			quiet = f.Key
		}
	}
	run := root.Subcommands[0]

	for _, f := range run.Flags {
		m := meta.Lookup(f.Key)
		switch f.Name {
		case "loud":
			if len(m.Conflicts) != 1 || m.Conflicts[0] != quiet {
				t.Errorf("--loud should conflict with the root's --quiet, got %v", m.Conflicts)
			}
		case "solo":
			if len(m.Conflicts) != 0 {
				t.Errorf("--local is not global, so nothing should resolve: %v", m.Conflicts)
			}
		}
	}
}

// A subcommand redeclaring an inherited name shadows it, here as at parse time.
func TestALocalFlagShadowsTheGlobalOfTheSameName(t *testing.T) {
	root, meta := build(&Spec{
		Name: "ex", Bin: "ex",
		Cmd: Cmd{Name: "ex",
			Flags: []Flag{{Name: "quiet", Long: []string{"quiet"}, Global: true}},
			Subcommands: map[string]Cmd{
				"run": {Name: "run", Flags: []Flag{
					{Name: "quiet", Long: []string{"quiet"}},
					{Name: "loud", Long: []string{"loud"}, Conflicts: []string{"--quiet"}},
				}},
			},
		},
	})

	run := root.Subcommands[0]
	var localQuiet uint64
	for _, f := range run.Flags {
		if f.Name == "quiet" {
			localQuiet = f.Key
		}
	}
	for _, f := range run.Flags {
		if f.Name != "loud" {
			continue
		}
		m := meta.Lookup(f.Key)
		if len(m.Conflicts) != 1 || m.Conflicts[0] != localQuiet {
			t.Errorf("should name run's own --quiet (%d), got %v", localQuiet, m.Conflicts)
		}
	}
}

// usage-lib treats `conflicts="--no-color"` as naming the `color` flag, and
// reports the conflict whichever of the two spellings was typed — the
// relationship is between entries, not tokens.
func TestARelationshipCanNameANegation(t *testing.T) {
	root, meta := build(&Spec{
		Name: "ex", Bin: "ex",
		Cmd: Cmd{Name: "ex", Flags: []Flag{
			{Name: "color", Long: []string{"color"}, Negate: "--no-color"},
			{Name: "plain", Long: []string{"plain"}, Conflicts: []string{"--no-color"}},
		}},
	})

	var color uint64
	for _, f := range root.Flags {
		if f.Name == "color" {
			color = f.Key
		}
	}
	m := metaFor(t, meta, root, "plain")
	if len(m.Conflicts) != 1 || m.Conflicts[0] != color {
		t.Errorf("should resolve to the color flag (%d), got %v", color, m.Conflicts)
	}
}

// A relationship names a flag by a form that flag actually has.
//
// usage-lib resolves neither `--q` for a short `-q` nor `-color` for a long
// `--color`, so resolving them here would have a generated CLI enforcing a rule
// the reference does not. A declaration naming the wrong form is a typo, and the
// useful failure is the rule not existing.
func TestARelationshipNeedsTheRightForm(t *testing.T) {
	root, meta := build(&Spec{
		Name: "ex", Bin: "ex",
		Cmd: Cmd{Name: "ex", Flags: []Flag{
			{Name: "quiet", Long: []string{"quiet"}, Short: []string{"q"}},
			{Name: "color", Long: []string{"color"}},
			{Name: "a", Long: []string{"a"}, Conflicts: []string{"--q"}},
			{Name: "b", Long: []string{"b"}, Conflicts: []string{"-color"}},
			{Name: "c", Long: []string{"c"}, Conflicts: []string{"-q"}},
			{Name: "d", Long: []string{"d"}, Conflicts: []string{"--color"}},
		}},
	})

	var quiet, color uint64
	for _, f := range root.Flags {
		switch f.Name {
		case "quiet":
			quiet = f.Key
		case "color":
			color = f.Key
		}
	}

	for _, c := range []struct {
		flag string
		want []uint64
	}{
		{"a", nil},             // --q is not a long form of anything
		{"b", nil},             // -color is not a short
		{"c", []uint64{quiet}}, // -q is
		{"d", []uint64{color}}, // --color is
	} {
		got := metaFor(t, meta, root, c.flag).Conflicts
		if len(got) != len(c.want) || (len(got) == 1 && got[0] != c.want[0]) {
			t.Errorf("--%s: want %v, got %v", c.flag, c.want, got)
		}
	}
}
