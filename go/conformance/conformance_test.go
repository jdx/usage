// Package conformance runs the shared corpus against the Go parser.
//
// The corpus is the definition of correct. It is plain JSON precisely so that an
// implementation in any language can run it without reimplementing a test format,
// and the corpus README says as much: passing it is what "compatible" means. This
// file is that claim, made executable for Go.
//
// A vector's spec is KDL, and this module has no KDL parser by design — see
// [github.com/jdx/usage/go/internal/spec]. The `usage` CLI does the lowering, so
// these tests need it built. `mise run test:go` builds it first.
package conformance

import (
	"bytes"
	"encoding/json"
	"fmt"
	"io"
	"os"
	"os/exec"
	"path/filepath"
	"reflect"
	"sort"
	"testing"

	"github.com/jdx/usage/go/argv"
	"github.com/jdx/usage/go/internal/spec"
)

// Vector is one corpus case.
//
// Every field the corpus can carry is declared here, including the ones binding
// never consults, because the loader refuses unknown fields — see [load]. A
// vector's meaning can be changed by a field this harness has not heard of:
// `layer` is exactly that, and a Go suite that ignored it would answer
// post-binding vectors it is not equipped for and report the wrong result with
// confidence.
type Vector struct {
	ID     string            `json:"id"`
	Doc    string            `json:"doc"`
	Spec   string            `json:"spec"`
	Argv   []string          `json:"argv"`
	Env    map[string]string `json:"env"`
	Expect Expect            `json:"expect"`
	// Layer says which layer of a parser the vector is a question for. Absent
	// means binding, which is the default and most of them.
	Layer string `json:"layer"`
	// Reference records whether usage-lib agrees with the grammar. It is a note
	// about the Rust reference implementation, not about this one, and is read
	// only so that a failure can mention it: a vector usage-lib diverges on is one
	// where copying usage-lib's behavior would have been the wrong move.
	Reference *struct {
		Diverges string `json:"diverges"`
	} `json:"reference"`
}

// Expect is the result a vector requires: a binding, or a class of failure.
type Expect struct {
	OK    *Parsed `json:"ok"`
	Error string  `json:"error"`
}

// Parsed is what a successful parse must produce. Both maps are keyed by the name
// the spec gives each flag or argument, never by the token that set it, so -j,
// --jobs and an env var all land under `jobs`.
type Parsed struct {
	Cmd   []string               `json:"cmd"`
	Flags map[string]interface{} `json:"flags"`
	Args  map[string]interface{} `json:"args"`
}

type file struct {
	Section string   `json:"section"`
	About   string   `json:"about"`
	Vectors []Vector `json:"vectors"`
}

func TestCorpus(t *testing.T) {
	usageBin := findUsage(t)
	vectors := load(t)

	// Lowering shells out, and vectors share specs, so this saves most of the
	// subprocesses.
	lowered := map[string]*spec.Spec{}

	var ran, skipped int
	for _, v := range vectors {
		v := v
		t.Run(v.ID, func(t *testing.T) {
			if reason, unsupported := notYet[v.ID]; unsupported {
				skipped++
				t.Skip(reason)
			}
			ran++

			s, ok := lowered[v.Spec]
			if !ok {
				s = lower(t, usageBin, v.Spec)
				lowered[v.Spec] = s
			}

			got, gotErr := run(s, v.Argv, v.Env)

			switch {
			case v.Expect.Error != "":
				if gotErr == nil {
					t.Fatalf("expected error %q, parsed %s%s", v.Expect.Error, show(got), note(v))
				}
				if gotErr.Code.String() != v.Expect.Error {
					t.Fatalf("expected error %q, got %q%s", v.Expect.Error, gotErr.Code, note(v))
				}
			case v.Expect.OK != nil:
				if gotErr != nil {
					t.Fatalf("expected a parse, got error %q%s", gotErr.Code, note(v))
				}
				want := normalizeExpected(v.Expect.OK)
				if !reflect.DeepEqual(got, want) {
					t.Fatalf("mismatch%s\n  want %s\n  got  %s", note(v), show(want), show(got))
				}
			default:
				t.Fatalf("vector expects neither a parse nor an error")
			}
		})
	}

	t.Logf("%d vectors: %d answered, %d not yet", len(vectors), ran, skipped)

	// The corpus is answered in full today. Asserted so that it stays a
	// measurement: a vector that starts failing gets skipped by nobody, and one
	// that gets quietly excluded shows up here.
	if skipped != 0 {
		t.Errorf("%d vectors were skipped; the whole corpus is meant to be answered", skipped)
	}

	// Asserted, so the unsupported set cannot quietly grow: a vector added to
	// `notYet` without this being raised deliberately fails here instead.
	if skipped != len(notYet) {
		t.Errorf("skipped %d vectors but `notYet` lists %d; a listed id may have been "+
			"renamed, which would silently stop excluding anything", skipped, len(notYet))
	}
}

