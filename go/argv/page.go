package argv

import (
	"slices"
	"sort"
	"strings"
	"unicode"
)

// The page `-h` prints.
//
// Ported from usage-argv's `short_help`, and held to the same standard: every one
// of mise's 211 pages, byte for byte against usage-lib. Reimplemented rules
// drift, and help text is the part of a CLI a user actually reads, so a rule
// reimplemented here is only worth having if something checks it.

// HelpSpec is what a page needs from the CLI as a whole rather than from one
// command: the parts of the header that come from the spec's root.
type HelpSpec struct {
	// Name is what the spec calls the program, and Bin what it is invoked as.
	// The header prefers Name and falls back to Bin.
	Name string
	Bin  string
	// Version is printed beside the name on the root's page, and only when the
	// spec declares one — a `--version` that answers with nothing is worse than
	// one that is not there.
	Version string
	// LongVersion is the extended text used for --version; -V uses Version.
	LongVersion string
	// About is the root's description, which the root's page uses in place of the
	// command's own.
	About string
	// LongAbout is what `--help` prefers over About.
	LongAbout string
	// Author and License are printed at the end of every long page.
	Author  string
	License string
	// BeforeHelp and AfterHelp bracket every page that does not override them,
	// and the long variants are what `--help` prefers.
	BeforeHelp     string
	AfterHelp      string
	BeforeLongHelp string
	AfterLongHelp  string
	// HelpTemplate is how every page in this CLI is laid out, as named sections:
	// `{{about}}`, `{{usage}}`, `{{commands}}`, `{{args}}`, `{{flags}}` and
	// `{{after_help}}`, which an author may reorder, omit or wrap. Empty means the
	// default order, which is what every page in the fleet is compared against.
	// See [HelpSections].
	HelpTemplate string
}

// Example is one worked invocation, as a page prints it.
type Example struct {
	Header string
	Code   string
	// Help introduces the line on the long page, printed above the command
	// rather than beside it.
	Help string
}

// shortCol is the width the short-flag column is padded to, so that `-J, --json`
// and `    --no-header` line their long forms up.
const shortCol = 4

// ShortHelp renders what `-h` prints for the command at the end of `chain`.
//
// `path` is the command as invoked, binary first. `chain` is the commands from
// the root down to this one, which is what a page needs to work out which
// inherited globals are still this command's to offer.
func ShortHelp(spec HelpSpec, path []string, chain []*Command, help HelpTable) string {
	if len(chain) == 0 {
		return ""
	}
	cmd := chain[len(chain)-1]
	meta := help.Lookup(cmd.Key)
	var sections helpSections
	out := &sections.about

	before := spec.BeforeHelp
	if meta != nil && meta.BeforeHelp != "" {
		before = meta.BeforeHelp
	}
	if before != "" {
		out.WriteString(before + "\n\n")
	}

	// The program, then what it is for — on the program's own page. A
	// subcommand's page says what the subcommand does. usage-lib prints the name
	// when the spec gives one and the binary otherwise, and only when there is a
	// version beside it.
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
		about = spec.About
	} else if meta != nil {
		about = meta.Short
	}
	// Trimmed as the long page trims it, and for the same reason: the blank line
	// under a description belongs to the renderer, so one already in the text is a
	// second one.
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

	// The path without the binary, which is what a listed subcommand shows:
	// usage-lib prints the whole path from the root rather than the child's own
	// name.
	if meta == nil || !meta.FlattenHelp {
		commandsSection(&sections.commands, path[min(1, len(path)):], cmd, help)
	}

	args := visibleArgs(cmd, help, false)
	nextLineHelp := meta != nil && meta.NextLineHelp
	argCol := 0
	for _, a := range args {
		if n := width(argUsage(a, help.Lookup(a.Key))); n > argCol {
			argCol = n
		}
	}
	groupsSection(&sections.args, &sections.ungroupedArgs, &sections.groupedArgs, "Arguments", len(args),
		func(i int) string { return headingOf(help, args[i].Key) },
		func(w *strings.Builder, i int) {
			a := args[i]
			h := help.Lookup(a.Key)
			usage := argUsage(a, h)
			if nextLineHelp {
				w.WriteString("  " + usage + "\n")
				if text := helpText(h); text != "" {
					writeIndented(w, text, 4)
				}
				longAnnotations(w, h, true)
				return
			}
			if help := helpText(h); help != "" {
				w.WriteString("  " + pad(usage, argCol) + "  " + help)
			} else {
				w.WriteString("  " + usage)
			}
			annotations(w, h, true)
		})

	own, inherited := ownAndGlobal(chain, help)
	own = filterHelpMode(own, help, false)
	inherited = filterHelpMode(inherited, help, false)

	// One column over *both* lists, so the two sections read as one table with a
	// rule through it rather than two tables that happen to be adjacent.
	flagCol := 0
	for _, f := range own {
		if n := width(f.usage); n > flagCol {
			flagCol = n
		}
	}
	for _, f := range inherited {
		if n := width(f.usage); n > flagCol {
			flagCol = n
		}
	}
	entry := func(w *strings.Builder, f shownFlag) {
		h := help.Lookup(f.key)
		if f.supplied != "" {
			// A flag the parser supplies has no table entry; its help is fixed.
			if nextLineHelp {
				w.WriteString("  " + f.usage + "\n")
				if text := f.suppliedHelp; text != "" {
					writeIndented(w, text, 4)
				}
				return
			}
			if text := f.suppliedHelp; text != "" {
				w.WriteString("  " + pad(f.usage, flagCol) + "  " + text)
			} else {
				w.WriteString("  " + f.usage)
			}
			w.WriteString("\n")
			return
		}
		if nextLineHelp {
			w.WriteString("  " + f.usage + "\n")
			if text := metaField(h, func(x *Help) string { return x.Short }); text != "" {
				writeIndented(w, text, 4)
			}
			longAnnotations(w, h, true)
			return
		}
		if text := helpText(h); text != "" {
			w.WriteString("  " + pad(f.usage, flagCol) + "  " + text)
		} else {
			w.WriteString("  " + f.usage)
		}
		annotations(w, h, true)
	}
	groupsSection(&sections.flags, &sections.ungroupedFlags, &sections.groupedFlags, "Flags", len(own),
		func(i int) string {
			if own[i].supplied != "" {
				return ""
			}
			return headingOf(help, own[i].key)
		},
		func(w *strings.Builder, i int) { entry(w, own[i]) })
	// After the command's own, and under a heading that says where they came
	// from: a global belongs to the program, not to this command, and a reader
	// should be able to see that.
	groupsSection(&sections.flags, &sections.ungroupedFlags, &sections.groupedFlags, "Global flags", len(inherited),
		func(int) string { return "" },
		func(w *strings.Builder, i int) { entry(w, inherited[i]) })
	if meta != nil && meta.FlattenHelp {
		flatCommandsShort(&sections.flattened, path[min(1, len(path)):], cmd, help, nextLineHelp)
	}

	examplesSection(&sections.afterHelp, pageExamples(chain, help, meta))

	after := spec.AfterHelp
	if meta != nil && meta.AfterHelp != "" {
		after = meta.AfterHelp
	}
	if after != "" {
		sections.afterHelp.WriteString("\n" + after + "\n")
	}

	return sections.assemble(spec.HelpTemplate)
}

