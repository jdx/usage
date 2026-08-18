package argv

import (
	"strings"
	"testing"
)

// The tables a generator would emit, written by hand. Package-level `var` with no
// computed values, which is the shape that lets the linker lay them out: `go tool
// nm` reports these as type D, and the package has no init function.
var (
	force   = &Flag{Key: 1, Name: "force", Longs: []string{"force"}, Shorts: []byte{'f'}}
	jobs    = &Flag{Key: 2, Name: "jobs", Longs: []string{"jobs"}, Shorts: []byte{'j'}, TakesValue: true}
	color   = &Flag{Key: 3, Name: "color", Longs: []string{"color"}, Negate: "no-color"}
	verbose = &Flag{Key: 4, Name: "verbose", Longs: []string{"verbose"}, Shorts: []byte{'v'}, Global: true}
	include = &Flag{Key: 5, Name: "include", Longs: []string{"include"}, TakesValue: true, Variadic: true}

	file = &Arg{Key: 10, Name: "file"}
	rest = &Arg{Key: 11, Name: "rest", Var: true}

	install = &Command{
		Name:    "install",
		Aliases: []string{"i"},
		Flags:   []*Flag{force},
		Key:     100,
	}

	root = &Command{
		Name:        "ex",
		Flags:       []*Flag{force, jobs, color, verbose, include},
		Args:        []*Arg{file, rest},
		Subcommands: []*Command{install},
	}

	// The same CLI, but one that owns all of its flags and wants typo detection.
	strictInstall = &Command{
		Name:         "install",
		Aliases:      []string{"i"},
		Flags:        []*Flag{force},
		UnknownFlags: UnknownFlagsError,
		Key:          100,
	}
	strict = &Command{
		Name:         "ex",
		Flags:        []*Flag{force, jobs, color, verbose},
		Args:         []*Arg{file, rest},
		Subcommands:  []*Command{strictInstall},
		UnknownFlags: UnknownFlagsError,
	}
)

// collect runs a parse and renders it as one line, which makes a table of cases
// readable as a table.
func collect(cmd *Command, args ...string) string {
	var out []string
	p := New(cmd, args)
	for p.Next() {
		ev := p.Event()
		switch ev.Kind {
		case KindCommand:
			out = append(out, "cmd:"+ev.Command.Name)
		case KindFlag:
			s := "flag:" + ev.Flag.Name
			if ev.HasValue {
				s += "=" + ev.Value
			}
			if ev.Negated {
				s += "!"
			}
			out = append(out, s)
		case KindArg:
			out = append(out, "arg:"+ev.Arg.Name+"="+ev.Value)
		case KindExternal:
			out = append(out, "external:"+strings.Join(ev.Values, ","))
		}
	}
	if err := p.Err(); err != nil {
		out = append(out, "err:"+err.(*Error).Code.String())
	}
	return strings.Join(out, " ")
}

