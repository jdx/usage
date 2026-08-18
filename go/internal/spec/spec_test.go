package spec

import (
	"encoding/json"
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
			Subcommands: Subcommands{{Name: "install", Cmd: Cmd{Name: "install",
				Flags: []Flag{{Name: "force", Long: []string{"force"}}},
				Args:  []Arg{{Name: "pkg"}}}}},
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
			Subcommands: Subcommands{{Name: "run", Cmd: Cmd{Name: "run", Flags: []Flag{
				{Name: "loud", Long: []string{"loud"}, Conflicts: []string{"--quiet"}},
				{Name: "solo", Long: []string{"solo"}, Conflicts: []string{"--local"}},
			}}}},
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
			Subcommands: Subcommands{{Name: "run", Cmd: Cmd{Name: "run", Flags: []Flag{
				{Name: "quiet", Long: []string{"quiet"}},
				{Name: "loud", Long: []string{"loud"}, Conflicts: []string{"--quiet"}},
			}}}},
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

// A negation is compared as the spec wrote it.
//
// `negate="-no-color"` is a form nobody can type as `--no-color`, and usage-lib
// does not resolve a relationship naming the latter to the flag declaring the
// former. Resolving it here would enforce a rule the reference does not.
func TestANegationIsMatchedAsWritten(t *testing.T) {
	root, meta := build(&Spec{
		Name: "ex", Bin: "ex",
		Cmd: Cmd{Name: "ex", Flags: []Flag{
			{Name: "color", Long: []string{"color"}, Negate: "--no-color"},
			{Name: "tint", Long: []string{"tint"}, Negate: "-no-tint"},
			{Name: "a", Long: []string{"a"}, Conflicts: []string{"--no-color"}},
			{Name: "b", Long: []string{"b"}, Conflicts: []string{"--no-tint"}},
		}},
	})
	var color uint64
	for _, f := range root.Flags {
		if f.Name == "color" {
			color = f.Key
		}
	}
	if got := metaFor(t, meta, root, "a").Conflicts; len(got) != 1 || got[0] != color {
		t.Errorf("--no-color should reach the color flag, got %v", got)
	}
	if got := metaFor(t, meta, root, "b").Conflicts; len(got) != 0 {
		t.Errorf("--no-tint is not the form `-no-tint`, so nothing: got %v", got)
	}
}

// The table has to agree with the binder it feeds.
//
// The parser tries every long form before any negation, so with `--a` declaring
// `negate="--zap"` and a separate `--zap`, typing `--zap` binds *zap*. Resolving
// per candidate handed the relationship to `a`, and the rule would then have been
// enforced against a flag the command line never binds.
func TestAnOrdinaryFormBeatsAnotherFlagsNegation(t *testing.T) {
	root, meta := build(&Spec{
		Name: "ex", Bin: "ex",
		Cmd: Cmd{Name: "ex", Flags: []Flag{
			{Name: "a", Long: []string{"a"}, Negate: "--zap"},
			{Name: "zap", Long: []string{"zap"}},
			{Name: "p", Long: []string{"p"}, Conflicts: []string{"--zap"}},
		}},
	})
	var zap uint64
	for _, f := range root.Flags {
		if f.Name == "zap" {
			zap = f.Key
		}
	}
	if got := metaFor(t, meta, root, "p").Conflicts; len(got) != 1 || got[0] != zap {
		t.Errorf("should name the flag --zap binds (%d), got %v", zap, got)
	}
}

// A negation is named by the form it was written as, whatever the dashes.
func TestASingleDashNegationIsNamedByItsOwnForm(t *testing.T) {
	root, meta := build(&Spec{
		Name: "ex", Bin: "ex",
		Cmd: Cmd{Name: "ex", Flags: []Flag{
			{Name: "tint", Long: []string{"tint"}, Negate: "-no-tint"},
			{Name: "plain", Long: []string{"plain"}, Conflicts: []string{"-no-tint"}},
			{Name: "other", Long: []string{"other"}, Conflicts: []string{"--no-tint"}},
		}},
	})
	var tint uint64
	for _, f := range root.Flags {
		if f.Name == "tint" {
			tint = f.Key
		}
	}
	if got := metaFor(t, meta, root, "plain").Conflicts; len(got) != 1 || got[0] != tint {
		t.Errorf("the exact form should resolve to tint (%d), got %v", tint, got)
	}
	if got := metaFor(t, meta, root, "other").Conflicts; len(got) != 0 {
		t.Errorf("--no-tint is not how it was declared, got %v", got)
	}
}

// A spec may declare the same alias twice, once hidden, and hiding wins.
//
// usage-lib reports such an alias in both `aliases` and `hidden_aliases`, so a
// visible list taken as it arrives advertises something the spec asked to keep
// out of the page. The emitter filters, and these two tables have to be the same
// table — see TestTheTwoProducersAgree, which cannot see this one because mise
// declares no alias twice.
func TestAnAliasDeclaredBothWaysStaysHidden(t *testing.T) {
	s := &Spec{Name: "ex", Bin: "ex",
		Cmd: Cmd{Name: "ex", Subcommands: Subcommands{{Name: "install", Cmd: Cmd{
			Name:          "install",
			Aliases:       []string{"i", "add"},
			HiddenAliases: []string{"i"},
		}}}},
	}
	root, _, help := s.BuildAll()

	install := root.Subcommands[0]
	// Still bound by both: hiding is about the page, and the parser never reads
	// which list a name came from.
	if !reflect.DeepEqual(install.Aliases, []string{"i", "add", "i"}) {
		t.Errorf("both aliases should bind, got %v", install.Aliases)
	}
	if got := help.Lookup(install.Key).VisibleAliases; !reflect.DeepEqual(got, []string{"add"}) {
		t.Errorf("only the alias that was not also hidden should show, got %v", got)
	}
}

// Subcommands keep the order the spec declared them in.
//
// The lowering arrives as a JSON object, and a Go map has no order — so this is
// decoded a key at a time. Order is not cosmetic here: the keys that index the
// cold tables are handed out in this order, so a lowering that sorted by name
// would number every entry differently from the generated tables built out of
// the same spec.
func TestSubcommandsKeepTheOrderTheyWereDeclaredIn(t *testing.T) {
	var s Spec
	const lowered = `{"name":"ex","bin":"ex","cmd":{"name":"ex","subcommands":{
		"run":{"name":"run"},"add":{"name":"add"},"build":{"name":"build"}}}}`
	if err := json.Unmarshal([]byte(lowered), &s); err != nil {
		t.Fatalf("lowered spec should decode: %v", err)
	}

	var got []string
	for _, sub := range s.Cmd.Subcommands {
		got = append(got, sub.Name)
	}
	if want := []string{"run", "add", "build"}; !reflect.DeepEqual(got, want) {
		t.Errorf("declared %v, decoded %v", want, got)
	}

	root, _ := build(&s)
	var names []string
	for _, sub := range root.Subcommands {
		names = append(names, sub.Name)
	}
	if want := []string{"run", "add", "build"}; !reflect.DeepEqual(names, want) {
		t.Errorf("the table should hold the same order, got %v", names)
	}
}

// Decoding into a spec that already holds one replaces its commands.
//
// A decoder does not get to assume a fresh value: appending to the receiver would
// leave the first spec's commands in the second spec's table, which is a parse
// table describing a CLI that does not exist.
func TestDecodingASecondSpecReplacesTheFirstsSubcommands(t *testing.T) {
	var s Spec
	first := `{"cmd":{"name":"ex","subcommands":{"run":{"name":"run"}}}}`
	second := `{"cmd":{"name":"ex","subcommands":{"add":{"name":"add"}}}}`
	if err := json.Unmarshal([]byte(first), &s); err != nil {
		t.Fatalf("the first spec should decode: %v", err)
	}
	if err := json.Unmarshal([]byte(second), &s); err != nil {
		t.Fatalf("the second spec should decode: %v", err)
	}
	var got []string
	for _, sub := range s.Cmd.Subcommands {
		got = append(got, sub.Name)
	}
	if want := []string{"add"}; !reflect.DeepEqual(got, want) {
		t.Errorf("the second spec's commands are all there is, got %v", got)
	}

	// And a command that says it has none says so, rather than keeping the last
	// answer.
	if err := json.Unmarshal([]byte(`{"cmd":{"subcommands":null}}`), &s); err != nil {
		t.Fatalf("null should decode: %v", err)
	}
	if len(s.Cmd.Subcommands) != 0 {
		t.Errorf("null is no subcommands, got %v", s.Cmd.Subcommands)
	}
}

// A truncated object is an error rather than a short list: the decoder reads the
// closing brace itself, so nothing else is there to notice.
func TestATruncatedSubcommandObjectIsAnError(t *testing.T) {
	var s Spec
	if err := json.Unmarshal([]byte(`{"cmd":{"subcommands":{"run":{}`), &s); err == nil {
		t.Error("a truncated lowering should not decode as a spec")
	}
}

// The tables carry how a flag is typed, because the rules that judge an entry
// never see one — and a one-character long form and a short form are both one
// character, so guessing from the name renders `--a` as `-a`.
func TestTheTableCarriesHowAFlagIsTyped(t *testing.T) {
	root, meta := build(&Spec{
		Name: "ex", Bin: "ex",
		Cmd: Cmd{Name: "ex", Flags: []Flag{
			{Name: "a", Long: []string{"a"}},
			{Name: "b", Short: []string{"b"}},
			{Name: "file", Long: []string{"file"}, Short: []string{"f"}},
		}},
	})
	for _, c := range []struct{ name, want string }{
		{"a", "--a"},       // one character, and a long form
		{"b", "-b"},        // short only
		{"file", "--file"}, // the long form wins where there is one
	} {
		if got := metaFor(t, meta, root, c.name).Spelling; got != c.want {
			t.Errorf("%s: want %q, got %q", c.name, c.want, got)
		}
	}
}

func TestAllowHyphenValuesCarriesFromNestedArg(t *testing.T) {
	root, _ := build(&Spec{
		Name: "ex", Bin: "ex",
		Cmd: Cmd{Name: "ex", Flags: []Flag{
			{Name: "args", Long: []string{"args"}, Arg: &Arg{Name: "ARGS", DoubleDash: "Automatic"}},
			{Name: "jobs", Long: []string{"jobs"}, Arg: &Arg{Name: "N"}},
		}},
	})
	byName := map[string]*argv.Flag{}
	for _, f := range root.Flags {
		byName[f.Name] = f
	}
	if !byName["args"].AllowHyphenValues {
		t.Error("allow_hyphen_values should come from the nested arg's double_dash=automatic")
	}
	if byName["jobs"].AllowHyphenValues {
		t.Error("a flag without double_dash=automatic should not take hyphen values")
	}
}

func TestRequireEqualsCarries(t *testing.T) {
	root, _ := build(&Spec{
		Name: "ex", Bin: "ex",
		Cmd: Cmd{Name: "ex", Flags: []Flag{
			{Name: "inspect", Long: []string{"inspect"}, RequireEquals: true, Arg: &Arg{Name: "PORT"}},
		}},
	})
	if !root.Flags[0].RequireEquals {
		t.Error("require_equals should carry")
	}
}

func TestDefaultMissingCarries(t *testing.T) {
	root, _ := build(&Spec{
		Name: "ex", Bin: "ex",
		Cmd: Cmd{Name: "ex", Flags: []Flag{
			{Name: "color", Long: []string{"color"}, DefaultMissing: "always", Arg: &Arg{Name: "WHEN"}},
		}},
	})
	if root.Flags[0].DefaultMissing != "always" {
		t.Errorf("default_missing: got %q", root.Flags[0].DefaultMissing)
	}
}