// Vectors this implementation does not answer yet, and why.
//
// Empty, and kept rather than deleted: the whole corpus is answered today, and
// the mechanism is what makes that a measurement instead of a claim. The count is
// asserted against this map above, so a vector added here has to be added
// deliberately, and one that stops being skipped without being removed fails.
//
// By id, if it ever refills. Inferring "has a `conflicts` in the spec" would
// exempt vectors nobody meant to exempt: `overrides-loser-is-not-refilled-from-env`
// is as much an env question as an overrides one.
var notYet = map[string]string{}

// note quotes the vector's own explanation on failure, and flags the ones where
// usage-lib diverges from the grammar: those are the cases where matching the
// reference implementation instead of the corpus would be the bug.
func note(v Vector) string {
	s := "\n  " + v.Doc
	if v.Reference != nil {
		s += "\n  usage-lib diverges here: " + v.Reference.Diverges
	}
	return s
}

// run binds a command line and accumulates the events the way the corpus records
// them.
//
// How a value accumulates depends on the declaration, which the parser
// deliberately does not know: it reports each occurrence and lets the caller
// decide. Generated code assigns to a field or appends to a slice; here the spec
// says which of the two to do.
func run(s *spec.Spec, args []string, env map[string]string) (*Parsed, *argv.Error) {
	root, meta := s.Build()
	multi := s.MultiFlags()

	// Accumulated by key rather than by name, because the post-binding rules are
	// keyed that way and two commands in one path may declare the same name — a
	// subcommand redeclaring a global is ordinary.
	type bound struct {
		values      []string
		occurrences int
		negated     bool
		// Where this entry's last token sat, which `overrides` needs and nothing
		// else does: it is the one rule decided by which of two flags came last.
		at int
	}
	seen := 0
	got := map[uint64]*bound{}
	// Keys an override removed. They are absent from here on, including from the
	// fallbacks, which is the whole point.
	lost := map[uint64]bool{}
	entry := func(key uint64) *bound {
		if got[key] == nil {
			got[key] = &bound{}
		}
		return got[key]
	}

	// The commands whose declarations are in scope. A required flag on a command
	// nobody selected is not missing; it is simply not this invocation's.
	path := []*argv.Command{root}

	p := argv.New(root, args)
	for p.Next() {
		ev := p.Event()
		switch ev.Kind {
		case argv.KindCommand:
			out := ev.Command
			path = append(path, out)
		case argv.KindFlag:
			seen++
			b := entry(ev.Flag.Key)
			b.occurrences++
			b.negated = ev.Negated
			b.at = seen
			if ev.HasValue {
				b.values = append(b.values, ev.Value)
			}
		case argv.KindArg:
			seen++
			b := entry(ev.Arg.Key)
			b.occurrences++
			b.at = seen
			b.values = append(b.values, ev.Value)
		}
	}
	if err := p.Err(); err != nil {
		e, ok := err.(*argv.Error)
		if !ok {
			panic("the parser returns only *argv.Error")
		}
		return nil, e
	}

	out := &Parsed{
		Cmd:   []string{},
		Flags: map[string]interface{}{},
		Args:  map[string]interface{}{},
	}
	for _, cmd := range path[1:] {
		out.Cmd = append(out.Cmd, cmd.Name)
	}

	lookup := func(name string) (string, bool) {
		// The vector's own environment, never the process's, so no result can
		// depend on the machine running the suite.
		v, ok := env[name]
		return v, ok
	}

	// Overrides first, and before anything fills from `env` or `default`: a flag
	// that lost is not merely unset, and refilling it afterwards would leave both
	// standing and undo the last-one-wins.
	order := map[uint64]int{}
	for key, b := range got {
		order[key] = b.at
	}
	for key := range argv.ApplyOverrides(meta, order) {
		delete(got, key)
		lost[key] = true
	}

	// The fallbacks are applied to everything in scope before anything is judged,
	// because the rules that compare two entries need both of their final states —
	// and `conflicts` in particular asks only whether a flag has a value, not how
	// it got one.
	type resolved struct {
		flag        *argv.Flag
		arg         *argv.Arg
		values      []string
		source      argv.Source
		occurrences int
		negated     bool
	}
	final := map[uint64]*resolved{}
	var scope []uint64

	fill := func(key uint64, takesValue bool) *resolved {
		b := got[key]
		var given []string
		if b != nil && (len(b.values) > 0 || !takesValue) {
			// A value-less flag that was given has no values, and nil would read as
			// "the command line said nothing" — so the empty slice is the
			// distinction.
			given = b.values
			if given == nil {
				given = []string{}
			}
		}
		r := &resolved{}
		r.values, r.source = argv.Fill(meta.Lookup(key), given, lookup)
		if b != nil {
			r.occurrences = b.occurrences
			r.negated = b.negated
		}
		final[key] = r
		scope = append(scope, key)
		return r
	}

	for _, cmd := range path {
		for _, f := range cmd.Flags {
			// A flag that lost an override is out of the running rather than
			// merely absent: it is not filled from `env` or `default`, and it is
			// not judged either. A `required` loser reported as missing would undo
			// the last-one-wins the user asked for by typing the other flag, and
			// usage-lib skips overridden flags in the requirement pass for exactly
			// that reason.
			if lost[f.Key] {
				continue
			}
			fill(f.Key, f.TakesValue).flag = f
		}
		for _, a := range cmd.Args {
			fill(a.Key, true).arg = a
		}
	}

	// What one entry ended up with, judged on its own.
	for _, key := range scope {
		r := final[key]
		occurrences := r.occurrences
		if r.arg != nil {
			occurrences = 0
		}
		if err := argv.Check(meta.Lookup(key), r.values, occurrences); err != nil {
			return nil, err
		}
	}

	// Then the rules that read one entry to judge another.
	sourceOf := func(key uint64) argv.Source {
		if r := final[key]; r != nil {
			return r.source
		}
		return argv.Unset
	}
	if err := argv.CheckRelationships(meta, scope, sourceOf); err != nil {
		return nil, err
	}

	for _, key := range scope {
		r := final[key]
		switch {
		case r.flag != nil:
			if v, ok := renderFlag(r.flag, multi, r.values, r.source, r.negated,
				r.occurrences); ok {
				out.Flags[r.flag.Name] = v
			}
		case r.arg != nil:
			if len(r.values) == 0 {
				continue
			}
			if r.arg.Var {
				out.Args[r.arg.Name] = toList(r.values)
			} else {
				out.Args[r.arg.Name] = r.values[len(r.values)-1]
			}
		}
	}
	return out, nil
}

