package argv

import (
	"fmt"
	"os"
	"strings"

	"github.com/expr-lang/expr"
)

// The rules that are decided once the last token has been read.
//
// Binding says which token becomes which flag or argument. These say whether
// what landed is acceptable, and fill in what the command line left empty — and
// every one of them needs to know something about a value that no single token
// can tell you. `required` needs to know nothing was given anywhere, `var_min`
// needs the final count, `choices` needs the declared list. So they live here,
// off the hot path, reading a second table the parser never touches.
//
// Deliberately not a framework. These are pure functions over what binding
// produced, because the caller is the one that knows how it accumulated: a
// generated struct assigns to a field, and a harness with no target type appends
// to a slice. Inventing a value model here would force both of them through it.
//
// The split is the same one usage-argv makes in Rust, and the reason is the same:
// a successful parse never reaches this file, so nothing here is in front of the
// parser.

// Meta is the cold half of a flag or argument's declaration.
//
// Everything binding deliberately does not know. A generated table holds one of
// these per entry, indexed by [Meta.Key] — see [Metadata] — and a program that
// never applies the rules never touches them.
type Meta struct {
	// Key matches the [Flag.Key] or [Arg.Key] this describes, so the two tables
	// cannot drift apart on identity even though they are separate data.
	Key uint64
	// Name is what the spec calls it, for the error.
	Name string
	// Spelling is how a user types it — `--file`, `-f` — for the errors raised
	// here, which judge an *entry* and so never see a [Flag].
	//
	// Carried rather than derived from the name: a name is a long form wherever
	// there is one, but `--a` and `-a` are both one character and guessing
	// between them can name a different flag entirely. Empty for an argument,
	// which is typed as its value rather than as a form.
	Spelling string
	// ValueName is what a flag's value is called — the `DIR` of `--into <DIR>` —
	// and empty where the entry is an argument, which is named by its value
	// already. Read by completion rather than by any rule here: what a value is
	// called is what says whether a path belongs there.
	ValueName string
	// CompleteType is the type a spec's `complete` block names for this entry,
	// where it names one. Also completion's, and carried for the same reason: an
	// author who wrote `complete "input" type="file"` said what the position
	// takes, and the alternative is inferring it from a name they did not choose.
	CompleteType string
	// Flag distinguishes a missing flag from a missing argument, which the
	// grammar reports as different classes.
	Flag bool
	// RequiresIfBoolean says this entry's conditional relationships compare a
	// boolean rather than text, so their explicit values need normalization.
	RequiresIfBoolean bool
	// Required means it must end up with a value, from anywhere.
	Required bool
	// Choices is the visible set shown in diagnostics.
	Choices []string
	// AcceptedChoices also includes hidden values and aliases.
	AcceptedChoices []string
	IgnoreCase      bool
	// Default fills in when neither the command line nor the environment did.
	Default []string
	// Env names an environment variable to fall back to. Empty means none.
	Env string
	// VarMin is the fewest values a variadic may end up with. Zero means no
	// bound. It is a check rather than a limit, because nothing about a single
	// word tells you a variadic will end up short.
	VarMin uint32
	// VarMax is the most times a repeatable flag may be given. Zero means no
	// bound.
	//
	// Occurrences, not values. A variadic's per-occurrence bound is a limit that
	// binding applies, and lives on [Flag.VarMax] and [Arg.VarMax] instead — a
	// value bound here would fail an invocation that never broke it.
	VarMax uint32
	// Validate is a portable expr expression evaluated once for each raw value.
	// The environment contains one string variable, `value`.
	Validate string
	// ValidateError is reported when Validate returns false. Empty uses the
	// runtime's generic validation message.
	ValidateError string

	// The relationships that need a second entry to answer, all resolved to keys rather
	// than left as the names the spec writes. Resolution happens where the whole
	// command is visible — the generator, or the table builder — so that nothing
	// downstream has to search by name, and a declaration naming a flag that does
	// not exist is caught there rather than silently doing nothing.
	// See relationships.go.

	// Conflicts names entries this one cannot be given alongside.
	Conflicts []uint64
	// Overrides names entries this one is mutually exclusive with, resolved by
	// whichever was given last rather than reported as a mistake.
	Overrides []uint64
	// RequiredUnless makes this required when none of them is present.
	RequiredUnless    []uint64
	RequiredUnlessAll []uint64
	// RequiredIf makes this required when any of them is present.
	RequiredIf      []uint64
	RequiredIfEq    []ValueCondition
	RequiredIfEqAll []ValueCondition
	// Requires names entries that must be satisfied when this one is given.
	Requires []uint64
	// RequiresIf names entries required when this entry explicitly has Value.
	// Defaults do not activate the condition, but may satisfy the requirement.
	RequiresIf []ValueRequirement
	// DefaultIf binds Value when another entry matches. First match wins.
	// Two-argument form (When empty) is presence; When set is equality.
	// An applied DefaultIf is a default, not an explicit value.
	DefaultIf []DefaultIf
}

// ValueRequirement is one value-conditional relationship.
type ValueRequirement struct {
	Value string
	Key   uint64
}

