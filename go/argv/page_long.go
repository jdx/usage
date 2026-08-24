package argv

import (
	"sort"
	"strings"
)

// The page `--help` prints.
//
// The same content as [ShortHelp] through a wider layout: help is aligned into a
// column and wrapped, the long form of each description is preferred over the
// short one, and the annotations each get their own line.
//
// An entry whose help contains a line break is laid out as a block instead, its
// text indented under the usage rather than beside it — there is no column that
// keeps a line the author already broke readable.

// helpWidth is the width the long page wraps to. usage-lib reads the terminal and
// falls back to 80; a page rendered into a test, a file or a pipe has no terminal,
// so 80 is what both sides use and what keeps the two comparable.
const helpWidth = 80

// blockIndent is what a page uses where it cannot align to its column.
const blockIndent = 4

// LongHelp renders what `--help` prints for the command at the end of `chain`.
func LongHelp(spec HelpSpec, path []string, chain []*Command, help HelpTable) string {
	if len(chain) == 0 {
		return ""
	}
	cmd := chain[len(chain)-1]
	meta := help.Lookup(cmd.Key)
	nextLineHelp := meta != nil && meta.NextLineHelp
	var sections helpSections
	out := &sections.about

	before := firstOf(metaField(meta, func(h *Help) string { return h.BeforeLongHelp }),
		metaField(meta, func(h *Help) string { return h.BeforeHelp }),
		spec.BeforeLongHelp, spec.BeforeHelp)
	if before != "" {
		out.WriteString(before + "\n\n")
	}

	// The banner and the program's own description belong to the program's page.
	// A subcommand's page describes the subcommand, which is the question that
	// was asked.
	root := len(path) <= 1
	if root && spec.Version != "" {
		name := spec.Name
		if name == "" {
			name = spec.Bin
		}
		out.WriteString(name + " " + spec.Version + "\n")
	}
	about := ""
	if root {
		about = firstOf(spec.LongAbout, spec.About)
	} else if meta != nil {
		about = firstOf(meta.Long, meta.Short)
	}
	// Trimmed for the same reason the entries below are: the blank line after the
	// description is written here, so one already in the text doubles it. clap's
	// `long_about` often ends in a break — a `///` block whose last line is empty,
	// an examples section written with a trailing newline — and it reaches the spec
	// verbatim.
	if about := trimEnd(about); about != "" {
		out.WriteString(about + "\n\n")
	}
	if label := deprecationLabel(meta); label != "" {
		out.WriteString(label + "\n\n")
	}

	for i, line := range usageLines(path, cmd, help) {
		if i == 0 {
			sections.usage.WriteString("Usage: " + line + "\n")
		} else {
			sections.usage.WriteString("       " + line + "\n")
		}
	}

	if meta == nil || !meta.FlattenHelp {
		commandsSection(&sections.commands, path[min(1, len(path)):], cmd, help)
	}

	// One column width per section, over its visible entries — separately, so a
	// long flag does not push the arguments out.
	args := visibleArgs(cmd, help, true)
	argCol := 0
	for _, a := range args {
		if n := width(argUsage(a, help.Lookup(a.Key))); n > argCol {
			argCol = n
		}
	}
	groupsSection(&sections.args, &sections.ungroupedArgs, &sections.groupedArgs, "Arguments", len(args),
		func(i int) string { return headingOf(help, args[i].Key) },
		func(w *strings.Builder, i int) {
			h := help.Lookup(args[i].Key)
			indent := entry(w, argUsage(args[i], h), firstOf(metaField(h, func(x *Help) string { return x.Long }),
				metaField(h, func(x *Help) string { return x.Short })), argCol, nextLineHelp)
			longAnnotations(w, h, true, indent)
		})

	own, inherited := ownAndGlobal(chain, help)
	own = filterHelpMode(own, help, true)
	inherited = filterHelpMode(inherited, help, true)

	// One column over *both* lists, so the two sections read as one table with a
	// rule through it rather than two tables that happen to be adjacent.
	flagCol := 0
	for _, f := range append(append([]shownFlag{}, own...), inherited...) {
		if n := width(f.usage); n > flagCol {
			flagCol = n
		}
	}
	writeFlag := func(w *strings.Builder, f shownFlag) {
		if f.supplied != "" {
			entry(w, f.usage, f.suppliedHelp, flagCol, nextLineHelp)
			return
		}
		h := help.Lookup(f.key)
		indent := entry(w, f.usage, firstOf(metaField(h, func(x *Help) string { return x.Long }),
			metaField(h, func(x *Help) string { return x.Short })), flagCol, nextLineHelp)
		longAnnotations(w, h, true, indent)
	}
	groupsSection(&sections.flags, &sections.ungroupedFlags, &sections.groupedFlags, "Flags", len(own),
		func(i int) string {
			if own[i].supplied != "" {
				return ""
			}
			return headingOf(help, own[i].key)
		},
		func(w *strings.Builder, i int) { writeFlag(w, own[i]) })
	// Not grouped by heading: an ancestor's headings describe that command's page,
	// and borrowing them here would put a section title on flags that are only
	// visiting.
	groupsSection(&sections.flags, &sections.ungroupedFlags, &sections.groupedFlags, "Global flags", len(inherited),
		func(int) string { return "" },
		func(w *strings.Builder, i int) { writeFlag(w, inherited[i]) })
	if meta != nil && meta.FlattenHelp {
		flatCommandsLong(&sections.flattened, path[min(1, len(path)):], cmd, help, nextLineHelp)
	}

	out = &sections.afterHelp
	if examples := pageExamples(chain, help, meta); len(examples) > 0 {
		out.WriteString("\nExamples:\n")
		for _, e := range examples {
			if e.Header != "" {
				out.WriteString("  " + e.Header + ":\n")
			}
			// The description comes *before* the command, which is the order the
			// reference prints them in: it introduces the line rather than
			// commenting on it.
			if e.Help != "" {
				out.WriteString("    " + e.Help + "\n")
			}
			out.WriteString("    $ " + e.Code + "\n")
		}
	}

	after := firstOf(metaField(meta, func(h *Help) string { return h.AfterLongHelp }),
		metaField(meta, func(h *Help) string { return h.AfterHelp }),
		spec.AfterLongHelp, spec.AfterHelp)
	if after != "" {
		out.WriteString("\n" + after + "\n")
	}
	if spec.Author != "" || spec.License != "" {
		out.WriteByte('\n')
		if spec.Author != "" {
			out.WriteString("Author: " + spec.Author + "\n")
		}
		if spec.License != "" {
			out.WriteString("License: " + spec.License + "\n")
		}
	}

	return sections.assemble(spec.HelpTemplate)
}

