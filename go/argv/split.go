package argv

import "unicode/utf8"

// Turning a command line back into the words a shell would have passed.
//
// A completion request arrives as text, not as argv: the shell hands over the
// line and where the cursor is in it, because the words do not exist yet — the
// one being typed is half-written and may be inside quotes that are not closed.
// So the words have to be recovered here, by the same rules the shell would have
// applied had the line been run.
//
// Ported from usage-argv's `split`, which is where the rules were worked out.
// Both sides answer the same request from the same kind of shell script, so a
// line that means one thing to a Rust CLI and another to a Go one would be the
// two frameworks disagreeing about the shell rather than about the spec.

// SplitLine is a command line as the shell would have passed it, plus where the
// cursor was.
type SplitLine struct {
	// Words are the words, unquoted — what argv would hold had the line been run.
	//
	// Always at least one: a cursor sitting after a space is completing a word
	// that does not exist yet, and an empty word is how that is said. Candidates
	// for "anything at all" and candidates for "something starting with `no`" are
	// the same question with a different prefix, and a caller should not have to
	// special-case the empty one.
	Words []string
	// Cword is which of Words the cursor is in.
	Cword int
	// Prefix is the part of that word before the cursor, unquoted — what a
	// candidate must start with.
	Prefix string
}

// Argv is the words a parser should walk: after the program name, before the
// cursor's word.
//
// Two things dropped for two reasons. The program name, because argv does not
// contain it and the parse tables describe what comes *after* it. The word being
// completed, because it is half-typed by definition — feeding it in would ask
// what can follow a word the user has not finished, when the question is what
// that word could be.
func (s SplitLine) Argv() []string {
	start := min(1, s.Cword)
	return s.Words[start:s.Cword]
}

// Split splits a line at a byte cursor, the way `shell` would have split it.
//
// `cursor` is a byte offset into `line`; anything past the end is treated as the
// end, and an offset landing inside a multi-byte character is moved back to that
// character's start rather than being taken literally — a completion request is
// not a place to be strict about a shell's arithmetic.
func Split(line string, cursor int, shell Shell) SplitLine {
	if cursor > len(line) {
		cursor = len(line)
	}
	if cursor < 0 {
		cursor = 0
	}
	cursor = floorCharBoundary(line, cursor)

	var words []string
	var word []rune
	// Whether anything has been written into `word` — including a quote that so
	// far contains nothing, so that `ex ""` is a word and not a gap between two.
	started := false
	cword, prefix, found := 0, "", false
	// Whether the cursor sat inside a word rather than in the gap before one. A
	// gap is a word the user is about to type, so one has to be made for them.
	cursorInWord := false

	// The cursor is reached before the character it sits in front of is read, so
	// the word in hand is the word being completed and what is in it is the
	// prefix. Checked at the top of each character and again before an escape
	// swallows the one after it: only checking the top left a cursor sitting on an
	// escaped character unnoticed, and the split then described the last word of
	// the line rather than the one being typed.
	reached := func(i int) {
		if i == cursor && !found {
			cword, prefix, found = len(words), string(word), true
			cursorInWord = started
		}
	}

	var quote rune
	runes := []rune(line)
	// Byte offsets alongside, because the cursor is one.
	offsets := make([]int, len(runes)+1)
	at := 0
	for i, r := range runes {
		offsets[i] = at
		at += utf8.RuneLen(r)
	}
	offsets[len(runes)] = at

	for i := 0; i < len(runes); i++ {
		c := runes[i]
		reached(offsets[i])

		peek := func() (rune, bool) {
			if i+1 < len(runes) {
				return runes[i+1], true
			}
			return 0, false
		}

		switch {
		case quote == '\'':
			if c == '\'' {
				// PowerShell writes a quote inside a quoted string by doubling it;
				// the POSIX-shaped shells have no such rule, and there a second
				// quote always ends the string.
				if next, ok := peek(); shell.doublesQuotes() && ok && next == '\'' {
					reached(offsets[i+1])
					word = append(word, '\'')
					i++
				} else {
					quote = 0
				}
			} else {
				word = append(word, c)
			}

		case quote != 0:
			if c == quote {
				if next, ok := peek(); shell.doublesQuotes() && ok && next == quote {
					reached(offsets[i+1])
					word = append(word, quote)
					i++
				} else {
					quote = 0
				}
			} else if isEscape(c, shell) {
				// Inside double quotes an escape is only an escape before a
				// character it could mean something to; before anything else it is
				// a literal, which is why a Windows path in double quotes survives.
				if next, ok := peek(); ok && escapableInQuotes(next, shell) {
					reached(offsets[i+1])
					word = append(word, next)
					i++
				} else {
					word = append(word, c)
				}
			} else {
				word = append(word, c)
			}

		default:
			switch {
			case c == '\'' || c == '"':
				quote = c
				started = true
			case isEscape(c, shell):
				// Before the check, because the escape has already started the
				// word: a cursor on the escaped character is inside it, not in the
				// gap before it.
				started = true
				if next, ok := peek(); ok {
					reached(offsets[i+1])
					word = append(word, next)
					i++
				}
				// A trailing escape is a line the user is still typing, not a
				// mistake to report: it escapes the character they have not typed.
			case isSpace(c):
				if started {
					words = append(words, string(word))
					word = word[:0]
					started = false
				}
			default:
				word = append(word, c)
				started = true
			}
		}
	}

	// The cursor at the very end of the line: the loop above only sees positions it
	// reads a character at, and there is no character there.
	if !found {
		cword, prefix = len(words), string(word)
		cursorInWord = started
	}
	if started {
		words = append(words, string(word))
	}

	// A cursor in a gap is completing a word that is not in the line yet — at the
	// end of it, or between two that are already there. `ex ⌶use` is asking what
	// can go *before* `use`, and answering about `use` itself would complete the
	// wrong word.
	if !cursorInWord {
		words = append(words, "")
		copy(words[cword+1:], words[cword:])
		words[cword] = ""
	}
	return SplitLine{Words: words, Cword: cword, Prefix: prefix}
}

// isEscape reports whether a character starts an escape in this shell.
func isEscape(c rune, shell Shell) bool {
	if shell.backtickEscapes() {
		return c == '`'
	}
	return c == '\\'
}

// escapableInQuotes reports whether an escape inside double quotes applies to the
// character after it.
func escapableInQuotes(c rune, shell Shell) bool {
	if shell.backtickEscapes() {
		return c == '"' || c == '`' || c == '$'
	}
	return c == '"' || c == '\\' || c == '$' || c == '`'
}

// isSpace is the whitespace a shell splits words on. Written out rather than
// taken from `unicode`, which would pull the tables in for a package whose whole
// claim is that it costs nothing unused.
func isSpace(c rune) bool {
	return c == ' ' || c == '\t' || c == '\n' || c == '\r' || c == '\v' || c == '\f'
}

// floorCharBoundary is the start of the character containing `index`.
func floorCharBoundary(s string, index int) int {
	// The end of the string is a boundary and has no byte to look at.
	for index > 0 && index < len(s) && !utf8.RuneStart(s[index]) {
		index--
	}
	return index
}
