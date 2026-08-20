package argv

import (
	"slices"
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

// LongHelp renders what `--help` prints for the command at the end of `chain`.
func LongHelp(spec HelpSpec, path []string, chain []*Command, help HelpTable) string {
	if len(chain) == 0 {
		return ""
	}
	cmd := chain[len(chain)-1]
	meta := help.Lookup(cmd.Key)
	nextLineHelp := meta != nil && meta.NextLineHelp
	var out strings.Builder

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
			out.WriteString("Usage: " + line + "\n")
		} else {
			out.WriteString("       " + line + "\n")
		}
	}

	if meta == nil || !meta.FlattenHelp {
		longCommandsSection(&out, path[min(1, len(path)):], cmd, help)
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
	groupsSection(&out, "Arguments", len(args),
		func(i int) string { return headingOf(help, args[i].Key) },
		func(w *strings.Builder, i int) {
			h := help.Lookup(args[i].Key)
			entry(w, argUsage(args[i], h), firstOf(metaField(h, func(x *Help) string { return x.Long }),
				metaField(h, func(x *Help) string { return x.Short })), argCol, nextLineHelp)
			longAnnotations(w, h, true)
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
		entry(w, f.usage, firstOf(metaField(h, func(x *Help) string { return x.Long }),
			metaField(h, func(x *Help) string { return x.Short })), flagCol, nextLineHelp)
		longAnnotations(w, h, true)
	}
	groupsSection(&out, "Flags", len(own),
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
	groupsSection(&out, "Global flags", len(inherited),
		func(int) string { return "" },
		func(w *strings.Builder, i int) { writeFlag(w, inherited[i]) })
	if meta != nil && meta.FlattenHelp {
		flatCommandsLong(&out, path[min(1, len(path)):], cmd, help, nextLineHelp)
	}

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

	return strings.TrimSpace(out.String()) + "\n"
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
func longCommandsSection(out *strings.Builder, path []string, cmd *Command, help HelpTable) {
	type line struct {
		usage string
		sub   *Command
	}
	var lines []line
	for _, sub := range cmd.Subcommands {
		if h := help.Lookup(sub.Key); h != nil && h.Hide {
			continue
		}
		subPath := append(append([]string{}, path...), sub.Name)
		lines = append(lines, line{UsageLine(subPath, sub, help), sub})
	}
	if len(lines) == 0 {
		return
	}
	heading := "Commands"
	if h := help.Lookup(cmd.Key); h != nil && h.SubcommandHelpHeading != "" {
		heading = h.SubcommandHelpHeading
	}
	sort.SliceStable(lines, func(i, j int) bool {
		left, right := helpOrder(help, lines[i].sub.Key, 999), helpOrder(help, lines[j].sub.Key, 999)
		if left != right {
			return left < right
		}
		return lines[i].usage < lines[j].usage
	})

	headings := []string{""}
	for _, l := range lines {
		if h := headingOf(help, l.sub.Key); h == heading {
			continue
		} else if h != "" && !slices.Contains(headings, h) {
			headings = append(headings, h)
		}
	}
	for _, section := range headings {
		title := section
		if title == "" {
			title = heading
		}
		out.WriteString("\n" + title + ":\n")
		for _, l := range lines {
			itemSection := headingOf(help, l.sub.Key)
			if itemSection == heading {
				itemSection = ""
			}
			if itemSection != section {
				continue
			}
			out.WriteString("  " + l.usage)
			h := help.Lookup(l.sub.Key)
			if h != nil && len(h.VisibleAliases) > 0 {
				out.WriteString(" [aliases: " + strings.Join(h.VisibleAliases, ", ") + "]")
			}
			out.WriteString("\n")
			if h != nil {
				// Trailing whitespace trimmed: the blank line after each entry is
				// written below, and a description that happens to end in a newline
				// added a second one — a stray blank in the middle of the list.
				if about := trimEnd(firstOf(h.Long, h.Short)); about != "" {
					writeIndented(out, about, 4)
				}
				if label := deprecationLabel(h); label != "" {
					writeIndented(out, label, 4)
				}
			}
			// A blank line between entries, which the wider layout can afford and
			// which keeps a multi-line description from running into the next name.
			out.WriteString("\n")
		}
		if section == "" && !cmd.DisableHelpSubcommand {
			out.WriteString("  help\n    Print this message or the help of the given subcommand(s)\n")
		}
	}
}

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
			longAnnotations(out, ah, true)
		}
		for _, f := range flags {
			fh := help.Lookup(f.Key)
			entry(out, columnUsage(f, allShown(f), help), firstOf(
				metaField(fh, func(x *Help) string { return x.Long }),
				metaField(fh, func(x *Help) string { return x.Short })), flagCol, nextLine)
			longAnnotations(out, fh, true)
		}
		if h != nil && h.FlattenHelp {
			flatCommandsLong(out, subPath, sub, help, h.NextLineHelp)
		}
		out.WriteString("\n")
	}
}

// entry writes one flag or argument: its help in a column beside it, wrapped — or
// indented underneath, where the text has line breaks of its own.
func entry(out *strings.Builder, usage, help string, col int, nextLine bool) {
	if strings.TrimSpace(help) == "" {
		out.WriteString("  " + usage + "\n")
		return
	}

	// The column layout only works for text that has not been broken already, and
	// only when there is room left for it to say anything.
	indent := 2 + col + 2
	room := helpWidth - indent
	if room < 0 {
		room = 0
	}
	if nextLine || strings.Contains(help, "\n") || room < 10 {
		out.WriteString("  " + usage + "\n")
		writeIndented(out, help, 4)
		return
	}

	lines := wrap(help, room)
	out.WriteString("  " + pad(usage, col) + "  " + lines[0] + "\n")
	for _, line := range lines[1:] {
		out.WriteString(strings.Repeat(" ", indent) + line + "\n")
	}
	// No blank line after a wrapped entry: the reference's template asks for one
	// and its whitespace trimming eats it before it reaches the output.
}

// longAnnotations gives each annotation its own line, which is the room the wide
// layout has and the short one does not.
func longAnnotations(out *strings.Builder, h *Help, withDefault bool) {
	if h == nil {
		return
	}
	if !h.HidePossibleValues && len(h.Choices) > 0 {
		out.WriteString("    [possible values: " + strings.Join(h.Choices, ", ") + "]\n")
	}
	if !h.HideEnv && h.Env != "" {
		out.WriteString("    [env: " + h.Env + "]\n")
	}
	if !h.HideEnv {
		for _, env := range h.EnvFallback {
			out.WriteString("    [env fallback: " + env + "]\n")
		}
		for _, env := range h.DeprecatedEnv {
			out.WriteString("    [deprecated env: " + env + "]\n")
		}
	}
	if withDefault && !h.HideDefaultValue && len(h.Default) > 0 {
		out.WriteString("    (default: " + strings.Join(h.Default, ", ") + ")\n")
	}
	if label := deprecationLabel(h); label != "" {
		out.WriteString("    " + label + "\n")
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