// AllHelp renders long help for the selected command and every visible descendant.
func AllHelp(spec HelpSpec, path []string, chain []*Command, help HelpTable) string {
	var out strings.Builder
	var appendPages func([]string, []*Command)
	appendPages = func(currentPath []string, currentChain []*Command) {
		if out.Len() > 0 {
			out.WriteByte('\n')
		}
		out.WriteString(LongHelp(spec, currentPath, currentChain, help))
		current := currentChain[len(currentChain)-1]
		children := append([]*Command(nil), current.Subcommands...)
		sort.SliceStable(children, func(i, j int) bool {
			left := helpOrder(help, children[i].Key, 999)
			right := helpOrder(help, children[j].Key, 999)
			if left != right {
				return left < right
			}
			return children[i].Name < children[j].Name
		})
		for _, child := range children {
			if childHelp := help.Lookup(child.Key); childHelp != nil && childHelp.Hide {
				continue
			}
			nextPath := append(append([]string(nil), currentPath...), child.Name)
			nextChain := append(append([]*Command(nil), currentChain...), child)
			appendPages(nextPath, nextChain)
		}
	}
	appendPages(path, chain)
	return out.String()
}

// longCommandsSection lists the subcommands, each description on its own indented
// line rather than beside the name.
func flatCommandsLong(out *strings.Builder, path []string, cmd *Command, help HelpTable, nextLine bool) {
	visible := append([]*Command{}, cmd.Subcommands...)
	orderCommands(visible, help)
	for _, sub := range visible {
		h := help.Lookup(sub.Key)
		if h != nil && h.Hide {
			continue
		}
		subPath := append(append([]string{}, path...), sub.Name)
		out.WriteString("\n" + strings.Join(subPath, " ") + ":\n")
		if about := firstOf(metaField(h, func(x *Help) string { return x.Long }),
			metaField(h, func(x *Help) string { return x.Short })); strings.TrimSpace(about) != "" {
			out.WriteString(trimEnd(about) + "\n")
		}
		if label := deprecationLabel(h); label != "" {
			out.WriteString(label + "\n")
		}

		args := visibleArgs(sub, help, true)
		flags := make([]*Flag, 0, len(sub.Flags))
		argCol := 0
		for _, a := range args {
			argCol = max(argCol, width(argUsage(a, help.Lookup(a.Key))))
		}
		flagCol := 0
		for _, f := range sub.Flags {
			fh := help.Lookup(f.Key)
			if f.Global || (fh != nil && (fh.Hide || fh.HideLongHelp)) {
				continue
			}
			flags = append(flags, f)
			flagCol = max(flagCol, width(columnUsage(f, allShown(f), help)))
		}
		orderFlags(flags, help)
		for _, a := range args {
			ah := help.Lookup(a.Key)
			entry(out, argUsage(a, ah), firstOf(metaField(ah, func(x *Help) string { return x.Long }),
				metaField(ah, func(x *Help) string { return x.Short })), argCol, nextLine)
			longAnnotations(out, ah, true, blockIndent)
		}
		for _, f := range flags {
			fh := help.Lookup(f.Key)
			entry(out, columnUsage(f, allShown(f), help), firstOf(
				metaField(fh, func(x *Help) string { return x.Long }),
				metaField(fh, func(x *Help) string { return x.Short })), flagCol, nextLine)
			longAnnotations(out, fh, true, blockIndent)
		}
		if h != nil && h.FlattenHelp {
			flatCommandsLong(out, subPath, sub, help, h.NextLineHelp)
		}
		out.WriteString("\n")
	}
}

