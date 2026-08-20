package argv

import (
	"sort"
	"strings"
)

// What a help page prints.
//
// A third table, separate from [Meta] rather than folded into it, because Go's
// linker drops an unreferenced package-level symbol whole. One table would mean
// a CLI that applies the post-binding rules also carries every help string in
// the spec — mise's run to several hundred kilobytes. Three tables let a program
// pay for what it uses: binding alone carries neither, adding the rules carries
// [Metadata], and printing help carries this.
//
// Indexed by key like the others, so an entry's three halves are joined by
// identity rather than by position in three lists that could drift.

// Help is what a page needs to say about one command, flag or argument.
type Help struct {
	// Key matches the entry this describes in the parse tables.
	Key uint64
	// Hide keeps an entry out of help without keeping it out of the parse. A
	// hidden flag still binds; help simply does not invite anyone to type it.
	Hide               bool
	HideDefaultValue   bool
	HideEnv            bool
	HideEnvValues      bool
	HidePossibleValues bool
	HideShortHelp      bool
	HideLongHelp       bool
	// Demanded is `required` and undefaulted, which is what decides whether the
	// usage line angles an entry or brackets it.
	//
	// Precomputed rather than read from [Meta], so that rendering a page does not
	// drag the post-binding table in with it — which would undo the whole reason
	// these are separate.
	Demanded bool
	// Repeatable is the spec's `var` on a flag: the `…` in `--tag… <t>`, meaning
	// the flag may be given again, not that one occurrence takes several values.
	Repeatable bool
	// ValueName is what a flag's value is called. Empty for a flag that takes
	// none.
	ValueName string
	// ValueNames preserves a fixed-arity value's distinct placeholders. It is
	// empty for the ordinary single-name case represented by ValueName.
	ValueNames []string
	// ValueArity is the exact number of values when a fixed-arity argument uses
	// one placeholder for every slot. Zero means the arity is not exact.
	ValueArity uint32
	// ValueDemanded is the same required-and-undefaulted test as [Help.Demanded],
	// applied to the flag's *value* rather than to the flag.
	//
	// The two are independent, and usage-lib writes both: `<--v <n>>` is a
	// required flag whose value must be given, and `<--jobs [n]>` is a required
	// flag whose value has a default. Angling the value unconditionally — which
	// is what usage-argv does — is invisible until a spec has a flag whose value
	// is optional or defaulted, and mise has none.
	ValueDemanded bool
	// Short is the one-line help, and Long the fuller text `--help` prefers.
	Short string
	Long  string
	// Deprecated is the migration message, with optional warn/remove milestones.
	Deprecated         string
	DeprecatedWarnAt   string
	DeprecatedRemoveAt string
	// Heading groups an entry into a section of the page. Presentational only.
	Heading string
	// DisplayOrderSet distinguishes an explicit zero from declaration order.
	DisplayOrder    uint32
	DisplayOrderSet bool

	// The rest a page prints and the usage line does not.

	// VisibleAliases are the aliases a command advertises. The parse table merges
	// hidden ones in beside these, because binding does not care which is which;
	// a page does, and that is the whole of the distinction.
	VisibleAliases []string
	// Choices, Env and Default are the annotations a page appends to an entry's
	// help: `[a, b]`, `[env: X]`, `(default: y)`.
	//
	// Duplicated from [Meta] rather than read from it, which is the price of
	// keeping the two tables separable: a CLI that prints help should not have to
	// carry the post-binding table, and one that applies the rules should not have
	// to carry the help strings.
	Choices []string
	Env     string
	Default []string
	// BeforeHelp and AfterHelp bracket this command's page, overriding the
	// spec-wide text. The long variants are preferred by `--help`.
	BeforeHelp            string
	AfterHelp             string
	BeforeLongHelp        string
	AfterLongHelp         string
	SubcommandHelpHeading string
	SubcommandValueName   string
	NextLineHelp          bool
	FlattenHelp           bool
	SubcommandRequired    bool
	// Examples are worked invocations, printed last.
	Examples []Example
}

