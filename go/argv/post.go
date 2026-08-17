package argv

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
	// Flag distinguishes a missing flag from a missing argument, which the
	// grammar reports as different classes.
	Flag bool
	// Required means it must end up with a value, from anywhere.
	Required bool
	// Choices is the exact set of values allowed. Matching is case-sensitive:
	// case-insensitive matching would have to be declared rather than assumed.
	Choices []string
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

	// The four that need a second entry to answer, all resolved to keys rather
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
	RequiredUnless []uint64
	// RequiredIf makes this required when any of them is present.
	RequiredIf []uint64
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
	if len(m.Default) > 0 {
		return m.Default, FromDefault
	}
	return nil, Unset
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
		return &Error{Code: code, Name: m.Name}
	}

	if len(m.Choices) > 0 {
		// Every value, not just the first: a variadic can be given a good value
		// and a bad one, and so can a repeatable flag across occurrences.
		for _, v := range values {
			if !contains(m.Choices, v) {
				return &Error{Code: CodeInvalidChoice, Name: m.Name, Choices: m.Choices}
			}
		}
	}

	// Only where something was given. An absent optional variadic has not broken
	// its minimum; it simply is not there, and reporting `var_too_few` for it
	// would make every bounded variadic effectively required.
	if m.VarMin > 0 && len(values) > 0 && uint32(len(values)) < m.VarMin {
		return &Error{Code: CodeVarTooFew, Name: m.Name, Bound: m.VarMin, Got: len(values)}
	}

	if m.VarMax > 0 && occurrences > int(m.VarMax) {
		return &Error{Code: CodeVarTooMany, Name: m.Name, Bound: m.VarMax, Got: occurrences}
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

func contains(list []string, s string) bool {
	for _, x := range list {
		if x == s {
			return true
		}
	}
	return false
}