// entry writes one flag or argument: its help in a column beside it, wrapped — or
// indented underneath, where the text has line breaks of its own.
// entry writes one flag or argument and returns the indent its annotations should take:
// the description column when the description reached it, and blockIndent when it did not.
func entry(out *strings.Builder, usage, help string, col int, nextLine bool) int {
	// The column layout only works for text that has not been broken already, and
	// only when there is room left for it to say anything.
	indent := 2 + col + 2
	room := helpWidth - indent
	if room < 0 {
		room = 0
	}
	// Whether anything on this page reaches the column at all.
	block := nextLine || room < 10
	if strings.TrimSpace(help) == "" {
		out.WriteString("  " + usage + "\n")
		// An entry with nothing in the column still has annotations to place, and the
		// column is where they go: it is this entry's row that is empty, not the table's.
		if block {
			return blockIndent
		}
		return indent
	}

	if block || strings.Contains(help, "\n") {
		out.WriteString("  " + usage + "\n")
		writeIndented(out, help, blockIndent)
		return blockIndent
	}

	lines := wrap(help, room)
	out.WriteString("  " + pad(usage, col) + "  " + lines[0] + "\n")
	for _, line := range lines[1:] {
		out.WriteString(strings.Repeat(" ", indent) + line + "\n")
	}
	// No blank line after a wrapped entry: the reference's template asks for one
	// and its whitespace trimming eats it before it reaches the output.
	return indent
}

// longAnnotations gives each annotation its own line, which is the room the wide
// layout has and the short one does not.
// longAnnotations gives each annotation its own line, indented to where the entry above
// them ended up: the description column when the description reached it, and blockIndent
// when it did not. An annotation is a note about the same entry, so it belongs under the
// text it qualifies rather than in the gutter beside a column it is ignoring.
func longAnnotations(out *strings.Builder, h *Help, withDefault bool, indent int) {
	if h == nil {
		return
	}
	pad := strings.Repeat(" ", indent)
	if !h.HidePossibleValues && len(h.Choices) > 0 {
		out.WriteString(pad + "[possible values: " + strings.Join(h.Choices, ", ") + "]\n")
	}
	if !h.HideEnv && h.Env != "" {
		out.WriteString(pad + "[env: " + h.Env + "]\n")
	}
	if !h.HideEnv {
		for _, env := range h.EnvFallback {
			out.WriteString(pad + "[env fallback: " + env + "]\n")
		}
		for _, env := range h.DeprecatedEnv {
			out.WriteString(pad + "[deprecated env: " + env + "]\n")
		}
	}
	if withDefault && !h.HideDefaultValue && len(h.Default) > 0 {
		out.WriteString(pad + "(default: " + strings.Join(h.Default, ", ") + ")\n")
	}
	if label := deprecationLabel(h); label != "" {
		out.WriteString(pad + label + "\n")
	}
}

// writeIndented writes text with every line indented, leaving blank lines blank —
// an indented empty line is trailing whitespace, which the reference does not
// emit. Indenting them instead differs on every page, which is how this was
// settled rather than by reading the template.
func writeIndented(out *strings.Builder, text string, by int) {
	prefix := strings.Repeat(" ", by)
	for i, line := range strings.Split(text, "\n") {
		// The first line carries the indent whatever it holds, and later blank
		// lines do not. mise has a command whose description *begins* with an
		// empty line, and the reference prints four spaces there and nothing on
		// the blank lines below it — a template indenting where it starts writing
		// rather than trimming each line.
		if line == "" && i > 0 {
			out.WriteString("\n")
			continue
		}
		out.WriteString(prefix + line + "\n")
	}
}

// wrap breaks text to a width, preserving the breaks the author already made.
func wrap(text string, width int) []string {
	var lines []string
	for _, paragraph := range strings.Split(text, "\n") {
		if paragraph == "" {
			lines = append(lines, "")
			continue
		}
		line := ""
		for _, word := range strings.Fields(paragraph) {
			if line != "" && runeLen(line)+1+runeLen(word) > width {
				lines = append(lines, line)
				line = ""
			}
			if line != "" {
				line += " "
			}
			line += word
		}
		if line != "" {
			lines = append(lines, line)
		}
	}
	if len(lines) == 0 {
		lines = append(lines, "")
	}
	return lines
}

func runeLen(s string) int { return len([]rune(s)) }

func firstOf(values ...string) string {
	for _, v := range values {
		if v != "" {
			return v
		}
	}
	return ""
}

func metaField(h *Help, get func(*Help) string) string {
	if h == nil {
		return ""
	}
	return get(h)
}
