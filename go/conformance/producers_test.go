package conformance

import (
	"fmt"
	"path/filepath"
	"reflect"
	"testing"

	"github.com/jdx/usage/go/argv"
	"github.com/jdx/usage/go/internal/shadow/mise"
)

// Do the two things that build tables build the same tables?
//
// There are two producers, and they are written in different languages. `usage
// generate go` emits Go source at build time, field by field, from Rust; the
// lowering in `internal/spec` builds the same structs at run time, from the same
// spec, in Go. An adopter picks one and gets whichever set of rules that half
// happens to implement.
//
// Nothing else notices when they drift. The corpus runs against the lowering;
// the shadow package's tests run against the generated tables; the page tests
// compare either one against usage-lib, and a field neither renderer reads —
// or that both fall back for — is invisible to all of them. So this compares
// the two producers directly, over mise's spec, which is what both are pointed
// at anyway: `mise.Root` is generated from `benches/mise.usage.kdl`, and that
// same file is lowered here.
//
// Whole structs rather than named fields, so that a field added to a table is
// compared by having been added rather than by someone remembering to list it.

func TestTheTwoProducersAgree(t *testing.T) {
	usageBin := findUsage(t)
	lowered := lowerFile(t, usageBin, filepath.Join("..", "..", "benches", "mise.usage.kdl"))
	root, meta, help := lowered.BuildAll()

	// The hot table, walked in parallel: same shape, same order, same keys. Order
	// is part of the agreement rather than an accident of it — the parser takes
	// the first flag in scope that matches, so two tables listing the same flags
	// in different orders bind differently.
	var walk func(path string, a, b *argv.Command)
	walk = func(path string, a, b *argv.Command) {
		if a.Name != b.Name || a.Key != b.Key {
			t.Errorf("%s: lowered %q/%d, generated %q/%d", path, a.Name, a.Key, b.Name, b.Key)
			return
		}
		compare(t, path, *a, *b, "Flags", "Args", "Subcommands", "DefaultSubcommand")
		compareSlice(t, path+": flags", a.Flags, b.Flags)
		compareSlice(t, path+": args", a.Args, b.Args)
		// By key rather than by pointer: the two trees are different objects, so
		// pointer identity says nothing and the key is what the tables use to
		// mean "this one".
		if keyOf(a.DefaultSubcommand) != keyOf(b.DefaultSubcommand) {
			t.Errorf("%s: default subcommand is %d lowered, %d generated",
				path, keyOf(a.DefaultSubcommand), keyOf(b.DefaultSubcommand))
		}
		if len(a.Subcommands) != len(b.Subcommands) {
			t.Errorf("%s: %d subcommands lowered, %d generated",
				path, len(a.Subcommands), len(b.Subcommands))
			return
		}
		for i := range a.Subcommands {
			walk(path+" "+a.Subcommands[i].Name, a.Subcommands[i], b.Subcommands[i])
		}
	}
	walk("mise", root, mise.Root)

	// And the two cold tables, per entry. Dense from 1 on both sides, which is
	// what lets a key index them, so a length difference is itself a failure.
	if len(meta) != len(mise.Meta) {
		t.Fatalf("%d metadata entries lowered, %d generated", len(meta), len(mise.Meta))
	}
	if len(help) != len(mise.HelpText) {
		t.Fatalf("%d help entries lowered, %d generated", len(help), len(mise.HelpText))
	}
	for i := range meta {
		compare(t, fmt.Sprintf("meta[%d]", i), meta[i], mise.Meta[i])
	}
	for i := range help {
		compare(t, fmt.Sprintf("help[%d]", i), help[i], mise.HelpText[i])
	}
}

func keyOf(c *argv.Command) uint64 {
	if c == nil {
		return 0
	}
	return c.Key
}

// compare reports the fields of two table entries that differ, skipping the
// named ones — the pointers into the tree, which are compared by key instead.
func compare[T any](t *testing.T, where string, lowered, generated T, skip ...string) {
	t.Helper()
	a := reflect.ValueOf(lowered)
	b := reflect.ValueOf(generated)
	for i := 0; i < a.NumField(); i++ {
		name := a.Type().Field(i).Name
		if contains(skip, name) {
			continue
		}
		x, y := a.Field(i).Interface(), b.Field(i).Interface()
		// An empty slice and an unset one are the same table: the emitter writes
		// nothing where the lowering may have made a slice and put nothing in it.
		if isEmptySlice(a.Field(i)) && isEmptySlice(b.Field(i)) {
			continue
		}
		if !reflect.DeepEqual(x, y) {
			t.Errorf("%s: %s is %#v lowered, %#v generated", where, name, x, y)
		}
	}
}

func compareSlice[T any](t *testing.T, where string, lowered, generated []*T) {
	t.Helper()
	if len(lowered) != len(generated) {
		t.Errorf("%s: %d lowered, %d generated", where, len(lowered), len(generated))
		return
	}
	for i := range lowered {
		compare(t, fmt.Sprintf("%s[%d]", where, i), *lowered[i], *generated[i])
	}
}

func isEmptySlice(v reflect.Value) bool {
	return v.Kind() == reflect.Slice && v.Len() == 0
}

func contains(list []string, s string) bool {
	for _, x := range list {
		if x == s {
			return true
		}
	}
	return false
}
