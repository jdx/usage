package argv

import (
	"reflect"
	"testing"
)

// Keys 1 and 2 are `--file` and `--stdin`, the pair the corpus uses.
const (
	keyFile uint64 = iota + 1
	keyStdin
	keyURL
)

func pair(fileDeclares, stdinDeclares []uint64, field string) Metadata {
	m := Metadata{
		{Key: keyFile, Name: "file", Flag: true},
		{Key: keyStdin, Name: "stdin", Flag: true},
		{Key: keyURL, Name: "url", Flag: true},
	}
	set := func(at int, keys []uint64) {
		switch field {
		case "overrides":
			m[at].Overrides = keys
		case "conflicts":
			m[at].Conflicts = keys
		case "required_unless":
			m[at].RequiredUnless = keys
		case "required_if":
			m[at].RequiredIf = keys
		}
	}
	set(0, fileDeclares)
	set(1, stdinDeclares)
	return m
}

func TestApplyOverrides(t *testing.T) {
	cases := []struct {
		name  string
		meta  Metadata
		order map[uint64]int
		want  map[uint64]bool
	}{
		{
			// Declared on both sides, `--file` typed last.
			"the last one given wins",
			pair([]uint64{keyStdin}, []uint64{keyFile}, "overrides"),
			map[uint64]int{keyStdin: 1, keyFile: 2},
			map[uint64]bool{keyStdin: true},
		},
		{
			// The relationship is symmetric however it was declared: `--file`
			// declares it and `--file` is the one that loses, because `--stdin`
			// came last.
			"the declaring side can be the loser",
			pair([]uint64{keyStdin}, nil, "overrides"),
			map[uint64]int{keyFile: 1, keyStdin: 2},
			map[uint64]bool{keyFile: true},
		},
		{
			// The declaration is about which of two *given* flags survives.
			"nothing is lost when only one was given",
			pair([]uint64{keyStdin}, nil, "overrides"),
			map[uint64]int{keyFile: 1},
			nil,
		},
		{
			"an unrelated flag is untouched",
			pair([]uint64{keyStdin}, nil, "overrides"),
			map[uint64]int{keyFile: 1, keyStdin: 2, keyURL: 3},
			map[uint64]bool{keyFile: true},
		},
		{
			"nothing declared, nothing lost",
			pair(nil, nil, "overrides"),
			map[uint64]int{keyFile: 1, keyStdin: 2},
			nil,
		},
	}

	for _, c := range cases {
		t.Run(c.name, func(t *testing.T) {
			got := ApplyOverrides(c.meta, c.order)
			if len(got) == 0 && len(c.want) == 0 {
				return
			}
			if !reflect.DeepEqual(got, c.want) {
				t.Errorf("want %v, got %v", c.want, got)
			}
		})
	}
}

// set builds a `sourceOf` where the named keys came from the command line and
// everything else is absent.
func set(keys ...uint64) func(uint64) Source {
	in := map[uint64]bool{}
	for _, k := range keys {
		in[k] = true
	}
	return func(k uint64) Source {
		if in[k] {
			return FromArgv
		}
		return Unset
	}
}

func TestConflicts(t *testing.T) {
	meta := pair([]uint64{keyStdin}, nil, "conflicts")
	all := []uint64{keyFile, keyStdin, keyURL}

	if err := CheckRelationships(meta, all, set(keyFile, keyStdin)); err == nil {
		t.Error("both given should conflict")
	} else {
		if err.Code != CodeConflictingFlags {
			t.Errorf("want conflicting_flags, got %q", err.Code)
		}
		// Both names, because either alone reads as a puzzle.
		if err.Name != "file" || err.Other != "stdin" {
			t.Errorf("want both names, got %q and %q", err.Name, err.Other)
		}
	}

	// Declared on one side only, and the relationship holds either way round:
	// it is between the two flags, not between a flag and the tokens after it.
	if err := CheckRelationships(meta, []uint64{keyStdin, keyFile, keyURL},
		set(keyStdin, keyFile)); err == nil {
		t.Error("the order the entries are visited in should not matter")
	}

	for _, given := range [][]uint64{{keyFile}, {keyStdin}, {keyFile, keyURL}, {}} {
		if err := CheckRelationships(meta, all, set(given...)); err != nil {
			t.Errorf("%v should be allowed, got %q", given, err.Code)
		}
	}
}

