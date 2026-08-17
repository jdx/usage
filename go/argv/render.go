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
		return "unknown flag `" + err.Token + "`"
	case CodeMissingFlagValue:
		name := "a flag"
		if err.Flag != nil {
			name = "`--" + primaryLong(err.Flag) + "`"
		}
		// The likeliest cause, said out loud: a flag-like token following the flag
		// is refused as its value, and the attached form is how to force it.
		return "missing value for " + name +
			" (a value beginning with `-` has to be attached: `--flag=-x`)"
	case CodeUnexpectedArg:
		return "unexpected argument `" + err.Token + "`"
	case CodeArgRequiresDoubleDash:
		name := "that argument"
		if err.Arg != nil {
			name = "`" + err.Arg.Name + "`"
		}
		return name + " is only read after a `--` separator"
	case CodeTooDeep:
		return "the command tree is nested deeper than this parser will go"
	case CodeMissingRequiredFlag:
		return "missing required flag `--" + err.Name + "`"
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
			return "`--" + err.Name + "` cannot be given with another flag it conflicts with"
		}
		return "`--" + err.Name + "` and `--" + other + "` cannot be given together"
	}
	return "the command line could not be parsed"
}

// primaryLong is the spelling to name a flag by: its first long form, or its name
// where it has none.
func primaryLong(f *Flag) string {
	if len(f.Longs) > 0 {
		return f.Longs[0]
	}
	return f.Name
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