// renderFlag turns what a flag ended up with into the shape the corpus records,
// which depends on what the flag is rather than on where the value came from.
func renderFlag(f *argv.Flag, multi map[string]spec.Multi,
	values []string, source argv.Source, negated bool, occurrences int) (interface{}, bool) {

	if !f.TakesValue {
		// A count flag records one entry per occurrence, so it is asked before
		// anything that collapses the flag to a single answer.
		if multi[f.Name] == spec.MultiCount {
			if occurrences == 0 {
				return nil, false
			}
			list := make([]interface{}, occurrences)
			for i := range list {
				list[i] = true
			}
			return list, true
		}
		switch source {
		case argv.FromArgv:
			return !negated, true
		case argv.FromEnv:
			// The text has nowhere to go, so it is read as a yes or a no.
			return argv.EnvTruth(values[0]), true
		case argv.FromDefault:
			return len(values) > 0 && values[0] == "true", true
		}
		return nil, false
	}

	if len(values) == 0 {
		return nil, false
	}
	if multi[f.Name] == spec.MultiVar {
		return toList(values), true
	}
	// The last one wins for a flag that is not collecting, which is what a field
	// assignment does.
	return values[len(values)-1], true
}

func toList(values []string) []interface{} {
	out := make([]interface{}, len(values))
	for i, v := range values {
		out[i] = v
	}
	return out
}

func strs(v interface{}) []interface{} {
	if v == nil {
		return nil
	}
	return v.([]interface{})
}

func bools(v interface{}) []interface{} { return strs(v) }

// normalizeExpected puts the JSON expectation into the shape run produces, so the
// two can be compared directly.
//
// Absent and empty mean the same thing here: some vectors omit `cmd` when nothing
// was selected and others write `"cmd": []`, and both say the parse stayed at the
// root. Likewise for the two maps. The corpus is the definition of correct about
// bindings, not about which of two spellings of "nothing" a decoder produces.
func normalizeExpected(p *Parsed) *Parsed {
	out := &Parsed{Cmd: p.Cmd, Flags: p.Flags, Args: p.Args}
	if out.Cmd == nil {
		out.Cmd = []string{}
	}
	if out.Flags == nil {
		out.Flags = map[string]interface{}{}
	}
	if out.Args == nil {
		out.Args = map[string]interface{}{}
	}
	return out
}