// commandsSection lists the subcommands, and the `help` command every CLI with
// subcommands has.
func commandsSection(out *strings.Builder, path []string, cmd *Command, help HelpTable) {
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
	// Nothing visible, no section — a command may have subcommands and every one
	// of them hidden. The usage *line* still says `<SUBCOMMAND>`, because
	// usage-lib computes it before filtering; matching the reference means
	// matching that too, odd as the pair looks together.
	if len(lines) == 0 {
		return
	}
	heading := "Commands"
	nextLineHelp := false
	if h := help.Lookup(cmd.Key); h != nil {
		if h.SubcommandHelpHeading != "" {
			heading = h.SubcommandHelpHeading
		}
		nextLineHelp = h.NextLineHelp
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
			if h := help.Lookup(l.sub.Key); h != nil {
				// Visible aliases only: a hidden alias works and is not advertised,
				// which is the whole of the distinction.
				if len(h.VisibleAliases) > 0 {
					out.WriteString(" [aliases: " + strings.Join(h.VisibleAliases, ", ") + "]")
				}
				if nextLineHelp {
					out.WriteString("\n")
					if strings.TrimSpace(h.Short) != "" {
						writeIndented(out, trimEnd(h.Short), 4)
					}
					if label := deprecationLabel(h); label != "" {
						writeIndented(out, label, 4)
					}
					continue
				}
				if text := helpText(h); text != "" {
					// The row owns its terminating newline. Trim the description in both
					// layouts, as usage-lib does before selecting a layout.
					out.WriteString("  " + trimEnd(text))
				}
			}
			out.WriteString("\n")
		}
		if section == "" && !cmd.DisableHelpSubcommand {
			if nextLineHelp {
				out.WriteString("  help\n    Print this message or the help of the given subcommand(s)\n")
			} else {
				out.WriteString("  help  Print this message or the help of the given subcommand(s)\n")
			}
		}
	}
}

