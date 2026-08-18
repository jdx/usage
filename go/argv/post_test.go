package argv

import (
	"reflect"
	"testing"
)

func env(pairs map[string]string) func(string) (string, bool) {
	return func(name string) (string, bool) {
		v, ok := pairs[name]
		return v, ok
	}
}

func TestFillOrder(t *testing.T) {
	meta := &Meta{Name: "jobs", Flag: true, Env: "EX_JOBS", Default: []string{"1"}}

	cases := []struct {
		name   string
		given  []string
		env    map[string]string
		want   []string
		source Source
	}{
		{"the command line wins", []string{"8"}, map[string]string{"EX_JOBS": "4"},
			[]string{"8"}, FromArgv},
		{"then the environment", nil, map[string]string{"EX_JOBS": "4"},
			[]string{"4"}, FromEnv},
		{"then the default", nil, nil, []string{"1"}, FromDefault},
		// Treating empty as unset would make `EX_JOBS=` mean something no other
		// empty value in the grammar means.
		{"an empty variable is set", nil, map[string]string{"EX_JOBS": ""},
			[]string{""}, FromEnv},
		// The grammar never re-splits a value: quoting is the shell's job, and
		// there was no shell involved here at all.
		{"a value is one token", nil, map[string]string{"EX_JOBS": "a b,c"},
			[]string{"a b,c"}, FromEnv},
		// `--jobs=` binds the empty string, which is a value the command line gave.
		{"an empty value from argv is still a value", []string{""},
			map[string]string{"EX_JOBS": "4"}, []string{""}, FromArgv},
	}

	for _, c := range cases {
		t.Run(c.name, func(t *testing.T) {
			got, source := Fill(meta, c.given, env(c.env))
			if !reflect.DeepEqual(got, c.want) || source != c.source {
				t.Errorf("want %q from %v, got %q from %v", c.want, c.source, got, source)
			}
		})
	}
}

func TestFillWithNothingDeclared(t *testing.T) {
	// No metadata at all is ordinary: an entry with nothing to say about itself
	// beyond how it binds.
	if got, source := Fill(nil, nil, env(nil)); got != nil || source != Unset {
		t.Errorf("want nothing, got %q from %v", got, source)
	}
	if got, _ := Fill(nil, []string{"x"}, env(nil)); !reflect.DeepEqual(got, []string{"x"}) {
		t.Errorf("what binding gave should survive: %q", got)
	}
}

