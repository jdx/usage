package argv

import "strings"

// What could go where the cursor is.
//
// A completion is a parse of an unfinished command line, which is why it lives
// beside the parser rather than on top of it: the words before the cursor decide
// what may follow, and the rules that decide are the binding rules. Asking the
// parser rather than re-deriving them is what keeps what is *offered* and what is
// *accepted* from disagreeing — a completion advertising a flag the parser would
// refuse is worse than no completion at all.
//
// This is the position and the candidates. Turning candidates into the format a
// particular shell wants, and running the `complete` scripts a spec can declare,
// are separate jobs: the first is per-shell text, the second runs subprocesses,
// and neither belongs in a package whose whole claim is that it does not allocate.

// Position is what the cursor is standing in, after the words before it.
type Position struct {
	// Cmd is the command in scope: the deepest one the words selected.
	Cmd *Command
	// Chain is the commands the words passed through, root first, which is what
	// [ShortHelp] and the scope rules want.
	Chain []*Command
	// FlagsPossible is whether a dash-prefixed word here would still be read as a
	// flag. False past a `--`, and past the first value of an `automatic`
	// argument — there is no flag of *this* CLI to offer in either place.
	FlagsPossible bool
	// SubcommandsPossible is whether a word here could still name a subcommand.
	// False once a positional of this command has taken a word: the parser stops
	// descending there, so a later word matching a subcommand name is a value.
	SubcommandsPossible bool
	// AwaitingValue is a flag whose value the cursor is standing in, because the
	// last word was a flag that takes one and has not been given it. Nothing else
	// belongs here: the parser refuses a flag-like token in that place.
	AwaitingValue *Flag
	// Collecting is a variadic flag still claiming words. The next word would be
	// another of its values — so the positional after it is not offered — but a
	// flag-like token ends the collection and binds, so flags are.
	Collecting *Flag
	// NextArg is the positional a word here would fill, if any are left.
	NextArg *Arg
	// SeparatorSeen is whether a `--` has been typed. Narrower than
	// FlagsPossible, and what an argument requiring a separator is asking about.
	SeparatorSeen bool
	// HelpTopic is whether the word here names a command to *read about* rather
	// than one to run — after `help`, where nothing else belongs.
	HelpTopic bool
}

// Walk reads the words before the cursor and reports what the cursor is at.
//
// Errors are not failures here. A line being completed is by definition
// unfinished — a flag with no value yet, a word that names nothing yet — so a
// parse error means "the grammar runs out here", which is exactly the position
// being asked about. The walk stops at the first one and reports the state it
// reached, where a real parse must discard everything.
func Walk(root *Command, words []string) Position {
	p := New(root, words)
	chain := []*Command{root}
	var awaiting *Flag

	for p.Next() {
		if ev := p.Event(); ev.Kind == KindCommand {
			chain = append(chain, ev.Command)
		}
	}
	if err, ok := p.Err().(*Error); ok && err != nil {
		switch err.Code {
		// The one failure that says something about the cursor rather than about
		// the line: the last word was a flag that takes a value, so the cursor is
		// standing in it.
		case CodeMissingFlagValue:
			awaiting = err.Flag
		// `ex help config ⌶` asks which command to read about, and the answer is a
		// command under `config` — the one the help request already resolved. The
		// parser never descended into it, on purpose, so the position comes from
		// the request. Nothing else can be typed there: a topic takes no flags and
		// fills no argument.
		case CodeHelp:
			// SubcommandsPossible stays false: a topic is not descended into, and
			// the commands under it are offered by HelpTopic instead.
			return Position{Cmd: err.Cmd, Chain: chain, HelpTopic: true}
		}
	}

	return Position{
		Cmd:   p.Command(),
		Chain: chain,
		// A variadic flag still claiming words stands in the same place as a flag
		// waiting for its first value: the next word belongs to it, not to the
		// positional after it.
		AwaitingValue:       awaiting,
		Collecting:          p.Collecting(),
		FlagsPossible:       !p.FlagsStopped(),
		SubcommandsPossible: p.SubcommandsPossible(),
		NextArg:             p.PendingArg(),
		SeparatorSeen:       p.DoubleDashSeen(),
	}
}