func TestConditionalRequirements(t *testing.T) {
	all := []uint64{keyFile, keyStdin, keyURL}

	unless := pair([]uint64{keyStdin}, nil, "required_unless")
	// With neither present, the requirement stands.
	if err := CheckRelationships(unless, all, set()); err == nil {
		t.Error("neither given should be missing_required_flag")
	} else if err.Code != CodeMissingRequiredFlag || err.Name != "file" {
		t.Errorf("want missing file, got %q %q", err.Code, err.Name)
	}
	// Satisfied either by the flag itself or by the one it names.
	for _, given := range [][]uint64{{keyFile}, {keyStdin}, {keyFile, keyStdin}} {
		if err := CheckRelationships(unless, all, set(given...)); err != nil {
			t.Errorf("%v should satisfy it, got %q", given, err.Code)
		}
	}

	ifm := pair([]uint64{keyURL}, nil, "required_if")
	if err := CheckRelationships(ifm, all, set(keyURL)); err == nil {
		t.Error("the trigger being present should make it required")
	} else if err.Code != CodeMissingRequiredFlag {
		t.Errorf("want missing_required_flag, got %q", err.Code)
	}
	if err := CheckRelationships(ifm, all, set()); err != nil {
		t.Errorf("without the trigger there is no requirement, got %q", err.Code)
	}
	if err := CheckRelationships(ifm, all, set(keyURL, keyFile)); err != nil {
		t.Errorf("given, it is satisfied, got %q", err.Code)
	}
}

func TestValueConditionalRequirements(t *testing.T) {
	meta := pair(nil, nil, "")
	meta[0].RequiresIf = []ValueRequirement{{Value: "json", Key: keyStdin}}
	all := []uint64{keyFile, keyStdin, keyURL}
	values := func(value string) func(uint64) []string {
		return func(key uint64) []string {
			if key == keyFile {
				return []string{value}
			}
			return nil
		}
	}

	if err := CheckRelationshipsWithValues(meta, all, set(keyFile), values("json")); err == nil {
		t.Error("the matching value should require stdin")
	} else if err.Code != CodeMissingRequiredFlag || err.Name != "stdin" {
		t.Errorf("want missing stdin, got %q %q", err.Code, err.Name)
	}
	if err := CheckRelationshipsWithValues(meta, all, set(keyFile), values("text")); err != nil {
		t.Errorf("a different value should require nothing, got %q", err.Code)
	}
	if err := CheckRelationshipsWithValues(meta, all, set(keyFile, keyStdin), values("json")); err != nil {
		t.Errorf("the required flag satisfies the condition, got %q", err.Code)
	}

	fromDefault := func(key uint64) Source {
		if key == keyFile {
			return FromDefault
		}
		return Unset
	}
	if err := CheckRelationshipsWithValues(meta, all, fromDefault, values("json")); err != nil {
		t.Errorf("a default should not activate the condition, got %q", err.Code)
	}

	defaultSatisfies := func(key uint64) Source {
		if key == keyFile {
			return FromArgv
		}
		if key == keyStdin {
			return FromDefault
		}
		return Unset
	}
	if err := CheckRelationshipsWithValues(meta, all, defaultSatisfies, values("json")); err != nil {
		t.Errorf("a default on the required flag satisfies it, got %q", err.Code)
	}
}