func flatCommandsShort(out *strings.Builder, path []string, cmd *Command, help HelpTable, nextLine bool) {
	visible := append([]*Command{}, cmd.Subcommands...)
	orderCommands(visible, help)
	for _, sub := range visible {
		h := help.Lookup(sub.Key)
		if h != nil && h.Hide {
			continue
		}
		subPath := append(append([]string{}, path...), sub.Name)
		out.WriteString("\n" + strings.Join(subPath, " ") + ":\n")
		if text := metaField(h, func(x *Help) string { return x.Short }); strings.TrimSpace(text) != "" {
			out.WriteString(trimEnd(text) + "\n")
		}
		if label := deprecationLabel(h); label != "" {
			out.WriteString(label + "\n")
		}

		args := visibleArgs(sub, help, false)
		flags := make([]*Flag, 0, len(sub.Flags))
		argCol := 0
		for _, a := range args {
			argCol = max(argCol, width(argUsage(a, help.Lookup(a.Key))))
		}
		flagCol := 0
		for _, f := range sub.Flags {
			fh := help.Lookup(f.Key)
			if f.Global || (fh != nil && (fh.Hide || fh.HideShortHelp)) {
				continue
			}
			flags = append(flags, f)
			flagCol = max(flagCol, width(columnUsage(f, allShown(f), help)))
		}
		orderFlags(flags, help)
		for _, a := range args {
			ah := help.Lookup(a.Key)
			usage := argUsage(a, ah)
			if nextLine {
				out.WriteString("  " + usage + "\n")
				if text := metaField(ah, func(x *Help) string { return x.Short }); text != "" {
					writeIndented(out, text, 4)
				}
				longAnnotations(out, ah, true)
			} else {
				if text := helpText(ah); text != "" {
					out.WriteString("  " + pad(usage, argCol) + "  " + text)
				} else {
					out.WriteString("  " + usage)
				}
				annotations(out, ah, true)
			}
		}
		for _, f := range flags {
			fh := help.Lookup(f.Key)
			usage := columnUsage(f, allShown(f), help)
			if nextLine {
				out.WriteString("  " + usage + "\n")
				if text := metaField(fh, func(x *Help) string { return x.Short }); text != "" {
					writeIndented(out, text, 4)
				}
				longAnnotations(out, fh, true)
			} else {
				if text := helpText(fh); text != "" {
					out.WriteString("  " + pad(usage, flagCol) + "  " + text)
				} else {
					out.WriteString("  " + usage)
				}
				annotations(out, fh, true)
			}
		}
		if h != nil && h.FlattenHelp {
			flatCommandsShort(out, subPath, sub, help, h.NextLineHelp)
		}
		out.WriteString("\n")
	}
}

// groupsSection writes one section per heading, unheaded first, in the order the
// headings first appear.
func groupsSection(out, ungrouped, grouped *strings.Builder, defaultTitle string, n int,
	headingOf func(int) string, writeItem func(*strings.Builder, int)) {

	if n == 0 {
		return
	}
	var headings []string
	seen := map[string]bool{}
	for i := 0; i < n; i++ {
		h := headingOf(i)
		if !seen[h] {
			seen[h] = true
			headings = append(headings, h)
		}
	}
	// The unheaded group first, then the rest in first-seen order.
	sort.SliceStable(headings, func(i, j int) bool {
		return headings[i] == "" && headings[j] != ""
	})

	for _, heading := range headings {
		var section strings.Builder
		title := heading
		if title == "" {
			title = defaultTitle
		}
		section.WriteString("\n" + title + ":\n")
		for i := 0; i < n; i++ {
			if headingOf(i) == heading {
				writeItem(&section, i)
			}
		}
		out.WriteString(section.String())
		if heading == "" && ungrouped != nil {
			ungrouped.WriteString(section.String())
		} else if heading != "" && grouped != nil {
			grouped.WriteString(section.String())
		}
	}
}

// pageExamples is a command's own examples, or the root's where it has none.
//
// The same fallback `BeforeHelp` and `AfterHelp` get, and for the same reason: a
// CLI writing examples once at the top means them to appear. mise declares none
// at its root, so the 211-page parity test cannot see this either way — it is
// checked against the reference's rule rather than against the fixture.
func pageExamples(chain []*Command, help HelpTable, meta *Help) []Example {
	if meta != nil && len(meta.Examples) > 0 {
		return meta.Examples
	}
	if len(chain) > 0 {
		if root := help.Lookup(chain[0].Key); root != nil {
			return root.Examples
		}
	}
	return nil
}

func examplesSection(out *strings.Builder, examples []Example) {
	if len(examples) == 0 {
		return
	}
	out.WriteString("\nExamples:\n")
	for _, e := range examples {
		if e.Header != "" {
			out.WriteString("  " + e.Header + ":\n")
		}
		out.WriteString("    $ " + e.Code + "\n")
	}
}

