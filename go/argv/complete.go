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
	// AwaitingValue is a flag whose value the cursor is standing in, if the last
	// word was one that takes a value or a variadic still claiming words.
	AwaitingValue *Flag
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
			return Position{Cmd: err.Cmd, Chain: chain, HelpTopic: true}
		}
	}

	return Position{
		Cmd:   p.Command(),
		Chain: chain,
		// A variadic flag still claiming words stands in the same place as a flag
		// waiting for its first value: the next word belongs to it, not to the
		// positional after it.
		AwaitingValue: firstFlag(awaiting, p.Collecting()),
		FlagsPossible: !p.FlagsStopped(),
		NextArg:       p.PendingArg(),
		SeparatorSeen: p.DoubleDashSeen(),
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
	// belongs where a flag is waiting for its argument.
	if pos.AwaitingValue != nil {
		for _, c := range choicesFor(pos.AwaitingValue.Key, meta) {
			add(CandidateValue, c, "")
		}
		return out
	}

	// A help topic is a question, not an invocation: only command names belong.
	if pos.HelpTopic {
		for _, sub := range subcommandsOf(pos.Cmd) {
			add(CandidateCommand, sub.Name, describe(sub.Key, help))
		}
		return out
	}

	if pos.Cmd != nil {
		for _, sub := range subcommandsOf(pos.Cmd) {
			add(CandidateCommand, sub.Name, describe(sub.Key, help))
			// Aliases too: a completion that hides them makes them undiscoverable,
			// and the parser accepts them either way. Hidden ones stay hidden.
			if h := help.Lookup(sub.Key); h != nil {
				for _, alias := range h.VisibleAliases {
					add(CandidateCommand, alias, describe(sub.Key, help))
				}
			}
		}
	}

	// Flags, only where one could still be typed, and taken from the parser's own
	// scope so that shadowing is respected: a subcommand redeclaring an inherited
	// name offers its own.
	if pos.FlagsPossible {
		for _, f := range flagsInScope(pos.Chain) {
			if h := help.Lookup(f.Key); h != nil && h.Hide {
				continue
			}
			for _, long := range f.Longs {
				add(CandidateFlag, "--"+long, describe(f.Key, help))
			}
			for _, short := range f.Shorts {
				add(CandidateFlag, "-"+string(short), describe(f.Key, help))
			}
			if f.Negate != "" {
				add(CandidateFlag, "--"+f.Negate, describe(f.Key, help))
			}
		}
	}

	// And the values a positional will only accept.
	if pos.NextArg != nil {
		for _, c := range choicesFor(pos.NextArg.Key, meta) {
			add(CandidateValue, c, "")
		}
	}
	return out
}

// flagsInScope is this command's own flags, then any ancestor's globals — the
// order the parser looks in, so a redeclared name shadows the inherited one.
func flagsInScope(chain []*Command) []*Flag {
	if len(chain) == 0 {
		return nil
	}
	here := chain[len(chain)-1]
	out := append([]*Flag{}, here.Flags...)
	seen := map[string]bool{}
	for _, f := range here.Flags {
		for _, form := range formsOf(f) {
			seen[form] = true
		}
	}
	for i := len(chain) - 2; i >= 0; i-- {
		for _, f := range chain[i].Flags {
			if !f.Global {
				continue
			}
			taken := false
			for _, form := range formsOf(f) {
				if seen[form] {
					taken = true
				}
			}
			if taken {
				continue
			}
			for _, form := range formsOf(f) {
				seen[form] = true
			}
			out = append(out, f)
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
	h := help.Lookup(key)
	if h == nil {
		return ""
	}
	// The first line only: a shell shows one line beside a candidate, and a
	// description that wraps turns a completion menu into a wall.
	if at := strings.IndexByte(h.Short, '\n'); at >= 0 {
		return h.Short[:at]
	}
	return h.Short
}

func firstFlag(flags ...*Flag) *Flag {
	for _, f := range flags {
		if f != nil {
			return f
		}
	}
	return nil
}
