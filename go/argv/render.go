package argv

import "strings"

// Turning a binding failure into something a person can act on.
//
// Unlike everything else in this package, there is no reference to match. usage-lib
// prints a one-line message inside miette's frame; usage-argv renders through
// miette too, with the offending token underlined in the command line. Neither
// travels: miette is a Rust library, and a Go CLI drawing ASCII art around an
// error would be imitating a diagnostic format rather than sharing one.
//
// So this is judged on a different standard — whether the message says what went
// wrong, where, and what to do about it — and it is tested by asserting those
// three things rather than by comparing bytes. The shape is clap's, which is what
// a Go user will have seen before:
//
//	error: unknown flag `--wat`
//
//	Usage: ex [-f --force] <file>
//
//	For more information, try `--help`.

// Render turns a failure into the text a CLI should print to stderr.
//
// `path` and `chain` are the command as invoked, as for [ShortHelp], so the usage
// line names the command the user was actually in rather than the program.
//
// Help and version are not failures and render as nothing: a caller that gets
// [CodeHelp] should print the page, not this.
func Render(err *Error, path []string, chain []*Command, help HelpTable) string {
	if err == nil || err.Code == CodeHelp || err.Code == CodeVersion {
		return ""
	}

	var out strings.Builder
	out.WriteString("error: " + explain(err, help) + "\n")

	if len(chain) > 0 {
		out.WriteString("\nUsage: " + UsageLine(path, chain[len(chain)-1], help) + "\n")
	}
	out.WriteString("\nFor more information, try `--help`.\n")
	return out.String()
}

// explain is the one-line summary: what went wrong, naming the thing it went
// wrong with.
//
// Backticks around anything the user typed or could type, so a flag reads as a
// flag rather than as part of the sentence — `unknown flag --for` is ambiguous
// about where the name ends in a way that “unknown flag `--for` “ is not.
func explain(err *Error, help HelpTable) string {
	switch err.Code {
	case CodeUnknownFlag:
		return "unknown flag `" + safe(err.Token) + "`"
	case CodeMissingFlagValue:
		if err.Flag == nil {
			return "missing value for a flag"
		}
		// The likeliest cause, said out loud: a flag-like token following the flag
		// is refused as its value, and attaching it is how to force it.
		//
		// The example is `-x` rather than `-1`, because a negative number is the
		// one dash-prefixed token the parser *does* take detached — `--jobs -1`
		// binds. Illustrating the rule with the case that is exempt from it was
		// advice that contradicted itself.
		//
		// And it is spelled in the form the flag actually has: telling someone
		// with a short-only `-j` to write `--j=-x` sends them to an unknown flag.
		spelling := spell(err.Flag)
		return "missing value for `" + spelling + "`" +
			" (a value beginning with `-` has to be attached: `" + spelling + "=-x`)"
	case CodeUnexpectedArg:
		return "unexpected argument `" + safe(err.Token) + "`"
	case CodeArgRequiresDoubleDash:
		name := "that argument"
		if err.Arg != nil {
			name = "`" + err.Arg.Name + "`"
		}
		return name + " is only read after a `--` separator"
	case CodeTooDeep:
		return "the command tree is nested deeper than this parser will go"
	case CodeMissingRequiredFlag:
		return "missing required flag `" + typedAs(err.Spelling, err.Name) + "`"
	case CodeMissingRequiredArg:
		return "missing required argument `" + err.Name + "`"
	case CodeInvalidChoice:
		msg := "`" + err.Name + "` does not accept that value"
		if len(err.Choices) > 0 {
			msg += " (expected one of: " + strings.Join(err.Choices, ", ") + ")"
		}
		return msg
	case CodeVarTooFew:
		return "`" + err.Name + "` needs at least " + plural(int(err.Bound), "value") +
			", got " + itoa(err.Got)
	case CodeVarTooMany:
		return "`" + err.Name + "` accepts at most " + plural(int(err.Bound), "time") +
			", given " + itoa(err.Got)
	case CodeConflictingFlags:
		other := err.Other
		if other == "" {
			return "`" + typedAs(err.Spelling, err.Name) +
				"` cannot be given with another flag it conflicts with"
		}
		return "`" + typedAs(err.Spelling, err.Name) + "` and `" +
			typedAs(err.OtherSpelling, other) + "` cannot be given together"
	}
	return "the command line could not be parsed"
}

// spell names a flag the way a user would type it: its first long form, else its
// first short. Naming a short-only flag `--f` gives advice that cannot be
// followed.
func spell(f *Flag) string {
	if len(f.Longs) > 0 {
		return "--" + f.Longs[0]
	}
	if len(f.Shorts) > 0 {
		return "-" + string(f.Shorts[0])
	}
	return f.Name
}

// typedAs prefers the spelling the tables carry, and falls back to the bare name.
//
// It used to guess: a one-character name was read as a short flag, on the reasoning
// that a name is a long form wherever there is one. That is wrong for a long-only
// `--a`, which it rendered as `-a` — a form that does not exist, and which may
// belong to a *different* flag. So the spelling is carried now, and where it is
// missing the name is printed bare rather than dressed as something the user
// cannot type.
func typedAs(spelling, name string) string {
	if spelling != "" {
		return spelling
	}
	return name
}

// safe makes text from the command line printable.
//
// An error quotes back what the user typed, and what the user typed can contain
// control characters: an escape sequence in an argument would otherwise reach the
// terminal through the error message, where it can recolour the output, move the
// cursor, or forge lines that look like they came from the program. Rendering a
// rejected value is not a reason to execute it.
func safe(s string) string {
	var out strings.Builder
	for _, r := range s {
		switch {
		case r == '\t':
			out.WriteString("\\t")
		case r == '\n':
			out.WriteString("\\n")
		case r == '\r':
			out.WriteString("\\r")
		case r < 0x20 || r == 0x7f:
			out.WriteString("\\x" + hex(byte(r)))
		default:
			out.WriteRune(r)
		}
	}
	return out.String()
}

func hex(b byte) string {
	const digits = "0123456789abcdef"
	return string([]byte{digits[b>>4], digits[b&0xf]})
}

func plural(n int, noun string) string {
	if n == 1 {
		return "1 " + noun
	}
	return itoa(n) + " " + noun + "s"
}

func itoa(n int) string {
	if n == 0 {
		return "0"
	}
	neg := n < 0
	if neg {
		n = -n
	}
	var digits []byte
	for n > 0 {
		digits = append([]byte{byte('0' + n%10)}, digits...)
		n /= 10
	}
	if neg {
		return "-" + string(digits)
	}
	return string(digits)
}
