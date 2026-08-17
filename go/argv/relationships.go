package argv

// The rules that compare one entry against another.
//
// Everything in post.go judges an entry on its own: is it there, is its value
// allowed, are there enough of them. These four need a second entry to answer at
// all, which is why they are separate — a name in the declaration has to be
// resolved to the entry it refers to before any of it can be checked, and that
// resolution happens where the whole command is visible rather than here.
//
// `overrides` is the odd one. The other three are decided once the last token has
// been read; this one asks which of two flags came *last*, which only the
// arriving tokens know. So it is applied first, on the order binding reports, and
// what it removes is removed before anything else looks.

// ApplyOverrides decides last-one-wins between flags declared to override each
// other, returning the keys that lost.
//
// `order` gives the position of each key's last occurrence on the command line.
// A key absent from it was not typed, and cannot win or lose: the declaration is
// about which of two *given* flags survives.
//
// A loser must be treated as absent by everything downstream, and in particular
// must not be refilled from `env` or `default`. Filling it afterwards would leave
// both flags standing and undo the last-one-wins the user asked for by typing the
// second one.
//
// The relationship is symmetric however it was declared. `--file overrides
// --stdin` establishes the pair; it does not mean `--file` always wins. The
// corpus pins that directly: with `--file` declaring it and `--stdin` typed last,
// `--file` is the one that loses.
func ApplyOverrides(meta Metadata, order map[uint64]int) map[uint64]bool {
	if len(order) < 2 {
		return nil
	}
	var lost map[uint64]bool
	drop := func(key uint64) {
		if lost == nil {
			lost = map[uint64]bool{}
		}
		lost[key] = true
	}

	for key, at := range order {
		m := meta.Lookup(key)
		if m == nil {
			continue
		}
		for _, other := range m.Overrides {
			otherAt, given := order[other]
			if !given {
				continue
			}
			// Equal positions cannot happen — two flags cannot share a token — but
			// were it ever to, dropping neither is the safer answer than dropping
			// both and losing the value entirely.
			if otherAt < at {
				drop(other)
			} else if at < otherAt {
				drop(key)
			}
		}
	}
	return lost
}

// CheckRelationships verifies the rules that read one entry's state to judge
// another, once every entry's final state is known.
//
// `entries` is every key in scope, so each declaration is visited once, and
// `isSet` reports whether an entry counts as *given* — see [Source.Given], which
// is what a caller should use to answer it.
//
// The command line and the environment count; a default does not. That asymmetry
// is the part worth getting right: `conflicts` asks whether a flag has a value
// rather than how it got one, so an environment variable counts on both sides
// and the corpus pins the one-sided and neither-side-typed cases. But a default
// is a fallback rather than something the user said, and counting it would make
// a defaulted flag conflict with every partner anyone types. usage-lib and clap
// both draw the line there.
//
// A key removed by [ApplyOverrides] should not appear in `entries` at all: it
// lost, so it is out of the running rather than merely absent.
func CheckRelationships(meta Metadata, entries []uint64, isSet func(uint64) bool) *Error {
	for _, key := range entries {
		m := meta.Lookup(key)
		if m == nil {
			continue
		}

		if isSet(key) {
			for _, other := range m.Conflicts {
				if isSet(other) {
					// Both names, because either alone reads as a puzzle: which flag
					// is unwelcome depends entirely on what else was given.
					return &Error{
						Code:  CodeConflictingFlags,
						Name:  m.Name,
						Other: nameOf(meta, other),
					}
				}
			}
			continue
		}

		// Absent, so the two conditional requirements are the question. Both are
		// skipped where the entry is already `Required`, since that has been
		// answered by Check and reporting it twice helps nobody.
		if m.Required {
			continue
		}

		// Required unless one of these is present. With none of them present the
		// requirement stands.
		if len(m.RequiredUnless) > 0 && !anySet(m.RequiredUnless, isSet) {
			return missingRequired(m)
		}

		// Required because one of these is present.
		if len(m.RequiredIf) > 0 && anySet(m.RequiredIf, isSet) {
			return missingRequired(m)
		}
	}
	return nil
}

func missingRequired(m *Meta) *Error {
	code := CodeMissingRequiredArg
	if m.Flag {
		code = CodeMissingRequiredFlag
	}
	return &Error{Code: code, Name: m.Name}
}

func anySet(keys []uint64, isSet func(uint64) bool) bool {
	for _, k := range keys {
		if isSet(k) {
			return true
		}
	}
	return false
}

// nameOf renders the other side of a relationship, falling back to nothing rather
// than to a number: an error naming `key 7` is worse than one naming only the
// flag the reader already knows about.
func nameOf(meta Metadata, key uint64) string {
	if m := meta.Lookup(key); m != nil {
		return m.Name
	}
	return ""
}