type ValueCondition struct {
	Key   uint64
	Value string
}

// DefaultIf is one conditional default, resolved to the selector's key.
type DefaultIf struct {
	Key   uint64
	When  string
	Value string
}

// Metadata is the cold table, indexed by key.
//
// Keys are dense from 1, so entry `Key` sits at `Metadata[Key-1]` and a lookup is
// an index rather than a map — which also keeps the table static data the linker
// can lay out, where a Go map would need building at init.
type Metadata []Meta

// Lookup returns the metadata for a key, or nil if the table has none.
func (m Metadata) Lookup(key uint64) *Meta {
	if key == 0 || key > uint64(len(m)) {
		return nil
	}
	entry := &m[key-1]
	if entry.Key != key {
		// A table built by hand, or one that got out of step with the parse
		// tables. Searching would paper over it; reporting nothing makes the
		// caller's own test fail instead.
		return nil
	}
	return entry
}

// Source says where a value came from, which callers need because the rules
// distinguish them: an `overrides` loser is not refilled from the environment,
// and a value that arrived from `env` is still checked against `choices`.
type Source uint8

const (
	// FromArgv means the command line supplied it.
	FromArgv Source = iota
	// FromEnv means the environment did.
	FromEnv
	// FromDefault means neither did, and the declaration filled it in.
	FromDefault
	// Unset means nothing supplied it.
	Unset
)

// Given reports whether a source counts as the flag having been *given*, which
// is the question the rules comparing two entries ask.
//
// A default does not count, and that is the whole reason this exists. usage-lib
// and clap both treat `env` as a value source and a default as a fallback: with
// `--file` defaulted and only `--stdin` typed, a declared conflict between them
// does not fire. Counting the default would make a defaulted flag conflict with
// every partner anyone types, which is a CLI nobody can use.
func (s Source) Given() bool { return s == FromArgv || s == FromEnv }

// Fill applies the fallbacks: command line, then environment, then default.
//
// `given` is what binding produced, and nil means the command line said nothing —
// which is distinct from a flag given an empty value, since `--jobs=` binds the
// empty string and that is a value.
//
// `lookupEnv` is passed in rather than read from the process, so that a caller
// testing a parse is not testing the machine it runs on. [LookupEnv] wraps
// os.LookupEnv for callers that do want the process environment.
//
// An environment variable set to the empty string is set. Treating empty as unset
// would make `EX_JOBS=` mean something no other empty value in the grammar means.
func Fill(m *Meta, given []string, lookupEnv func(string) (string, bool)) ([]string, Source) {
	if given != nil {
		return given, FromArgv
	}
	if m == nil {
		return nil, Unset
	}
	if m.Env != "" && lookupEnv != nil {
		if v, ok := lookupEnv(m.Env); ok {
			// One token. The grammar never re-splits a value on whitespace —
			// quoting is the shell's job and there was no shell here at all.
			return []string{v}, FromEnv
		}
	}
	// Conditional defaults need sibling state, which this function cannot see.
	// Leave the entry unset so [ApplyDefaultIf] can choose, then fall back to
	// Default there as well.
	if len(m.DefaultIf) > 0 {
		return nil, Unset
	}
	if len(m.Default) > 0 {
		return m.Default, FromDefault
	}
	return nil, Unset
}

// ApplyDefaultIf fills entries still unset after [Fill], using sibling state.
//
// First matching DefaultIf wins; an unconditional Default applies only when
// none did. Mutates `filled` and `sources` in place. A no-op when nothing
// declares DefaultIf, so generated parsers can always call it.
//
// `negated` is which flags arrived as their negate form (`--no-json`). An Equals
// DefaultIf on a bool reads that as "false", the same way [RelationshipValues]
// does for requires_if. A nil map is all false.
func ApplyDefaultIf(meta Metadata, scope []uint64, filled map[uint64][]string, sources map[uint64]Source, negated map[uint64]bool) {
	if filled == nil || sources == nil {
		return
	}
	for _, key := range scope {
		if sources[key] != Unset {
			continue
		}
		m := meta.Lookup(key)
		if m == nil {
			continue
		}
		applied := false
		for i := range m.DefaultIf {
			if defaultIfMatches(&m.DefaultIf[i], filled, sources, meta, negated) {
				filled[key] = []string{m.DefaultIf[i].Value}
				sources[key] = FromDefault
				applied = true
				break
			}
		}
		if !applied && len(m.Default) > 0 {
			filled[key] = m.Default
			sources[key] = FromDefault
		}
	}
}

func defaultIfMatches(cond *DefaultIf, filled map[uint64][]string, sources map[uint64]Source, meta Metadata, negated map[uint64]bool) bool {
	src := sources[cond.Key]
	if !src.Given() {
		return false
	}
	if cond.When == "" {
		return true
	}
	values := explicitRelationshipValues(meta.Lookup(cond.Key), filled[cond.Key], src, negated[cond.Key])
	for _, value := range values {
		if value == cond.When {
			return true
		}
	}
	return false
}

