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
//		case argv.KindExternal:
//			// ev.Values is the unmatched name, then the rest of argv
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
	// Effective inherited trailing-delimiter policy.
	dontDelimitTrailingValues bool
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
	// commandArgFound includes flags as well as positionals for the command
	// policy that makes either exclude a later subcommand.
	commandArgFound bool
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
	return Parser{
		argv:                      argv,
		cmd:                       root,
		dontDelimitTrailingValues: root.DontDelimitTrailingValues,
	}
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
// its name, or at the unmatched word routed into a default subcommand.
// argv[CommandStart():] is what that command was given.
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
	if e.Kind == KindFlag || e.Kind == KindArg {
		p.commandArgFound = true
	}
	return true
}

func (p *Parser) step() bool {
	for {
		// A partly-read short bundle takes priority: its remaining bytes are still
		// part of the token being processed.
		if len(p.bundle) > 0 {
			return p.shortFlag()
		}

		if p.cmd.SubcommandPrecedenceOverArg && !p.flagsStopped && p.pos < len(p.argv) {
			if sub := p.findSubcommand(p.argv[p.pos]); sub != nil {
				if p.cmd.ArgsConflictWithSubcommands && p.commandArgFound {
					return p.fail(Error{Code: CodeSubcommandConflict, Token: p.argv[p.pos], Cmd: p.cmd})
				}
				p.pos++
				if !p.descend(sub) {
					return false
				}
				return p.emit(Event{Kind: KindCommand, Command: sub})
			}
		}

		// A variadic flag keeps claiming tokens until one of them could be something
		// else.
		if flag := p.collecting; flag != nil {
			if p.pos < len(p.argv) {
				next := p.argv[p.pos]
				if flag.ValueTerminator != "" && next == flag.ValueTerminator {
					p.pos++
					p.collecting = nil
					continue
				}
				if (!isFlagLike(next) || (flag.AllowNegativeNumbers && isNegativeNumber(next))) && next != "--" {
					p.pos++
					p.collected += valuesIn(next, flag.Delimiter)
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

		if p.argTaken > 0 {
			if arg := p.nextArg(); arg != nil && arg.ValueTerminator != "" && token == arg.ValueTerminator {
				p.advanceArg()
				continue
			}
		}

		// An exact declared short outranks the numeric shape. This keeps ordinary negative
		// numbers available as values while allowing clap-compatible spellings such as fd's
		// `-0` / `--print0` switch.
		declaredNumericShort := len(token) == 2 && token[1] >= '0' && token[1] <= '9' && p.findShort(token[1]) != nil
		if isNegativeNumber(token) && !declaredNumericShort {
			if arg := p.nextArg(); arg != nil && arg.AllowNegativeNumbers {
				return p.word(token)
			}
			if p.cmd.ExternalSubcommand && !p.argFilled {
				return p.word(token)
			}
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
				v, present, ok := p.takeDetachedValue(flag, token, 0)
				if !ok {
					return false
				}
				value = v
				hasValue = present
			}
		} else if flag.BoolValue && hasAttached {
			if attached != "true" && attached != "false" {
				return p.fail(Error{Code: CodeInvalidChoice, Name: flag.Name, Choices: []string{"true", "false"}})
			}
			value, hasValue = attached, true
		}
		if flag.Variadic && hasValue {
			p.startCollecting(flag, value)
		}
		if flag.Action != ActionSet {
			return p.flagAction(flag, true)
		}
		return p.emit(Event{Kind: KindFlag, Flag: flag, Value: value, HasValue: hasValue})
	}

	if flag := p.findNegation(name); flag != nil {
		if flag.BoolValue && hasAttached {
			if attached != "true" && attached != "false" {
				return p.fail(Error{Code: CodeInvalidChoice, Name: flag.Name, Choices: []string{"true", "false"}})
			}
			return p.emit(Event{Kind: KindFlag, Flag: flag, Value: attached, HasValue: true, Negated: true})
		}
		return p.emit(Event{Kind: KindFlag, Flag: flag, Negated: true})
	}

	// Where the CLI declared a version, --version answers with it — asked after the
	// command's own flags, so a CLI declaring its own keeps it.
	//
	// Reported as an ordinary flag event rather than as a failure. Whether asking
	// for help ends the parse is the caller's decision, and the layer that owns the
	// target struct is where usage-argv makes it; the parser only says the flag was
	// given. Use [IsHelpFlag] and [IsVersionFlag] to recognize them.
	if name == "version" && p.cmd.Version && !p.cmd.DisableVersionFlag {
		return p.emit(Event{Kind: KindFlag, Flag: VersionLong})
	}

	// Every CLI answers to --help, and none of them declares it. Asked after the
	// command's own flags, for the same reason.
	if name == "help" && !p.cmd.DisableHelpFlag {
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
		if flag.Action != ActionSet {
			p.bundle = ""
			return p.flagAction(flag, false)
		}
		return p.emit(Event{Kind: KindFlag, Flag: flag})
	}

	// A value-taking short ends the token: everything after it is the value, less
	// one separating =.
	p.bundle = ""
	var value string
	hasValue := true
	switch {
	case rest == "":
		v, present, ok := p.takeDetachedValue(flag, "", b)
		if !ok {
			return false
		}
		value = v
		hasValue = present
	case rest[0] == '=':
		value = rest[1:]
	default:
		value = rest
	}
	if flag.Variadic && hasValue {
		p.startCollecting(flag, value)
	}
	if flag.Action != ActionSet {
		return p.flagAction(flag, false)
	}
	return p.emit(Event{Kind: KindFlag, Flag: flag, Value: value, HasValue: hasValue})
}

func (p *Parser) flagAction(flag *Flag, longSpelling bool) bool {
	if flag == HelpLong || flag == HelpShort || flag == VersionLong || flag == VersionShort {
		return p.emit(Event{Kind: KindFlag, Flag: flag})
	}
	switch flag.Action {
	case ActionHelp:
		return p.fail(Error{Code: CodeHelp, Cmd: p.cmd, Long: longSpelling})
	case ActionHelpShort:
		return p.fail(Error{Code: CodeHelp, Cmd: p.cmd, Long: false})
	case ActionHelpLong:
		return p.fail(Error{Code: CodeHelp, Cmd: p.cmd, Long: true})
	case ActionHelpAll:
		return p.fail(Error{Code: CodeHelp, Cmd: p.cmd, Long: true, All: true})
	case ActionVersion:
		return p.fail(Error{Code: CodeVersion, Cmd: p.cmd, Long: longSpelling})
	default:
		return p.emit(Event{Kind: KindFlag, Flag: flag})
	}
}

// takeDetachedValue takes the following token as a flag's value.
//
// It refuses a flag-like token unless AllowHyphenValues is set: `--jobs --force`
// is far more likely a forgotten value than a deliberate one, and the attached
// form is available for the deliberate case. Declared, the next token is taken
// whatever it looks like, including `--`. RequireEquals refuses the following
// word either way. The negative-number exception means `--offset -1` still works.
func (p *Parser) takeDetachedValue(flag *Flag, long string, short byte) (string, bool, bool) {
	if flag.RequireEquals {
		return p.missingOrDefault(flag, long, short)
	}
	if p.pos < len(p.argv) && (flag.AllowHyphenValues || !isFlagLike(p.argv[p.pos]) || (flag.AllowNegativeNumbers && isNegativeNumber(p.argv[p.pos]))) {
		v := p.argv[p.pos]
		p.pos++
		return v, true, true
	}
	return p.missingOrDefault(flag, long, short)
}

// missingOrDefault is the value when a detached one is refused or absent.
//
// DefaultMissing binds without consuming the next token, so `--color --verbose`
// still sets verbose and `--inspect 80` with RequireEquals leaves `80` for a
// positional. Empty DefaultMissing is unset; ValueOptional then emits a bare
// occurrence, while a required value produces the missing-value error.
func (p *Parser) missingOrDefault(flag *Flag, long string, short byte) (string, bool, bool) {
	if flag.DefaultMissing != "" {
		return flag.DefaultMissing, true, true
	}
	if flag.ValueOptional {
		return "", false, true
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
	return "", false, false
}

func (p *Parser) word(token string) bool {
	// Subcommands are only matched where descent is still possible: once a
	// positional of this command has taken a word, a later word that happens to
	// equal a subcommand name is just a value.
	if !p.argFilled && !p.flagsStopped {
		if sub := p.findSubcommand(token); sub != nil {
			if p.cmd.ArgsConflictWithSubcommands && p.commandArgFound {
				return p.fail(Error{Code: CodeSubcommandConflict, Token: token, Cmd: p.cmd})
			}
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
		if token == "help" && !p.cmd.DisableHelpSubcommand && len(p.cmd.Subcommands) > 0 {
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
		d := p.cmd.DefaultSubcommand
		defaultAcceptsNegative := d != nil &&
			isNegativeNumber(token) &&
			len(d.Args) > 0 &&
			d.Args[0].AllowNegativeNumbers
		if d != nil && !p.defaultTaken && (!isFlagLike(token) || defaultAcceptsNegative) && token != "--" {
			p.defaultTaken = true
			if !p.descend(d) {
				return false
			}
			p.pos--
			// A default command receives the word that caused descent. Keep its argv
			// boundary at that word so policies and completion callbacks see it.
			p.cmdStart = p.pos
			return p.emit(Event{Kind: KindCommand, Command: d})
		}

		// An unmatched word that names no subcommand is forwarded as an external
		// command: this word, then every token after it, including flags. Known
		// subcommands already won above, and a default subcommand already caught.
		if p.cmd.ExternalSubcommand && (!isFlagLike(token) || isNegativeNumber(token)) && token != "--" && token != "-" {
			from := p.pos - 1
			values := p.argv[from:]
			p.pos = len(p.argv)
			return p.emit(Event{Kind: KindExternal, Values: values})
		}
	}

	p.reserveForRequiredPositionals()
	arg := p.nextArg()
	if arg == nil {
		return p.fail(Error{Code: CodeUnexpectedArg, Token: token})
	}

	if arg.DoubleDash == DoubleDashRequired && !p.separatorSeen {
		return p.fail(Error{Code: CodeArgRequiresDoubleDash, Arg: arg})
	}

	p.argFilled = true
	trailingValue := p.separatorSeen || arg.DoubleDash == DoubleDashAutomatic
	delimit := !(p.dontDelimitTrailingValues && trailingValue)
	// An automatic argument stops flag interpretation from here on, as though the
	// caller had typed the separator themselves.
	if arg.DoubleDash == DoubleDashAutomatic {
		p.flagsStopped = true
	}
	// A variadic keeps taking values, so the cursor stays put — until it reaches
	// its bound, at which point the words after it belong to whatever comes next.
	// That is what makes `[a]… [b]` expressible at all.
	if arg.Var {
		delimiter := arg.Delimiter
		if !delimit {
			delimiter = 0
		}
		p.argTaken += valuesIn(token, delimiter)
		if arg.VarMax != 0 && p.argTaken >= arg.VarMax {
			p.advanceArg()
		}
	} else {
		p.advanceArg()
	}
	return p.emit(Event{
		Kind: KindArg, Arg: arg, Value: token, HasValue: true, Delimit: delimit,
	})
}

func (p *Parser) descend(sub *Command) bool {
	if p.depth >= MaxDepth {
		return p.fail(Error{Code: CodeTooDeep})
	}
	p.ancestors[p.depth] = p.cmd
	p.starts[p.depth] = p.cmdStart
	p.depth++
	p.cmd = sub
	p.dontDelimitTrailingValues = p.dontDelimitTrailingValues || sub.DontDelimitTrailingValues
	// Where this command's own words start, which is what lets a completion hand a
	// callback the half-parsed struct of the command it was declared on rather than
	// of the root.
	p.cmdStart = p.pos
	p.argPos = 0
	p.argTaken = 0
	p.argFilled = false
	p.commandArgFound = false
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
func (p *Parser) startCollecting(flag *Flag, first string) {
	p.collected = valuesIn(first, flag.Delimiter)
	if flag.VarMax != 0 && flag.VarMax <= 1 {
		p.collecting = nil
	} else {
		p.collecting = flag
	}
}

func valuesIn(value string, delimiter byte) uint32 {
	count := uint32(1)
	if delimiter == 0 {
		return count
	}
	for i := 0; i < len(value); i++ {
		if value[i] == delimiter {
			count++
		}
	}
	return count
}

func (p *Parser) nextArg() *Arg {
	if p.argPos < len(p.cmd.Args) {
		return p.cmd.Args[p.argPos]
	}
	return nil
}

// reserveForRequiredPositionals implements the opt-in clap policy that lets a
// later required positional claim the last available word while an earlier
// optional positional remains empty.
func (p *Parser) reserveForRequiredPositionals() {
	if !p.cmd.AllowMissingPositional || p.argTaken != 0 {
		return
	}
	for p.argPos < len(p.cmd.Args) && !p.cmd.Args[p.argPos].Required {
		requiredAfter := 0
		for _, arg := range p.cmd.Args[p.argPos+1:] {
			if arg.Required {
				requiredAfter++
			}
		}
		if requiredAfter == 0 {
			return
		}
		remainingValues := 1
		for _, word := range p.argv[p.pos:] {
			if p.flagsStopped || !isFlagLike(word) {
				remainingValues++
			}
		}
		if remainingValues > requiredAfter {
			return
		}
		p.advanceArg()
	}
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
	case b == 'h' && !p.cmd.DisableHelpFlag:
		return HelpShort
	case b == 'V' && p.cmd.Version && !p.cmd.DisableVersionFlag:
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
// `-` alone is a value, conventionally stdin. Other dash-prefixed tokens are
// flag-like; a field may make the narrower negative-number exception.
func isFlagLike(token string) bool {
	return len(token) > 1 && token[0] == '-'
}

func isNegativeNumber(token string) bool {
	return len(token) > 1 && token[0] == '-' && isNumber(token[1:])
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