func TestCheck(t *testing.T) {
	cases := []struct {
		name        string
		meta        Meta
		values      []string
		occurrences int
		want        Code
		ok          bool
	}{
		{"a required flag nobody gave", Meta{Name: "file", Flag: true, Required: true},
			nil, 0, CodeMissingRequiredFlag, false},
		{"a required argument nobody filled", Meta{Name: "file", Required: true},
			nil, 0, CodeMissingRequiredArg, false},
		{"a required flag given a value", Meta{Name: "file", Flag: true, Required: true},
			[]string{"x"}, 1, 0, true},
		// A value-less flag has no values to count, so being seen at all is the
		// only evidence it was given.
		{"a required flag that holds no value", Meta{Name: "v", Flag: true, Required: true},
			nil, 1, 0, true},

		{"a value outside the choices", Meta{Name: "shell", Choices: []string{"bash", "zsh"}},
			[]string{"csh"}, 1, CodeInvalidChoice, false},
		// Matching is case-sensitive: case-insensitive matching would have to be
		// declared rather than assumed.
		{"choices are case-sensitive", Meta{Name: "shell", Choices: []string{"bash"}},
			[]string{"BASH"}, 1, CodeInvalidChoice, false},
		// Every value, because a variadic can be given a good one and a bad one.
		{"every value is checked", Meta{Name: "shell", Choices: []string{"bash", "zsh"}},
			[]string{"bash", "csh"}, 1, CodeInvalidChoice, false},
		{"all of them allowed", Meta{Name: "shell", Choices: []string{"bash", "zsh"}},
			[]string{"bash", "zsh"}, 2, 0, true},

		{"fewer values than var_min", Meta{Name: "files", VarMin: 2},
			[]string{"a"}, 1, CodeVarTooFew, false},
		{"enough values", Meta{Name: "files", VarMin: 2}, []string{"a", "b"}, 1, 0, true},
		// An absent optional variadic has not broken its minimum; it simply is not
		// there, and reporting it would make every bounded variadic required.
		{"an absent variadic has not broken its minimum", Meta{Name: "files", VarMin: 2},
			nil, 0, 0, true},

		// Occurrences, not values: a variadic occurrence can bring several.
		{"more occurrences than var_max", Meta{Name: "include", Flag: true, VarMax: 1},
			[]string{"a", "b"}, 2, CodeVarTooMany, false},
		{"one occurrence bringing several values", Meta{Name: "include", Flag: true, VarMax: 1},
			[]string{"a", "b"}, 1, 0, true},

		// Required is asked first: a required variadic given nothing is missing
		// rather than short, which is the more useful thing to be told.
		{"missing beats short", Meta{Name: "files", Required: true, VarMin: 2},
			nil, 0, CodeMissingRequiredArg, false},
	}

	for _, c := range cases {
		t.Run(c.name, func(t *testing.T) {
			err := Check(&c.meta, c.values, c.occurrences)
			if c.ok {
				if err != nil {
					t.Fatalf("want no error, got %q", err.Code)
				}
				return
			}
			if err == nil {
				t.Fatalf("want %q, got no error", c.want)
			}
			if err.Code != c.want {
				t.Errorf("want %q, got %q", c.want, err.Code)
			}
			if err.Name != c.meta.Name {
				t.Errorf("the error should name the entry, got %q", err.Name)
			}
		})
	}
}

// TestEnvTruth pins the allow-list, including what it deliberately leaves out.
func TestEnvTruth(t *testing.T) {
	for _, s := range []string{"1", "true", "True", "TRUE"} {
		if !EnvTruth(s) {
			t.Errorf("%q should set the flag", s)
		}
	}
	// `0` and `false` are what the corpus pins. The rest are the cases the
	// allow-list quietly excludes, recorded so nobody assumes otherwise.
	for _, s := range []string{"0", "false", "", "yes", "on", "TrUe", "2"} {
		if EnvTruth(s) {
			t.Errorf("%q should not set the flag", s)
		}
	}
}

func TestMetadataLookup(t *testing.T) {
	m := Metadata{{Key: 1, Name: "a"}, {Key: 2, Name: "b"}, {Key: 3, Name: "c"}}
	if got := m.Lookup(2); got == nil || got.Name != "b" {
		t.Errorf("want b, got %v", got)
	}
	// Out of range in both directions, and key 0, which nothing is ever assigned.
	for _, key := range []uint64{0, 4, 1 << 40} {
		if got := m.Lookup(key); got != nil {
			t.Errorf("key %d should find nothing, got %v", key, got)
		}
	}
	// A table out of step with the parse tables reports nothing rather than
	// silently describing the wrong entry.
	crooked := Metadata{{Key: 7, Name: "wrong"}}
	if got := crooked.Lookup(1); got != nil {
		t.Errorf("a mismatched key should find nothing, got %v", got)
	}
}

// TestPostBindingIsOffTheHotPath is the reason this file is separate.
//
// A parse that never asks for the rules must not pay for them, and the check
// worth making is that the tables above are not consulted during binding at all —
// which shows up as the parse still allocating nothing with metadata present.
func TestPostBindingIsOffTheHotPath(t *testing.T) {
	args := []string{"install", "--verbose", "-f", "a", "b"}
	n := testing.AllocsPerRun(100, func() {
		p := New(root, args)
		for p.Next() {
			_ = p.Event()
		}
		_ = p.Err()
	})
	if n != 0 {
		t.Errorf("want 0 allocations, got %v", n)
	}
}