func explicitRelationshipValues(m *Meta, values []string, source Source, negated bool) []string {
	if m != nil && m.Flag && m.ValueName == "" && len(m.Choices) == 0 {
		return RelationshipValues(&Meta{RequiresIfBoolean: true}, values, source, negated)
	}
	if len(values) == 0 && source.Given() && m != nil && m.Flag {
		return RelationshipValues(&Meta{RequiresIfBoolean: true}, values, source, negated)
	}
	return values
}

// Check applies the rules that judge what ended up bound.
//
// `values` is the result of [Fill], and `occurrences` is how many times a
// repeatable flag was given — which is not `len(values)`, since one occurrence of
// a variadic can bring several. Pass 0 for an argument.
//
// The first failure is returned; a caller wanting all of them should call this
// per entry, which it is doing anyway.
func Check(m *Meta, values []string, occurrences int) *Error {
	if m == nil {
		return nil
	}

	// `occurrences` is what makes a value-less flag work here: it has no values to
	// count, so "was it given" is the only question, and the answer is that it was
	// seen at least once.
	if m.Required && len(values) == 0 && occurrences == 0 {
		// Two classes, because the grammar reports them separately: what is
		// missing reads differently for a flag nobody typed than for an argument
		// the command needed.
		code := CodeMissingRequiredArg
		if m.Flag {
			code = CodeMissingRequiredFlag
		}
		return &Error{Code: code, Name: m.Name, Spelling: m.Spelling}
	}

	accepted := m.AcceptedChoices
	if len(accepted) == 0 {
		accepted = m.Choices
	}
	if len(accepted) > 0 {
		// Every value, not just the first: a variadic can be given a good value
		// and a bad one, and so can a repeatable flag across occurrences.
		for _, v := range values {
			if !containsChoice(accepted, v, m.IgnoreCase) {
				return &Error{Code: CodeInvalidChoice, Name: m.Name,
					Spelling: m.Spelling, Choices: m.Choices}
			}
		}
	}

	if m.Validate != "" {
		program, err := expr.Compile(m.Validate, expr.Env(map[string]any{"value": ""}))
		if err != nil {
			return &Error{Code: CodeInvalidValue, Name: m.Name, Spelling: m.Spelling,
				Reason: "validation expression failed: " + err.Error()}
		}
		for _, value := range values {
			result, err := expr.Run(program, map[string]any{"value": value})
			if err != nil {
				return &Error{Code: CodeInvalidValue, Name: m.Name, Spelling: m.Spelling,
					Value: value, Reason: "validation expression failed: " + err.Error()}
			}
			valid, ok := result.(bool)
			if !ok {
				return &Error{Code: CodeInvalidValue, Name: m.Name, Spelling: m.Spelling,
					Value: value, Reason: fmt.Sprintf("validation expression must return a boolean, got %T", result)}
			}
			if !valid {
				reason := m.ValidateError
				if reason == "" {
					reason = "does not satisfy the validation expression"
				}
				return &Error{Code: CodeInvalidValue, Name: m.Name, Spelling: m.Spelling,
					Value: value, Reason: reason}
			}
		}
	}

	// Only where something was given. An absent optional variadic has not broken
	// its minimum; it simply is not there, and reporting `var_too_few` for it
	// would make every bounded variadic effectively required.
	if m.VarMin > 0 && len(values) > 0 && uint32(len(values)) < m.VarMin {
		return &Error{Code: CodeVarTooFew, Name: m.Name, Spelling: m.Spelling,
			Bound: m.VarMin, Got: len(values)}
	}

	if m.VarMax > 0 && occurrences > int(m.VarMax) {
		return &Error{Code: CodeVarTooMany, Name: m.Name, Spelling: m.Spelling,
			Bound: m.VarMax, Got: occurrences}
	}

	return nil
}

// EnvTruth reports whether an environment value sets a flag that holds no value.
//
// A flag with no value has nowhere to put the text, so the variable has to be
// read as a yes or a no — and `EX_VERBOSE=0` meaning "verbose" would be a trap.
//
// An allow-list rather than a not-falsy test, matching usage-lib exactly, which
// means `yes`, `on` and `TrUe` are all false. That is worth knowing rather than
// discovering: the corpus pins `1`, `true`, `false` and `0`, and the rest of the
// list is here so the two implementations cannot drift on the cases it does not.
func EnvTruth(value string) bool {
	switch value {
	case "1", "true", "True", "TRUE":
		return true
	}
	return false
}

func containsChoice(list []string, s string, ignoreCase bool) bool {
	for _, x := range list {
		if x == s || ignoreCase && isASCII(x) && isASCII(s) && strings.EqualFold(x, s) {
			return true
		}
	}
	return false
}

func isASCII(s string) bool {
	for i := 0; i < len(s); i++ {
		if s[i] >= 0x80 {
			return false
		}
	}
	return true
}

// LookupEnv reads the process environment, for callers that want it.
//
// [Fill] takes the lookup as a parameter rather than reading the environment
// itself, so that a test of a parse is not a test of the machine it runs on.
// Generated code passes this, because a real CLI does want the real environment.
func LookupEnv(name string) (string, bool) { return os.LookupEnv(name) }
