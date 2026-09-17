package argv

// RenderRequest renders a help or version request returned by a parser. The
// boolean is false for nil and for ordinary errors, which belong to [Render].
// It never writes output or exits: the caller chooses the writer and exit code.
//
// words must be the same arguments passed to the parser, without argv[0]. The
// command path is recovered from those words, preserving aliases and the route
// through shared command nodes. Implicit default commands use their names.
// spec is passed by value so callers can supply a build-time Version without
// mutating generated tables. Its Bin is the displayed invocation name.
func RenderRequest(request *Error, spec HelpSpec, root *Command, words []string, help HelpTable) (string, bool) {
	if request == nil {
		return "", false
	}
	switch request.Code {
	case CodeVersion:
		version := spec.Version
		if request.Long && spec.LongVersion != "" {
			version = spec.LongVersion
		}
		return version + "\n", true
	case CodeHelp:
		path, chain := requestPath(root, words, spec.Bin)
		if request.All {
			return AllHelp(spec, path, chain, help), true
		}
		if request.Long {
			return LongHelp(spec, path, chain, help), true
		}
		return ShortHelp(spec, path, chain, help), true
	default:
		return "", false
	}
}

// requestPath follows parser events rather than searching by command pointer:
// a command may be reachable through more than one parent. Unlike completion,
// this walk stops at synthetic help/version flags, before any later words.
func requestPath(root *Command, words []string, bin string) ([]string, []*Command) {
	path := []string{bin}
	chain := []*Command{root}
	p := New(root, words)
	nextWord := p.pos
	for p.Next() {
		ev := p.Event()
		switch ev.Kind {
		case KindCommand:
			name := ev.Command.Name
			if p.pos > nextWord {
				name = words[p.pos-1]
			}
			path = append(path, name)
			chain = append(chain, ev.Command)
		case KindFlag:
			if IsHelpFlag(ev.Flag) || IsVersionFlag(ev.Flag) {
				return path, chain
			}
		}
		nextWord = p.pos
	}
	if err, ok := p.Err().(*Error); ok && err.Code == CodeHelp && err.Cmd != p.Command() {
		cmd := p.Command()
		for _, word := range words[nextWord+1 : p.pos] {
			cmd = findNamed(cmd, word)
			path = append(path, word)
			chain = append(chain, cmd)
		}
	}
	return path, chain
}
