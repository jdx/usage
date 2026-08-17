package argv

import "strings"

// Which flags a page offers, and under which spellings.
//
// A page should offer a spelling only where the flag it is describing is the one
// that would *bind* it. That is not a nicety: a global inherited from the root
// and a nearer command's flag can answer to the same word, and advertising the
// far one while the near one binds is a lie the reader has no way to catch.
//
// So this follows the parser's own rule exactly. `eachInScope` walks a command's
// own flags before its ancestors' — nearest first — and takes the first match;
// `longFlag` asks for every long form across the whole scope before it asks for
// any negation. Both of those show up below, and both are load-bearing.

// shownFlag is one entry of a flag section: which table entry it describes, and
// the column text it is displayed as.
type shownFlag struct {
	key   uint64
	usage string
	// supplied names a flag the parser provides rather than the spec declaring
	// it — `--help` and `--version`. They have no table entry, so they carry
	// their own help text.
	supplied     string
	suppliedHelp string
}

// shown is the spellings of one flag that a page should offer.
//
// Not "hide the long" and "hide the short": a flag may answer to several of each,
// and a descendant claiming `--jobs` leaves an inherited `--workers` working.
// What is shown is the first of each kind that nothing nearer has taken.
type shown struct {
	long     string
	hasLong  bool
	short    byte
	hasShort bool
	// negate is whether the negation is still this flag's to offer. `--no-color`
	// is a spelling like any other and something nearer can claim it.
	negate bool
}

func (s shown) nothing() bool { return !s.hasLong && !s.hasShort && !s.negate }

func allShown(f *Flag) shown {
	out := shown{negate: f.Negate != ""}
	if len(f.Longs) > 0 {
		out.long, out.hasLong = f.Longs[0], true
	}
	if len(f.Shorts) > 0 {
		out.short, out.hasShort = f.Shorts[0], true
	}
	return out
}

func formsOf(f *Flag) []string {
	out := make([]string, 0, len(f.Longs)+len(f.Shorts))
	for _, l := range f.Longs {
		out = append(out, "--"+l)
	}
	for _, s := range f.Shorts {
		out = append(out, "-"+string(s))
	}
	return out
}

func negationOf(f *Flag) string {
	if f.Negate == "" {
		return ""
	}
	return "--" + f.Negate
}

func has(list []string, s string) bool {
	for _, x := range list {
		if x == s {
			return true
		}
	}
	return false
}

// surviving is the spellings left to a flag once everything nearer has taken
// what it answers to.
func surviving(f *Flag, taken, takenNegations, everyForm []string) shown {
	var out shown
	for _, l := range f.Longs {
		if !has(taken, "--"+l) {
			out.long, out.hasLong = l, true
			break
		}
	}
	for _, s := range f.Shorts {
		if !has(taken, "-"+string(s)) {
			out.short, out.hasShort = s, true
			break
		}
	}
	if n := negationOf(f); n != "" {
		// A long anywhere in scope wins over a negation — this flag's own
		// excepted — because the parser asks for every long before any negation.
		out.negate = !has(takenNegations, n) && (!has(everyForm, n) || has(formsOf(f), n))
	}
	return out
}