// HelpTable is the cold help table, indexed by key: entry `Key` sits at
// `HelpTable[Key-1]`.
type HelpTable []Help

// Lookup returns the help for a key, or nil if the table has none.
func (h HelpTable) Lookup(key uint64) *Help {
	if key == 0 || key > uint64(len(h)) {
		return nil
	}
	entry := &h[key-1]
	if entry.Key != key {
		// Out of step with the parse tables. Reporting nothing makes a caller's
		// own test fail, where searching would quietly describe the wrong entry.
		return nil
	}
	return entry
}

// inlineLimit is how many entries a usage line spells out before collapsing them
// into `[FLAGS]` or `[ARGS]…`. Two, as usage-lib has it.
const inlineLimit = 2

// UsageLine renders the line a page prints after `Usage: `.
//
// `path` is the command as invoked, binary first: `[]string{"mise", "use"}`.
//
// Hidden entries are absent from the line as they are from the sections — help
// describes what a user is invited to type.
func UsageLine(path []string, cmd *Command, help HelpTable) string {
	return usageLine(path, cmd, help, true)
}

func usageLine(path []string, cmd *Command, help HelpTable, includeSubcommands bool) string {
	var out strings.Builder
	out.WriteString(strings.Join(path, " "))

	visibleFlags := make([]*Flag, 0, len(cmd.Flags))
	demandedFlag := false
	for _, f := range cmd.Flags {
		h := help.Lookup(f.Key)
		if h != nil && h.Hide {
			continue
		}
		visibleFlags = append(visibleFlags, f)
		if h != nil && h.Demanded {
			demandedFlag = true
		}
	}
	if n := len(visibleFlags); n > 0 {
		if n <= inlineLimit {
			for _, f := range visibleFlags {
				h := help.Lookup(f.Key)
				// A required flag is angled like a required argument: the brackets
				// are what say whether leaving it out is allowed.
				open, close := "[", "]"
				if h != nil && h.Demanded {
					open, close = "<", ">"
				}
				out.WriteString(" " + open + flagUsage(f, h) + close)
			}
		} else if demandedFlag {
			out.WriteString(" <FLAGS>")
		} else {
			out.WriteString(" [FLAGS]")
		}
	}

	visibleArgs := make([]*Arg, 0, len(cmd.Args))
	demandedArg := false
	for _, a := range cmd.Args {
		h := help.Lookup(a.Key)
		if h != nil && h.Hide {
			continue
		}
		visibleArgs = append(visibleArgs, a)
		if h != nil && h.Demanded {
			demandedArg = true
		}
	}
	if n := len(visibleArgs); n > 0 {
		if n <= inlineLimit {
			for _, a := range visibleArgs {
				out.WriteString(" " + argUsage(a, help.Lookup(a.Key)))
			}
		} else if demandedArg {
			out.WriteString(" <ARGS>…")
		} else {
			out.WriteString(" [ARGS]…")
		}
	}

	if includeSubcommands && len(cmd.Subcommands) > 0 {
		name := "SUBCOMMAND"
		if h := help.Lookup(cmd.Key); h != nil && h.SubcommandValueName != "" {
			name = h.SubcommandValueName
		}
		out.WriteString(" <" + name + ">")
	}
	return out.String()
}

func usageLines(path []string, cmd *Command, help HelpTable) []string {
	h := help.Lookup(cmd.Key)
	if h == nil || !h.FlattenHelp {
		return []string{UsageLine(path, cmd, help)}
	}
	visible := make([]*Command, 0, len(cmd.Subcommands))
	for _, sub := range cmd.Subcommands {
		if subHelp := help.Lookup(sub.Key); subHelp == nil || !subHelp.Hide {
			visible = append(visible, sub)
		}
	}
	if len(visible) == 0 {
		return []string{UsageLine(path, cmd, help)}
	}
	sort.Slice(visible, func(i, j int) bool { return visible[i].Name < visible[j].Name })
	lines := make([]string, 0, len(visible)+1)
	if !h.SubcommandRequired || cmd.ArgsConflictWithSubcommands {
		lines = append(lines, usageLine(path, cmd, help, false))
	}
	for _, sub := range visible {
		subPath := append(append([]string{}, path...), sub.Name)
		lines = append(lines, UsageLine(subPath, sub, help))
	}
	return lines
}

