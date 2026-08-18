package argv

// Parser reads a command line once, left to right, against static tables.
//
// There is no backtracking, no reordering, and no second pass: what a token binds
// to is decided when it is read, from the command in scope at that moment. That
// is what makes the grammar a single loop, and also why a -- or a subcommand word
// changes the meaning of everything after it and nothing before it.
//
// Use it as a scanner:
//
//	p := argv.New(root, os.Args[1:])
//	for p.Next() {
//		switch ev := p.Event(); ev.Kind {
//		case argv.KindCommand:
//			// ev.Command was selected
//		case argv.KindFlag:
//			// ev.Flag was given, with ev.Value if ev.HasValue
//		case argv.KindArg:
//			// ev.Value filled ev.Arg
//		}
//	}
//	if err := p.Err(); err != nil {
//		// binding failed, or help was asked for
//	}
//
// A Parser holds everything it needs inline, so a parse reaches the allocator
// zero times — on success and on failure alike. Keep it on the stack (New returns
// it by value) and Go will not heap-allocate it either.
type Parser struct {
	argv []string
	// pos is the index of the next token to read.
	pos int
	// cmd is the command currently in scope.
	cmd *Command
	// ancestors is the chain above cmd, used to find inherited global flags.
	// Fixed size so that nothing is allocated.
	ancestors [MaxDepth]*Command
	// starts records where each ancestor's own words began, in step with
	// ancestors.
	starts [MaxDepth]int
	depth  int
	// bundle is the bytes left in a short-flag bundle, if one is partly read.
	bundle string
	// bundleToken is the whole token the current bundle came from, so an error
	// raised part way through it can still name what the user typed.
	bundleToken string
	// collecting is a variadic flag that is still taking values.
	collecting *Flag
	// collected is how many values it has taken, so a bound can stop it.
	collected uint32
	// cmdStart is where the command in scope began, as an index into argv.
	cmdStart int
	// argPos is which of cmd.Args is next to fill.
	argPos int
	// argTaken is how many words the variadic at argPos has taken.
	argTaken uint32
	// argFilled records whether any word has been bound to a positional of cmd.
	// Once one has, no further word can select a subcommand.
	argFilled bool
	// flagsStopped records whether flag interpretation has stopped. A -- does
	// this, and so does an automatic argument taking a value.
	flagsStopped bool
	// separatorSeen records whether a -- was actually consumed as a separator.
	//
	// Tracked apart from flagsStopped because the two can differ: an automatic
	// argument stops flag interpretation without any separator being typed, and a
	// preserve argument keeps one as a value rather than consuming it. Callers
	// asking this question want to know what the user wrote, not what state the
	// parser reached.
	separatorSeen bool
	// defaultTaken records whether the default subcommand has been used, which may
	// happen at most once: a default that itself declares one would otherwise
	// descend on every word until the tree ran out.
	defaultTaken bool
	// done stops iteration, set when argv runs out or an error is reported.
	done bool

	// The current event and the failure, both held inline so that neither Next nor
	// Err allocates.
	event  Event
	err    Error
	failed bool
}

// New begins parsing argv against root. argv excludes the program name.
func New(root *Command, argv []string) Parser {
	return Parser{argv: argv, cmd: root}
}

// Next reads the next event, reporting false when argv is exhausted or the parse
// failed. Check [Parser.Err] afterwards to tell those two apart.
//
// An error is terminal: the parse stops there, since continuing past a token that
// could not be understood would only produce bindings derived from a guess.
// Events already yielded before an error are therefore not a partial result — a
// caller that assigned them into fields should discard the whole attempt.
func (p *Parser) Next() bool {
	if p.done {
		return false
	}
	ok := p.step()
	if !ok {
		p.done = true
	}
	return ok
}

// Event returns what the last [Parser.Next] bound. Valid only while Next
// reported true.
func (p *Parser) Event() Event { return p.event }