// ownAndGlobal splits the flags a page shows into the command's own and the
// globals it inherits, each with the spellings still available to it.
func ownAndGlobal(chain []*Command, help HelpTable) (own, inherited []shownFlag) {
	if len(chain) == 0 {
		return nil, nil
	}
	here, ancestors := chain[len(chain)-1], chain[:len(chain)-1]

	// Every long and short anything in scope answers to, near or far: one of these
	// always beats a negation, so a negation survives only where none of them is
	// the same word.
	var everyForm []string
	for _, f := range here.Flags {
		everyForm = append(everyForm, formsOf(f)...)
	}
	for _, a := range ancestors {
		for _, f := range a.Flags {
			if f.Global {
				everyForm = append(everyForm, formsOf(f)...)
			}
		}
	}

	var taken, takenNegations []string
	for _, f := range here.Flags {
		taken = append(taken, formsOf(f)...)
		if n := negationOf(f); n != "" {
			takenNegations = append(takenNegations, n)
		}
	}

	for _, f := range here.Flags {
		if h := help.Lookup(f.Key); h != nil && h.Hide {
			continue
		}
		own = append(own, shownFlag{key: f.Key, usage: columnUsage(f, allShown(f), help)})
	}

	keep := map[*Flag]shown{}
	for i := len(ancestors) - 1; i >= 0; i-- {
		for _, f := range ancestors[i].Flags {
			if !f.Global {
				continue
			}
			show := surviving(f, taken, takenNegations, everyForm)
			// Reserved whether or not it is shown: a hidden one still binds, and so
			// does one whose every spelling something nearer already took.
			taken = append(taken, formsOf(f)...)
			if n := negationOf(f); n != "" {
				takenNegations = append(takenNegations, n)
			}
			h := help.Lookup(f.Key)
			if (h != nil && h.Hide) || show.nothing() {
				continue
			}
			keep[f] = show
		}
	}
	// In declaration order from the root down, which is the order a page lists
	// them, rather than the nearest-first order they were resolved in.
	for _, a := range ancestors {
		for _, f := range a.Flags {
			if show, ok := keep[f]; ok {
				inherited = append(inherited,
					shownFlag{key: f.Key, usage: columnUsage(f, show, help)})
			}
		}
	}

	// Last in the command's own section, which is where clap has them: they carry
	// no heading, so a CLI that groups its flags gets them at the end of the
	// ungrouped list rather than inside somebody's section.
	claimed := append(append([]string{}, taken...), takenNegations...)
	own = append(own, suppliedEntries(here, claimed)...)
	return own, inherited
}

// suppliedEntries are the flags the parser answers without the spec declaring
// them, under whichever spellings nothing else has claimed.
func suppliedEntries(cmd *Command, claimed []string) []shownFlag {
	pick := func(long string, short byte, help string) (shownFlag, bool) {
		hasLong := !has(claimed, "--"+long)
		hasShort := !has(claimed, "-"+string(short))
		switch {
		case !hasLong && !hasShort:
			return shownFlag{}, false
		case !hasLong:
			return shownFlag{supplied: long, suppliedHelp: help,
				usage: "-" + string(short)}, true
		case !hasShort:
			return shownFlag{supplied: long, suppliedHelp: help,
				usage: pad("", shortCol) + "--" + long}, true
		default:
			return shownFlag{supplied: long, suppliedHelp: help,
				usage: pad("-"+string(short)+",", shortCol) + "--" + long}, true
		}
	}

	var out []shownFlag
	if e, ok := pick("help", 'h', "Print help"); ok {
		out = append(out, e)
	}
	// Only where the parser accepts one, which is the root of a CLI that declared
	// a version.
	if cmd.Version {
		if e, ok := pick("version", 'V', "Print version"); ok {
			out = append(out, e)
		}
	}
	return out
}

// columnUsage is how a flag appears in a section's left column: the short form in
// its own four-wide column, so that the long forms line up under each other.
func columnUsage(f *Flag, show shown, help HelpTable) string {
	rest := displayUsage(f, show, help)
	if !show.hasLong {
		return rest
	}
	// Only when the text actually begins with the long form. The `name:` prefix
	// case does not, and splitting it would put `verbose:` in a column meant for
	// `-v, `.
	at := strings.Index(rest, "--"+show.long)
	if at < 0 {
		return rest
	}
	before, after := rest[:at], rest[at:]
	short := strings.TrimSpace(before)
	// Only a bare short form belongs in the short column. A flag may carry a
	// declared name the forms do not imply — `jobs: -j --parallel` — and that
	// prefix is not something to line up with a comma after.
	bare := short == "" || (strings.HasPrefix(short, "-") &&
		!strings.HasPrefix(short, "--") && width(short) == 2)
	if !bare {
		return rest
	}
	if short != "" {
		short += ","
	}
	return pad(short, shortCol) + after
}

// displayUsage is the flag's spellings and value, plus its negation where the
// page is still offering one.
func displayUsage(f *Flag, show shown, help HelpTable) string {
	usage := flagUsageShown(f, show, help.Lookup(f.Key))
	if show.negate && f.Negate != "" {
		return usage + " / --" + f.Negate
	}
	return usage
}