func show(p *Parsed) string {
	if p == nil {
		return "<nothing>"
	}
	b, err := json.Marshal(p)
	if err != nil {
		return fmt.Sprintf("%+v", p)
	}
	return string(b)
}

// lower turns a vector's KDL spec into the JSON the tables are built from.
func lower(t *testing.T, usageBin, kdl string) *spec.Spec {
	t.Helper()
	out, err := exec.Command(usageBin, "generate", "json", "--spec", kdl).Output()
	if err != nil {
		if ee, ok := err.(*exec.ExitError); ok {
			t.Fatalf("lowering the spec failed: %v\n%s", err, ee.Stderr)
		}
		t.Fatalf("lowering the spec failed: %v", err)
	}
	var s spec.Spec
	if err := json.Unmarshal(out, &s); err != nil {
		t.Fatalf("the lowered spec would not decode: %v", err)
	}
	return &s
}

func load(t *testing.T) []Vector {
	t.Helper()
	dir := filepath.Join("..", "..", "corpus")
	paths, err := filepath.Glob(filepath.Join(dir, "*.json"))
	if err != nil || len(paths) == 0 {
		t.Fatalf("no corpus files under %s (err: %v)", dir, err)
	}
	sort.Strings(paths)

	var out []Vector
	for _, p := range paths {
		b, err := os.ReadFile(p)
		if err != nil {
			t.Fatalf("reading %s: %v", p, err)
		}
		// Unknown fields are an error rather than something to skip past. The
		// corpus is shared, it grows, and a field added for everyone's benefit is
		// no use to an implementation that silently drops it: `layer` decides
		// whether a vector is this parser's to answer at all, so a suite that had
		// ignored it when it arrived would have answered 22 vectors it cannot
		// answer and called the results a pass. Failing to decode is how this
		// harness finds out the corpus has moved.
		dec := json.NewDecoder(bytes.NewReader(b))
		dec.DisallowUnknownFields()
		var f file
		if err := dec.Decode(&f); err != nil {
			t.Fatalf("decoding %s: %v\n"+
				"If the corpus has grown a field, teach Vector about it rather than "+
				"relaxing this check.", p, err)
		}
		// A Decoder stops at the end of the first value, where json.Unmarshal
		// refuses trailing content — so reaching for one to get
		// DisallowUnknownFields gave up a check that was already there. A second
		// value in the file would otherwise be dropped silently, which for a corpus
		// file means vectors that never run and a suite that passes for it.
		if err := dec.Decode(new(json.RawMessage)); err != io.EOF {
			t.Fatalf("%s: content after the top-level object; the whole file should "+
				"be one JSON object, and anything past it would not be run", p)
		}
		out = append(out, f.Vectors...)
	}
	return out
}

// findUsage locates the CLI that lowers a spec.
//
// Debug before release, which is the opposite of what looks natural. `mise run
// test:go` depends on `build`, and `build` refreshes the *debug* binary — so on
// any machine that has ever run `cargo build --release`, preferring release means
// lowering every vector with a binary this run did not build and may be many
// commits old. A stale oracle is the one failure mode a conformance suite cannot
// afford, because it reports agreement with something nobody is shipping.
//
// PATH comes last for the same reason in reverse: an installed `usage` is the
// least likely to match the working tree, so it is the fallback for running these
// tests from outside the repo layout rather than the first choice.
//
// Not skipped when nothing is found. A conformance suite that quietly passes
// because it could not find its oracle is worse than one that fails: the whole
// point of the corpus is that agreement is measured rather than assumed.
func findUsage(t *testing.T) string {
	t.Helper()
	if bin := os.Getenv("USAGE_BIN"); bin != "" {
		return bin
	}
	for _, p := range []string{
		filepath.Join("..", "..", "target", "debug", "usage"),
		filepath.Join("..", "..", "target", "release", "usage"),
	} {
		if _, err := os.Stat(p); err == nil {
			abs, err := filepath.Abs(p)
			if err == nil {
				return abs
			}
		}
	}
	if p, err := exec.LookPath("usage"); err == nil {
		return p
	}
	t.Fatal("the `usage` CLI is needed to lower each vector's spec, and was not found.\n" +
		"Run `mise run test:go`, which builds it first, or `cargo build -p usage-cli`,\n" +
		"or point USAGE_BIN at one.")
	return ""
}