// Err returns the failure that stopped the parse, or nil.
//
// Help and version requests arrive here too, with [Error.Code] set to [CodeHelp]
// or [CodeVersion]. They are not failures; they are the other way a parse ends
// without producing a value, and every caller already handles that shape.
func (p *Parser) Err() error {
	if !p.failed {
		return nil
	}
	return &p.err
}

// Command reports the command in scope: the root, or the deepest subcommand
// selected so far.
func (p *Parser) Command() *Command { return p.cmd }

// DoubleDashSeen reports whether a -- was consumed as a separator.
//
// False when flag interpretation stopped for another reason, such as an automatic
// argument taking a value, and false for a -- that a preserve argument kept as a
// value.
func (p *Parser) DoubleDashSeen() bool { return p.separatorSeen }

// FlagsStopped reports whether flag interpretation has stopped, for any reason.
//
// Wider than [Parser.DoubleDashSeen], and the question completion asks: past a
// separator or past the first value of an automatic argument, a dash-prefixed
// word is a value, so there is no flag there to offer.
func (p *Parser) FlagsStopped() bool { return p.flagsStopped }

// SubcommandsPossible reports whether a word here could still name a subcommand.
//
// The other half of the rule [Parser.FlagsStopped] answers for flags: descent
// stops once a positional of this command has taken a word, so a later word that
// happens to equal a subcommand name is just a value. A completion that offered
// one there would be advertising a word the parser no longer accepts as a
// command.
func (p *Parser) SubcommandsPossible() bool { return !p.argFilled && !p.flagsStopped }

// CommandStart is where the command in scope began: the index in argv just after
// its name. argv[CommandStart():] is what that command was given.
func (p *Parser) CommandStart() int { return p.cmdStart }

// Collecting reports a variadic flag that is still claiming words, if there is
// one.
//
// Ask between events, because the answer is gone by the end: the call that finds
// argv exhausted is the one that clears it. A completion needs it — the next word
// after `--tools a ⌶` is another tool, not the positional that follows.
func (p *Parser) Collecting() *Flag { return p.collecting }

// PendingArg is the positional the next word would fill, if there is one left.
//
// A variadic stays here until it reaches its bound, which is what makes it the
// answer to "what could go where the cursor is" as many times as it can be
// filled.
func (p *Parser) PendingArg() *Arg { return p.nextArg() }

// fail records a terminal error and reports that iteration is over.
func (p *Parser) fail(e Error) bool {
	p.err = e
	p.failed = true
	return false
}

// emit records an event and reports that iteration continues.
func (p *Parser) emit(e Event) bool {
	p.event = e
	return true
}