// Kind of thing a candidate is, so a shell can decorate or filter them.
type CandidateKind uint8

const (
	// CandidateCommand is a subcommand name or alias.
	CandidateCommand CandidateKind = iota
	// CandidateFlag is a flag spelling.
	CandidateFlag
	// CandidateValue is one of a declared `choices` list.
	CandidateValue
)

// Candidate is one thing that could be typed where the cursor is.
type Candidate struct {
	Kind CandidateKind
	// Value is the text to insert.
	Value string
	// Describe is the one-line help, where there is any. A shell that can show a
	// description beside a completion uses it; one that cannot ignores it.
	Describe string
}

// Candidates is everything that could go at a position, given a partial word.
//
// `partial` is what the user has typed of the current word, and filtering happens
// here rather than in the shell so that every shell agrees about what matches.
func Candidates(pos Position, partial string, help HelpTable, meta Metadata) []Candidate {
	var out []Candidate
	add := func(kind CandidateKind, value, describe string) {
		if strings.HasPrefix(value, partial) {
			out = append(out, Candidate{Kind: kind, Value: value, Describe: describe})
		}
	}

	// A value the cursor is standing in takes the position entirely: nothing else
	// belongs where a flag is waiting for its argument, because the parser refuses
	// a flag-like token there.
	if pos.AwaitingValue != nil {
		for _, c := range choicesFor(pos.AwaitingValue.Key, meta) {
			add(CandidateValue, c, "")
		}
		return out
	}

	// Commands, and their aliases: the parser accepts either, and a completion
	// that hides an alias makes it undiscoverable. A hidden command binds and is
	// not advertised — the same rule the help pages follow, and the reason `hide`
	// exists at all.
	//
	// The same list after `help`, because `findNamed` resolves a topic by name or
	// alias exactly as it resolves a command to run.
	commands := func() {
		for _, sub := range subcommandsOf(pos.Cmd) {
			h := help.Lookup(sub.Key)
			if h != nil && h.Hide {
				continue
			}
			add(CandidateCommand, sub.Name, describe(sub.Key, help))
			if h != nil {
				for _, alias := range h.VisibleAliases {
					add(CandidateCommand, alias, describe(sub.Key, help))
				}
			}
		}
	}

	// A help topic is a question, not an invocation: only command names belong.
	if pos.HelpTopic {
		commands()
		return out
	}

	// A variadic flag that has already taken a value is a weaker claim than a flag
	// waiting for its first: another word goes to the variadic, but a flag-like
	// one ends the collection and binds. So its values are offered here *and* the
	// flags below are — what is not offered is anything a plain word could not be:
	// a subcommand, or the positional the variadic is standing in front of.
	collecting := pos.Collecting != nil
	if collecting {
		for _, c := range choicesFor(pos.Collecting.Key, meta) {
			add(CandidateValue, c, "")
		}
	}

	// Only while descent is still possible. Once a positional has taken a word the
	// parser stops matching subcommands, and a name offered there would be bound
	// as a value or refused outright.
	if pos.SubcommandsPossible && !collecting {
		commands()
	}

	// Flags, only where one could still be typed *and* the user has started typing
	// one. The reference offers no flags for a bare cursor — `ex ⌶` lists
	// subcommands, `ex -⌶` lists both forms, `ex --⌶` the longs — and checked
	// against `usage complete-word` rather than assumed. Offering them anyway made
	// every position look answered, which is also what decides whether the shell
	// should fall back to paths.
	//
	// The prefix filter below would narrow them to the same set; what this changes
	// is the empty prefix, where it would not.
	if pos.FlagsPossible && strings.HasPrefix(partial, "-") {
		for _, s := range flagsInScope(pos.Chain) {
			if h := help.Lookup(s.flag.Key); h != nil && h.Hide {
				continue
			}
			// Negations included: flagsInScope works out which spellings are still
			// this flag's, and a negation is one of them.
			for _, form := range s.forms {
				add(CandidateFlag, form, describe(s.flag.Key, help))
			}
		}
	}

	// And the values a positional will only accept — unless it is one that reads
	// only after a `--` and no separator has been typed. Offering them there
	// produces a command line the parser answers with
	// `arg_requires_double_dash`, which is the exact failure this design exists
	// to prevent.
	if pos.NextArg != nil && !collecting &&
		!(pos.NextArg.DoubleDash == DoubleDashRequired && !pos.SeparatorSeen) {
		for _, c := range choicesFor(pos.NextArg.Key, meta) {
			add(CandidateValue, c, "")
		}
	}
	return out
}