// annotations writes what a page appends to an entry's help, then the newline.
//
// `withDefault` is false for a flag, which is not an oversight: usage-lib prints
// `(default: …)` for an argument and not for a flag, and the short page follows
// it. A flag's default shows up in the long page instead.
func annotations(out *strings.Builder, h *Help, withDefault bool) {
	if h != nil {
		if !h.HidePossibleValues && len(h.Choices) > 0 {
			out.WriteString(" [" + strings.Join(h.Choices, ", ") + "]")
		}
		if !h.HideEnv && h.Env != "" {
			out.WriteString(" [env: " + h.Env + "]")
		}
		if !h.HideEnv {
			for _, env := range h.EnvFallback {
				out.WriteString(" [env fallback: " + env + "]")
			}
			for _, env := range h.DeprecatedEnv {
				out.WriteString(" [deprecated env: " + env + "]")
			}
		}
		if withDefault && !h.HideDefaultValue && len(h.Default) > 0 {
			out.WriteString(" (default: " + strings.Join(h.Default, ", ") + ")")
		}
	}
	out.WriteString("\n")
}

func helpText(h *Help) string {
	if h == nil {
		return ""
	}
	label := deprecationLabel(h)
	if strings.TrimSpace(h.Short) == "" {
		return label
	}
	if label == "" {
		return h.Short
	}
	return h.Short + " " + label
}

func deprecationLabel(h *Help) string {
	if h == nil || (h.Deprecated == "" && h.DeprecatedWarnAt == "" && h.DeprecatedRemoveAt == "") {
		return ""
	}
	parts := []string{}
	if h.Deprecated != "" {
		parts = append(parts, h.Deprecated)
	}
	if h.DeprecatedWarnAt != "" {
		parts = append(parts, "warns at "+h.DeprecatedWarnAt)
	}
	if h.DeprecatedRemoveAt != "" {
		parts = append(parts, "removed at "+h.DeprecatedRemoveAt)
	}
	return "[deprecated: " + strings.Join(parts, "; ") + "]"
}

func headingOf(help HelpTable, key uint64) string {
	if h := help.Lookup(key); h != nil {
		return h.Heading
	}
	return ""
}

func helpOrder(help HelpTable, key uint64, fallback int) uint32 {
	if h := help.Lookup(key); h != nil && h.DisplayOrderSet {
		return h.DisplayOrder
	}
	return uint32(fallback)
}

func orderCommands(commands []*Command, help HelpTable) {
	sort.SliceStable(commands, func(i, j int) bool {
		left, right := helpOrder(help, commands[i].Key, 999), helpOrder(help, commands[j].Key, 999)
		if left != right {
			return left < right
		}
		return commands[i].Name < commands[j].Name
	})
}

func orderFlags(flags []*Flag, help HelpTable) {
	positions := map[uint64]int{}
	for i, flag := range flags {
		positions[flag.Key] = i
	}
	sort.SliceStable(flags, func(i, j int) bool {
		return helpOrder(help, flags[i].Key, positions[flags[i].Key]) <
			helpOrder(help, flags[j].Key, positions[flags[j].Key])
	})
}

func visibleArgs(cmd *Command, help HelpTable, long bool) []*Arg {
	out := make([]*Arg, 0, len(cmd.Args))
	for _, a := range cmd.Args {
		if h := help.Lookup(a.Key); h != nil {
			if h.Hide || (!long && h.HideShortHelp) || (long && h.HideLongHelp) {
				continue
			}
		}
		out = append(out, a)
	}
	positions := map[uint64]int{}
	for i, arg := range out {
		positions[arg.Key] = i
	}
	sort.SliceStable(out, func(i, j int) bool {
		return helpOrder(help, out[i].Key, positions[out[i].Key]) <
			helpOrder(help, out[j].Key, positions[out[j].Key])
	})
	return out
}

func filterHelpMode(flags []shownFlag, help HelpTable, long bool) []shownFlag {
	out := flags[:0]
	for _, flag := range flags {
		h := help.Lookup(flag.key)
		if h != nil && ((!long && h.HideShortHelp) || (long && h.HideLongHelp)) {
			continue
		}
		out = append(out, flag)
	}
	return out
}

// pad left-justifies to a column measured in characters, not bytes: the usage
// strings carry `…`.
func pad(s string, col int) string {
	if n := width(s); n < col {
		return s + strings.Repeat(" ", col-n)
	}
	return s
}

func width(s string) int { return len([]rune(s)) }

// trimEnd drops trailing whitespace, which is what `str::trim_end` does on the
// two sides this is ported from.
func trimEnd(s string) string { return strings.TrimRightFunc(s, unicode.IsSpace) }

// sortLines orders a section's entries by their rendered usage, which is how
// usage-lib orders them — for a command with no flags or arguments that agrees
// with sorting by name, and where it differs this is what a reader sees.
func sortLines[T any](lines []T, key func(int) string) {
	sort.SliceStable(lines, func(i, j int) bool { return key(i) < key(j) })
}

func min(a, b int) int {
	if a < b {
		return a
	}
	return b
}