func (p *Parser) step() bool {
	for {
		// A partly-read short bundle takes priority: its remaining bytes are still
		// part of the token being processed.
		if len(p.bundle) > 0 {
			return p.shortFlag()
		}

		// A variadic flag keeps claiming tokens until one of them could be something
		// else.
		if flag := p.collecting; flag != nil {
			if p.pos < len(p.argv) {
				next := p.argv[p.pos]
				if !isFlagLike(next) && next != "--" {
					p.pos++
					p.collected++
					// Same rule as a positional: a bounded occurrence takes that many and
					// leaves the rest to whatever follows.
					if flag.VarMax != 0 && p.collected >= flag.VarMax {
						p.collecting = nil
					}
					return p.emit(Event{Kind: KindFlag, Flag: flag, Value: next, HasValue: true})
				}
				// A token that could be something else ends the run — but the end of argv
				// decides nothing. Clearing there would throw away the answer to "would the
				// next word be claimed?", which is the question a completion asks and no
				// parse ever does: once argv is exhausted there are no more events either
				// way.
				p.collecting = nil
			}
		}

		if p.pos >= len(p.argv) {
			return false
		}
		token := p.argv[p.pos]
		p.pos++

		if p.flagsStopped {
			return p.word(token)
		}

		if token == "--" {
			// preserve wants the separator itself as a value, so ask the argument that
			// would receive it before treating it as syntax.
			if a := p.nextArg(); a != nil && a.DoubleDash == DoubleDashPreserve {
				return p.word(token)
			}
			p.flagsStopped = true
			p.separatorSeen = true
			// An explicit separator unlocks any argument that required one, even if
			// earlier arguments are still unfilled.
			for i := p.argPos; i < len(p.cmd.Args); i++ {
				if p.cmd.Args[i].DoubleDash == DoubleDashRequired {
					// The count belongs to the argument at argPos, so jumping past it has to
					// leave the count behind: a bounded variadic before the separator would
					// otherwise lend its total to the argument after it, which then stops
					// early or at once.
					p.argPos = i
					p.argTaken = 0
					break
				}
			}
			continue
		}

		if isFlagLike(token) {
			if len(token) >= 2 && token[:2] == "--" {
				return p.longFlag(token)
			}
			// Check the whole bundle before emitting anything from it. Events go out one
			// at a time, so discovering an unknown letter half way through would mean the
			// earlier letters had already been applied — and the grammar rejects the
			// entire token, not the tail of it.
			if !p.bundleKnown(token) {
				if p.cmd.UnknownFlags == UnknownFlagsError {
					return p.fail(Error{Code: CodeUnknownFlag, Token: token})
				}
				// Unrecognized, so it is a word like any other.
				return p.word(token)
			}
			p.bundle = token[1:]
			p.bundleToken = token
			return p.shortFlag()
		}

		return p.word(token)
	}
}

func (p *Parser) longFlag(token string) bool {
	body := token[2:]
	name := body
	attached := ""
	hasAttached := false
	for i := 0; i < len(body); i++ {
		if body[i] == '=' {
			name, attached, hasAttached = body[:i], body[i+1:], true
			break
		}
	}

	if flag := p.findLong(name); flag != nil {
		value := ""
		hasValue := false
		if flag.TakesValue {
			hasValue = true
			if hasAttached {
				value = attached
			} else {
				// `token`, not `--`+name: with no attached value the token is the
				// spelling, and slicing it costs nothing on a path that must not
				// allocate.
				v, ok := p.takeDetachedValue(flag, token, 0)
				if !ok {
					return false
				}
				value = v
			}
		}
		if flag.Variadic {
			p.startCollecting(flag)
		}
		return p.emit(Event{Kind: KindFlag, Flag: flag, Value: value, HasValue: hasValue})
	}

	if flag := p.findNegation(name); flag != nil {
		return p.emit(Event{Kind: KindFlag, Flag: flag, Negated: true})
	}

	// Where the CLI declared a version, --version answers with it — asked after the
	// command's own flags, so a CLI declaring its own keeps it.
	//
	// Reported as an ordinary flag event rather than as a failure. Whether asking
	// for help ends the parse is the caller's decision, and the layer that owns the
	// target struct is where usage-argv makes it; the parser only says the flag was
	// given. Use [IsHelpFlag] and [IsVersionFlag] to recognize them.
	if name == "version" && p.cmd.Version {
		return p.emit(Event{Kind: KindFlag, Flag: VersionLong})
	}

	// Every CLI answers to --help, and none of them declares it. Asked after the
	// command's own flags, for the same reason.
	if name == "help" {
		return p.emit(Event{Kind: KindFlag, Flag: HelpLong})
	}

	if p.cmd.UnknownFlags == UnknownFlagsError {
		return p.fail(Error{Code: CodeUnknownFlag, Token: token})
	}
	// Not a flag here, so it is a word like any other.
	return p.word(token)
}

// bundleKnown walks a short-flag token without binding anything, to find out
// whether all of it is recognized.
//
// Scanning stops at the first letter whose flag takes a value, because everything
// after it is that value rather than more letters.
func (p *Parser) bundleKnown(token string) bool {
	for i := 1; i < len(token); i++ {
		flag := p.findShort(token[i])
		if flag == nil {
			return false
		}
		if flag.TakesValue {
			return true
		}
	}
	return true
}