// inScope is a flag a page or a completion may offer, and the spellings still
// left to it.
type inScope struct {
	flag  *Flag
	forms []string
}

// flagsInScope is this command's own flags, then any ancestor's globals, each
// with the spellings nothing nearer has taken.
//
// Per spelling, not per flag. A flag answers to several forms, and a nearer
// command reclaiming `--jobs` leaves an inherited `-j` and `--workers` binding —
// dropping the whole inherited flag would hide spellings the parser still
// accepts. That is the same rule the help pages follow, for the same reason.
func flagsInScope(chain []*Command) []inScope {
	if len(chain) == 0 {
		return nil
	}
	everyForm := everyFormInScope(chain)
	var taken, takenNegations []string
	var out []inScope

	// Nearest first, which is the order the parser resolves in, so that "nothing
	// nearer has taken it" is just "not seen yet".
	offer := func(f *Flag) {
		var left []string
		for _, form := range formsOf(f) {
			if !has(taken, form) {
				left = append(left, form)
			}
		}
		// A negation is a spelling like any other, and it loses to a long anywhere
		// in scope rather than only to a nearer one — see negationSurvives, which
		// the pages use for the same decision.
		//
		// Not twice, though. A flag may spell its negation the same as its own long
		// — `flag "--no-color" negate="--no-color"` — and then it is already in the
		// list. usage-lib prints that flag as `--no-color / --no-color`, so the page
		// says it twice on purpose and matching the reference means keeping that;
		// a completion is a list of things to type, and the same thing twice is
		// just a repeated row.
		// `!has(taken, n)` before the exemption inside negationSurvives: that
		// exemption is for a flag whose *own* long is spelled like its negation,
		// and it must not reach past something nearer that has claimed the word.
		// A child declaring `--x` takes it from an inherited global that answers to
		// `--x` both ways.
		if n := negationOf(f); n != "" && !has(left, n) && !has(taken, n) &&
			negationSurvives(f, n, takenNegations, everyForm) {
			left = append(left, n)
		}
		// Claimed whether or not anything is left: a spelling this flag answers to
		// is not available to something farther away either.
		taken = append(taken, formsOf(f)...)
		if n := negationOf(f); n != "" {
			takenNegations = append(takenNegations, n)
		}
		if len(left) > 0 {
			out = append(out, inScope{flag: f, forms: left})
		}
	}

	for _, f := range chain[len(chain)-1].Flags {
		offer(f)
	}
	for i := len(chain) - 2; i >= 0; i-- {
		for _, f := range chain[i].Flags {
			if f.Global {
				offer(f)
			}
		}
	}
	return out
}

func subcommandsOf(cmd *Command) []*Command {
	if cmd == nil {
		return nil
	}
	return cmd.Subcommands
}

func choicesFor(key uint64, meta Metadata) []string {
	if m := meta.Lookup(key); m != nil {
		return m.Choices
	}
	return nil
}

func describe(key uint64, help HelpTable) string {
	if h := help.Lookup(key); h != nil {
		// Whole, breaks included. Putting a description on one line is the
		// renderer's job — see oneLine — because it is the line-based protocols
		// that need it, and collapsing there keeps both halves of a two-line
		// description instead of dropping the second.
		return h.Short
	}
	return ""
}