func TestCompleteRelationshipFamilies(t *testing.T) {
	all := []uint64{keyFile, keyStdin, keyURL}
	values := func(key uint64) []string {
		switch key {
		case keyStdin:
			return []string{"remote"}
		case keyURL:
			return []string{"global"}
		default:
			return nil
		}
	}

	meta := pair(nil, nil, "")
	meta[0].RequiredIfEq = []ValueCondition{{Key: keyStdin, Value: "remote"}}
	if err := CheckRelationshipsWithValues(meta, all, set(keyStdin), values); err == nil {
		t.Fatal("a matching any-value condition should make file required")
	}
	meta[0].RequiredIfEq = nil
	meta[0].RequiredIfEqAll = []ValueCondition{
		{Key: keyStdin, Value: "remote"},
		{Key: keyURL, Value: "global"},
	}
	if err := CheckRelationshipsWithValues(meta, all, set(keyStdin), values); err != nil {
		t.Fatalf("one of two all-value conditions should not require file: %v", err)
	}
	if err := CheckRelationshipsWithValues(meta, all, set(keyStdin, keyURL), values); err == nil {
		t.Fatal("both all-value conditions should make file required")
	}

	meta[0].RequiredIfEqAll = nil
	meta[0].RequiredUnlessAll = []uint64{keyStdin, keyURL}
	if err := CheckRelationships(meta, all, set(keyStdin)); err == nil {
		t.Fatal("one of two required-unless-all partners is insufficient")
	}
	if err := CheckRelationships(meta, all, set(keyStdin, keyURL)); err != nil {
		t.Fatalf("every required-unless-all partner should waive file: %v", err)
	}

	meta[0].RequiredUnlessAll = nil
	meta[2].Requires = []uint64{keyFile, keyStdin}
	if err := CheckRelationships(meta, all, set(keyURL, keyFile)); err == nil {
		t.Fatal("requires_all should report the unsatisfied stdin partner")
	}
	if err := CheckRelationships(meta, all, set(keyURL, keyFile, keyStdin)); err != nil {
		t.Fatalf("requires_all should accept every satisfied partner: %v", err)
	}
}

func TestRelationshipValuesNormalizesBooleans(t *testing.T) {
	m := &Meta{RequiresIfBoolean: true}
	cases := []struct {
		name    string
		values  []string
		source  Source
		negated bool
		want    string
	}{
		{"typed flag", nil, FromArgv, false, "true"},
		{"typed negation", nil, FromArgv, true, "false"},
		{"truthy environment", []string{"TRUE"}, FromEnv, false, "true"},
		{"false environment", []string{"0"}, FromEnv, false, "false"},
	}
	for _, c := range cases {
		t.Run(c.name, func(t *testing.T) {
			got := RelationshipValues(m, c.values, c.source, c.negated)
			if len(got) != 1 || got[0] != c.want {
				t.Errorf("want %q, got %v", c.want, got)
			}
		})
	}
}

// An entry already marked Required has been answered by Check, and saying it
// twice in two different voices helps nobody.
func TestAnAlreadyRequiredEntryIsNotReportedTwice(t *testing.T) {
	meta := Metadata{
		{Key: keyFile, Name: "file", Flag: true, Required: true,
			RequiredUnless: []uint64{keyStdin}},
		{Key: keyStdin, Name: "stdin", Flag: true},
	}
	none := func(uint64) Source { return Unset }
	if err := CheckRelationships(meta, []uint64{keyFile, keyStdin}, none); err != nil {
		t.Errorf("Check owns this one, got %q", err.Code)
	}
}

// A default counts for the entry being judged and not for the partners judging
// it. Getting that backwards is silent in either direction, so both halves are
// pinned — and both were checked against usage-lib rather than reasoned about.
func TestADefaultCountsOnlyForTheEntryItHolds(t *testing.T) {
	unless := pair([]uint64{keyStdin}, nil, "required_unless")
	all := []uint64{keyFile, keyStdin, keyURL}

	// `--file` has a value, from its default, so it is not missing.
	defaulted := func(k uint64) Source {
		if k == keyFile {
			return FromDefault
		}
		return Unset
	}
	if err := CheckRelationships(unless, all, defaulted); err != nil {
		t.Errorf("a defaulted entry has a value and is not missing, got %q", err.Code)
	}

	// A defaulted *partner* does not satisfy the requirement: nobody said
	// `--stdin`, so `--file` is still required and still absent.
	partner := func(k uint64) Source {
		if k == keyStdin {
			return FromDefault
		}
		return Unset
	}
	if err := CheckRelationships(unless, all, partner); err == nil {
		t.Error("a defaulted partner should not satisfy required_unless")
	} else if err.Code != CodeMissingRequiredFlag || err.Name != "file" {
		t.Errorf("want missing file, got %q %q", err.Code, err.Name)
	}

	// Nor does a defaulted partner trigger a conflict.
	conflicts := pair([]uint64{keyStdin}, nil, "conflicts")
	both := func(k uint64) Source {
		switch k {
		case keyFile:
			return FromArgv
		case keyStdin:
			return FromDefault
		}
		return Unset
	}
	if err := CheckRelationships(conflicts, all, both); err != nil {
		t.Errorf("a defaulted partner should not conflict, got %q", err.Code)
	}
}