func TestBinding(t *testing.T) {
	cases := []struct {
		name string
		cmd  *Command
		argv []string
		want string
	}{
		{"a bare flag", root, []string{"--force"}, "flag:force"},
		{"an attached value", root, []string{"--jobs=8"}, "flag:jobs=8"},
		{"a detached value", root, []string{"--jobs", "8"}, "flag:jobs=8"},
		{"an attached empty value", root, []string{"--jobs="}, "flag:jobs="},
		{"an attached value keeps later =", root, []string{"--jobs=a=b"}, "flag:jobs=a=b"},
		{"a short bundle", root, []string{"-fv"}, "flag:force flag:verbose"},
		{"a value ends a bundle", root, []string{"-fj8"}, "flag:force flag:jobs=8"},
		{"one = separates a short value", root, []string{"-j=8"}, "flag:jobs=8"},
		{"only one =", root, []string{"-j==8"}, "flag:jobs==8"},
		{"a negation", root, []string{"--no-color"}, "flag:color!"},
		{"names match exactly", root, []string{"--for"}, "arg:file=--for"},
		{"positionals in order", root, []string{"a", "b", "c"}, "arg:file=a arg:rest=b arg:rest=c"},
		{"flags and words interleave", root, []string{"a", "--force", "b"}, "arg:file=a flag:force arg:rest=b"},
		{"a subcommand descends", root, []string{"install"}, "cmd:install"},
		{"an alias descends", root, []string{"i"}, "cmd:install"},
		{"a global reaches a subcommand", root, []string{"install", "--verbose"}, "cmd:install flag:verbose"},
		{"a subcommand flag is not in scope above it", root, []string{"--force", "install"}, "flag:force cmd:install"},
		{"only the descent position routes", root, []string{"other", "install"}, "arg:file=other arg:rest=install"},
		{"a separator makes values", root, []string{"--", "--force"}, "arg:file=--force"},
		{"a second separator is a value", root, []string{"--", "a", "--"}, "arg:file=a arg:rest=--"},
		{"a lone dash is a value", root, []string{"-"}, "arg:file=-"},
		{"a negative number is a value", root, []string{"--jobs", "-1"}, "flag:jobs=-1"},
		{"an exponent is a number", root, []string{"--jobs", "-1e5"}, "flag:jobs=-1e5"},
		{"-1x is a flag that names nothing", root, []string{"-1x"}, "arg:file=-1x"},
		{"a variadic flag collects", root, []string{"--include", "a", "b"}, "flag:include=a flag:include=b"},
		{"a variadic flag stops at a flag", root, []string{"--include", "a", "--force"}, "flag:include=a flag:force"},
		{"an unknown flag becomes a word", root, []string{"--wat"}, "arg:file=--wat"},
		{"a detached value may not be flag-like", root, []string{"--jobs", "--force"}, "err:missing_flag_value"},
		{"a flag needing a value at the end", root, []string{"--jobs"}, "err:missing_flag_value"},
		{"more words than arguments", root, []string{"install", "a"}, "cmd:install err:unexpected_arg"},

		// unknown_flags="error" is the other reading, for a CLI that owns all of its
		// flags.
		{"a strict CLI rejects an unknown long", strict, []string{"--wat"}, "err:unknown_flag"},
		{"a strict CLI rejects an unknown short", strict, []string{"-z"}, "err:unknown_flag"},
		{"a bundle is rejected whole", strict, []string{"-fz"}, "err:unknown_flag"},
		{"a lone dash is still a value", strict, []string{"-"}, "arg:file=-"},
		{"a negative number is still a value", strict, []string{"-1"}, "arg:file=-1"},
	}

	for _, c := range cases {
		t.Run(c.name, func(t *testing.T) {
			if got := collect(c.cmd, c.argv...); got != c.want {
				t.Errorf("%v\n  want %s\n  got  %s", c.argv, c.want, got)
			}
		})
	}
}

// TestBundleIsRejectedWhole pins the rule that costs the parser a second scan.
//
// A token containing an unrecognized letter is not a bundle at all, so none of its
// letters are applied: -fz does not set -f on the way to discovering that z names
// nothing. Emitting events one at a time makes this a real hazard rather than a
// theoretical one, which is why check happens before emit.
func TestBundleIsRejectedWhole(t *testing.T) {
	p := New(strict, []string{"-fz"})
	if p.Next() {
		t.Fatalf("no event should be emitted, got %+v", p.Event())
	}
	err, ok := p.Err().(*Error)
	if !ok || err.Code != CodeUnknownFlag {
		t.Fatalf("want unknown_flag, got %v", p.Err())
	}
	// The whole token, which is the unit it was rejected in.
	if err.Token != "-fz" {
		t.Errorf("want the whole token, got %q", err.Token)
	}
}

// TestHelpIsAFlagNotAFailure records where the line is drawn.
//
// --help and -h are reported as ordinary flag events, because whether asking for
// help ends the parse is a decision for the layer that owns the target struct.
// The bare word `help` is different: it is a question rather than an invocation,
// and it names the command it is asking about, so it stops the parse.
func TestHelpIsAFlagNotAFailure(t *testing.T) {
	if got := collect(root, "--help"); got != "flag:help" {
		t.Errorf("--help: want flag:help, got %s", got)
	}
	if got := collect(root, "-h"); got != "flag:help" {
		t.Errorf("-h: want flag:help, got %s", got)
	}

	p := New(root, []string{"help", "install"})
	for p.Next() {
	}
	err, ok := p.Err().(*Error)
	if !ok || err.Code != CodeHelp {
		t.Fatalf("`help install`: want a help request, got %v", p.Err())
	}
	if err.Cmd != install {
		t.Errorf("want help about install, got %v", err.Cmd)
	}
}