// TestSourceGiven pins the asymmetry the relationship rules depend on.
//
// The command line and the environment count as the user having said something;
// a default does not. Counting a default would make a defaulted flag conflict
// with every partner anyone types, and usage-lib agrees: with `--file` defaulted
// and only `--stdin` given, a declared conflict between them does not fire.
func TestSourceGiven(t *testing.T) {
	if !FromArgv.Given() || !FromEnv.Given() {
		t.Error("argv and env are both the user supplying a value")
	}
	if FromDefault.Given() || Unset.Given() {
		t.Error("a default is a fallback, not something the user said")
	}
}

func TestApplyDefaultIf(t *testing.T) {
	json := uint64(1)
	bin := uint64(2)
	meta := Metadata{
		{Key: json, Name: "json", Flag: true},
		{Key: bin, Name: "bin-names", Flag: true, DefaultIf: []DefaultIf{
			{Key: json, Value: "true"},
		}, Default: []string{"false"}},
	}
	filled := map[uint64][]string{json: {}, bin: nil}
	sources := map[uint64]Source{json: FromArgv, bin: Unset}
	ApplyDefaultIf(meta, []uint64{json, bin}, filled, sources, nil)
	if sources[bin] != FromDefault || !reflect.DeepEqual(filled[bin], []string{"true"}) {
		t.Errorf("IsPresent should bind: %q from %v", filled[bin], sources[bin])
	}

	filled = map[uint64][]string{json: nil, bin: nil}
	sources = map[uint64]Source{json: Unset, bin: Unset}
	ApplyDefaultIf(meta, []uint64{json, bin}, filled, sources, nil)
	if sources[bin] != FromDefault || !reflect.DeepEqual(filled[bin], []string{"false"}) {
		t.Errorf("unconditional default when no match: %q from %v", filled[bin], sources[bin])
	}

	filled = map[uint64][]string{json: {}, bin: []string{"already"}}
	sources = map[uint64]Source{json: FromArgv, bin: FromEnv}
	ApplyDefaultIf(meta, []uint64{json, bin}, filled, sources, nil)
	if sources[bin] != FromEnv || !reflect.DeepEqual(filled[bin], []string{"already"}) {
		t.Errorf("env on the target suppresses: %q from %v", filled[bin], sources[bin])
	}

	pretty := uint64(2)
	meta = Metadata{
		{Key: json, Name: "json", Flag: true},
		{Key: pretty, Name: "pretty", Flag: true, DefaultIf: []DefaultIf{
			{Key: json, When: "false", Value: "true"},
		}},
	}
	filled = map[uint64][]string{json: {}, pretty: nil}
	sources = map[uint64]Source{json: FromArgv, pretty: Unset}
	ApplyDefaultIf(meta, []uint64{json, pretty}, filled, sources, map[uint64]bool{json: true})
	if sources[pretty] != FromDefault || !reflect.DeepEqual(filled[pretty], []string{"true"}) {
		t.Errorf("--no-json should match when=false: %q from %v", filled[pretty], sources[pretty])
	}

	filled = map[uint64][]string{json: {}, pretty: nil}
	sources = map[uint64]Source{json: FromArgv, pretty: Unset}
	ApplyDefaultIf(meta, []uint64{json, pretty}, filled, sources, nil)
	if sources[pretty] != Unset {
		t.Errorf("--json should not match when=false: %q from %v", filled[pretty], sources[pretty])
	}

	meta = Metadata{
		{Key: json, Name: "json", Flag: true, Default: []string{"true"}},
		{Key: pretty, Name: "pretty", Flag: true, DefaultIf: []DefaultIf{
			{Key: json, Value: "true"},
		}},
	}
	filled = map[uint64][]string{json: {"true"}, pretty: nil}
	sources = map[uint64]Source{json: FromDefault, pretty: Unset}
	ApplyDefaultIf(meta, []uint64{json, pretty}, filled, sources, nil)
	if sources[pretty] != Unset {
		t.Errorf("a default is not Given: %q from %v", filled[pretty], sources[pretty])
	}
}
