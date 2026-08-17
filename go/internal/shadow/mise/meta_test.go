package mise

import (
	"strings"
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

	// A value-less flag that was given has no values, and nil would read as "the
	// command line said nothing" — so it is recorded as an empty slice, which is
	// the distinction Fill draws. Getting this wrong makes a typed boolean fall
	// through to env and default.
	given := map[uint64][]string{}
	occurrences := map[uint64]int{}
	path := []*argv.Command{Root}

	p := argv.New(Root, args)
	for p.Next() {
		ev := p.Event()
		switch ev.Kind {
		case argv.KindCommand:
			path = append(path, ev.Command)
		case argv.KindFlag:
			occurrences[ev.Flag.Key]++
			if ev.HasValue {
				given[ev.Flag.Key] = append(given[ev.Flag.Key], ev.Value)
			} else if given[ev.Flag.Key] == nil {
				given[ev.Flag.Key] = []string{}
			}
		case argv.KindArg:
			occurrences[ev.Arg.Key]++
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
			if e := argv.Check(Meta.Lookup(f.Key), v, occurrences[f.Key]); e != nil && err == nil {
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
		for _, condition := range m.RequiresIf {
			if Meta.Lookup(condition.Key) == nil {
				t.Errorf("%q conditionally points at key %d, which is not an entry", m.Name, condition.Key)
			}
		}
	}
}

// A value-less flag typed on the command line must not fall through to the
// fallbacks. mise's `--quiet` is one, and the distinction is invisible unless
// something asks: `Fill` reads a nil `given` as "the command line said nothing",
// so a helper that only records values reports a typed boolean as unset.
func TestATypedBooleanCountsAsGiven(t *testing.T) {
	_, source, err := resolve(t, "quiet", []string{"--quiet"}, nil)
	if err != nil {
		t.Fatalf("unexpected error: %v", err)
	}
	if source != argv.FromArgv {
		t.Errorf("a typed boolean should come from argv, got %v", source)
	}

	_, source, err = resolve(t, "quiet", nil, nil)
	if err != nil {
		t.Fatalf("unexpected error: %v", err)
	}
	if source != argv.Unset {
		t.Errorf("untyped, with nothing declared to fill it, should be unset: %v", source)
	}
}

// The generated tables render the same pages as the runtime-built ones.
//
// `go/conformance` proves the renderer against usage-lib using tables built from
// a lowered spec at run time. That leaves the emitter's half unchecked, and the
// help table is where a dropped field is least visible: a missing alias or
// annotation changes one line of one page. So the whole tree is rendered from
// what the generator actually wrote.
func TestGeneratedTablesRenderEveryPage(t *testing.T) {
	var rendered int
	var walk func(chain []*argv.Command, path []string)
	walk = func(chain []*argv.Command, path []string) {
		page := argv.ShortHelp(HelpMeta, path, chain, HelpText)
		if page == "" || !strings.Contains(page, "Usage: "+strings.Join(path, " ")) {
			t.Errorf("%s: page does not lead with its own usage line:\n%s",
				strings.Join(path, " "), page)
		}
		rendered++
		cmd := chain[len(chain)-1]
		for _, sub := range cmd.Subcommands {
			walk(append(append([]*argv.Command{}, chain...), sub),
				append(append([]string{}, path...), sub.Name))
		}
	}
	walk([]*argv.Command{Root}, []string{"mise"})

	if rendered < 200 {
		t.Errorf("only %d pages rendered; mise's tree is larger", rendered)
	}
}

// One page in full, so a reader can see what the generated tables produce rather
// than only that something was produced.
func TestAGeneratedPageReadsAsAPage(t *testing.T) {
	var configLs, config *argv.Command
	for _, c := range Root.Subcommands {
		if c.Name == "config" {
			config = c
			for _, s := range c.Subcommands {
				if s.Name == "ls" {
					configLs = s
				}
			}
		}
	}
	if configLs == nil {
		t.Fatal("mise has a `config ls`")
	}
	page := argv.ShortHelp(HelpMeta,
		[]string{"mise", "config", "ls"},
		[]*argv.Command{Root, config, configLs}, HelpText)

	for _, want := range []string{
		"List config files currently in use",
		"Usage: mise config ls [FLAGS]",
		"Flags:",
		"  -J, --json",
		"  -h, --help",
		// The globals come from the root, under a heading that says so.
		"Global flags:",
		"  -C, --cd <DIR>",
	} {
		if !strings.Contains(page, want) {
			t.Errorf("page is missing %q:\n%s", want, page)
		}
	}
}

// The generated front door, on mise's real command lines.
//
// `Parse` is what an author actually calls, and it is generated from the same
// tables the rest of the suite checks — so this is the join between them: the
// structs exist, the right one is filled, and the rules still run.
func TestGeneratedParseFillsTheStructs(t *testing.T) {
	cli, err := Parse([]string{"use", "-g", "node@20"})
	if err != nil {
		t.Fatalf("unexpected error: %v", err)
	}
	if cli.Use == nil {
		t.Fatal("`use` should be selected")
	}
	if !cli.Use.Global {
		t.Error("-g should set Global")
	}
	if got := cli.Use.ToolVersion; len(got) != 1 || got[0] != "node@20" {
		t.Errorf("want [node@20], got %q", got)
	}
	// A command nobody ran is nil, which is how a caller tells which was chosen.
	if cli.Config != nil {
		t.Error("`config` was not on the command line")
	}
}

// The shape that made the Rust derive's validation wrong, through the front door.
func TestGeneratedParseSplitsArgsAcrossASeparator(t *testing.T) {
	cli, err := Parse([]string{"tasks", "run", "build", "extra", "--", "--verbose"})
	if err != nil {
		t.Fatalf("unexpected error: %v", err)
	}
	run := cli.Tasks.Run
	if run == nil {
		t.Fatal("`tasks run` should be selected")
	}
	if run.Task != "build" {
		t.Errorf("want build, got %q", run.Task)
	}
	if len(run.Args) != 1 || run.Args[0] != "extra" {
		t.Errorf("want [extra] before the separator, got %q", run.Args)
	}
	if len(run.ArgsLast) != 1 || run.ArgsLast[0] != "--verbose" {
		t.Errorf("want [--verbose] after it, got %q", run.ArgsLast)
	}
}

// A default reaches the field. mise's `bootstrap packages import --manager`
// defaults to `brew`, and a front door that enforces a default and hands back the
// zero value is worse than one with no defaults at all.
func TestGeneratedParseAppliesADefault(t *testing.T) {
	cli, err := Parse([]string{"bootstrap", "packages", "import"})
	if err != nil {
		t.Fatalf("unexpected error: %v", err)
	}
	if got := cli.Bootstrap.Packages.Import.Manager; got != "brew" {
		t.Errorf("want the declared default, got %q", got)
	}
}

// And the rules still run: a value outside the choices comes back rather than
// reaching the struct.
func TestGeneratedParseEnforcesChoices(t *testing.T) {
	_, err := Parse([]string{"--log-level", "chatty"})
	if err == nil {
		t.Fatal("a value outside the choices should be refused")
	}
	if e, ok := err.(*argv.Error); !ok || e.Code != argv.CodeInvalidChoice {
		t.Errorf("want invalid_choice, got %v", err)
	}
}