// TestParseAllocatesNothing is the property the design rests on, measured rather
// than asserted.
//
// The tables are static, the parser holds its state and its error inline, and a
// value is a slice of the argv string rather than a copy of it. So binding reaches
// the allocator zero times — on failure as well as on success, which is the half
// that usually gets away with allocating because nobody measures the sad path.
func TestParseAllocatesNothing(t *testing.T) {
	cases := []struct {
		name string
		cmd  *Command
		argv []string
	}{
		{"a subcommand, a global, flags and words", root,
			[]string{"install", "--verbose", "-f", "a", "b", "c"}},
		{"values in every form", root,
			[]string{"--jobs=8", "-j", "9", "-fv", "--no-color", "--", "-x"}},
		{"a variadic flag collecting", root, []string{"--include", "a", "b", "c"}},
		{"a failure", strict, []string{"--wat"}},
		{"a failure part way through a bundle", strict, []string{"-fz"}},
	}

	for _, c := range cases {
		t.Run(c.name, func(t *testing.T) {
			// Hoisted: a slice literal inside the closure would be the allocation this
			// test then blamed on the parser.
			cmd, args := c.cmd, c.argv
			n := testing.AllocsPerRun(100, func() {
				p := New(cmd, args)
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

// A command's own name beats another command's alias, whichever order the table
// lists them in. Checking each subcommand's name and aliases together made this
// depend on which came first — and usage-lib, building a map, answered with the
// last. Neither was a rule anyone had picked, so the grammar picked this one.
func TestNameOutranksAnotherCommandsAlias(t *testing.T) {
	alpha := &Command{Name: "alpha", Aliases: []string{"run"}, Key: 300}
	plainRun := &Command{Name: "run", Key: 301}

	for _, subs := range [][]*Command{
		{alpha, plainRun},
		{plainRun, alpha},
	} {
		cmd := &Command{Name: "ex", Subcommands: subs}
		if got := findNamed(cmd, "run"); got != plainRun {
			t.Errorf("findNamed(%q) = %v, want the command named run", "run", got)
		}
		// The alias's own command is still reachable by every name it does not share.
		if got := findNamed(cmd, "alpha"); got != alpha {
			t.Errorf("findNamed(%q) = %v, want alpha", "alpha", got)
		}
	}
}

func TestAllowHyphenValues(t *testing.T) {
	args := &Flag{Key: 6, Name: "args", Longs: []string{"args"}, Shorts: []byte{'a'},
		TakesValue: true, AllowHyphenValues: true}
	dir := &Flag{Key: 7, Name: "working-dir", Longs: []string{"working-dir"}, Shorts: []byte{'d'},
		TakesValue: true}
	rest := &Arg{Key: 11, Name: "rest", Var: true}
	cmd := &Command{Name: "ex", Flags: []*Flag{args, dir}, Args: []*Arg{rest}}

	if got := collect(cmd, "-a", "-destroy"); got != "flag:args=-destroy" {
		t.Errorf("-a -destroy: got %s", got)
	}
	if got := collect(cmd, "--args", "--", "-x"); got != "flag:args=-- arg:rest=-x" {
		t.Errorf("--args -- -x: got %s", got)
	}
}

func TestRequireEquals(t *testing.T) {
	inspect := &Flag{Key: 8, Name: "inspect", Longs: []string{"inspect"}, Shorts: []byte{'i'},
		TakesValue: true, RequireEquals: true}
	all := &Flag{Key: 9, Name: "all", Longs: []string{"all"}, Shorts: []byte{'a'}}
	cmd := &Command{Name: "ex", Flags: []*Flag{inspect, all}}

	if got := collect(cmd, "--inspect=9229"); got != "flag:inspect=9229" {
		t.Errorf("--inspect=9229: got %s", got)
	}
	if got := collect(cmd, "--inspect", "9229"); got != "err:missing_flag_value" {
		t.Errorf("--inspect 9229: got %s", got)
	}
	if got := collect(cmd, "-i9229"); got != "flag:inspect=9229" {
		t.Errorf("-i9229: got %s", got)
	}
	if got := collect(cmd, "-ai", "9229"); got != "flag:all err:missing_flag_value" {
		t.Errorf("-ai 9229: got %s", got)
	}
}

func TestDefaultMissing(t *testing.T) {
	color := &Flag{Key: 9, Name: "color", Longs: []string{"color"}, Shorts: []byte{'c'},
		TakesValue: true, DefaultMissing: "always"}
	verbose := &Flag{Key: 10, Name: "verbose", Longs: []string{"verbose"}, Shorts: []byte{'v'}}
	cmd := &Command{Name: "ex", Flags: []*Flag{color, verbose}}

	if got := collect(cmd, "--color"); got != "flag:color=always" {
		t.Errorf("--color: got %s", got)
	}
	if got := collect(cmd, "--color=never"); got != "flag:color=never" {
		t.Errorf("--color=never: got %s", got)
	}
	if got := collect(cmd, "--color", "never"); got != "flag:color=never" {
		t.Errorf("--color never: got %s", got)
	}
	if got := collect(cmd, "--color", "--verbose"); got != "flag:color=always flag:verbose" {
		t.Errorf("--color --verbose: got %s", got)
	}
	if got := collect(cmd, "--color="); got != "flag:color=" {
		t.Errorf("--color=: got %s", got)
	}
	if got := collect(cmd, "-c"); got != "flag:color=always" {
		t.Errorf("-c: got %s", got)
	}
	if got := collect(cmd, "-cnever"); got != "flag:color=never" {
		t.Errorf("-cnever: got %s", got)
	}
}

// Bind only: choices live in Check. A missing default that is not on the list
// still binds, and Check is what refuses it — the same path as `--color=wat`.
func TestDefaultMissingGoesThroughChoices(t *testing.T) {
	color := &Flag{Key: 13, Name: "color", Longs: []string{"color"},
		TakesValue: true, DefaultMissing: "always"}
	cmd := &Command{Name: "ex", Flags: []*Flag{color}}
	meta := &Meta{Name: "color", Flag: true, Choices: []string{"auto", "always", "never"}}

	if got := collect(cmd, "--color"); got != "flag:color=always" {
		t.Errorf("--color: got %s", got)
	}
	if err := Check(meta, []string{"always"}, 1); err != nil {
		t.Errorf("always is a choice: %v", err)
	}

	bad := &Flag{Key: 14, Name: "color", Longs: []string{"color"},
		TakesValue: true, DefaultMissing: "wat"}
	if got := collect(&Command{Name: "ex", Flags: []*Flag{bad}}, "--color"); got != "flag:color=wat" {
		t.Errorf("bind still happens: got %s", got)
	}
	err := Check(meta, []string{"wat"}, 1)
	if err == nil || err.Code != CodeInvalidChoice {
		t.Errorf("want invalid choice after bind, got %v", err)
	}
}

func TestDefaultMissingWithRequireEquals(t *testing.T) {
	inspect := &Flag{Key: 11, Name: "inspect", Longs: []string{"inspect"},
		TakesValue: true, RequireEquals: true, DefaultMissing: "9229"}
	rest := &Arg{Key: 12, Name: "rest"}
	cmd := &Command{Name: "ex", Flags: []*Flag{inspect}, Args: []*Arg{rest}}

	if got := collect(cmd, "--inspect"); got != "flag:inspect=9229" {
		t.Errorf("--inspect: got %s", got)
	}
	if got := collect(cmd, "--inspect", "80"); got != "flag:inspect=9229 arg:rest=80" {
		t.Errorf("--inspect 80: got %s", got)
	}
	if got := collect(cmd, "--inspect="); got != "flag:inspect=" {
		t.Errorf("--inspect=: got %s", got)
	}
}

func TestExternalSubcommand(t *testing.T) {
	install := &Command{Name: "install", Key: 100}
	catch := &Command{
		Name:               "ex",
		Flags:              []*Flag{verbose},
		Subcommands:        []*Command{install},
		ExternalSubcommand: true,
		UnknownFlags:       UnknownFlagsError,
	}
	if got := collect(catch, "foo", "--help", "bar"); got != "external:foo,--help,bar" {
		t.Errorf("forward: got %s", got)
	}
	if got := collect(catch, "install"); got != "cmd:install" {
		t.Errorf("known command: got %s", got)
	}
	if got := collect(catch, "--verbose", "foo", "--verbose"); got != "flag:verbose external:foo,--verbose" {
		t.Errorf("global before the word: got %s", got)
	}
	if got := collect(catch, "--wat"); got != "err:unknown_flag" {
		t.Errorf("unknown flag: got %s", got)
	}

	run := &Command{Name: "run", Args: []*Arg{{Key: 10, Name: "task"}}}
	both := &Command{
		Name:               "ex",
		Subcommands:        []*Command{run},
		DefaultSubcommand:  run,
		ExternalSubcommand: true,
	}
	if got := collect(both, "build"); got != "cmd:run arg:task=build" {
		t.Errorf("default outranks: got %s", got)
	}
}

func BenchmarkParse(b *testing.B) {
	args := []string{"install", "--verbose", "-f", "a", "b", "c"}
	b.ReportAllocs()
	b.ResetTimer()
	for i := 0; i < b.N; i++ {
		p := New(root, args)
		for p.Next() {
			_ = p.Event()
		}
	}
}
