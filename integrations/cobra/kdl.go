package cobra_usage

import (
	"fmt"
	"strings"
)

// renderKDL renders a Spec as a KDL document string, matching usage-lib's output format.
func renderKDL(spec *Spec) string {
	var b strings.Builder

	// Top-level metadata
	if spec.Name != "" {
		fmt.Fprintf(&b, "name %s\n", kdlQuote(spec.Name))
	}
	if spec.Bin != "" {
		fmt.Fprintf(&b, "bin %s\n", kdlQuote(spec.Bin))
	}
	if spec.Version != "" {
		fmt.Fprintf(&b, "version %s\n", kdlQuote(spec.Version))
	}
	if spec.About != "" {
		fmt.Fprintf(&b, "about %s\n", kdlQuote(spec.About))
	}
	if spec.Long != "" {
		fmt.Fprintf(&b, "long_about %s\n", kdlQuoteAlways(spec.Long))
	}
	if spec.Usage != "" {
		fmt.Fprintf(&b, "usage %s\n", kdlQuote(spec.Usage))
	}

	// Root flags
	for _, flag := range spec.Flags {
		b.WriteString(renderFlag(&flag, 0))
	}

	// Root args
	for _, arg := range spec.Args {
		b.WriteString(renderArg(&arg, 0))
	}

	// Subcommands
	for _, cmd := range spec.Cmds {
		b.WriteString(renderCommand(&cmd, 0))
	}

	return b.String()
}

// renderCommand renders a SpecCommand as a KDL cmd node.
func renderCommand(cmd *SpecCommand, depth int) string {
	var b strings.Builder
	indent := strings.Repeat("    ", depth)

	// cmd "name" [properties]
	fmt.Fprintf(&b, "%scmd %s", indent, kdlQuote(cmd.Name))

	if cmd.Hide {
		b.WriteString(" hide=#true")
	}
	if cmd.SubcommandRequired {
		b.WriteString(" subcommand_required=#true")
	}
	if cmd.Help != "" {
		fmt.Fprintf(&b, " help=%s", kdlQuote(cmd.Help))
	}
	if cmd.Deprecated != "" {
		fmt.Fprintf(&b, " deprecated=%s", kdlQuote(cmd.Deprecated))
	}

	// Determine if we need children
	hasChildren := cmd.HelpLong != "" ||
		len(cmd.Aliases) > 0 ||
		len(cmd.Flags) > 0 ||
		len(cmd.Args) > 0 ||
		len(cmd.Cmds) > 0

	if !hasChildren {
		b.WriteString("\n")
		return b.String()
	}

	b.WriteString(" {\n")
	childIndent := strings.Repeat("    ", depth+1)

	// Aliases
	if len(cmd.Aliases) > 0 {
		fmt.Fprintf(&b, "%salias", childIndent)
		for _, a := range cmd.Aliases {
			fmt.Fprintf(&b, " %s", kdlQuote(a))
		}
		b.WriteString("\n")
	}

	// Long help
	if cmd.HelpLong != "" {
		fmt.Fprintf(&b, "%slong_help %s\n", childIndent, kdlQuoteAlways(cmd.HelpLong))
	}

	// Flags
	for _, flag := range cmd.Flags {
		b.WriteString(renderFlag(&flag, depth+1))
	}

	// Args
	for _, arg := range cmd.Args {
		b.WriteString(renderArg(&arg, depth+1))
	}

	// Subcommands
	for _, sub := range cmd.Cmds {
		b.WriteString(renderCommand(&sub, depth+1))
	}

	fmt.Fprintf(&b, "%s}\n", indent)
	return b.String()
}

// renderFlag renders a SpecFlag as a KDL flag node.
func renderFlag(flag *SpecFlag, depth int) string {
	var b strings.Builder
	indent := strings.Repeat("    ", depth)

	// Build the flag name: "-s --long"
	var nameParts []string
	if flag.Short != "" {
		nameParts = append(nameParts, "-"+flag.Short)
	}
	if flag.Long != "" {
		nameParts = append(nameParts, "--"+flag.Long)
	}
	flagName := strings.Join(nameParts, " ")

	fmt.Fprintf(&b, "%sflag %s", indent, kdlQuote(flagName))

	// Inline properties
	if flag.Help != "" {
		fmt.Fprintf(&b, " help=%s", kdlQuote(flag.Help))
	}
	if flag.Required {
		b.WriteString(" required=#true")
	}
	if flag.Var {
		b.WriteString(" var=#true")
	}
	if flag.Hide {
		b.WriteString(" hide=#true")
	}
	if flag.Global {
		b.WriteString(" global=#true")
	}
	if flag.Count {
		b.WriteString(" count=#true")
	}
	if flag.Deprecated != "" {
		fmt.Fprintf(&b, " deprecated=%s", kdlQuote(flag.Deprecated))
	}

	// Single default as property
	if len(flag.Default) == 1 {
		fmt.Fprintf(&b, " default=%s", kdlQuote(flag.Default[0]))
	}

	// Children: long_help, arg, multiple defaults
	hasChildren := flag.HelpLong != "" || flag.Arg != nil || len(flag.Default) > 1

	if !hasChildren {
		b.WriteString("\n")
		return b.String()
	}

	b.WriteString(" {\n")
	childIndent := strings.Repeat("    ", depth+1)

	if flag.HelpLong != "" {
		fmt.Fprintf(&b, "%slong_help %s\n", childIndent, kdlQuoteAlways(flag.HelpLong))
	}

	// Multiple defaults
	if len(flag.Default) > 1 {
		fmt.Fprintf(&b, "%sdefault {\n", childIndent)
		innerIndent := strings.Repeat("    ", depth+2)
		for _, d := range flag.Default {
			fmt.Fprintf(&b, "%s%s\n", innerIndent, kdlQuoteAlways(d))
		}
		fmt.Fprintf(&b, "%s}\n", childIndent)
	}

	if flag.Arg != nil {
		fmt.Fprintf(&b, "%sarg <%s>", childIndent, flag.Arg.Name)
		if flag.Arg.Help != "" {
			fmt.Fprintf(&b, " help=%s", kdlQuote(flag.Arg.Help))
		}
		if flag.Arg.Choices != nil && len(flag.Arg.Choices.Values) > 0 {
			b.WriteString(" {\n")
			innerIndent := strings.Repeat("    ", depth+2)
			fmt.Fprintf(&b, "%schoices {\n", innerIndent)
			choiceIndent := strings.Repeat("    ", depth+3)
			for _, c := range flag.Arg.Choices.Values {
				fmt.Fprintf(&b, "%s%s\n", choiceIndent, kdlQuoteAlways(c))
			}
			fmt.Fprintf(&b, "%s}\n", innerIndent)
			fmt.Fprintf(&b, "%s}\n", childIndent)
		} else {
			b.WriteString("\n")
		}
	}

	fmt.Fprintf(&b, "%s}\n", indent)
	return b.String()
}

