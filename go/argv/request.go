package argv

import "strings"

// The hidden command a shell calls, and what it answers.
//
// A completion is not something the CLI runs. It is recognized before the parse
// rather than inside it: putting it in the tables would make it a command —
// visible to the grammar, to the help, to the spec — and every CLI would grow a
// subcommand nobody typed.
//
// The request is usage-argv's, spelled the same way on purpose:
//
//	mycli __complete_word__ --shell zsh --line "mycli install no"
//
// One convention, so a shell script written for either framework says the same
// thing, and so a spec's `complete "x" run="mycli __complete_word__ …"` means one
// thing whichever language answers it.

// RequestName is the argument that marks a completion request.
//
// Long and ugly on purpose: it is typed by a script, never by a person, and it
// has to be a word no CLI would want for itself.
const RequestName = "__complete_word__"

// Request is a completion request as a shell sent it.
type Request struct {
	Shell Shell
	// Line is the command line as typed, and Cursor a byte offset into it.
	Line   string
	Cursor int
}

// ParseRequest reads a completion request out of argv, and reports whether this
// was one at all.
//
// `argv` is what the program was given, without its own name — the same slice
// [New] takes, so a caller asks this first and parses only if the answer is no.
//
// The flags are read by hand. There are three, and reading them with the parser
// would mean putting them in the tables this is deliberately outside of. Anything
// unrecognized is ignored rather than refused: a completion that errors out is a
// shell that beeps at every keystroke.
func ParseRequest(argv []string) (Request, bool) {
	if len(argv) == 0 || argv[0] != RequestName {
		return Request{}, false
	}
	req := Request{Shell: Bash, Cursor: -1}
	for i := 1; i < len(argv); i++ {
		switch argv[i] {
		case "--shell":
			if i+1 < len(argv) {
				i++
				if shell, ok := ShellNamed(argv[i]); ok {
					req.Shell = shell
				}
			}
		case "--line":
			if i+1 < len(argv) {
				i++
				req.Line = argv[i]
			}
		case "--cursor":
			if i+1 < len(argv) {
				i++
				if n, err := atoi(argv[i]); err == nil {
					req.Cursor = n
				}
			}
		}
	}
	// No cursor means the end of the line, which is where a shell puts it when it
	// has no way to say — nushell, whose completer only ever sees the words.
	if req.Cursor < 0 {
		req.Cursor = len(req.Line)
	}
	return req, true
}

// Answer works out what could be typed at the cursor.
//
// The candidates come from the tables, and so does the question of whether paths
// belong here — a shell can complete those itself, and asking it to is how a CLI
// says "anything at all goes here" without listing the filesystem.
func (r Request) Answer(root *Command, help HelpTable, meta Metadata) Answer {
	split := Split(r.Line, r.Cursor, r.Shell)
	pos := Walk(root, split.Argv())
	candidates := Candidates(pos, split.Prefix, help, meta)
	return Answer{Candidates: candidates, Files: filesAt(pos, split, candidates, meta)}
}

// Respond is the whole of what a CLI has to do: recognize the request, answer it,
// and write the text its shell reads.
//
// Returns false where argv is an ordinary invocation, which is the caller's cue
// to parse it as one.
func Respond(argv []string, root *Command, help HelpTable, meta Metadata) (string, bool) {
	req, ok := ParseRequest(argv)
	if !ok {
		return "", false
	}
	return RenderAnswer(req.Answer(root, help, meta), req.Shell), true
}

// filesAt decides whether the shell should offer paths here as well.
//
// Four questions, in the order the reference asks them:
//
//   - A dash-prefixed word is a flag or nothing. No path starts with one.
//   - An argument that is not readable until after a `--` is not fillable yet, so
//     nothing belongs there — not even a path, which the parser would refuse
//     exactly as it refuses a value.
//   - Did the spec say what this position takes? A `complete` block naming a type
//     answers outright, and so does a name like `<FILE>` or `<DIR>`: the reference
//     resolves a completer by name and falls back to reading the name as the type.
//   - Otherwise: is the position *closed*? Something was offered, or the entry
//     declares its own set, in which case an unmatched prefix means "no matches"
//     rather than "ask somebody else". Offering the working directory for a
//     mistyped choice answers the second as though it were the first.
func filesAt(pos Position, split SplitLine, candidates []Candidate, meta Metadata) Files {
	if pos.FlagsPossible && strings.HasPrefix(split.Prefix, "-") {
		return NoFiles
	}
	// Only when the cursor is *at* that argument. A flag taking a value is a
	// different position that happens to have an unfilled positional behind it,
	// and a rule about the positional has nothing to say about the flag: `ex
	// --from ⌶` takes a path whatever the argument after it needs. A variadic
	// still collecting is the same position by a weaker claim, and the reference
	// folds the two together here.
	if pos.AwaitingValue == nil && pos.Collecting == nil && pos.NextArg != nil &&
		pos.NextArg.DoubleDash == DoubleDashRequired && !pos.SeparatorSeen {
		return NoFiles
	}

	var m *Meta
	named := ""
	switch {
	case pos.AwaitingValue != nil:
		m = meta.Lookup(pos.AwaitingValue.Key)
		named = pos.AwaitingValue.Name
	case pos.Collecting != nil:
		m = meta.Lookup(pos.Collecting.Key)
		named = pos.Collecting.Name
	case pos.NextArg != nil:
		m = meta.Lookup(pos.NextArg.Key)
		named = pos.NextArg.Name
	}
	if m != nil {
		if m.ValueName != "" {
			named = m.ValueName
		}
		if asked := filesFor(m.CompleteType); asked != NoFiles {
			return asked
		}
	}
	if asked := filesFor(named); asked != NoFiles {
		return asked
	}

	declaresChoices := m != nil && (len(m.Choices) > 0 || len(m.AcceptedChoices) > 0)
	if len(candidates) > 0 || declaresChoices || pos.HelpTopic {
		return NoFiles
	}
	return AnyFile
}

// filesFor is the paths a name asks for, by the name itself.
//
// The same names the reference reads, compared without case: an argument called
// `<FILE>` completes files and `<DIR>` directories without a spec saying so.
func filesFor(name string) Files {
	switch {
	case name == "":
		return NoFiles
	case strings.EqualFold(name, "file"), strings.EqualFold(name, "path"),
		strings.EqualFold(name, "config_file"):
		return AnyFile
	case strings.EqualFold(name, "dir"), strings.EqualFold(name, "directory"):
		return Dirs
	}
	return NoFiles
}

// atoi is [strconv.Atoi] for a non-negative number, written out because this
// package takes no imports it can avoid.
//
// A number too large to hold is refused rather than wrapped. Wrapping is worse
// than refusing here: a cursor that came back as a small number would describe a
// position near the start of the line, and the completion would answer confidently
// about the wrong word — where a refusal falls back to the end of the line, which
// is where a shell means when it says nothing.
func atoi(s string) (int, error) {
	const maxInt = int(^uint(0) >> 1)
	if s == "" {
		return 0, errNotANumber
	}
	n := 0
	for i := 0; i < len(s); i++ {
		if s[i] < '0' || s[i] > '9' {
			return 0, errNotANumber
		}
		digit := int(s[i] - '0')
		if n > (maxInt-digit)/10 {
			return 0, errNotANumber
		}
		n = n*10 + digit
	}
	return n, nil
}

type numberError struct{}

func (numberError) Error() string { return "not a number" }

var errNotANumber = numberError{}