func (p *Parser) shortFlag() bool {
	b := p.bundle[0]
	rest := p.bundle[1:]

	flag := p.findShort(b)
	if flag == nil {
		// bundleKnown already rejected any token containing an unrecognized letter,
		// so this is unreachable — but a parser should report rather than panic if
		// that ever stops being true.
		p.bundle = ""
		return p.fail(Error{Code: CodeUnknownFlag, Token: p.bundleToken})
	}

	if !flag.TakesValue {
		p.bundle = rest
		return p.emit(Event{Kind: KindFlag, Flag: flag})
	}

	// A value-taking short ends the token: everything after it is the value, less
	// one separating =.
	p.bundle = ""
	var value string
	switch {
	case rest == "":
		v, ok := p.takeDetachedValue(flag, "", b)
		if !ok {
			return false
		}
		value = v
	case rest[0] == '=':
		value = rest[1:]
	default:
		value = rest
	}
	if flag.Variadic {
		p.startCollecting(flag)
	}
	return p.emit(Event{Kind: KindFlag, Flag: flag, Value: value, HasValue: true})
}

// takeDetachedValue takes the following token as a flag's value.
//
// It refuses a flag-like token unless AllowHyphenValues is set: `--jobs --force`
// is far more likely a forgotten value than a deliberate one, and the attached
// form is available for the deliberate case. Declared, the next token is taken
// whatever it looks like, including `--`. The negative-number exception means
// `--offset -1` still works.
func (p *Parser) takeDetachedValue(flag *Flag, long string, short byte) (string, bool) {
	if p.pos < len(p.argv) && (flag.AllowHyphenValues || !isFlagLike(p.argv[p.pos])) {
		v := p.argv[p.pos]
		p.pos++
		return v, true
	}
	// The form the user actually wrote, carried so the advice can use it. A flag
	// answers to several spellings and the first is not always the one in front of
	// them: with an inherited `--jobs --workers` whose `--jobs` a nearer command
	// has taken, `--workers` is what bound, and telling them to write `--jobs=…`
	// sends them to a different flag.
	//
	// The long form arrives as a slice of the token, and the short one is built
	// here rather than by the caller — this branch is the failure, and the caller
	// is the hot path that must not allocate.
	typed := long
	if typed == "" && short != 0 {
		typed = "-" + string(short)
	}
	p.fail(Error{Code: CodeMissingFlagValue, Flag: flag, Token: typed})
	return "", false
}