// renderArg renders a SpecArg as a KDL arg node.
func renderArg(arg *SpecArg, depth int) string {
	var b strings.Builder
	indent := strings.Repeat("    ", depth)

	// Build usage: <required> or [optional], with trailing … (unicode ellipsis) for variadic
	var usage string
	if arg.Required {
		usage = fmt.Sprintf("<%s>", arg.Name)
	} else {
		usage = fmt.Sprintf("[%s]", arg.Name)
	}
	if arg.Var {
		usage += "\u2026" // unicode ellipsis …
	}

	// In KDL, [brackets] are type annotations and must be quoted.
	// <angle_brackets> are fine as bare identifiers.
	if strings.HasPrefix(usage, "[") {
		fmt.Fprintf(&b, "%sarg %s", indent, kdlQuoteAlways(usage))
	} else {
		fmt.Fprintf(&b, "%sarg %s", indent, usage)
	}

	if arg.Help != "" {
		fmt.Fprintf(&b, " help=%s", kdlQuote(arg.Help))
	}
	if !arg.Required {
		b.WriteString(" required=#false")
	}
	if arg.Var {
		b.WriteString(" var=#true")
	}
	if arg.Hide {
		b.WriteString(" hide=#true")
	}

	// Single default as property
	if len(arg.Default) == 1 {
		fmt.Fprintf(&b, " default=%s", kdlQuote(arg.Default[0]))
	}

	// Children: choices, multiple defaults
	hasChildren := arg.Choices != nil || len(arg.Default) > 1

	if !hasChildren {
		b.WriteString("\n")
		return b.String()
	}

	b.WriteString(" {\n")
	childIndent := strings.Repeat("    ", depth+1)

	// Multiple defaults
	if len(arg.Default) > 1 {
		fmt.Fprintf(&b, "%sdefault {\n", childIndent)
		innerIndent := strings.Repeat("    ", depth+2)
		for _, d := range arg.Default {
			fmt.Fprintf(&b, "%s%s\n", innerIndent, kdlQuoteAlways(d))
		}
		fmt.Fprintf(&b, "%s}\n", childIndent)
	}

	if arg.Choices != nil && len(arg.Choices.Values) > 0 {
		fmt.Fprintf(&b, "%schoices {\n", childIndent)
		innerIndent := strings.Repeat("    ", depth+2)
		for _, c := range arg.Choices.Values {
			fmt.Fprintf(&b, "%s%s\n", innerIndent, kdlQuoteAlways(c))
		}
		fmt.Fprintf(&b, "%s}\n", childIndent)
	}

	fmt.Fprintf(&b, "%s}\n", indent)
	return b.String()
}

// kdlQuote returns a KDL-safe representation of a string.
// Simple identifiers are unquoted; strings with spaces or special chars are quoted.
func kdlQuote(s string) string {
	if s == "" {
		return `""`
	}
	// If it contains spaces, quotes, backslashes, or special KDL chars, always quote
	if needsQuoting(s) {
		return kdlQuoteAlways(s)
	}
	return s
}

// kdlQuoteAlways always wraps the string in KDL double quotes with escaping.
func kdlQuoteAlways(s string) string {
	s = strings.ReplaceAll(s, `\`, `\\`)
	s = strings.ReplaceAll(s, `"`, `\"`)
	return `"` + s + `"`
}

// needsQuoting returns true if the string needs KDL quoting.
func needsQuoting(s string) bool {
	if len(s) == 0 {
		return true
	}
	// Strings starting with a digit are not valid KDL bare identifiers
	if s[0] >= '0' && s[0] <= '9' {
		return true
	}
	for _, c := range s {
		switch c {
		case ' ', '\t', '\n', '\r', '"', '\\', '/', '(', ')', '{', '}', ';', '=', '#', '.', ',', ':':
			return true
		}
	}
	return false
}