// flagUsage is how one flag appears in the usage line: `-f --force`, plus its
// value if it takes one. The line always offers every spelling, since nothing on
// it is competing for a word.
func flagUsage(f *Flag, h *Help) string {
	return flagUsageShown(f, allShown(f), h)
}

// flagUsageShown is the same, restricted to the spellings a page is still
// offering for this flag — see [shown].
func flagUsageShown(f *Flag, show shown, h *Help) string {
	var out strings.Builder

	long, short := "", byte(0)
	if show.hasLong {
		long = show.long
	}
	if show.hasShort {
		short = show.short
	}

	// The declared name, when it is not the one the forms would imply. A flag
	// called `verbose` reachable only as `-v` has to say so, or help would name
	// something the spec does not.
	implied := false
	switch {
	case long != "":
		implied = long == f.Name
	case short != 0:
		implied = string(short) == f.Name
	}
	if !implied {
		out.WriteString(f.Name + ":")
	}
	if short != 0 {
		if out.Len() > 0 {
			out.WriteByte(' ')
		}
		out.WriteString("-" + string(short))
	}
	if long != "" {
		if out.Len() > 0 {
			out.WriteByte(' ')
		}
		out.WriteString("--" + long)
	}

	// A repeatable flag, which is the spec's `var` — not one occurrence taking
	// several values, which is the value's own business below.
	if h != nil && h.Repeatable {
		out.WriteString("…")
	}
	if f.TakesValue {
		name := f.Name
		if h != nil && h.ValueName != "" {
			name = h.ValueName
		}
		open, close := "[", "]"
		if h != nil && h.ValueDemanded {
			open, close = "<", ">"
		}
		if h != nil && len(h.ValueNames) > 1 {
			for _, valueName := range h.ValueNames {
				out.WriteString(" " + open + valueName + close)
			}
		} else if h != nil && h.ValueArity > 1 {
			for i := uint32(0); i < h.ValueArity; i++ {
				out.WriteString(" " + open + name + close)
			}
		} else {
			out.WriteString(" " + open + name + close)
		}
		if f.Variadic && (h == nil || (len(h.ValueNames) <= 1 && h.ValueArity == 0)) {
			out.WriteString("…")
		}
	}
	return out.String()
}

// argUsage is how one positional appears in the usage line.
func argUsage(a *Arg, h *Help) string {
	open, close := "[", "]"
	if h != nil && h.Demanded {
		open, close = "<", ">"
	}
	var out strings.Builder
	// An argument that only takes what follows a `--` shows the separator, because
	// typing the value without it does not reach this argument at all — and the
	// brackets go outside it, as usage-lib writes it: `[-- COMMAND]…`, one
	// optional thing rather than a literal `--` followed by an optional word.
	if h != nil && (len(h.ValueNames) > 1 || h.ValueArity > 1) {
		if a.DoubleDash == DoubleDashRequired {
			out.WriteString("-- ")
		}
		names := h.ValueNames
		if len(names) <= 1 {
			name := a.Name
			if len(names) == 1 {
				name = names[0]
			}
			names = make([]string, h.ValueArity)
			for i := range names {
				names[i] = name
			}
		}
		for i, name := range names {
			if i > 0 {
				out.WriteByte(' ')
			}
			out.WriteString(open + name + close)
		}
	} else if a.DoubleDash == DoubleDashRequired {
		out.WriteString(open + "-- " + a.Name + close)
	} else {
		out.WriteString(open + a.Name + close)
	}
	if a.Var && (h == nil || (len(h.ValueNames) <= 1 && h.ValueArity == 0)) {
		out.WriteString("…")
	}
	return out.String()
}