func (p *Parser) word(token string) bool {
	// Subcommands are only matched where descent is still possible: once a
	// positional of this command has taken a word, a later word that happens to
	// equal a subcommand name is just a value.
	if !p.argFilled && !p.flagsStopped {
		if sub := p.findSubcommand(token); sub != nil {
			if !p.descend(sub) {
				return false
			}
			return p.emit(Event{Kind: KindCommand, Command: sub})
		}

		// `ex help config ls` — the line every page with a Commands section prints.
		// Asked after the subcommand lookup, so a CLI that declares a help of its own
		// keeps it, which is the rule the two help flags follow too.
		//
		// The words after it name a command, resolved here rather than descended
		// into: descending would bind them, and they are a question rather than an
		// invocation.
		if token == "help" && len(p.cmd.Subcommands) > 0 {
			cmd := p.cmd
			for p.pos < len(p.argv) {
				sub := findNamed(cmd, p.argv[p.pos])
				if sub == nil {
					break
				}
				cmd = sub
				p.pos++
			}
			// The long form, as `ex config --help` gives: someone who typed a whole word
			// to ask for help wants the fuller answer.
			return p.fail(Error{Code: CodeHelp, Cmd: cmd, Long: true})
		}

		// A word that names no subcommand goes to the default one, if there is one.
		//
		// Only a word, though. A dash-prefixed token that named no flag arrives here
		// as a value — that is what unknown_flags="value" means — and it was never a
		// candidate to select anything, so it binds where it was typed. `--` is
		// excluded on the same grounds: it reaches this function only when a preserve
		// argument wants it as a value.
		//
		// The token is not consumed: the cursor steps back so the next event reads it
		// again, now against the command just descended into. That is what lets it be
		// a subcommand of the default as easily as an argument of it, without this
		// function having to decide which — and without yielding two events for one
		// word.
		if d := p.cmd.DefaultSubcommand; d != nil && !p.defaultTaken && !isFlagLike(token) && token != "--" {
			p.defaultTaken = true
			if !p.descend(d) {
				return false
			}
			p.pos--
			return p.emit(Event{Kind: KindCommand, Command: d})
		}
	}

	arg := p.nextArg()
	if arg == nil {
		return p.fail(Error{Code: CodeUnexpectedArg, Token: token})
	}

	if arg.DoubleDash == DoubleDashRequired && !p.separatorSeen {
		return p.fail(Error{Code: CodeArgRequiresDoubleDash, Arg: arg})
	}

	p.argFilled = true
	// An automatic argument stops flag interpretation from here on, as though the
	// caller had typed the separator themselves.
	if arg.DoubleDash == DoubleDashAutomatic {
		p.flagsStopped = true
	}
	// A variadic keeps taking values, so the cursor stays put — until it reaches
	// its bound, at which point the words after it belong to whatever comes next.
	// That is what makes `[a]… [b]` expressible at all.
	if arg.Var {
		p.argTaken++
		if arg.VarMax != 0 && p.argTaken >= arg.VarMax {
			p.advanceArg()
		}
	} else {
		p.advanceArg()
	}
	return p.emit(Event{Kind: KindArg, Arg: arg, Value: token, HasValue: true})
}

func (p *Parser) descend(sub *Command) bool {
	if p.depth >= MaxDepth {
		return p.fail(Error{Code: CodeTooDeep})
	}
	p.ancestors[p.depth] = p.cmd
	p.starts[p.depth] = p.cmdStart
	p.depth++
	p.cmd = sub
	// Where this command's own words start, which is what lets a completion hand a
	// callback the half-parsed struct of the command it was declared on rather than
	// of the root.
	p.cmdStart = p.pos
	p.argPos = 0
	p.argTaken = 0
	p.argFilled = false
	return true
}

// advanceArg moves to the next positional, forgetting what the last one took.
func (p *Parser) advanceArg() {
	p.argPos++
	p.argTaken = 0
}

// startCollecting begins a variadic flag occurrence.
//
// The value it was given on the same token counts, which is why this starts at
// one: `--include a b` with VarMax 2 takes a and b, not three words.
func (p *Parser) startCollecting(flag *Flag) {
	p.collected = 1
	if flag.VarMax != 0 && flag.VarMax <= 1 {
		p.collecting = nil
	} else {
		p.collecting = flag
	}
}

func (p *Parser) nextArg() *Arg {
	if p.argPos < len(p.cmd.Args) {
		return p.cmd.Args[p.argPos]
	}
	return nil
}

// eachInScope calls fn with every flag a token here could name: this command's
// own, then any ancestor's globals, stopping at the first one fn accepts.
//
// Own flags come first so that a subcommand redeclaring an inherited name shadows
// it, which is what mise relies on when it redeclares root globals on `run` with
// different shorts. A callback rather than a slice, because collecting them would
// allocate on the hot path.
func (p *Parser) eachInScope(fn func(*Flag) bool) *Flag {
	for _, f := range p.cmd.Flags {
		if fn(f) {
			return f
		}
	}
	for i := p.depth - 1; i >= 0; i-- {
		c := p.ancestors[i]
		if c == nil {
			continue
		}
		for _, f := range c.Flags {
			if f.Global && fn(f) {
				return f
			}
		}
	}
	return nil
}

// FlagsInScope calls fn for every flag a word here could name, in the order the
// parser itself would look in, so what a completion offers and what the parser
// accepts cannot disagree — including the shadowing rule. Return true from fn to
// stop early.
func (p *Parser) FlagsInScope(fn func(*Flag) bool) {
	p.eachInScope(fn)
}

func (p *Parser) findLong(name string) *Flag {
	return p.eachInScope(func(f *Flag) bool {
		for _, l := range f.Longs {
			if l == name {
				return true
			}
		}
		return false
	})
}

func (p *Parser) findNegation(name string) *Flag {
	return p.eachInScope(func(f *Flag) bool {
		return f.Negate != "" && f.Negate == name
	})
}

func (p *Parser) findShort(b byte) *Flag {
	f := p.eachInScope(func(f *Flag) bool {
		for _, s := range f.Shorts {
			if s == b {
				return true
			}
		}
		return false
	})
	if f != nil {
		return f
	}
	// As for --help: supplied by the parser, and only where the command has not
	// declared one of its own.
	switch {
	case b == 'h':
		return HelpShort
	case b == 'V' && p.cmd.Version:
		return VersionShort
	}
	return nil
}

func (p *Parser) findSubcommand(name string) *Command {
	return findNamed(p.cmd, name)
}

// findNamed resolves a word against a command's subcommands, by name or alias.
//
// Names are checked across every subcommand before any alias is, so a command's
// own name outranks another command's alias and the answer does not depend on
// the order the table lists them in.
func findNamed(cmd *Command, name string) *Command {
	for _, sub := range cmd.Subcommands {
		if sub.Name == name {
			return sub
		}
	}
	for _, sub := range cmd.Subcommands {
		for _, a := range sub.Aliases {
			if a == name {
				return sub
			}
		}
	}
	return nil
}

// isFlagLike reports whether a token should be read as a flag.
//
// `-` alone is a value, conventionally stdin. A negative number is a value too,
// without which no CLI could accept `--offset -1`.
func isFlagLike(token string) bool {
	return len(token) > 1 && token[0] == '-' && !isNumber(token[1:])
}

// isNumber reports whether the text after a `-` is a number, so -1, -2.5, and
// -1e5 are values while -1x is a flag-shaped token that names nothing.
//
// Digits, at most one `.`, and an optional exponent. Deliberately narrower than
// what a float parser accepts, which also takes inf and NaN — `-inf` is far
// likelier to be a misspelled flag than a number somebody meant to pass.
//
// Written out rather than deferred to strconv.ParseFloat because this runs on the
// hot path, and because ParseFloat accepts more than the grammar does: usage-lib
// and usage-argv disagreed about -1e5 when one side hand-rolled this and the other
// parsed a float, and the corpus pins the edges so the three cannot drift apart.
func isNumber(rest string) bool {
	mantissa, exponent := rest, ""
	hasExponent := false
	for i := 0; i < len(rest); i++ {
		if rest[i] == 'e' || rest[i] == 'E' {
			mantissa, exponent, hasExponent = rest[:i], rest[i+1:], true
			break
		}
	}

	seenDigit, seenDot := false, false
	for i := 0; i < len(mantissa); i++ {
		switch b := mantissa[i]; {
		case b >= '0' && b <= '9':
			seenDigit = true
		case b == '.' && !seenDot:
			seenDot = true
		default:
			return false
		}
	}
	if !seenDigit {
		return false
	}

	if !hasExponent {
		return true
	}
	// An exponent needs digits of its own, and may carry a sign.
	digits := exponent
	if len(digits) > 0 && (digits[0] == '+' || digits[0] == '-') {
		digits = digits[1:]
	}
	if digits == "" {
		return false
	}
	for i := 0; i < len(digits); i++ {
		if digits[i] < '0' || digits[i] > '9' {
			return false
		}
	}
	return true
}
